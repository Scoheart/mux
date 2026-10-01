"""MUX capture adapter. No raw flow dump or credentials are persisted."""
import asyncio
import json
import html
import ipaddress
import os
import re
import subprocess
import time
import urllib.request
from mitmproxy import dns
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit, parse_qsl, urlencode, quote
from mitmproxy import ctx

ROOT = Path(os.environ['MUX_CAPTURE_DIR'])
CONFIG = json.loads((ROOT / 'session.json').read_text())
LIMIT = 2 * 1024 * 1024
MAX_FLOWS = 5000
SENSITIVE = re.compile(r'authorization|cookie|password|passwd|secret|token|api[-_]?key|credential|session[-_]?key|^key$', re.I)


def private_json(path, data):
    temporary = path.with_suffix('.tmp')
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'w') as out:
        json.dump(data, out, ensure_ascii=False)
    os.replace(temporary, path)


def redact(data):
    if isinstance(data, dict):
        return {key: '[REDACTED]' if SENSITIVE.search(key) else redact(value) for key, value in data.items()}
    if isinstance(data, list):
        return [redact(value) for value in data]
    return data


def credentials(flow):
    values = set()
    def add(value):
        if isinstance(value, str) and len(value) >= 4 and len(value) <= 4096 and len(values) < 128:
            values.add(value)
    def walk(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if SENSITIVE.search(key):
                    add(child)
                else:
                    walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)
    for msg in [flow.request, flow.response]:
        if msg is None:
            continue
        for key, value in msg.headers.items(multi=True):
            if SENSITIVE.search(key):
                add(value)
                if key.lower() in ('authorization', 'proxy-authorization'):
                    add(value.split(' ', 1)[-1])
                if 'cookie' in key.lower():
                    for item in (value.split(';')[:1] if key.lower() == 'set-cookie' else value.split(';')):
                        if '=' in item:
                            add(item.split('=', 1)[1].strip())
        try:
            if len(msg.raw_content or b'') <= LIMIT:
                walk(json.loads(msg.content))
        except (ValueError, TypeError):
            pass
    for key, value in parse_qsl(urlsplit(flow.request.pretty_url).query):
        if SENSITIVE.search(key):
            add(value)
    return sorted(values, key=len, reverse=True)


def clean_text(text, secrets=()):
    for secret in secrets:
        for form in {secret, quote(secret, safe=''), html.escape(secret), json.dumps(secret, ensure_ascii=False)[1:-1]}:
            text = text.replace(form, '[REDACTED]')
    # Also mask common credential formats outside structured JSON.
    text = re.sub(r'(?i)(Bearer\s+)[A-Za-z0-9._~+/-]+', r'\1[REDACTED]', text)
    return re.sub(r'\b(?:sk-[A-Za-z0-9_-]{16,}|AIza[A-Za-z0-9_-]{20,}|ya29\.[A-Za-z0-9._-]+)\b', '[REDACTED]', text)


def clean_url(url):
    value = urlsplit(url)
    return urlunsplit((value.scheme, value.netloc, value.path, urlencode([(key, '[REDACTED]' if SENSITIVE.search(key) else val) for key, val in parse_qsl(value.query, keep_blank_values=True)]), ''))


def message(msg, secrets):
    if msg is None:
        return None
    raw = msg.raw_content or b''
    try:
        body = msg.content or b''
    except ValueError:
        return {'headers': [], 'body': '[无法解压正文]', 'bytes': len(raw), 'truncated': True, 'encoding': 'text'}
    if len(body) > LIMIT:
        # Do not partially persist structured credentials from a truncated JSON.
        text = '[正文超过 2 MiB；未保存正文]'
        encoding = 'text'
    else:
        try:
            text = body.decode('utf-8')
            try:
                text = json.dumps(redact(json.loads(text)), ensure_ascii=False, indent=2)
            except (ValueError, TypeError):
                # SSE may carry structured credentials in individual events.
                if msg.headers.get('content-type', '').startswith('text/event-stream'):
                    lines = []
                    for line in text.splitlines():
                        if line.startswith('data:'):
                            try:
                                line = 'data: ' + json.dumps(redact(json.loads(line[5:].strip())), ensure_ascii=False)
                            except ValueError:
                                pass
                        lines.append(line)
                    text = '\n'.join(lines)
                else:
                    text = re.sub(r'(?i)((?:password|secret|token|api[-_]?key)\s*[=:]\s*)[^\s&<]+', r'\1[REDACTED]', text)
            text = clean_text(text, secrets)
            encoding = 'text'
        except UnicodeDecodeError:
            # Binary bodies may contain secrets and cannot be safely redacted.
            text = '[二进制正文；未保存正文]'
            encoding = 'binary'
    headers = [[key, '[REDACTED]' if SENSITIVE.search(key) else clean_text(value, secrets)] for key, value in msg.headers.items(multi=True)]
    return {'headers': headers, 'body': text, 'bytes': len(raw), 'truncated': len(body) > LIMIT, 'encoding': encoding}


def processes():
    result = subprocess.run(['/bin/ps', '-axo', 'pid=,ppid=,comm='], capture_output=True, text=True, timeout=5, check=True)
    rows = []
    for line in result.stdout.splitlines():
        fields = line.strip().split(None, 2)
        if len(fields) == 3 and fields[0].isdigit() and fields[1].isdigit():
            rows.append((int(fields[0]), int(fields[1]), fields[2]))
    kind, target = CONFIG['target_id'].split(':', 1)
    ids = {pid for pid, _, command in rows if (command.startswith(target + '/') if kind == 'app' else command == target)}
    while True:
        expanded = ids | {pid for pid, parent, _ in rows if parent in ids}
        if expanded == ids:
            return sorted(ids)
        ids = expanded


def loopback(address):
    if not address:
        return False
    host = address[0].strip('[]')
    if host.lower() == 'localhost':
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


class Capture:
    def __init__(self):
        self.count = 0
        self.pids = CONFIG['pids']
        self.task = None
        self.warnings = []
        self.saved_bytes = 0
        self.inflight = set()

    def status(self, state, message=''):
        if message and message not in self.warnings:
            self.warnings.append(message)
        private_json(ROOT / 'status.json', {'state': state, 'message': ' '.join(self.warnings[-5:]), 'pids': self.pids, 'flow_count': self.count})

    async def running(self):
        self.status('running')
        self.task = asyncio.create_task(self.watch())

    async def watch(self):
        while True:
            await asyncio.sleep(2)
            owner = os.environ.get('MUX_CAPTURE_OWNER')
            if owner:
                try:
                    os.kill(int(owner), 0)
                except ProcessLookupError:
                    self.status('stopped', 'MUX 已退出，抓包已停止。')
                    ctx.master.shutdown()
                    return
            if (ROOT / 'stop').exists():
                ctx.master.shutdown()
                return
            try:
                ids = await asyncio.to_thread(processes)
                if not ids:
                    self.status('stopped', '目标进程已退出；请重新选择正在运行的应用。')
                    ctx.master.shutdown()
                    return
                if ids != self.pids:
                    if self.inflight:
                        self.status('running', '发现新进程，正在等待当前请求完成后更新范围；新进程暂未覆盖。')
                        continue
                    ctx.options.update(mode=['local:' + ','.join(map(str, ids))])
                    self.pids = ids
                    self.status('running', '进程范围已更新；切换期间可能存在短暂间隙。')
            except Exception:
                self.status('failed', '无法更新拦截进程范围，抓包已停止。')
                ctx.master.shutdown()
                return

    async def dns_request(self, flow):
        if not CONFIG['proxy_url']:
            return
        self.inflight.add('dns:' + flow.id)
        try:
            def resolve():
                handler = urllib.request.ProxyHandler({'https': CONFIG['proxy_url']})
                client = urllib.request.build_opener(handler)
                request = urllib.request.Request('https://cloudflare-dns.com/dns-query', data=flow.request.packed, headers={'Content-Type': 'application/dns-message', 'Accept': 'application/dns-message'})
                with client.open(request, timeout=10) as response:
                    return dns.DNSMessage.unpack(response.read(65536))
            flow.response = await asyncio.to_thread(resolve)
        except Exception:
            flow.response = flow.request.fail(2)
            self.status('running', '通过所选代理解析 DNS 失败；未使用直连 DNS。')
        finally:
            self.inflight.discard('dns:' + flow.id)

    def tls_clienthello(self, data):
        if loopback(data.context.server.address):
            # Preserve the application's own TLS / IPC, including its certificate.
            data.ignore_connection = True

    def server_connect(self, data):
        if loopback(data.server.address):
            return
        # Fail closed: unsupported protocols must not bypass the chosen proxy.
        if CONFIG['proxy_url']:
            proxy = urlsplit(CONFIG['proxy_url'])
            if data.server.transport_protocol != 'tcp' or data.server.address != (proxy.hostname, proxy.port or 80):
                data.server.error = '所选 HTTP 出口不支持此连接；未回退直连。'
                self.status('running', '有连接未被 HTTP 出口支持；未回退直连。')

    def requestheaders(self, flow):
        self.inflight.add(flow.id)
        if loopback((flow.request.host, flow.request.port)):
            flow.server_conn.via = None
        elif CONFIG['proxy_url']:
            proxy = urlsplit(CONFIG['proxy_url'])
            flow.server_conn.via = ('http', (proxy.hostname, proxy.port or 80))
        else:
            flow.server_conn.via = None

    def response(self, flow):
        self.save(flow)

    def error(self, flow):
        self.save(flow)

    def tls_failed_client(self, data):
        self.status('running', '客户端 TLS 握手失败；请检查抓包 CA 信任或证书固定。')

    def save(self, flow):
        self.inflight.discard(flow.id)
        if self.count >= MAX_FLOWS:
            self.status('stopped', '已达到 5000 条请求上限，请开始新的会话。')
            ctx.master.shutdown()
            return
        if not flow.request:
            return
        self.count += 1
        secrets = credentials(flow)
        record = {
            'id': flow.id, 'method': flow.request.method, 'url': clean_text(clean_url(flow.request.pretty_url), secrets),
            'status': flow.response.status_code if flow.response else None,
            'started_at': flow.request.timestamp_start,
            'duration_ms': max(0, round(1000 * ((flow.response.timestamp_end or time.time()) - flow.request.timestamp_start))) if flow.response else None,
            'error': ('连接或转发失败；未回退直连。' if CONFIG['proxy_url'] else '直连请求失败。') if flow.error else None,
            'request': message(flow.request, secrets), 'response': message(flow.response, secrets),
            'request_http_version': flow.request.http_version,
            'response_http_version': flow.response.http_version if flow.response else None,
            'proxy_url': CONFIG['proxy_url'],
        }
        size = len(json.dumps(record, ensure_ascii=False).encode('utf-8'))
        if self.saved_bytes + size > 100 * 1024 * 1024:
            self.status('stopped', '会话达到 100 MiB 保存上限，请开始新的会话。')
            ctx.master.shutdown()
            return
        self.saved_bytes += size
        private_json(ROOT / 'flows' / (flow.id + '.json'), record)
        private_json(ROOT / 'summaries' / (flow.id + '.json'), {key: record[key] for key in ['id', 'method', 'url', 'status', 'started_at', 'duration_ms', 'error']})
        self.status('running')

    def done(self):
        if self.task:
            self.task.cancel()
        if (ROOT / 'status.json').exists():
            previous = json.loads((ROOT / 'status.json').read_text())
            if previous['state'] == 'running':
                self.status('stopped', previous.get('message', ''))


addons = [Capture()]
