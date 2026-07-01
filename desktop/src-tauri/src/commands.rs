use crate::core::registry::{
    delete_registry_entry as delete_entry, read_registry, user_override_keys,
    write_registry_entry as write_entry,
};
use crate::core::types::RegistryEntry;

#[tauri::command]
pub fn list_registry() -> Vec<RegistryEntry> {
    // Read user overrides from settings.registry merged over builtin — same source
    // scan_installed / apply_install resolve against, so the UI stays consistent.
    read_registry()
}

/// Persist (create or overwrite) a user registry entry into settings.registry.
#[tauri::command]
pub fn upsert_registry_entry(entry: RegistryEntry) -> Result<(), String> {
    write_entry(&entry).map_err(|e| e.to_string())
}

/// Remove a user registry override for a given name+transport; reverts to
/// builtin if one exists.
#[tauri::command]
pub fn delete_registry_entry(name: String, transport: String) -> Result<(), String> {
    delete_entry(&name, &transport).map_err(|e| e.to_string())
}

/// Composite keys (`name::transport`) of registry entries that currently have a
/// user override.
#[tauri::command]
pub fn list_custom_registry_keys() -> Vec<String> {
    user_override_keys()
}

use crate::core::agents::{load_agents, save_agents};
use crate::core::scanner::scan_agents;
use crate::core::types::AgentDefinition;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub format: String,
    pub key: String,
    pub has_global: bool,
    pub has_project: bool,
    pub enabled: bool,
    /// Raw stored config paths (e.g. `~/Library/Application Support/…/mcp.json`),
    /// so the UI can display + prefill the path editor.
    pub global: Option<String>,
    pub project: Option<String>,
}

/// Collapse an absolute path under the user's home directory back to `~/…` so
/// stored agent paths stay portable (we never hardcode `/Users/<name>`). Paths
/// that already start with `~`, or that live outside home, are returned as-is.
fn collapse_home(path: &str) -> String {
    let path = path.trim();
    if path.starts_with('~') {
        return path.to_string();
    }
    if let Some(home) = dirs::home_dir() {
        if let Some(home_str) = home.to_str() {
            if path == home_str {
                return "~".to_string();
            }
            if let Some(rest) = path.strip_prefix(&format!("{}/", home_str)) {
                return format!("~/{}", rest);
            }
        }
    }
    path.to_string()
}

/// Validate + normalize an agent definition, then write it. `allow_overwrite`
/// distinguishes create (`add_agent`, errors on existing) from edit
/// (`update_agent`, replaces in place). Global paths are collapsed to `~/…`;
/// project paths are relative to the project root and left untouched (only trimmed).
fn put_agent(id: String, mut def: AgentDefinition, allow_overwrite: bool) -> Result<(), String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("agent id 不能为空".into());
    }
    if def.key.trim().is_empty() {
        return Err("配置 key 不能为空".into());
    }
    def.global = def
        .global
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(collapse_home);
    def.project = def
        .project
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if def.global.is_none() && def.project.is_none() {
        return Err("至少需要填写一个配置路径（全局或项目）".into());
    }
    let mut agents = load_agents();
    if !allow_overwrite && agents.contains_key(&id) {
        return Err(format!("agent 已存在: {}", id));
    }
    agents.insert(id, def);
    save_agents(&agents).map_err(|e| e.to_string())
}

/// 新增一个自定义 agent，持久化到 settings.agents（在内置/已有定义之上合并）。
/// id 为空或已存在时报错，避免误覆盖内置 agent。
#[tauri::command]
pub fn add_agent(id: String, def: AgentDefinition) -> Result<(), String> {
    put_agent(id, def, false)
}

/// 编辑一个已存在 agent 的配置（路径 / 格式 / key），覆盖写回 settings.agents。
#[tauri::command]
pub fn update_agent(id: String, def: AgentDefinition) -> Result<(), String> {
    put_agent(id, def, true)
}

#[tauri::command]
pub fn list_agents() -> Vec<AgentInfo> {
    load_agents()
        .into_iter()
        .map(|(id, d)| AgentInfo {
            id,
            format: d.format,
            key: d.key,
            has_global: d.global.is_some(),
            has_project: d.project.is_some(),
            enabled: d.enabled,
            global: d.global,
            project: d.project,
        })
        .collect()
}

#[cfg(test)]
mod agent_path_tests {
    use super::collapse_home;
    #[test]
    fn tilde_and_outside_paths_unchanged() {
        assert_eq!(collapse_home("~/Library/X/mcp.json"), "~/Library/X/mcp.json");
        assert_eq!(collapse_home("/etc/elsewhere.json"), "/etc/elsewhere.json");
    }
    #[test]
    fn absolute_home_path_collapses_to_tilde() {
        if let Some(home) = dirs::home_dir() {
            let abs = format!("{}/Library/App Support/mcp.json", home.display());
            assert_eq!(collapse_home(&abs), "~/Library/App Support/mcp.json");
        }
    }
}

#[derive(Serialize)]
pub struct InstalledMcp {
    pub name: String,
    pub agent: String,
    pub scope: String,
    pub file_path: String,
    /// Transport bucket of the installed config ("stdio" | "http"), used to
    /// attribute the install to the matching registry variant.
    pub transport: String,
    #[serde(default)]
    pub customized: bool,
    /// Whether this server is currently active in the agent's config file
    /// (`true`) or merely remembered in MUX's disabled store (`false`).
    #[serde(default)]
    pub enabled: bool,
}

/// 扫描真实配置文件，返回「谁装在哪」。project_dir 为空则只扫 global。
#[tauri::command]
pub fn scan_installed(project_dir: Option<String>) -> Vec<InstalledMcp> {
    use crate::core::effective::base_config;
    use crate::core::types::transport_of;
    // Build (name::transport) -> base McpConfig map from read_registry (same
    // source as apply_install) so each transport variant compares independently.
    let base_map: HashMap<String, crate::core::types::McpConfig> = {
        let reg = read_registry();
        reg.into_iter()
            .filter_map(|e| {
                let key = e.key();
                let base = base_config(&e)?;
                Some((key, base))
            })
            .collect()
    };
    let agents = load_agents();
    let pd = project_dir.as_deref().map(Path::new);
    let mut out: Vec<InstalledMcp> = scan_agents(&agents, pd, true)
        .into_iter()
        .map(|s| {
            let transport = transport_of(&s.config);
            let key = format!("{}::{}", s.name, transport);
            let customized = base_map
                .get(&key)
                .map(|base| base != &s.config)
                .unwrap_or(false);
            InstalledMcp {
                name: s.name, agent: s.agent, scope: s.scope,
                file_path: s.file_path, transport: transport.to_string(),
                customized, enabled: true,
            }
        })
        .collect();
    // Append MUX-remembered disabled servers so the UI can show an "off" row for
    // a server that was removed from the agent file but can be re-enabled.
    let active: std::collections::HashSet<(String, String, String, String)> = out
        .iter()
        .map(|i| (i.agent.clone(), i.name.clone(), i.transport.clone(), i.scope.clone()))
        .collect();
    for (agent, list) in crate::core::disabled::load_disabled() {
        for d in list {
            // Edge case: if somehow also present in the file, the active row wins.
            if active.contains(&(agent.clone(), d.name.clone(), d.transport.clone(), d.scope.clone())) {
                continue;
            }
            out.push(InstalledMcp {
                name: d.name, agent: agent.clone(), scope: d.scope,
                file_path: String::new(), transport: d.transport,
                customized: false, enabled: false,
            });
        }
    }
    out
}

/// Pre-detect: scan every agent's real config and register any discovered server
/// that the Registry doesn't already know (keyed by `name::transport`) as an
/// `origin=discovered` entry carrying its actual on-disk config. Idempotent — only
/// adds what's missing, so builtins / user entries aren't duplicated. Returns the
/// number newly imported. This is what makes an agent's pre-existing MCPs show up
/// in the Registry (with a「来自 X」label) and become manageable like any other.
#[tauri::command]
pub fn import_discovered(project_dir: Option<String>) -> Result<usize, String> {
    use crate::core::types::{transport_of, RegistryConfig, RegistryOrigin};
    let agents = load_agents();
    let pd = project_dir.as_deref().map(Path::new);
    let existing: std::collections::HashSet<String> =
        read_registry().iter().map(|e| e.key()).collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut imported = 0usize;
    for s in scan_agents(&agents, pd, true) {
        let key = format!("{}::{}", s.name, transport_of(&s.config));
        if existing.contains(&key) || !seen.insert(key) {
            continue;
        }
        let config = match &s.config {
            McpConfig::Stdio(c) => RegistryConfig { stdio: Some(c.clone()), http: None },
            McpConfig::Http(c) => RegistryConfig { stdio: None, http: Some(c.clone()) },
        };
        let entry = RegistryEntry {
            name: s.name.clone(),
            description: String::new(),
            tags: Vec::new(),
            config,
            origin: Some(RegistryOrigin {
                kind: "discovered".into(),
                agent: Some(s.agent.clone()),
                scope: Some(s.scope.clone()),
            }),
        };
        write_entry(&entry).map_err(|e| e.to_string())?;
        imported += 1;
    }
    Ok(imported)
}

use crate::core::applier::apply_diffs;
use crate::core::differ::DiffEntry;
use crate::core::differ::DiffAction;
use crate::core::effective::effective_config;
use crate::core::r#override::OverridePatch;
use crate::core::paths::backups_dir;
use crate::core::types::McpConfig;
use std::collections::{BTreeMap, HashMap};

#[derive(serde::Deserialize)]
pub struct PatchInput {
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}
impl PatchInput {
    fn to_patch(&self) -> OverridePatch {
        OverridePatch { args: self.args.clone(), env: self.env.clone(),
            url: self.url.clone(), headers: self.headers.clone() }
    }
}

#[derive(serde::Deserialize)]
pub struct InstallRequest {
    pub server_name: String,
    /// Transport variant to resolve ("stdio" | "http"). Defaults to stdio for
    /// older callers. The on-disk app config is still keyed by `server_name`.
    #[serde(default = "default_transport")]
    pub transport: String,
    pub scope: String,                       // "global" | "project"
    pub agents: Vec<String>,
    pub project_dir: Option<String>,
    #[serde(default)]
    pub overrides: HashMap<String, PatchInput>, // agentId -> patch
}

fn default_transport() -> String {
    "stdio".to_string()
}

#[derive(serde::Serialize)]
pub struct PlannedWrite {
    pub agent: String,
    pub file_path: String,
    pub config_json: String,
}

fn resolve_entry(server_name: &str, transport: &str) -> Result<crate::core::types::RegistryEntry, String> {
    let reg = read_registry();
    reg.into_iter()
        .find(|e| e.name == server_name && e.transport() == transport)
        .ok_or_else(|| format!("server not found: {} ({})", server_name, transport))
}

fn target_file(agent: &crate::core::types::AgentDefinition, scope: &str, project_dir: Option<&str>) -> Option<std::path::PathBuf> {
    use crate::core::scanner::expand_tilde;
    if scope == "global" {
        agent.global.as_ref().map(|g| expand_tilde(g))
    } else {
        match (&agent.project, project_dir) {
            (Some(p), Some(base)) => Some(std::path::Path::new(base).join(p)),
            _ => None,
        }
    }
}

/// Write a single server's config into one agent's file (Add diff). Shared by
/// `apply_install` and `enable_mcp`.
fn add_one(
    agent_id: &str,
    def: &crate::core::types::AgentDefinition,
    server_name: &str,
    cfg: McpConfig,
    scope: &str,
    project_dir: Option<&str>,
    timestamp: &str,
) -> Result<(), Vec<crate::core::applier::ApplyError>> {
    let mut one: BTreeMap<String, McpConfig> = BTreeMap::new();
    one.insert(server_name.to_string(), cfg);
    let mut adef = BTreeMap::new();
    adef.insert(agent_id.to_string(), def.clone());
    let diff = vec![DiffEntry { action: DiffAction::Add,
        mcp_name: server_name.to_string(), agent: agent_id.to_string(), scope: scope.to_string() }];
    apply_diffs(&diff, &adef, &one, &backups_dir(),
        project_dir.map(std::path::Path::new), timestamp)
}

/// Remove a single server from one agent's file (Remove diff). Shared by
/// `uninstall`, `disable_mcp`, and `delete_mcp`.
fn remove_one(
    agent_id: &str,
    def: &crate::core::types::AgentDefinition,
    server_name: &str,
    scope: &str,
    project_dir: Option<&str>,
    timestamp: &str,
) -> Result<(), Vec<crate::core::applier::ApplyError>> {
    let mut adef = BTreeMap::new();
    adef.insert(agent_id.to_string(), def.clone());
    let diff = vec![DiffEntry { action: DiffAction::Remove,
        mcp_name: server_name.to_string(), agent: agent_id.to_string(), scope: scope.to_string() }];
    let empty: BTreeMap<String, McpConfig> = BTreeMap::new();
    apply_diffs(&diff, &adef, &empty, &backups_dir(),
        project_dir.map(std::path::Path::new), timestamp)
}

#[tauri::command]
pub fn preview_install(req: InstallRequest) -> Result<Vec<PlannedWrite>, String> {
    let entry = resolve_entry(&req.server_name, &req.transport)?;
    let agents = load_agents();
    let mut out = Vec::new();
    for agent_id in &req.agents {
        let Some(def) = agents.get(agent_id) else { continue };
        let Some(path) = target_file(def, &req.scope, req.project_dir.as_deref()) else { continue };
        let patch = req.overrides.get(agent_id).map(|p| p.to_patch());
        let cfg = effective_config(&entry, patch.as_ref())
            .ok_or_else(|| format!("no config for {}", req.server_name))?;
        out.push(PlannedWrite {
            agent: agent_id.clone(),
            file_path: path.display().to_string(),
            config_json: serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn apply_install(req: InstallRequest) -> Result<(), Vec<String>> {
    let entry = resolve_entry(&req.server_name, &req.transport).map_err(|e| vec![e])?;
    let agents = load_agents();
    let mut errors: Vec<String> = Vec::new();
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    for agent_id in &req.agents {
        let Some(def) = agents.get(agent_id) else { continue };
        if target_file(def, &req.scope, req.project_dir.as_deref()).is_none() { continue; }
        let patch = req.overrides.get(agent_id).map(|p| p.to_patch());
        let Some(cfg) = effective_config(&entry, patch.as_ref()) else {
            // align with preview_install: surface unconfigurable entries instead of silently skipping
            errors.push(format!("{}: no config (no stdio/http transport)", agent_id));
            continue;
        };
        if let Err(errs) = add_one(agent_id, def, &req.server_name, cfg, &req.scope,
            req.project_dir.as_deref(), &timestamp) {
            for e in errs { errors.push(format!("{}: {}", e.target, e.error)); }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[tauri::command]
pub fn uninstall(req: InstallRequest) -> Result<(), Vec<String>> {
    let agents = load_agents();
    let mut errors = Vec::new();
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    for agent_id in &req.agents {
        let Some(def) = agents.get(agent_id) else { continue };
        if target_file(def, &req.scope, req.project_dir.as_deref()).is_none() { continue; }
        if let Err(errs) = remove_one(agent_id, def, &req.server_name, &req.scope,
            req.project_dir.as_deref(), &timestamp) {
            for e in errs { errors.push(format!("{}: {}", e.target, e.error)); }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Disable a server: snapshot its current on-disk config into MUX's disabled
/// store, then remove it from the agent file so the agent stops loading it. The
/// snapshot lets `enable_mcp` restore the exact config (customizations included).
#[tauri::command]
pub fn disable_mcp(req: InstallRequest) -> Result<(), Vec<String>> {
    use crate::core::disabled::{load_disabled, save_disabled, DisabledEntry};
    use crate::core::scanner::scan_agents;
    use crate::core::types::transport_of;
    let agents = load_agents();
    let pd = req.project_dir.as_deref().map(std::path::Path::new);
    let scanned = scan_agents(&agents, pd, true);
    let mut store = load_disabled();
    let mut errors = Vec::new();
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    for agent_id in &req.agents {
        let Some(def) = agents.get(agent_id) else { continue };
        if target_file(def, &req.scope, req.project_dir.as_deref()).is_none() { continue; }
        // Snapshot the config currently installed for this (agent, name, transport, scope).
        let Some(found) = scanned.iter().find(|s| {
            s.agent == *agent_id && s.name == req.server_name && s.scope == req.scope
                && transport_of(&s.config) == req.transport
        }) else {
            errors.push(format!("{}: not installed", agent_id));
            continue;
        };
        let entry = DisabledEntry {
            name: req.server_name.clone(), transport: req.transport.clone(),
            scope: req.scope.clone(), config: found.config.clone(),
        };
        // Remove from the file first; only remember it once the removal succeeds.
        if let Err(errs) = remove_one(agent_id, def, &req.server_name, &req.scope,
            req.project_dir.as_deref(), &timestamp) {
            for e in errs { errors.push(format!("{}: {}", e.target, e.error)); }
            continue;
        }
        let list = store.entry(agent_id.clone()).or_default();
        list.retain(|d| !(d.name == entry.name && d.transport == entry.transport && d.scope == entry.scope));
        list.push(entry);
    }
    if let Err(e) = save_disabled(&store) { errors.push(format!("save disabled: {}", e)); }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Re-enable a previously disabled server: write its remembered config snapshot
/// back into the agent file, then drop it from the disabled store.
#[tauri::command]
pub fn enable_mcp(req: InstallRequest) -> Result<(), Vec<String>> {
    use crate::core::disabled::{load_disabled, save_disabled};
    let agents = load_agents();
    let mut store = load_disabled();
    let mut errors = Vec::new();
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    for agent_id in &req.agents {
        let Some(def) = agents.get(agent_id) else { continue };
        let entry = store.get(agent_id).and_then(|list| {
            list.iter()
                .find(|d| d.name == req.server_name && d.transport == req.transport && d.scope == req.scope)
                .cloned()
        });
        let Some(entry) = entry else {
            errors.push(format!("{}: no disabled snapshot", agent_id));
            continue;
        };
        if let Err(errs) = add_one(agent_id, def, &req.server_name, entry.config.clone(),
            &req.scope, req.project_dir.as_deref(), &timestamp) {
            for e in errs { errors.push(format!("{}: {}", e.target, e.error)); }
            continue;
        }
        if let Some(list) = store.get_mut(agent_id) {
            list.retain(|d| !(d.name == req.server_name && d.transport == req.transport && d.scope == req.scope));
            if list.is_empty() { store.remove(agent_id); }
        }
    }
    if let Err(e) = save_disabled(&store) { errors.push(format!("save disabled: {}", e)); }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Hard-delete a server from an agent: remove it from the file (if present) and
/// purge any remembered disabled snapshot. Covers deleting both active rows and
/// already-disabled ones.
#[tauri::command]
pub fn delete_mcp(req: InstallRequest) -> Result<(), Vec<String>> {
    use crate::core::disabled::{load_disabled, save_disabled};
    let agents = load_agents();
    let mut store = load_disabled();
    let mut store_changed = false;
    let mut errors = Vec::new();
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    for agent_id in &req.agents {
        let Some(def) = agents.get(agent_id) else { continue };
        if target_file(def, &req.scope, req.project_dir.as_deref()).is_some() {
            if let Err(errs) = remove_one(agent_id, def, &req.server_name, &req.scope,
                req.project_dir.as_deref(), &timestamp) {
                for e in errs { errors.push(format!("{}: {}", e.target, e.error)); }
            }
        }
        if let Some(list) = store.get_mut(agent_id) {
            let before = list.len();
            list.retain(|d| !(d.name == req.server_name && d.transport == req.transport && d.scope == req.scope));
            if list.len() != before { store_changed = true; }
            if list.is_empty() { store.remove(agent_id); }
        }
    }
    if store_changed {
        if let Err(e) = save_disabled(&store) { errors.push(format!("save disabled: {}", e)); }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{McpConfig, StdioConfig};

    #[test]
    fn preview_returns_planned_write_for_known_server() {
        // filesystem 是内置服务器
        let req = InstallRequest {
            server_name: "filesystem".into(), transport: "stdio".into(), scope: "global".into(),
            agents: vec!["claude-code".into()], project_dir: None,
            overrides: HashMap::new(),
        };
        let plan = preview_install(req).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].agent, "claude-code");
        assert!(plan[0].config_json.contains("command"));
    }

    #[test]
    fn customized_comparison_uses_partial_eq() {
        // 验证 customized 比较逻辑：base != scanned.config → customized=true
        let base = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: None,
        });
        let same = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: None,
        });
        let modified = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: Some(std::collections::HashMap::from([("KEY".into(), "val".into())])),
        });
        // 未修改 → customized = false
        assert!(!(&base != &same));
        // 已修改 → customized = true
        assert!(&base != &modified);
    }
}
