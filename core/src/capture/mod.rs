//! Local, opt-in Agent traffic capture. The engine is a separately installed mitmproxy.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const ADDON: &str = include_str!("addon.py");
static ACTIVE: LazyLock<Mutex<Option<Worker>>> = LazyLock::new(|| Mutex::new(None));
struct Worker { session_id: String, child: Child }

#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureTarget { pub id: String, pub name: String, pub pids: Vec<u32>, pub agent_ids: Vec<String> }
#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureEnvironment {
    pub supported: bool, pub engine: Option<String>, pub extension_enabled: bool,
    pub certificate_present: bool, pub certificate_trusted: bool, pub targets: Vec<CaptureTarget>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct StartCapture { pub agent_id: String, pub target_id: String, pub egress: String, pub proxy_url: Option<String> }
#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureSession {
    pub id: String, pub agent_id: String, pub agent_name: String, pub target_id: String,
    pub target_name: String, pub pids: Vec<u32>, pub proxy_url: Option<String>, pub started_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureState { pub state: String, pub message: String, pub pids: Vec<u32>, pub flow_count: usize }
#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureSnapshot { pub session: CaptureSession, pub status: CaptureState, pub flows: Vec<FlowSummary> }
#[derive(Clone, Serialize, Deserialize)]
pub struct FlowSummary {
    pub id: String, pub method: String, pub url: String, pub status: Option<u16>,
    pub started_at: f64, pub duration_ms: Option<u64>, pub error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CapturedMessage { pub headers: Vec<(String, String)>, pub body: String, pub bytes: usize, pub truncated: bool, pub encoding: String }
#[derive(Clone, Serialize, Deserialize)]
pub struct CapturedFlow {
    #[serde(flatten)] pub summary: FlowSummary,
    pub request: Option<CapturedMessage>, pub response: Option<CapturedMessage>,
    pub request_http_version: String, pub response_http_version: Option<String>, pub proxy_url: Option<String>,
}

fn root() -> PathBuf { crate::paths::mux_dir().join("captures") }
fn session_dir(id: &str) -> Result<PathBuf, String> {
    Uuid::parse_str(id).map_err(|_| "抓包会话 ID 无效。".to_owned())?;
    let path = root().join(id);
    reject_symlink(&root())?;
    reject_symlink(&path)?;
    Ok(path)
}
fn reject_symlink(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return Err("抓包文件不能是符号链接。".into());
    }
    Ok(())
}
fn private_dir(path: &Path) -> Result<(), String> {
    reject_symlink(path)?;
    fs::create_dir_all(path).map_err(|_| "无法创建抓包目录。".to_owned())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| "无法收紧抓包目录权限。".to_owned())?; }
    Ok(())
}
fn private_write(path: &Path, content: &[u8]) -> Result<(), String> {
    reject_symlink(path)?;
    let mut options = OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    options.open(path).and_then(|mut f| f.write_all(content)).map_err(|_| "无法保存抓包文件；目标可能已经存在。".into())
}
fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> Result<T, String> {
    reject_symlink(path)?;
    let mut file = fs::File::open(path).map_err(|_| "抓包记录不存在。".to_owned())?;
    if file.metadata().map_err(|_| "无法读取抓包记录。".to_owned())?.len() > 20 * 1024 * 1024 { return Err("抓包记录超过读取上限。".into()); }
    let mut bytes = Vec::new(); file.read_to_end(&mut bytes).map_err(|_| "无法读取抓包记录。".to_owned())?;
    serde_json::from_slice(&bytes).map_err(|_| "抓包记录尚未写完或已经损坏。".into())
}
fn tool() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    dirs.extend([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")]);
    if let Some(home) = dirs::home_dir() { dirs.push(home.join(".local/bin")); }
    dirs.into_iter().map(|d| d.join("mitmdump")).find(|p| p.is_file())
}
fn output(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program).args(args).stderr(Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}
struct Process { pid: u32, parent: u32, command: String }
fn processes() -> Vec<Process> {
    output("/bin/ps", &["-axo", "pid=,ppid=,comm="]).unwrap_or_default().lines().filter_map(|line| {
        let line = line.trim(); let split = line.find(char::is_whitespace)?;
        let pid = line[..split].parse().ok()?;
        let rest = line[split..].trim_start(); let split = rest.find(char::is_whitespace)?;
        Some(Process { pid, parent: rest[..split].parse().ok()?, command: rest[split..].trim_start().to_owned() })
    }).collect()
}
fn expand_pids(rows: &[Process], initial: &[u32]) -> Vec<u32> {
    let mut pids: BTreeSet<u32> = initial.iter().copied().collect();
    loop { let before = pids.len(); for row in rows { if pids.contains(&row.parent) { pids.insert(row.pid); } } if before == pids.len() { break; } }
    pids.into_iter().collect()
}
fn targets() -> Vec<CaptureTarget> {
    let rows = processes();
    let agents = crate::agents::load_agents();
    let mut groups: BTreeMap<String, CaptureTarget> = BTreeMap::new();
    for row in &rows {
        let (id, name) = if let Some(end) = row.command.find(".app/Contents/") {
            let app = &row.command[..end + 4];
            (format!("app:{app}"), Path::new(app).file_stem().unwrap_or_default().to_string_lossy().into_owned())
        } else {
            let name = Path::new(&row.command).file_name().unwrap_or_default().to_string_lossy().into_owned();
            let known = agents.iter().any(|(_, def)| def.skills.as_ref().is_some_and(|skills| skills.probes.iter().any(|probe| matches!(probe, crate::domain::types::AgentInstallProbe::Command { name: command } if command == &name))));
            if !known || matches!(name.as_str(), "node" | "python" | "python3" | "bash" | "zsh") { continue; }
            (format!("exe:{}", row.command), name)
        };
        if name == "MUX" || name == "Mitmproxy Redirector" { continue; }
        let entry = groups.entry(id.clone()).or_insert_with(|| CaptureTarget { id, name, pids: vec![], agent_ids: vec![] });
        entry.pids.push(row.pid);
    }
    for target in groups.values_mut() {
        target.pids = expand_pids(&rows, &target.pids);
        for (id, def) in &agents {
            let name_match = def.name.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(&target.name));
            let probe_match = def.skills.as_ref().is_some_and(|skills| skills.probes.iter().any(|probe| match probe {
                crate::domain::types::AgentInstallProbe::Path { path } => target.id == format!("app:{path}"),
                crate::domain::types::AgentInstallProbe::Command { name } => target.name == *name,
                _ => false,
            }));
            if name_match || probe_match { target.agent_ids.push(id.clone()); }
        }
    }
    groups.into_values().collect()
}

pub fn environment() -> CaptureEnvironment {
    let supported = cfg!(target_os = "macos");
    let extension_enabled = output("/usr/bin/systemextensionsctl", &["list"]).is_some_and(|s| s.lines().any(|l| l.contains("org.mitmproxy.macos-redirector.network-extension") && l.contains("activated enabled")));
    let ca = dirs::home_dir().unwrap_or_default().join(".mitmproxy/mitmproxy-ca-cert.pem");
    let certificate_trusted = ca.is_file() && Command::new("/usr/bin/security").args(["verify-cert", "-c"]).arg(&ca).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    CaptureEnvironment { supported, engine: tool().map(|p| p.to_string_lossy().into_owned()), extension_enabled, certificate_present: ca.is_file(), certificate_trusted, targets: if supported { targets() } else { vec![] } }
}

fn proxy(request: &StartCapture) -> Result<Option<String>, String> {
    let value = match request.egress.as_str() { "direct" => return Ok(None), "mux" => crate::network::configured_proxy_url()?.ok_or("MUX 尚未配置代理出口。")?, "custom" => request.proxy_url.clone().ok_or("请填写代理地址。")?, _ => return Err("出口类型无效。".into()) };
    let url = url::Url::parse(value.trim()).map_err(|_| "请输入完整 HTTP 代理地址。".to_owned())?;
    if url.scheme() != "http" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || !matches!(url.path(), "" | "/") {
        return Err("第一版支持无认证 HTTP 上游代理，例如 http://127.0.0.1:6789。不会回退直连。".into());
    }
    // Validate reachability without issuing a target request or exposing a credential.
    use std::net::ToSocketAddrs;
    let host = url.host_str().unwrap().trim_matches(|c| c == '[' || c == ']');
    let addresses = (host, url.port_or_known_default().unwrap_or(80)).to_socket_addrs().map_err(|_| "无法解析代理出口。".to_owned())?;
    if !addresses.take(4).any(|a| std::net::TcpStream::connect_timeout(&a, Duration::from_secs(2)).is_ok()) { return Err("代理出口无法连接；未回退直连。".into()); }
    Ok(Some(value.trim().trim_end_matches('/').to_owned()))
}

pub fn start(request: StartCapture) -> Result<CaptureSnapshot, String> {
    let mut active = ACTIVE.lock().map_err(|_| "抓包会话锁失败。".to_owned())?;
    if let Some(worker) = active.as_mut() { if worker.child.try_wait().map_err(|_| "无法查询抓包进程。".to_owned())?.is_none() { return Err("请先停止当前抓包会话。".into()); } }
    active.take();
    let env = environment();
    if !env.supported { return Err("第一版按应用抓包仅支持 macOS。".into()); }
    if !env.extension_enabled { return Err("请在系统设置的登录项与扩展中启用 Mitmproxy Redirector 网络扩展。".into()); }
    let engine = env.engine.ok_or("未找到 mitmdump，请先安装官方 mitmproxy。")?;
    let target = env.targets.into_iter().find(|t| t.id == request.target_id).ok_or("目标进程已退出，请刷新应用列表。")?;
    let agents = crate::agents::load_agents();
    let agent = agents.get(&request.agent_id).ok_or("请选择有效的 Agent。")?;
    let session = CaptureSession { id: Uuid::new_v4().to_string(), agent_id: request.agent_id.clone(), agent_name: agent.name.clone().unwrap_or(request.agent_id.clone()), target_id: target.id, target_name: target.name, pids: target.pids, proxy_url: proxy(&request)?, started_at: chrono::Utc::now().to_rfc3339() };
    private_dir(&root())?;
    let directory = session_dir(&session.id)?;
    private_dir(&directory)?; private_dir(&directory.join("flows"))?; private_dir(&directory.join("summaries"))?;
    private_write(&directory.join("session.json"), &serde_json::to_vec(&session).map_err(|_| "无法保存会话。".to_owned())?)?;
    private_write(&directory.join("addon.py"), ADDON.as_bytes())?;
    let child = Command::new(engine).args(["-q", "--mode", &format!("local:{}", session.pids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")), "--set", "connection_strategy=lazy", "--set", "upstream_cert=false", "--set", "rawtcp=false", "-s"]).arg(directory.join("addon.py")).env("MUX_CAPTURE_DIR", &directory).env("MUX_CAPTURE_OWNER", std::process::id().to_string()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_| "无法启动抓包引擎。".to_owned())?;
    *active = Some(Worker { session_id: session.id.clone(), child });
    drop(active);
    snapshot(&session.id)
}

pub fn sessions() -> Result<Vec<CaptureSession>, String> {
    if !root().exists() { return Ok(vec![]); }
    reject_symlink(&root())?;
    let mut result = vec![];
    for entry in fs::read_dir(root()).map_err(|_| "无法读取抓包会话。".to_owned())?.take(1000).flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(directory) = session_dir(&id) { if let Ok(session) = read_json(&directory.join("session.json")) { result.push(session); } }
    }
    result.sort_by(|a: &CaptureSession, b| b.started_at.cmp(&a.started_at));
    Ok(result)
}
pub fn snapshot(id: &str) -> Result<CaptureSnapshot, String> {
    let directory = session_dir(id)?;
    let session: CaptureSession = read_json(&directory.join("session.json"))?;
    let mut status: CaptureState = read_json(&directory.join("status.json")).unwrap_or(CaptureState { state: "starting".into(), message: "正在启动抓包引擎…".into(), pids: session.pids.clone(), flow_count: 0 });
    let mut active = ACTIVE.lock().map_err(|_| "抓包会话锁失败。".to_owned())?;
    let live = if let Some(worker) = active.as_mut().filter(|w| w.session_id == id) { worker.child.try_wait().map_err(|_| "无法查询抓包进程。".to_owned())?.is_none() } else { false };
    if !live && matches!(status.state.as_str(), "running" | "starting") { if status.state == "starting" { status.state = "failed".into(); status.message = "抓包引擎启动失败，请检查 mitmproxy 版本与网络扩展。".into(); } else { status.state = "stopped".into(); status.message = "抓包进程已结束；记录仍可查看。".into(); } }
    let mut flows = vec![];
    reject_symlink(&directory.join("summaries"))?;
    for file in fs::read_dir(directory.join("summaries")).map_err(|_| "无法读取抓包请求。".to_owned())?.take(5000).flatten() { if file.path().extension().is_some_and(|e| e == "json") { if let Ok(flow) = read_json::<FlowSummary>(&file.path()) { flows.push(flow); } } }
    flows.sort_by(|a, b| b.started_at.total_cmp(&a.started_at));
    status.flow_count = flows.len();
    Ok(CaptureSnapshot { session, status, flows })
}
pub fn detail(session_id: &str, flow_id: &str) -> Result<CapturedFlow, String> {
    Uuid::parse_str(flow_id).map_err(|_| "请求 ID 无效。".to_owned())?;
    let path = session_dir(session_id)?.join("flows"); reject_symlink(&path)?;
    read_json(&path.join(format!("{flow_id}.json")))
}
pub fn stop(id: &str) -> Result<CaptureSnapshot, String> {
    let mut active = ACTIVE.lock().map_err(|_| "抓包会话锁失败。".to_owned())?;
    if let Some(worker) = active.as_mut().filter(|w| w.session_id == id) {
        let path = session_dir(id)?.join("stop"); if !path.exists() { private_write(&path, b"stop")?; }
        let until = Instant::now() + Duration::from_secs(4);
        while worker.child.try_wait().map_err(|_| "无法停止抓包。".to_owned())?.is_none() {
            if Instant::now() >= until { worker.child.kill().map_err(|_| "无法停止抓包引擎。".to_owned())?; let _ = worker.child.wait(); break; }
            std::thread::sleep(Duration::from_millis(100));
        }
        active.take();
    }
    drop(active); snapshot(id)
}
pub fn shutdown() { let id = ACTIVE.lock().ok().and_then(|a| a.as_ref().map(|w| w.session_id.clone())); if let Some(id) = id { let _ = stop(&id); } }
pub fn export(session_id: &str, flow_id: Option<&str>, destination: &Path) -> Result<(), String> {
    let content = if let Some(id) = flow_id { serde_json::to_vec_pretty(&detail(session_id, id)?).map_err(|_| "无法导出请求。".to_owned())? } else {
        let snapshot = snapshot(session_id)?;
        let flows = snapshot.flows.iter().map(|f| detail(session_id, &f.id)).collect::<Result<Vec<_>, _>>()?;
        serde_json::to_vec_pretty(&serde_json::json!({"session": snapshot.session, "status": snapshot.status, "redacted": true, "flows": flows})).map_err(|_| "无法导出会话。".to_owned())?
    };
    private_write(destination, &content)
}
