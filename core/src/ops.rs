//! Install / uninstall / discovery orchestration, shared by the desktop (Tauri
//! commands) and the CLI. Tauri-free — plain functions over the core stores.

use crate::adapter::get_adapter;
use crate::agents::load_agents;
use crate::applier::{apply_diffs, ApplyError};
use crate::differ::{DiffAction, DiffEntry};
use crate::disabled::{load_disabled, save_disabled, DisabledEntry};
use serde::Serialize;
use std::collections::HashSet;
use crate::effective::{base_config, effective_config};
use crate::r#override::OverridePatch;
use crate::paths::{backup_timestamp, backups_dir};
use crate::registry::{delete_registry_entry, read_registry, write_discovered_entry, write_manual_entry};
use crate::scanner::{expand_tilde, scan_agents};
use crate::types::{transport_of, AgentDefinition, McpConfig, RegistryEntry, RegistryOrigin};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Find the catalog entry for a `name`+`transport`.
pub fn resolve_entry(server_name: &str, transport: &str) -> Result<RegistryEntry, String> {
    read_registry()
        .into_iter()
        .find(|e| e.name == server_name && e.transport() == transport)
        .ok_or_else(|| format!("server not found: {} ({})", server_name, transport))
}

/// The on-disk config file for an agent at a given scope.
pub fn target_file(agent: &AgentDefinition, scope: &str, project_dir: Option<&str>) -> Option<PathBuf> {
    if scope == "global" {
        agent.global.as_ref().map(|g| expand_tilde(g))
    } else {
        match (&agent.project, project_dir) {
            (Some(p), Some(base)) => Some(Path::new(base).join(p)),
            _ => None,
        }
    }
}

/// Flatten per-target apply errors into a command's error list.
pub fn push_apply_errors(errors: &mut Vec<String>, errs: Vec<ApplyError>) {
    for e in errs {
        errors.push(format!("{}: {}", e.target, e.error));
    }
}

/// Write one server's config into a single agent's file (Add diff, backed up).
pub fn add_one(
    agent_id: &str,
    def: &AgentDefinition,
    server_name: &str,
    cfg: McpConfig,
    scope: &str,
    project_dir: Option<&str>,
    timestamp: &str,
) -> Result<(), Vec<ApplyError>> {
    let mut one: BTreeMap<String, McpConfig> = BTreeMap::new();
    one.insert(server_name.to_string(), cfg);
    let mut adef = BTreeMap::new();
    adef.insert(agent_id.to_string(), def.clone());
    let diff = vec![DiffEntry {
        action: DiffAction::Add,
        mcp_name: server_name.to_string(),
        agent: agent_id.to_string(),
        scope: scope.to_string(),
    }];
    apply_diffs(&diff, &adef, &one, &backups_dir(), project_dir.map(Path::new), timestamp)
}

/// Remove one server from a single agent's file (Remove diff, backed up).
pub fn remove_one(
    agent_id: &str,
    def: &AgentDefinition,
    server_name: &str,
    scope: &str,
    project_dir: Option<&str>,
    timestamp: &str,
) -> Result<(), Vec<ApplyError>> {
    let mut adef = BTreeMap::new();
    adef.insert(agent_id.to_string(), def.clone());
    let diff = vec![DiffEntry {
        action: DiffAction::Remove,
        mcp_name: server_name.to_string(),
        agent: agent_id.to_string(),
        scope: scope.to_string(),
    }];
    let empty: BTreeMap<String, McpConfig> = BTreeMap::new();
    apply_diffs(&diff, &adef, &empty, &backups_dir(), project_dir.map(Path::new), timestamp)
}

/// Install a catalog server (`name`+`transport`) into the given agents.
pub fn install(
    server_name: &str,
    transport: &str,
    scope: &str,
    agents: &[String],
    project_dir: Option<&str>,
    overrides: &HashMap<String, OverridePatch>,
) -> Result<(), Vec<String>> {
    let entry = resolve_entry(server_name, transport).map_err(|e| vec![e])?;
    let defs = load_agents();
    let ts = backup_timestamp();
    let mut errors = Vec::new();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else { continue };
        if target_file(def, scope, project_dir).is_none() {
            continue;
        }
        let Some(cfg) = effective_config(&entry, overrides.get(agent_id)) else {
            errors.push(format!("{}: no config (no stdio/http transport)", agent_id));
            continue;
        };
        if let Err(errs) = add_one(agent_id, def, server_name, cfg, scope, project_dir, &ts) {
            push_apply_errors(&mut errors, errs);
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Remove a server from the given agents' files.
pub fn uninstall(
    server_name: &str,
    scope: &str,
    agents: &[String],
    project_dir: Option<&str>,
) -> Result<(), Vec<String>> {
    let defs = load_agents();
    let ts = backup_timestamp();
    let mut errors = Vec::new();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else { continue };
        if target_file(def, scope, project_dir).is_none() {
            continue;
        }
        if let Err(errs) = remove_one(agent_id, def, server_name, scope, project_dir, &ts) {
            push_apply_errors(&mut errors, errs);
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Scan every agent's config and register any server the catalog doesn't yet
/// know (keyed by `name::transport`) as a discovered entry. Returns the count.
pub fn import_discovered(project_dir: Option<&str>) -> Result<usize, String> {
    let agents = load_agents();
    let pd = project_dir.map(Path::new);
    let existing: std::collections::HashSet<String> =
        read_registry().iter().map(|e| e.key()).collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut imported = 0usize;
    for s in scan_agents(&agents, pd, true) {
        let key = format!("{}::{}", s.name, transport_of(&s.config));
        if existing.contains(&key) || !seen.insert(key) {
            continue;
        }
        let entry = RegistryEntry {
            name: s.name.clone(),
            description: String::new(),
            tags: Vec::new(),
            config: s.config.clone().into(),
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

/// Clear every MCP server from enabled agents' global config files. If
/// `only_agent` is set, restrict to that one agent. Sibling (non-section) bytes
/// are preserved — this removes the servers, it doesn't clobber the whole file.
/// Returns the ids of the agents that had something removed.
pub fn clean(only_agent: Option<&str>) -> Vec<String> {
    let defs = load_agents();
    let ts = backup_timestamp();
    let mut cleaned = Vec::new();
    for (agent_id, def) in &defs {
        if only_agent.is_some_and(|a| a != agent_id) {
            continue;
        }
        if !def.enabled {
            continue;
        }
        let Some(g) = &def.global else { continue };
        let path = expand_tilde(g);
        if !path.exists() {
            continue;
        }
        let names: Vec<String> = get_adapter(&def.format, &def.key)
            .read(&path)
            .into_keys()
            .collect();
        if names.is_empty() {
            continue;
        }
        let diffs: Vec<DiffEntry> = names
            .iter()
            .map(|n| DiffEntry {
                action: DiffAction::Remove,
                mcp_name: n.clone(),
                agent: agent_id.clone(),
                scope: "global".into(),
            })
            .collect();
        let mut adef = BTreeMap::new();
        adef.insert(agent_id.clone(), def.clone());
        let empty: BTreeMap<String, McpConfig> = BTreeMap::new();
        let _ = apply_diffs(&diffs, &adef, &empty, &backups_dir(), None, &ts);
        cleaned.push(agent_id.clone());
    }
    cleaned
}

// ── Registry entry editing (upsert / remove / paste-import) ────────────────

/// Create or overwrite a user registry entry (stored in the managed "manual"
/// source), propagating the config change to agents that installed it clean.
pub fn upsert_entry(entry: RegistryEntry) -> Result<(), String> {
    // Capture the currently-effective config BEFORE overwriting it, so we can
    // propagate the change to agents that installed it "clean".
    let prev = read_registry()
        .into_iter()
        .find(|e| e.name == entry.name && e.transport() == entry.transport());
    write_manual_entry(&entry).map_err(|e| e.to_string())?;
    propagate_edit_to_installs(prev.as_ref(), Some(&entry));
    Ok(())
}

/// Remove a user registry override for `name`+`transport`; the entry reverts to
/// whatever a source provides (or vanishes). The fallback config is propagated to
/// clean installs too, for symmetry with edit.
pub fn remove_entry(name: &str, transport: &str) -> Result<(), String> {
    let prev = read_registry()
        .into_iter()
        .find(|e| e.name == name && e.transport() == transport);
    delete_registry_entry(name, transport).map_err(|e| e.to_string())?;
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
/// (a "clean" install). Hand-customized installs are left untouched.
///
/// No-ops when the entry is brand-new (`prev` None), fully removed (`new` None),
/// or the config didn't change. Global scope only.
fn propagate_edit_to_installs(prev: Option<&RegistryEntry>, new: Option<&RegistryEntry>) {
    let (Some(prev), Some(new)) = (prev, new) else { return };
    let (Some(old_cfg), Some(new_cfg)) = (base_config(prev), base_config(new)) else { return };
    if old_cfg == new_cfg {
        return; // description/tags-only edit, or no real change
    }
    let transport = new.transport();
    let agents = load_agents();
    let timestamp = backup_timestamp();
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
/// to the managed "manual" source. Returns the names added. Entries that don't fit
/// the stdio/http shape are skipped; an empty result is an error so the caller can
/// tell the user nothing was recognized.
pub fn import_pasted(text: &str) -> Result<Vec<String>, String> {
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
        let entry = RegistryEntry {
            name: name.clone(),
            description: String::new(),
            tags: Vec::new(),
            config: cfg.into(),
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

// ── Install status scan + enable / disable / delete ────────────────────────

/// A server found installed in an agent's real config file, or remembered in
/// MUX's disabled store.
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

/// Scan real agent config files → "who installed what". `customized` flags an
/// on-disk config that differs from the registry base; MUX-remembered disabled
/// servers are appended as `enabled:false` rows. Global-only unless a project dir
/// is given.
pub fn scan_installed(project_dir: Option<&str>) -> Vec<InstalledMcp> {
    // (name::transport) -> base McpConfig, from read_registry (same source as
    // install) so each transport variant compares independently.
    let base_map: HashMap<String, McpConfig> = read_registry()
        .into_iter()
        .filter_map(|e| {
            let key = e.key();
            let base = base_config(&e)?;
            Some((key, base))
        })
        .collect();
    let agents = load_agents();
    let pd = project_dir.map(Path::new);
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
    // Append MUX-remembered disabled servers so a UI can show an "off" row for a
    // server removed from the file but re-enable-able.
    let active: HashSet<(String, String, String, String)> = out
        .iter()
        .map(|i| (i.agent.clone(), i.name.clone(), i.transport.clone(), i.scope.clone()))
        .collect();
    for (agent, list) in load_disabled() {
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

/// Outcome of [`resync_entry`]: which agents got the current config re-stamped,
/// and (when not forcing) which were skipped because their on-disk config was
/// hand-customized.
#[derive(Serialize)]
pub struct ResyncOutcome {
    pub synced: Vec<String>,
    pub skipped_customized: Vec<String>,
}

/// Re-stamp a catalog entry's *current* config into every agent that has it
/// actively installed at global scope — an explicit, user-invoked counterpart to
/// the conservative auto-propagation in [`upsert_entry`].
///
/// `force == false`: only "clean" installs (on-disk == registry base) are
/// updated; hand-customized ones are reported in `skipped_customized` so the
/// caller can offer to force. `force == true`: customized installs are
/// overwritten too.
///
/// Scope: global only. Disabled-store installs (`enabled == false`) are excluded
/// — they live in the snapshot store, not the agent file, and `install` would
/// wrongly re-activate them. Reuses [`scan_installed`] + [`install`], so the
/// re-stamp is backed up like any other write.
pub fn resync_entry(name: &str, transport: &str, force: bool) -> Result<ResyncOutcome, Vec<String>> {
    // Confirm the entry still exists in the aggregated catalog.
    resolve_entry(name, transport).map_err(|e| vec![e])?;

    // Active, global installs of this exact (name, transport).
    let (customized, clean): (Vec<InstalledMcp>, Vec<InstalledMcp>) = scan_installed(None)
        .into_iter()
        .filter(|i| i.enabled && i.scope == "global" && i.name == name && i.transport == transport)
        .partition(|i| i.customized);

    let mut target: Vec<String> = clean.iter().map(|i| i.agent.clone()).collect();
    let skipped_customized: Vec<String> = if force {
        target.extend(customized.iter().map(|i| i.agent.clone()));
        Vec::new()
    } else {
        customized.iter().map(|i| i.agent.clone()).collect()
    };

    if !target.is_empty() {
        install(name, transport, "global", &target, None, &HashMap::new())?;
    }
    Ok(ResyncOutcome { synced: target, skipped_customized })
}

/// Persist the disabled store, downgrading an IO failure to a reported error.
fn save_disabled_or_log(
    store: &BTreeMap<String, Vec<DisabledEntry>>,
    errors: &mut Vec<String>,
) {
    if let Err(e) = save_disabled(store) {
        errors.push(format!("save disabled: {}", e));
    }
}

/// Disable a server: snapshot its current on-disk config into the disabled store,
/// then remove it from the agent file. The snapshot lets [`enable`] restore the
/// exact config (customizations included).
pub fn disable(
    server_name: &str,
    transport: &str,
    scope: &str,
    agents: &[String],
    project_dir: Option<&str>,
) -> Result<(), Vec<String>> {
    let defs = load_agents();
    let scanned = scan_agents(&defs, project_dir.map(Path::new), true);
    let mut store = load_disabled();
    let mut errors = Vec::new();
    let ts = backup_timestamp();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else { continue };
        if target_file(def, scope, project_dir).is_none() {
            continue;
        }
        // Snapshot the config currently installed for this (agent, name, transport, scope).
        let Some(found) = scanned.iter().find(|s| {
            s.agent == *agent_id
                && s.name == server_name
                && s.scope == scope
                && transport_of(&s.config) == transport
        }) else {
            errors.push(format!("{}: not installed", agent_id));
            continue;
        };
        let entry = DisabledEntry {
            name: server_name.to_string(), transport: transport.to_string(),
            scope: scope.to_string(), config: found.config.clone(),
        };
        // Remove from the file first; only remember it once the removal succeeds.
        if let Err(errs) = remove_one(agent_id, def, server_name, scope, project_dir, &ts) {
            push_apply_errors(&mut errors, errs);
            continue;
        }
        let list = store.entry(agent_id.clone()).or_default();
        list.retain(|d| !(d.name == entry.name && d.transport == entry.transport && d.scope == entry.scope));
        list.push(entry);
    }
    save_disabled_or_log(&store, &mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Re-enable a previously disabled server: write its remembered config snapshot
/// back into the agent file, then drop it from the disabled store.
pub fn enable(
    server_name: &str,
    transport: &str,
    scope: &str,
    agents: &[String],
    project_dir: Option<&str>,
) -> Result<(), Vec<String>> {
    let defs = load_agents();
    let mut store = load_disabled();
    let mut errors = Vec::new();
    let ts = backup_timestamp();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else { continue };
        let entry = store.get(agent_id).and_then(|list| {
            list.iter()
                .find(|d| d.name == server_name && d.transport == transport && d.scope == scope)
                .cloned()
        });
        let Some(entry) = entry else {
            errors.push(format!("{}: no disabled snapshot", agent_id));
            continue;
        };
        if let Err(errs) = add_one(agent_id, def, server_name, entry.config.clone(), scope, project_dir, &ts) {
            push_apply_errors(&mut errors, errs);
            continue;
        }
        if let Some(list) = store.get_mut(agent_id) {
            list.retain(|d| !(d.name == server_name && d.transport == transport && d.scope == scope));
            if list.is_empty() {
                store.remove(agent_id);
            }
        }
    }
    save_disabled_or_log(&store, &mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Hard-delete a server from an agent: remove it from the file (if present) and
/// purge any remembered disabled snapshot. Covers active and already-disabled rows.
pub fn delete(
    server_name: &str,
    transport: &str,
    scope: &str,
    agents: &[String],
    project_dir: Option<&str>,
) -> Result<(), Vec<String>> {
    let defs = load_agents();
    let mut store = load_disabled();
    let mut store_changed = false;
    let mut errors = Vec::new();
    let ts = backup_timestamp();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else { continue };
        if target_file(def, scope, project_dir).is_some() {
            if let Err(errs) = remove_one(agent_id, def, server_name, scope, project_dir, &ts) {
                push_apply_errors(&mut errors, errs);
            }
        }
        if let Some(list) = store.get_mut(agent_id) {
            let before = list.len();
            list.retain(|d| !(d.name == server_name && d.transport == transport && d.scope == scope));
            if list.len() != before {
                store_changed = true;
            }
            if list.is_empty() {
                store.remove(agent_id);
            }
        }
    }
    if store_changed {
        save_disabled_or_log(&store, &mut errors);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
