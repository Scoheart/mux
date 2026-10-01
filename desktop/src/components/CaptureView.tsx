import { useCallback, useEffect, useRef, useState } from 'react';
import type { AgentInfo } from '../lib/types';
import { captureDetail, captureEnvironment, captureExport, captureSessions, captureSnapshot, captureStart, captureStop, type CaptureEnvironment, type CaptureSession, type CaptureSnapshot, type CapturedFlow } from '../lib/capture';
import { copyToClipboard } from '../lib/api';
import { formatError } from '../lib/format';
import { useToast } from './Toast';
import { NetworkIcon, RefreshIcon, DownloadIcon } from './icons';
import { Badge } from './ui';
import './CaptureView.css';

const stateLabel: Record<string, string> = { starting: '启动中', running: '拦截运行中', stopped: '已停止', failed: '启动或拦截失败' };
function urlParts(url: string) { try { const value = new URL(url); return { host: value.host, path: value.pathname + value.search }; } catch { return { host: '', path: url }; } }

export function CaptureView({ agents, initialAgentId, muxProxy }: { agents: AgentInfo[]; initialAgentId?: string; muxProxy: string | null }) {
  const toast = useToast();
  const [environment, setEnvironment] = useState<CaptureEnvironment | null>(null);
  const [sessions, setSessions] = useState<CaptureSession[]>([]);
  const [snapshot, setSnapshot] = useState<CaptureSnapshot | null>(null);
  const [agentId, setAgentId] = useState(initialAgentId ?? agents[0]?.id ?? '');
  const [targetId, setTargetId] = useState('');
  const [egress, setEgress] = useState(muxProxy ? 'mux' : 'direct');
  const [proxy, setProxy] = useState('');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [flow, setFlow] = useState<CapturedFlow | null>(null);
  const [flowId, setFlowId] = useState('');
  const [side, setSide] = useState<'request' | 'response'>('request');
  const [section, setSection] = useState<'body' | 'headers'>('body');
  const [busy, setBusy] = useState(false);
  const [loadingFlow, setLoadingFlow] = useState(false);
  const [loading, setLoading] = useState(true);
  const epoch = useRef(0);
  const detailEpoch = useRef(0);
  const active = snapshot?.status.state === 'running' || snapshot?.status.state === 'starting';
  const [failure, setFailure] = useState('');
  const refresh = useCallback(async () => {
    const result = await Promise.all([captureEnvironment(), captureSessions()]);
    setEnvironment(result[0]); setSessions(result[1]); return result;
  }, []);
  useEffect(() => { let disposed = false; void refresh().then(async ([, list]) => {
    if (disposed) return;
    if (list.length) { const current = await captureSnapshot(list[0].id); if (!disposed) setSnapshot(current); }
  }).catch(error => { if (!disposed) setFailure(formatError(error)); }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; epoch.current++; detailEpoch.current++; };
  }, [refresh]);
  useEffect(() => { if (initialAgentId) setAgentId(initialAgentId); }, [initialAgentId]);
  useEffect(() => {
    const suggested = environment?.targets.filter(target => target.agent_ids.includes(agentId)) ?? [];
    setTargetId(suggested[0]?.id ?? '');
  }, [agentId, environment]);
  useEffect(() => {
    if (!snapshot || !active) return;
    const id = snapshot.session.id; const generation = epoch.current;
    let stopped = false; let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { const current = await captureSnapshot(id); if (!stopped && generation === epoch.current) { setSnapshot(current); setFailure(''); } }
      catch (error) { if (!stopped) setFailure(formatError(error)); }
      if (!stopped) timer = setTimeout(() => void poll(), 1500);
    };
    timer = setTimeout(() => void poll(), 1500);
    return () => { stopped = true; clearTimeout(timer); };
  }, [snapshot?.session.id, active]);
  const perform = async (operation: () => Promise<void>) => { setBusy(true); try { await operation(); setFailure(''); } catch (error) { const message = formatError(error); setFailure(message); toast.show({ kind: 'error', msg: message }); } finally { setBusy(false); } };
  const selectSession = async (id: string) => {
    const generation = ++epoch.current; detailEpoch.current++; setFlow(null); setFlowId('');
    if (!id) { setSnapshot(null); return; }
    const current = await captureSnapshot(id); if (generation === epoch.current) setSnapshot(current);
  };
  const selectFlow = async (id: string) => {
    if (!snapshot) return;
    const generation = ++detailEpoch.current; setFlowId(id); setFlow(null); setLoadingFlow(true);
    try { const current = await captureDetail(snapshot.session.id, id); if (generation === detailEpoch.current) setFlow(current); }
    catch (error) { if (generation === detailEpoch.current) setFailure(formatError(error)); }
    finally { if (generation === detailEpoch.current) setLoadingFlow(false); }
  };
  const target = environment?.targets.find(value => value.id === targetId);
  const flows = (snapshot?.flows ?? []).filter(value => (`${value.method} ${value.url}`).toLowerCase().includes(query.toLowerCase()) && (filter === 'all' || (filter === 'errors' ? value.error || (value.status ?? 0) >= 400 : (value.status ?? 0) >= 200 && (value.status ?? 0) < 400)));
  const message = flow?.[side];
  const content = section === 'headers' ? message?.headers.map(([key, value]) => `${key}: ${value}`).join('\n') ?? '' : message?.body ?? '';
  const canStart = !loading && environment?.supported && environment.engine && environment.extension_enabled && agentId && targetId && !active && !busy;
  return <div className="mux-capture">
    <header className="mux-capture-heading"><div><h1><NetworkIcon className="w-5 h-5" />Agent 抓包</h1><p>选择运行中的 Agent 进程，指定出口，查看请求与响应。</p></div><button className="mux-capture-button" disabled={busy || loading} onClick={() => void perform(async () => { await refresh(); })}><RefreshIcon className="w-4 h-4" />刷新环境</button></header>
    <section className="mux-capture-controls" aria-label="抓包会话配置">
      <label>Agent<select value={agentId} disabled={Boolean(active)} onChange={event => setAgentId(event.target.value)}><option value="">选择 Agent</option>{agents.map(agent => <option key={agent.id} value={agent.id}>{agent.name}</option>)}</select></label>
      <label>运行中的应用 / 进程<select value={targetId} disabled={Boolean(active)} onChange={event => setTargetId(event.target.value)}><option value="">选择实际拦截目标</option>{environment?.targets.map(value => <option key={value.id} value={value.id}>{value.name}{value.agent_ids.includes(agentId) ? ' · 匹配此 Agent' : ''} · {value.pids.length} 个进程</option>)}</select></label>
      <label>出口<select value={egress} disabled={Boolean(active)} onChange={event => setEgress(event.target.value)}><option value="direct">直连</option><option value="mux" disabled={!muxProxy}>使用 MUX 代理</option><option value="custom">自定义 HTTP 代理</option></select></label>
      {egress === 'custom' && <label className="mux-capture-proxy">代理地址<input value={proxy} disabled={Boolean(active)} placeholder="http://proxy.example.com:8080" onChange={event => setProxy(event.target.value)} /></label>}
      <button className="mux-capture-button mux-capture-primary" disabled={!active && !canStart || busy} onClick={() => void perform(async () => {
        if (active && snapshot) { setSnapshot(await captureStop(snapshot.session.id)); }
        else { epoch.current++; detailEpoch.current++; setFlow(null); setFlowId(''); setSnapshot(await captureStart({ agent_id: agentId, target_id: targetId, egress, proxy_url: egress === 'custom' ? proxy : null })); await refresh(); }
      })}>{busy ? '处理中…' : active ? '停止抓包' : '开始抓包'}</button>
    </section>
    <div className="mux-capture-health" role="status">
      {loading ? <span>正在检查抓包环境…</span> : <><Badge tone={environment?.engine ? 'success' : 'warning'}>{environment?.engine ? '抓包引擎可用' : '请安装 mitmproxy'}</Badge><Badge tone={environment?.extension_enabled ? 'success' : 'warning'}>{environment?.extension_enabled ? '网络扩展已启用' : '网络扩展未启用'}</Badge><Badge tone={environment?.certificate_trusted ? 'success' : 'warning'}>{environment?.certificate_trusted ? '系统信任抓包 CA' : environment?.certificate_present ? '系统未信任抓包 CA' : '抓包 CA 尚未生成'}</Badge>{snapshot && <Badge>{stateLabel[snapshot.status.state] ?? snapshot.status.state} · {snapshot.status.flow_count} 条</Badge>}</>}
    </div>
    <div className="mux-capture-scope">{active ? `当前范围：${snapshot?.session.target_name} · PID ${snapshot?.status.pids.join(', ')}` : target ? `将拦截 ${target.name} 及其子进程：${target.pids.join(', ')}` : '先启动目标 Agent，再选择实际进程。共享 Node / Python 进程不自动绑定。'}<span>出口：{active ? snapshot?.session.proxy_url ?? '直连' : egress === 'direct' ? '直连' : egress === 'mux' ? muxProxy : proxy || '待填写'}{(active ? snapshot?.session.proxy_url : egress !== 'direct') ? ' · 失败不回退直连' : ''}</span></div>
    {Boolean(environment?.other_extensions?.length) && <p className="mux-capture-warning">检测到 {environment?.other_extensions.join('、')} 网络扩展。它们可能先接管目标连接；抓包为空时，请只暂停目标应用的原代理规则，再重启目标应用建立新连接。结束抓包后恢复原规则。</p>}
    {failure && <p className="mux-capture-warning" role="alert">{failure}</p>}
    {snapshot?.status.message && <details className="mux-capture-warning" open={snapshot.status.state !== 'stopped'}><summary>{snapshot.status.state === 'stopped' ? '会话历史提示' : '会话提示'}</summary><p>{[...new Set(snapshot.status.message.split(/(?<=。)\s*/).filter(Boolean))].join(' ')}</p></details>}
    <details className="mux-capture-notes"><summary>覆盖范围与数据保存</summary><p>第一版支持 macOS 本机进程的 HTTP / HTTPS。本机回环通信保留原出口，HTTPS 不解密，以免破坏应用内部通信；Agent 自身使用本机代理时，需要先停用原代理配置。客户端仍需信任 ~/.mitmproxy/mitmproxy-ca-cert.pem；系统信任不代表所有运行时均已信任。证书固定、UDP / QUIC、二进制正文及未触发功能不保证覆盖。HTTP 代理出口不支持的连接会失败，不回退直连。代理出口下的 DNS 经同一代理查询 Cloudflare DoH，不回退系统直连 DNS。新进程范围切换可能有短暂间隙。</p><p>记录仅保存本机。常见认证头、令牌字段会脱敏；每条正文最多 2 MiB，二进制或超限正文不保存。会话最多 5000 条 / 100 MiB，导出是脱敏 JSON。第一版不支持带认证的上游代理。</p></details>
    <div className="mux-capture-toolbar"><label>会话<select value={snapshot?.session.id ?? ''} disabled={busy || Boolean(active)} onChange={event => void perform(() => selectSession(event.target.value))}><option value="">新会话</option>{sessions.map(value => <option key={value.id} value={value.id}>{value.agent_name} · {new Date(value.started_at).toLocaleString()}</option>)}</select></label><input type="search" aria-label="搜索请求" placeholder="搜索域名、路径或方法" value={query} onChange={event => setQuery(event.target.value)} /><select aria-label="筛选请求" value={filter} onChange={event => setFilter(event.target.value)}><option value="all">全部请求</option><option value="success">成功</option><option value="errors">错误</option></select><button className="mux-capture-button" disabled={!snapshot || busy} onClick={() => void perform(async () => { if (snapshot && await captureExport(snapshot.session.id)) toast.show({ kind: 'success', msg: '会话已导出。' }); })}><DownloadIcon className="w-4 h-4" />导出会话</button></div>
    <div className="mux-capture-workspace">
      <section className="mux-capture-list" aria-label="请求列表"><div className="mux-capture-list-title">请求 <span>{flows.length}</span></div>{flows.length === 0 ? <div className="mux-capture-empty">{active ? '等待新请求…\n在目标 Agent 中执行操作以触发流量。' : '开始抓包后，请求会显示在这里。'}</div> : flows.map(value => { const url = urlParts(value.url); return <button key={value.id} className="mux-capture-row" data-selected={flowId === value.id} onClick={() => void selectFlow(value.id)}><div><b>{value.method}</b><span data-error={Boolean(value.error || (value.status ?? 0) >= 400)}>{value.status ?? '失败'}</span><time>{new Date(value.started_at * 1000).toLocaleTimeString()}</time></div><strong>{url.host}</strong><code>{url.path}</code><small>{value.duration_ms === null ? value.error : `${value.duration_ms} ms`}</small></button>; })}</section>
      <section className="mux-capture-detail" aria-label="请求响应详情">{!flow ? <div className="mux-capture-empty">{loadingFlow ? '正在读取请求…' : '选择一条请求，查看 req / res。'}</div> : <><div className="mux-capture-detail-title"><code>{flow.method} {flow.url}</code><button className="mux-capture-button" disabled={busy} onClick={() => void perform(async () => { if (snapshot && await captureExport(snapshot.session.id, flow.id)) toast.show({ kind: 'success', msg: '请求与响应已导出。' }); })}>导出 req/res</button></div><div className="mux-capture-tabs"><div className="mux-seg">{(['request', 'response'] as const).map(value => <button key={value} className="mux-seg-item" data-active={side === value || undefined} aria-pressed={side === value} onClick={() => setSide(value)}>{value === 'request' ? 'Request' : `Response · ${flow.status ?? '失败'}`}</button>)}</div><div className="mux-seg">{(['body', 'headers'] as const).map(value => <button key={value} className="mux-seg-item" data-active={section === value || undefined} aria-pressed={section === value} onClick={() => setSection(value)}>{value === 'body' ? 'Body' : 'Headers'}</button>)}</div><button className="mux-capture-button" disabled={!content} onClick={() => void perform(async () => { await copyToClipboard(content); toast.show({ kind: 'success', msg: '已复制。' }); })}>复制</button></div><div className="mux-capture-detail-meta">{message ? `${message.bytes.toLocaleString()} 字节 · ${message.encoding === 'text' ? '文本 / SSE' : '二进制未保存'} · 常见凭据已脱敏${message.truncated ? ' · 正文超限未保存' : ''}` : flow.error ?? '没有响应正文'}</div><pre tabIndex={0}>{content || '（空）'}</pre></>}</section>
    </div>
  </div>;
}
