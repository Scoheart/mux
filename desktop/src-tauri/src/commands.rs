use mux_core::registry::{read_registry, user_override_keys};
use mux_core::types::RegistryEntry;

#[tauri::command]
pub fn list_registry() -> Vec<RegistryEntry> {
    // Read user overrides from settings.registry merged over builtin — same source
    // scan_installed / apply_install resolve against, so the UI stays consistent.
    read_registry()
}

/// Persist (create or overwrite) a user registry entry; propagates to clean installs.
#[tauri::command]
pub fn upsert_registry_entry(entry: RegistryEntry) -> Result<(), String> {
    mux_core::ops::upsert_entry(entry)
}

/// Remove a user registry override for a given name+transport; reverts to
/// whatever a source provides (or nothing).
#[tauri::command]
pub fn delete_registry_entry(name: String, transport: String) -> Result<(), String> {
    mux_core::ops::remove_entry(&name, &transport)
}

/// Composite keys (`name::transport`) of registry entries that currently have a
/// user override.
#[tauri::command]
pub fn list_custom_registry_keys() -> Vec<String> {
    user_override_keys()
}

/// Parse a pasted config blob (JSON or TOML) and add every MCP server it contains
/// to the managed "manual" source. Returns the names that were added.
#[tauri::command]
pub fn import_pasted_config(text: String) -> Result<Vec<String>, String> {
    mux_core::ops::import_pasted(&text)
}

// ── Catalog sources (subscribe remote / add local) ────────────────────────
// Orchestration lives in `mux_core::sources`; these are thin command wrappers.

use mux_core::sources::{self, SourceView};

#[tauri::command]
pub fn list_sources() -> Vec<SourceView> {
    sources::list_views()
}

#[tauri::command]
pub fn subscribe_source(url: String, name: Option<String>) -> Result<SourceView, String> {
    sources::subscribe(url, name)
}

/// Add a local source from an explicit path.
#[tauri::command]
pub fn add_local_source(path: String, name: Option<String>) -> Result<SourceView, String> {
    sources::add_local(path, name)
}

/// Open a native file picker and add the chosen file as a local source. Returns
/// `None` if the user cancels. Desktop-only (native dialog); delegates to core.
#[tauri::command]
pub fn add_local_source_dialog(app: tauri::AppHandle) -> Result<Option<SourceView>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .add_filter("MCP 配置", &["json", "toml"])
        .blocking_pick_file();
    let Some(fp) = picked else { return Ok(None) };
    sources::add_local(fp.to_string(), None).map(Some)
}

/// Add the bundled curated collection as an opt-in *local* source.
#[tauri::command]
pub fn add_builtin_collection() -> Result<SourceView, String> {
    sources::add_official()
}

#[tauri::command]
pub fn refresh_source(id: String) -> Result<SourceView, String> {
    sources::refresh(id)
}

#[tauri::command]
pub fn set_source_enabled(id: String, enabled: bool) -> Result<(), String> {
    sources::set_enabled(id, enabled)
}

#[tauri::command]
pub fn remove_source(id: String) -> Result<(), String> {
    sources::remove(id)
}

use mux_core::agents::{load_agents, AgentInfo};
use mux_core::types::AgentDefinition;

/// 新增一个自定义 agent，持久化到 settings.agents（在内置/已有定义之上合并）。
/// id 为空或已存在时报错，避免误覆盖内置 agent。
#[tauri::command]
pub fn add_agent(id: String, def: AgentDefinition) -> Result<(), String> {
    mux_core::agents::put(id, def, false)
}

/// 编辑一个已存在 agent 的配置（路径 / 格式 / key），覆盖写回 settings.agents。
#[tauri::command]
pub fn update_agent(id: String, def: AgentDefinition) -> Result<(), String> {
    mux_core::agents::put(id, def, true)
}

#[tauri::command]
pub fn list_agents() -> Vec<AgentInfo> {
    mux_core::agents::list_infos()
}

pub use mux_core::ops::InstalledMcp;

/// 扫描真实配置文件，返回「谁装在哪」。project_dir 为空则只扫 global。
#[tauri::command]
pub fn scan_installed(project_dir: Option<String>) -> Vec<InstalledMcp> {
    mux_core::ops::scan_installed(project_dir.as_deref())
}

/// Pre-detect: scan every agent's real config and register any discovered server
/// that the Registry doesn't already know (keyed by `name::transport`) as an
/// `origin=discovered` entry carrying its actual on-disk config. Idempotent — only
/// adds what's missing, so builtins / user entries aren't duplicated. Returns the
/// number newly imported. This is what makes an agent's pre-existing MCPs show up
/// in the Registry (with a「来自 X」label) and become manageable like any other.
#[tauri::command]
pub fn import_discovered(project_dir: Option<String>) -> Result<usize, String> {
    mux_core::ops::import_discovered(project_dir.as_deref())
}

use mux_core::ops::{resolve_entry, target_file};
use mux_core::effective::effective_config;
use mux_core::r#override::OverridePatch;
use std::collections::HashMap;

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
    let overrides: HashMap<String, OverridePatch> =
        req.overrides.iter().map(|(k, v)| (k.clone(), v.to_patch())).collect();
    mux_core::ops::install(
        &req.server_name, &req.transport, &req.scope, &req.agents,
        req.project_dir.as_deref(), &overrides,
    )
}

#[tauri::command]
pub fn uninstall(req: InstallRequest) -> Result<(), Vec<String>> {
    mux_core::ops::uninstall(&req.server_name, &req.scope, &req.agents, req.project_dir.as_deref())
}

/// Disable a server: snapshot its current on-disk config into MUX's disabled
/// store, then remove it from the agent file.
#[tauri::command]
pub fn disable_mcp(req: InstallRequest) -> Result<(), Vec<String>> {
    mux_core::ops::disable(&req.server_name, &req.transport, &req.scope, &req.agents, req.project_dir.as_deref())
}

/// Re-enable a previously disabled server from its remembered config snapshot.
#[tauri::command]
pub fn enable_mcp(req: InstallRequest) -> Result<(), Vec<String>> {
    mux_core::ops::enable(&req.server_name, &req.transport, &req.scope, &req.agents, req.project_dir.as_deref())
}

/// Hard-delete a server from an agent: remove it from the file and purge any
/// remembered disabled snapshot.
#[tauri::command]
pub fn delete_mcp(req: InstallRequest) -> Result<(), Vec<String>> {
    mux_core::ops::delete(&req.server_name, &req.transport, &req.scope, &req.agents, req.project_dir.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mux_core::types::{McpConfig, StdioConfig};

    #[test]
    fn preview_returns_planned_write_for_seeded_server() {
        use mux_core::types::{RegistryConfig, RegistryEntry, StdioConfig};
        // No built-in catalog anymore: seed a manual entry through the real store
        // (a managed local source) in an isolated ~/.mux, then preview it.
        let home = std::env::temp_dir().join(format!("mux-preview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".mux")).unwrap();
        std::env::set_var("HOME", &home);
        mux_core::registry::write_manual_entry(&RegistryEntry {
            name: "seeded".into(),
            description: String::new(),
            tags: vec![],
            config: RegistryConfig {
                stdio: Some(StdioConfig { command: "npx".into(), args: Some(vec!["-y".into(), "seeded".into()]), env: None }),
                http: None,
            },
            origin: None,
        })
        .unwrap();
        let req = InstallRequest {
            server_name: "seeded".into(), transport: "stdio".into(), scope: "global".into(),
            agents: vec!["claude-code".into()], project_dir: None,
            overrides: HashMap::new(),
        };
        let plan = preview_install(req).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].agent, "claude-code");
        assert!(plan[0].config_json.contains("command"));
        std::env::remove_var("HOME");
        let _ = std::fs::remove_dir_all(&home);
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
