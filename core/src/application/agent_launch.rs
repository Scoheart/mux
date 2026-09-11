//! Agent runtime discovery and explicit user-initiated launch requests.
use crate::settings::{load_settings_strict, mutate_settings_checked};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, process::Command};

pub use crate::domain::agents::{LaunchPreferences, LaunchTarget};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CatalogTarget {
    App { candidates: Vec<String>, host_name: Option<String> },
    Cli { candidates: Vec<String>, #[serde(default)] args: Vec<String> },
    Web { url: String },
}

#[derive(Serialize)]
pub struct LaunchInfo {
    pub agent_id: String,
    pub name: String,
    pub kind: Option<String>,
    pub available: bool,
    pub supported: bool,
    pub host_name: Option<String>,
    pub install_url: Option<String>,
    pub directory: Option<String>,
    pub directory_exists: bool,
    pub configured_target: Option<LaunchTarget>,
    pub resolved_target: Option<LaunchTarget>,
}

fn require_agent(id: &str) -> Result<String, String> {
    crate::agents::list_infos().into_iter().find(|agent| agent.id == id)
        .map(|agent| agent.name).ok_or_else(|| "未找到该 Agent".into())
}

fn home() -> Result<PathBuf, String> { dirs::home_dir().ok_or_else(|| "无法找到用户目录".into()) }
fn expand(value: &str) -> PathBuf {
    value.strip_prefix("~/").and_then(|suffix| dirs::home_dir().map(|root| root.join(suffix)))
        .unwrap_or_else(|| PathBuf::from(value))
}
fn app_exists(path: &Path) -> bool {
    path.is_absolute() && path.extension().is_some_and(|ext| ext == "app")
        && path.join("Contents/Info.plist").is_file()
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else { return false; };
    if !metadata.is_file() { return false; }
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; metadata.permissions().mode() & 0o111 != 0 }
    #[cfg(not(unix))] { true }
}
fn application(names: &[String]) -> Option<PathBuf> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Ok(home) = home() { roots.push(home.join("Applications")); }
    roots.push(PathBuf::from("/System/Applications"));
    names.iter().flat_map(|name| roots.iter().map(move |root| root.join(format!("{name}.app"))))
        .find(|path| app_exists(path))
}

fn find_command(name: &str) -> Option<PathBuf> {
    let direct = expand(name);
    if direct.is_absolute() { return executable(&direct).then_some(direct); }
    if name.is_empty() || name.starts_with('-') || name.contains(['/', '\\']) || name.chars().any(char::is_whitespace) {
        return None;
    }
    let mut paths = std::env::var_os("PATH").map(|value| std::env::split_paths(&value).collect::<Vec<_>>()).unwrap_or_default();
    if let Ok(home) = home() {
        paths.extend([".local/bin", ".cargo/bin", ".bun/bin", ".volta/bin", ".npm-global/bin"].map(|p| home.join(p)));
        // GUI launches do not inherit a login shell's Node manager PATH.
        for (root, suffix) in [(".local/share/fnm/node-versions", "installation/bin"),
            ("Library/Application Support/fnm/node-versions", "installation/bin"), (".nvm/versions/node", "bin")] {
            if let Ok(entries) = fs::read_dir(home.join(root)) {
                let mut versions = entries.filter_map(Result::ok).take(128).map(|e| e.path()).collect::<Vec<_>>();
                versions.sort_by_cached_key(|p| p.file_name().unwrap_or_default().to_string_lossy().trim_start_matches('v')
                    .split('.').map(|n| n.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>());
                paths.extend(versions.into_iter().rev().map(|path| path.join(suffix)));
            }
        }
    }
    paths.extend([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin"), PathBuf::from("/usr/bin")]);
    paths.into_iter().filter(|p| p.is_absolute()).map(|p| p.join(name)).find(|p| executable(p))
}

fn validate_target(target: &LaunchTarget) -> Result<(), String> {
    let no_nul = |v: &str| !v.is_empty() && !v.contains('\0') && v.len() <= 8192;
    match target {
        LaunchTarget::App { path } => {
            if !no_nul(path) || !app_exists(&expand(path)) { return Err("请选择已安装的 .app 应用".into()); }
        }
        LaunchTarget::Cli { command, args } => {
            if !no_nul(command) || find_command(command).is_none() { return Err("找不到可执行程序，请选择文件或输入已安装的命令名称".into()); }
            if args.len() > 64 || args.iter().any(|arg| arg.contains('\0') || arg.len() > 8192) { return Err("启动参数无效".into()); }
        }
        LaunchTarget::Web { url } => {
            let parsed = url::Url::parse(url).map_err(|_| "请输入有效的网址")?;
            if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() ||
                !parsed.username().is_empty() || parsed.password().is_some() {
                return Err("仅支持不含登录凭据的 HTTP / HTTPS 网址".into());
            }
        }
    }
    Ok(())
}

fn resolve(target: &LaunchTarget) -> Option<LaunchTarget> {
    match target {
        LaunchTarget::App { path } => app_exists(&expand(path)).then(|| LaunchTarget::App { path: expand(path).to_string_lossy().into_owned() }),
        LaunchTarget::Cli { command, args } => find_command(command).map(|path| LaunchTarget::Cli { command: path.to_string_lossy().into_owned(), args: args.clone() }),
        LaunchTarget::Web { .. } => validate_target(target).is_ok().then(|| target.clone()),
    }
}

pub fn info(agent_id: &str) -> Result<LaunchInfo, String> {
    super::gate::read(|| {
        let name = require_agent(agent_id)?;
        let prefs = load_settings_strict().map_err(|e| e.to_string())?.agent_launch
            .and_then(|mut values| values.remove(agent_id)).unwrap_or_default();
        let catalog: BTreeMap<String, CatalogTarget> = serde_json::from_str(include_str!("../../../data/agent-launchers.json")).map_err(|e| e.to_string())?;
        let links: BTreeMap<String, serde_json::Value> = serde_json::from_str(include_str!("../../../data/agent-install-links.json")).map_err(|e| e.to_string())?;
        let mut host_name = None;
        let mut kind = None;
        let resolved_target = if let Some(target) = &prefs.target {
            kind = Some(match target { LaunchTarget::App { .. } => "app", LaunchTarget::Cli { .. } => "cli", LaunchTarget::Web { .. } => "web" }.into());
            resolve(target)
        } else {
            match catalog.get(agent_id) {
                Some(CatalogTarget::App { candidates, host_name: host }) => {
                    kind = Some("app".into()); host_name = host.clone();
                    application(candidates).map(|path| LaunchTarget::App { path: path.to_string_lossy().into_owned() })
                }
                Some(CatalogTarget::Cli { candidates, args }) => {
                    kind = Some("cli".into());
                    candidates.iter().find_map(|name| find_command(name)).map(|command| LaunchTarget::Cli { command: command.to_string_lossy().into_owned(), args: args.clone() })
                }
                Some(CatalogTarget::Web { url }) => { kind = Some("web".into()); resolve(&LaunchTarget::Web { url: url.clone() }) }
                None => None,
            }
        };
        let supported = cfg!(target_os = "macos");
        Ok(LaunchInfo { agent_id: agent_id.into(), name, kind, supported, host_name,
            available: supported && resolved_target.is_some(),
            install_url: links.get(agent_id).and_then(|v| v.get("url")).and_then(|v| v.as_str()).map(str::to_owned),
            directory_exists: prefs.directory.as_ref().is_some_and(|p| expand(p).is_dir()),
            directory: prefs.directory, configured_target: prefs.target, resolved_target,
        })
    })
}

pub fn configure(agent_id: &str, target: Option<LaunchTarget>) -> Result<LaunchInfo, String> {
    super::gate::write_independent(|| {
        require_agent(agent_id)?;
        if let Some(target) = &target { validate_target(target)?; }
        mutate_settings_checked(|settings| {
            settings.agent_launch.get_or_insert_default().entry(agent_id.into()).or_default().target = target;
            Ok(())
        }).map_err(|e| e.to_string())
    })?;
    info(agent_id)
}

fn quote_shell(value: &str) -> String { format!("'{}'", value.replace('\'', "'\\''")) }

fn dispatch(target: &LaunchTarget, directory: Option<&Path>) -> Result<(), String> {
    if !cfg!(target_os = "macos") { return Err("当前平台暂不支持启动 Agent".into()); }
    validate_target(target)?;
    let mut command;
    match target {
        LaunchTarget::App { path } => { command = Command::new("/usr/bin/open"); command.arg("-a").arg(path); }
        LaunchTarget::Web { url } => { command = Command::new("/usr/bin/open"); command.arg(url); }
        LaunchTarget::Cli { command: program, args } => {
            let directory = directory.ok_or("请先选择工作目录")?;
            let bin = Path::new(program).parent().ok_or("无效的可执行程序路径")?;
            let line = format!("cd -- {} && PATH={}:\"$PATH\" {}{}", quote_shell(&directory.to_string_lossy()),
                quote_shell(&bin.to_string_lossy()), quote_shell(program), args.iter().map(|arg| format!(" {}", quote_shell(arg))).collect::<String>());
            // Pass shell text as an AppleScript argument, never interpolate it
            // into AppleScript source. No Agent command executes during discovery.
            let script = "on run argv\ntell application \"Terminal\"\nactivate\ndo script (item 1 of argv)\nend tell\nend run";
            command = Command::new("/usr/bin/osascript");
            command.args(["-e", script, "--", &line]);
        }
    }
    let result = command.output().map_err(|_| "无法调用系统启动器")?;
    if result.status.success() { Ok(()) }
    else { Err("系统未能打开 Agent；请检查应用状态或终端自动化权限".into()) }
}

#[derive(Serialize)]
pub struct LaunchReceipt { pub directory_saved: bool }

pub fn launch(agent_id: &str, directory: Option<String>) -> Result<LaunchReceipt, String> {
    let info = info(agent_id)?;
    let target = info.resolved_target.ok_or("未找到可启动的 Agent，请安装或设置启动方式")?;
    let chosen = if matches!(target, LaunchTarget::Cli { .. }) {
        let path = expand(&directory.or(info.directory).ok_or("请先选择工作目录")?);
        if !path.is_absolute() || !path.is_dir() { return Err("工作目录不存在，请重新选择".into()); }
        Some(path)
    } else { None };
    dispatch(&target, chosen.as_deref())?;
    // Failure to remember a preference must not imply that launch failed or
    // invite a second click that would start a duplicate terminal session.
    let directory_saved = chosen.map(|path| super::gate::write_independent(|| {
        mutate_settings_checked(|settings| {
            settings.agent_launch.get_or_insert_default().entry(agent_id.into()).or_default().directory = Some(path.to_string_lossy().into_owned());
            Ok(())
        }).map_err(|e| e.to_string())
    }).is_ok()).unwrap_or(true);
    Ok(LaunchReceipt { directory_saved })
}
