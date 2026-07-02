use crate::core::registry::{
    delete_registry_entry as delete_entry, read_registry, user_override_keys,
    write_discovered_entry, write_manual_entry, DISCOVERED_ID, MANUAL_ID,
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
    // Capture the currently-effective config BEFORE overwriting it, so we can
    // propagate the change to agents that installed it "clean" (§ propagate).
    let prev = read_registry()
        .into_iter()
        .find(|e| e.name == entry.name && e.transport() == entry.transport());
    // User-created / edited entries live in the managed "manual" local source.
    write_manual_entry(&entry).map_err(|e| e.to_string())?;
    propagate_edit_to_installs(prev.as_ref(), Some(&entry));
    Ok(())
}

/// Remove a user registry override for a given name+transport; reverts to
/// builtin if one exists.
#[tauri::command]
pub fn delete_registry_entry(name: String, transport: String) -> Result<(), String> {
    // On revert, the entry falls back to whatever a source provides (or nothing).
    // Propagate that fallback config to clean installs too, for symmetry with edit.
    let prev = read_registry()
        .into_iter()
        .find(|e| e.name == name && e.transport() == transport);
    delete_entry(&name, &transport).map_err(|e| e.to_string())?;
    let now = read_registry()
        .into_iter()
        .find(|e| e.name == name && e.transport() == transport);
    propagate_edit_to_installs(prev.as_ref(), now.as_ref());
    Ok(())
}

/// Propagate a registry-entry config change to agents that have it installed.
///
/// Registry installs write a *snapshot* of the config into each agent file, so a
/// later edit would otherwise not reach those agents. Here we re-stamp the new
/// config into every agent that currently holds this server at global scope —
/// but ONLY where the on-disk config still equals the previous registry config
/// (a "clean" install). Agents whose config was hand-customized (differs from the
/// old base) are left untouched, preserving intentional per-agent tweaks.
///
/// No-ops when: the entry is brand-new (`prev` is None), it was fully removed
/// (`new` is None), or the config didn't actually change. Global scope only —
/// project-scoped installs need a project dir we don't have at edit time.
fn propagate_edit_to_installs(prev: Option<&RegistryEntry>, new: Option<&RegistryEntry>) {
    use crate::core::effective::base_config;
    use crate::core::types::transport_of;
    let (Some(prev), Some(new)) = (prev, new) else { return };
    let (Some(old_cfg), Some(new_cfg)) = (base_config(prev), base_config(new)) else { return };
    if old_cfg == new_cfg {
        return; // description/tags-only edit, or no real change
    }
    let transport = new.transport();
    let agents = load_agents();
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    for s in scan_agents(&agents, None, true) {
        if s.name == new.name
            && s.scope == "global"
            && transport_of(&s.config) == transport
            && s.config == old_cfg
        {
            if let Some(def) = agents.get(&s.agent) {
                let _ = add_one(&s.agent, def, &new.name, new_cfg.clone(), "global", None, &timestamp);
            }
        }
    }
}

/// Composite keys (`name::transport`) of registry entries that currently have a
/// user override.
#[tauri::command]
pub fn list_custom_registry_keys() -> Vec<String> {
    user_override_keys()
}

/// Locate the `name -> config` server map in a pasted config value: under
/// `mcpServers` / `mcp_servers` / `servers`, or the top-level object itself when
/// its values all look like server configs (have `command` or `url`).
fn extract_servers(v: &serde_json::Value) -> Option<serde_json::Map<String, serde_json::Value>> {
    let obj = v.as_object()?;
    for key in ["mcpServers", "mcp_servers", "servers"] {
        if let Some(m) = obj.get(key).and_then(|x| x.as_object()) {
            return Some(m.clone());
        }
    }
    let looks_like_map = !obj.is_empty()
        && obj.values().all(|val| {
            val.as_object()
                .map(|o| o.contains_key("command") || o.contains_key("url"))
                .unwrap_or(false)
        });
    if looks_like_map {
        return Some(obj.clone());
    }
    None
}

/// Parse a pasted config blob (JSON or TOML) and add every MCP server it contains
/// to the managed "manual" source. Returns the names that were added. Servers that
/// don't fit the stdio/http shape are skipped; an empty result is an error so the
/// UI can tell the user nothing was recognized.
#[tauri::command]
pub fn import_pasted_config(text: String) -> Result<Vec<String>, String> {
    use crate::core::types::{McpConfig, RegistryConfig};
    let text = text.trim();
    if text.is_empty() {
        return Err("粘贴内容为空".into());
    }
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            let t: toml::Value =
                toml::from_str(text).map_err(|e| format!("内容既不是有效的 JSON 也不是 TOML：{}", e))?;
            serde_json::to_value(t).map_err(|e| e.to_string())?
        }
    };
    let servers = extract_servers(&value)
        .ok_or("未识别到 MCP 配置：需要包含 mcpServers，或直接是「名称→配置」的映射")?;
    let mut added = Vec::new();
    for (name, cfg_val) in servers {
        let Ok(cfg) = serde_json::from_value::<McpConfig>(cfg_val) else {
            continue; // skip entries that aren't a valid stdio/http config
        };
        let config = match cfg {
            McpConfig::Stdio(c) => RegistryConfig { stdio: Some(c), http: None },
            McpConfig::Http(c) => RegistryConfig { stdio: None, http: Some(c) },
        };
        let entry = RegistryEntry {
            name: name.clone(),
            description: String::new(),
            tags: Vec::new(),
            config,
            origin: None,
        };
        write_manual_entry(&entry).map_err(|e| e.to_string())?;
        added.push(name);
    }
    if added.is_empty() {
        return Err("未在粘贴内容中找到可用的 MCP server".into());
    }
    Ok(added)
}

// ── Catalog sources (subscribe remote / add local) ────────────────────────

use crate::core::settings::{load_settings, mutate_settings};
use crate::core::sources;
use crate::core::types::SourceDef;

/// A source as shown in the UI: its stored definition plus a live server count.
#[derive(Serialize)]
pub struct SourceView {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub format: String,
    pub enabled: bool,
    pub added_at: Option<String>,
    pub synced_at: Option<String>,
    pub server_count: u32,
    pub error: Option<String>,
    /// True for the two auto-managed sources ("手动添加" / "自动探索") — the UI
    /// hides refresh/remove for these to avoid accidental data loss.
    pub managed: bool,
}

fn to_view(def: SourceDef, count: u32) -> SourceView {
    let managed = def.id == MANUAL_ID || def.id == DISCOVERED_ID;
    SourceView {
        id: def.id, kind: def.kind, name: def.name, url: def.url, path: def.path,
        format: def.format, enabled: def.enabled, added_at: def.added_at,
        synced_at: def.synced_at, server_count: count, error: def.error, managed,
    }
}

fn push_source(def: &SourceDef) -> Result<(), String> {
    mutate_settings(|s| s.sources.get_or_insert_with(Vec::new).push(def.clone()))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sources() -> Vec<SourceView> {
    load_settings()
        .sources
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            let count = sources::source_count(&d);
            to_view(d, count)
        })
        .collect()
}

/// Subscribe to a remote config URL: fetch, validate, cache it under
/// `~/.mux/sources/remote/<id>`, and register it as an enabled source.
#[tauri::command]
pub fn subscribe_source(url: String, name: Option<String>) -> Result<SourceView, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("URL 不能为空".into());
    }
    let body = sources::fetch(&url)?;
    let format = sources::detect_format(&url, &body).to_string();
    sources::validate_parseable(&body, &format)?;
    let display = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| sources::host_of(&url));
    let now = sources::now_iso();
    let mut def = SourceDef {
        id: sources::gen_id("remote", &display),
        kind: "remote".into(),
        name: display,
        url: Some(url),
        path: None,
        format,
        key: "mcpServers".into(),
        enabled: true,
        added_at: Some(now.clone()),
        synced_at: Some(now),
        server_count: None,
        error: None,
    };
    let path = sources::cached_path(&def).ok_or("无法确定缓存路径")?;
    sources::write_source_file(&path, &body)?;
    let count = sources::source_count(&def);
    def.server_count = Some(count);
    if count == 0 {
        def.error = Some("未在该文件中发现 MCP server".into());
    }
    push_source(&def)?;
    Ok(to_view(def, count))
}

/// Register a local config file as a source: read it, validate, and copy it under
/// `~/.mux/sources/local/<id>` (the app then reads the copy, not the original).
fn add_local_impl(path: String, name: Option<String>) -> Result<SourceView, String> {
    let src = crate::core::scanner::expand_tilde(&path);
    let content = std::fs::read_to_string(&src).map_err(|e| format!("读取文件失败: {}", e))?;
    let format = sources::detect_format(&path, &content).to_string();
    sources::validate_parseable(&content, &format)?;
    let display = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            std::path::Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("本地配置")
                .to_string()
        });
    let now = sources::now_iso();
    let mut def = SourceDef {
        id: sources::gen_id("local", &display),
        kind: "local".into(),
        name: display,
        url: None,
        path: Some(collapse_home(&path)),
        format,
        key: "mcpServers".into(),
        enabled: true,
        added_at: Some(now.clone()),
        synced_at: Some(now),
        server_count: None,
        error: None,
    };
    let cache = sources::cached_path(&def).ok_or("无法确定缓存路径")?;
    sources::write_source_file(&cache, &content)?;
    let count = sources::source_count(&def);
    def.server_count = Some(count);
    if count == 0 {
        def.error = Some("未在该文件中发现 MCP server".into());
    }
    push_source(&def)?;
    Ok(to_view(def, count))
}

/// Add a local source from an explicit path.
#[tauri::command]
pub fn add_local_source(path: String, name: Option<String>) -> Result<SourceView, String> {
    add_local_impl(path, name)
}

/// Open a native file picker and add the chosen file as a local source. Returns
/// `None` if the user cancels.
#[tauri::command]
pub fn add_local_source_dialog(app: tauri::AppHandle) -> Result<Option<SourceView>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .add_filter("MCP 配置", &["json", "toml"])
        .blocking_pick_file();
    let Some(fp) = picked else { return Ok(None) };
    add_local_impl(fp.to_string(), None).map(Some)
}

/// Add the bundled curated collection as an opt-in *local* source (it is not part
/// of the default catalog). Reuses the embedded `data/registry.json`.
#[tauri::command]
pub fn add_builtin_collection() -> Result<SourceView, String> {
    let entries = crate::core::registry::builtin_registry();
    let content = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    let now = sources::now_iso();
    let mut def = SourceDef {
        id: sources::gen_id("local", "curated"),
        kind: "local".into(),
        name: "官方精选合集".into(),
        url: None,
        path: None,
        format: "json".into(),
        key: "mcpServers".into(),
        enabled: true,
        added_at: Some(now.clone()),
        synced_at: Some(now),
        server_count: None,
        error: None,
    };
    let cache = sources::cached_path(&def).ok_or("无法确定缓存路径")?;
    sources::write_source_file(&cache, &content)?;
    let count = sources::source_count(&def);
    def.server_count = Some(count);
    push_source(&def)?;
    Ok(to_view(def, count))
}

/// Re-fetch (remote) or re-copy (local) a source's file and update its status.
#[tauri::command]
pub fn refresh_source(id: String) -> Result<SourceView, String> {
    let Some(mut def) = load_settings()
        .sources
        .unwrap_or_default()
        .into_iter()
        .find(|d| d.id == id)
    else {
        return Err("source 不存在".into());
    };
    let fetched: Result<String, String> = match def.kind.as_str() {
        "remote" => {
            let url = def.url.clone().ok_or("该来源缺少 URL")?;
            sources::fetch(&url)
        }
        "local" => match def.path.as_ref() {
            Some(p) => {
                let src = crate::core::scanner::expand_tilde(p);
                std::fs::read_to_string(&src).map_err(|e| format!("读取原文件失败: {}", e))
            }
            None => Err("该本地来源没有可刷新的原文件".into()),
        },
        _ => Err("不支持刷新该来源".into()),
    };
    match fetched {
        Ok(body) => {
            if let Some(path) = sources::cached_path(&def) {
                sources::write_source_file(&path, &body)?;
            }
            def.synced_at = Some(sources::now_iso());
            def.error = None;
        }
        Err(e) => {
            def.error = Some(e);
        }
    }
    let count = sources::source_count(&def);
    def.server_count = Some(count);
    let saved = def.clone();
    mutate_settings(move |s| {
        if let Some(list) = s.sources.as_mut() {
            for d in list.iter_mut() {
                if d.id == saved.id {
                    *d = saved.clone();
                }
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(to_view(def, count))
}

/// Enable or disable a source (its servers join/leave the catalog).
#[tauri::command]
pub fn set_source_enabled(id: String, enabled: bool) -> Result<(), String> {
    mutate_settings(|s| {
        if let Some(list) = s.sources.as_mut() {
            for d in list.iter_mut() {
                if d.id == id {
                    d.enabled = enabled;
                }
            }
        }
    })
    .map_err(|e| e.to_string())
}

/// Remove a source and delete its cached file.
#[tauri::command]
pub fn remove_source(id: String) -> Result<(), String> {
    mutate_settings(|s| {
        if let Some(list) = s.sources.as_mut() {
            if let Some(pos) = list.iter().position(|d| d.id == id) {
                let def = list.remove(pos);
                if let Some(p) = sources::cached_path(&def) {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
    })
    .map_err(|e| e.to_string())
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
                source: None,
            }),
        };
        write_discovered_entry(&entry).map_err(|e| e.to_string())?;
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
    fn preview_returns_planned_write_for_seeded_server() {
        use crate::core::types::{RegistryConfig, RegistryEntry, StdioConfig};
        // No built-in catalog anymore: seed a manual entry through the real store
        // (a managed local source) in an isolated ~/.mux, then preview it.
        let home = std::env::temp_dir().join(format!("mux-preview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".mux")).unwrap();
        std::env::set_var("HOME", &home);
        crate::core::registry::write_manual_entry(&RegistryEntry {
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
