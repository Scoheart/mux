//! Install / uninstall / discovery orchestration, shared by the desktop (Tauri
//! commands) and the CLI. Tauri-free — plain functions over the core stores.

use crate::adapter::get_agent_adapter_for;
use crate::agents::{load_agents, supports_transport};
use crate::applier::{apply_diffs, remove_snapshot, restore_snapshot, ApplyError};
use crate::codec::{decode_any, from_name, normalize_with_codec, Codec};
use crate::differ::{DiffAction, DiffEntry};
use crate::disabled::{load_disabled, purge, remember, remove_if_unchanged, DisabledEntry};
use crate::effective::{base_config, effective_config};
use crate::paths::{backup_timestamp, backups_dir};
use crate::r#override::OverridePatch;
use crate::registry::{
    delete_discovered_entry, delete_registry_entry, read_registry, write_discovered_entry,
    write_manual_entry,
};
use crate::scanner::{expand_tilde, scan_agents};
use crate::types::{transport_of, AgentDefinition, McpConfig, RegistryEntry, RegistryOrigin};
use serde::Serialize;
use std::collections::HashSet;
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
pub fn target_file(
    agent: &AgentDefinition,
    scope: &str,
    project_dir: Option<&str>,
) -> Option<PathBuf> {
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
    apply_diffs(
        &diff,
        &adef,
        &one,
        &backups_dir(),
        project_dir.map(Path::new),
        timestamp,
    )
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
    apply_diffs(
        &diff,
        &adef,
        &empty,
        &backups_dir(),
        project_dir.map(Path::new),
        timestamp,
    )
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
        let Some(def) = defs.get(agent_id) else {
            errors.push(format!("{agent_id}: unknown Agent"));
            continue;
        };
        if !supports_transport(agent_id, transport) {
            errors.push(format!(
                "{}: {} transport is not supported by this agent",
                agent_id, transport
            ));
            continue;
        }
        if target_file(def, scope, project_dir).is_none() {
            errors.push(format!("{agent_id}: {scope} config path is unavailable"));
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
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
        let Some(def) = defs.get(agent_id) else {
            errors.push(format!("{agent_id}: unknown Agent"));
            continue;
        };
        if target_file(def, scope, project_dir).is_none() {
            errors.push(format!("{agent_id}: {scope} config path is unavailable"));
            continue;
        }
        if let Err(errs) = remove_one(agent_id, def, server_name, scope, project_dir, &ts) {
            push_apply_errors(&mut errors, errs);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
            repo: None,
        };
        write_discovered_entry(&entry).map_err(|e| e.to_string())?;
        imported += 1;
    }
    Ok(imported)
}

/// Clear every MCP server from enabled agents' global config files. If
/// `only_agent` is set, restrict to that one agent. Sibling (non-section) bytes
/// are preserved — this removes the servers, it doesn't clobber the whole file.
pub struct CleanOutcome {
    pub cleaned: Vec<String>,
    pub errors: Vec<String>,
}

/// Returns both successful targets and any failures so a backup or write error
/// can never be reported as a successful clean.
pub fn clean(only_agent: Option<&str>) -> CleanOutcome {
    let defs = load_agents();
    let ts = backup_timestamp();
    let mut cleaned = Vec::new();
    let mut errors = Vec::new();
    if let Some(agent_id) = only_agent {
        match defs.get(agent_id) {
            None => errors.push(format!("{agent_id}: unknown Agent")),
            Some(def) if def.global.is_none() => {
                errors.push(format!("{agent_id}: global config path is unavailable"))
            }
            _ => {}
        }
    }
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
        let names: Vec<String> = get_agent_adapter_for(def, agent_id)
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
        match apply_diffs(&diffs, &adef, &empty, &backups_dir(), None, &ts) {
            Ok(()) => cleaned.push(agent_id.clone()),
            Err(apply_errors) => push_apply_errors(&mut errors, apply_errors),
        }
    }
    CleanOutcome { cleaned, errors }
}

// ── Registry entry editing (upsert / remove / paste-import) ────────────────

/// Create or overwrite a user registry entry (stored in the managed "manual"
/// source), auto-syncing the config change to every agent that has it installed.
/// Returns the agents that were synced (empty when nothing needed syncing).
pub fn upsert_entry(entry: RegistryEntry) -> Result<Vec<String>, String> {
    // Capture the currently-effective config BEFORE overwriting it, so we can
    // tell whether the config actually changed (vs a description/tags-only edit).
    let prev = read_registry()
        .into_iter()
        .find(|e| e.name == entry.name && e.transport() == entry.transport());
    write_manual_entry(&entry).map_err(|e| e.to_string())?;
    autosync_after_edit(prev.as_ref(), Some(&entry))
}

/// Remove a user registry override for `name`+`transport`; the entry reverts to
/// whatever a source provides (or vanishes). The fallback config is auto-synced
/// to installs too, for symmetry with edit. Returns the agents synced.
pub fn remove_entry(name: &str, transport: &str) -> Result<Vec<String>, String> {
    let prev = read_registry()
        .into_iter()
        .find(|e| e.name == name && e.transport() == transport);
    delete_registry_entry(name, transport).map_err(|e| e.to_string())?;
    let now = read_registry()
        .into_iter()
        .find(|e| e.name == name && e.transport() == transport);
    autosync_after_edit(prev.as_ref(), now.as_ref())
}

/// Auto-sync a registry-entry config change to agents that have it installed.
///
/// Registry installs write a *snapshot* of the config into each agent file, so a
/// later edit would otherwise not reach those agents. Saving an edit re-stamps
/// the new config into every agent that has this server actively installed at
/// global scope — including copies that drifted or were hand-customized (each
/// write is backed up first, like any install). This replaces the old
/// conservative "clean installs only" propagation, which left drifted installs
/// permanently stale and forced a manual 重新同步 after every edit.
///
/// No-ops when the entry is brand-new (`prev` None), fully removed (`new` None),
/// or the base config didn't change (description/tags-only edit). Global scope
/// only. The catalog edit is already durable when propagation begins, but any
/// failed Agent write is returned to the caller instead of being reported as a
/// successful sync.
/// Returns the agents that got the new config.
fn autosync_after_edit(
    prev: Option<&RegistryEntry>,
    new: Option<&RegistryEntry>,
) -> Result<Vec<String>, String> {
    let (Some(prev), Some(new)) = (prev, new) else {
        return Ok(Vec::new());
    };
    let (Some(old_cfg), Some(new_cfg)) = (base_config(prev), base_config(new)) else {
        return Ok(Vec::new());
    };
    if old_cfg == new_cfg {
        return Ok(Vec::new()); // description/tags-only edit, or no real change
    }
    match resync_entry(&new.name, new.transport(), true) {
        Ok(out) => Ok(out.synced),
        Err(errors) => Err(format!(
            "catalog saved, but Agent sync failed: {}",
            errors.join("; ")
        )),
    }
}

/// Locate the `name -> config` server map in a pasted config value: under a
/// known Agent section key, or the top-level object itself when its values all
/// look like server configs (including Agent-specific URL fields).
fn extract_servers(
    v: &serde_json::Value,
) -> Option<(serde_json::Map<String, serde_json::Value>, Option<Codec>)> {
    let obj = v.as_object()?;
    for (key, codec) in [
        ("mcpServers", None),
        ("mcp_servers", Some(Codec::Codex)),
        ("servers", Some(Codec::VsCode)),
        ("mcp", Some(Codec::OpenCode)),
        ("context_servers", Some(Codec::Standard)),
    ] {
        if let Some(m) = obj.get(key).and_then(|x| x.as_object()) {
            return Some((m.clone(), codec));
        }
    }
    let looks_like_map = !obj.is_empty()
        && obj.values().all(|val| {
            val.as_object()
                .map(|o| {
                    ["command", "url", "httpUrl", "serverUrl"]
                        .iter()
                        .any(|key| o.contains_key(*key))
                })
                .unwrap_or(false)
        });
    if looks_like_map {
        return Some((obj.clone(), None));
    }
    None
}

/// Parse a pasted config blob (JSON, TOML, or YAML) and add every MCP server it contains
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
        Err(_) => match toml::from_str::<toml::Value>(text) {
            Ok(value) => serde_json::to_value(value).map_err(|e| e.to_string())?,
            Err(toml_error) => {
                let value: serde_yaml::Value =
                    serde_yaml::from_str(text).map_err(|yaml_error| {
                        format!(
                            "内容不是有效的 JSON、TOML 或 YAML：TOML: {}; YAML: {}",
                            toml_error, yaml_error
                        )
                    })?;
                serde_json::to_value(value).map_err(|e| e.to_string())?
            }
        },
    };
    let (servers, codec) = extract_servers(&value).ok_or(
        "未识别到 MCP 配置：需要包含 mcpServers、mcp、mcp_servers、servers 或 context_servers",
    )?;
    let mut added = Vec::new();
    for (name, cfg_val) in servers {
        let cfg = codec
            .and_then(|codec| codec.decode(&cfg_val))
            .or_else(|| decode_any(&cfg_val));
        let Some(cfg) = cfg else {
            continue; // skip entries that aren't a valid stdio/http config
        };
        let entry = RegistryEntry {
            name: name.clone(),
            description: String::new(),
            tags: Vec::new(),
            config: cfg.into(),
            origin: None,
            repo: None,
        };
        write_manual_entry(&entry).map_err(|e| e.to_string())?;
        added.push(name);
    }
    if added.is_empty() {
        return Err("未在粘贴内容中找到可用的 MCP server".into());
    }
    Ok(added)
}

/// Serialize the complete effective catalog into a shareable MUX-array JSON
/// string. Shadowed copies are excluded by `read_registry`; origins are stripped
/// so the file is portable, and the output is sorted for stable diffs.
pub fn export_effective() -> Result<String, String> {
    let mut entries = read_registry();
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.transport().cmp(b.transport()))
    });
    for e in &mut entries {
        e.origin = None;
    }
    serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())
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
                .map(|base| {
                    agents
                        .get(&s.agent)
                        .map(|definition| {
                            normalize_with_codec(
                                from_name(definition.codec.as_deref(), &s.agent),
                                base,
                            )
                        })
                        .unwrap_or_else(|| base.clone())
                        != s.config
                })
                .unwrap_or(false);
            InstalledMcp {
                name: s.name,
                agent: s.agent,
                scope: s.scope,
                file_path: s.file_path,
                transport: transport.to_string(),
                customized,
                enabled: true,
            }
        })
        .collect();
    // Append MUX-remembered disabled servers so a UI can show an "off" row for a
    // server removed from the file but re-enable-able.
    let active: HashSet<(String, String, String, String)> = out
        .iter()
        .map(|i| {
            (
                i.agent.clone(),
                i.name.clone(),
                i.transport.clone(),
                i.scope.clone(),
            )
        })
        .collect();
    for (agent, list) in load_disabled() {
        for d in list {
            // Edge case: if somehow also present in the file, the active row wins.
            if active.contains(&(
                agent.clone(),
                d.name.clone(),
                d.transport.clone(),
                d.scope.clone(),
            )) {
                continue;
            }
            out.push(InstalledMcp {
                name: d.name,
                agent: agent.clone(),
                scope: d.scope,
                file_path: String::new(),
                transport: d.transport,
                customized: false,
                enabled: false,
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
/// actively installed at global scope. Runs automatically (forced) on every
/// config-changing save via [`upsert_entry`]; also exposed as an explicit
/// user-invoked repair for installs that drifted without a registry edit.
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
pub fn resync_entry(
    name: &str,
    transport: &str,
    force: bool,
) -> Result<ResyncOutcome, Vec<String>> {
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
    Ok(ResyncOutcome {
        synced: target,
        skipped_customized,
    })
}

/// Delete a user catalog entry (from the manual and/or discovered managed
/// sources) AND uninstall it from every agent that has it — active in a config
/// file or remembered in the disabled store — at global scope. Intended for
/// manual/discovered entries; entries provided by a remote/local source are not
/// removed here (there is nothing user-owned to delete — manage them via their
/// source). A discovered entry may reappear on the next scan if it's still in an
/// agent's config.
pub fn forget_entry(name: &str, transport: &str) -> Result<(), Vec<String>> {
    // Uninstall from every agent that has it (dedup; delete handles both an
    // active file entry and a remembered disabled snapshot).
    let mut agents: Vec<String> = scan_installed(None)
        .into_iter()
        .filter(|i| i.name == name && i.transport == transport)
        .map(|i| i.agent)
        .collect();
    agents.sort();
    agents.dedup();
    if !agents.is_empty() {
        delete(name, transport, "global", &agents, None)?;
    }
    // Drop the entry from the user-owned catalog sources.
    delete_registry_entry(name, transport).map_err(|e| vec![e.to_string()])?;
    delete_discovered_entry(name, transport).map_err(|e| vec![e.to_string()])?;
    Ok(())
}

/// Disable a server: durably snapshot its complete semantic entry, then remove it
/// from the Agent file. Saving first means a settings failure can never remove
/// the only live copy. If removal later fails, the still-active entry wins in the
/// UI and the retained snapshot makes a retry safe.
pub fn disable(
    server_name: &str,
    transport: &str,
    scope: &str,
    agents: &[String],
    project_dir: Option<&str>,
) -> Result<(), Vec<String>> {
    let defs = load_agents();
    let scanned = scan_agents(&defs, project_dir.map(Path::new), true);
    let mut errors = Vec::new();
    let ts = backup_timestamp();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else {
            errors.push(format!("{agent_id}: unknown Agent"));
            continue;
        };
        let Some(path) = target_file(def, scope, project_dir) else {
            errors.push(format!("{agent_id}: {scope} config path is unavailable"));
            continue;
        };
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
        let snapshot = match get_agent_adapter_for(def, agent_id).snapshot(&path, server_name) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                errors.push(format!(
                    "{}: target entry disappeared before snapshot",
                    agent_id
                ));
                continue;
            }
            Err(error) => {
                errors.push(format!("{}: {}", agent_id, error));
                continue;
            }
        };
        let entry = DisabledEntry {
            name: server_name.to_string(),
            transport: transport.to_string(),
            scope: scope.to_string(),
            config: found.config.clone(),
            snapshot: Some(snapshot.clone()),
        };
        if let Err(error) = remember(agent_id, entry) {
            errors.push(format!("{}: save disabled snapshot: {}", agent_id, error));
            continue;
        }

        if let Err(errs) = remove_snapshot(
            agent_id,
            def,
            server_name,
            &snapshot,
            scope,
            project_dir.map(Path::new),
            &ts,
        ) {
            push_apply_errors(&mut errors, errs);
            continue;
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
    let mut errors = Vec::new();
    let ts = backup_timestamp();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else {
            errors.push(format!("{agent_id}: unknown Agent"));
            continue;
        };
        let entry = load_disabled().get(agent_id).and_then(|list| {
            list.iter()
                .find(|d| d.name == server_name && d.transport == transport && d.scope == scope)
                .cloned()
        });
        let Some(entry) = entry else {
            errors.push(format!("{}: no disabled snapshot", agent_id));
            continue;
        };
        if target_file(def, scope, project_dir).is_none() {
            errors.push(format!("{}: target config path is unavailable", agent_id));
            continue;
        }
        let result = if let Some(snapshot) = entry.snapshot.as_ref() {
            restore_snapshot(
                agent_id,
                def,
                server_name,
                snapshot,
                scope,
                project_dir.map(Path::new),
                &ts,
            )
        } else {
            // Backward compatibility for snapshots written before v1.1.5.
            add_one(
                agent_id,
                def,
                server_name,
                entry.config.clone(),
                scope,
                project_dir,
                &ts,
            )
        };
        if let Err(errs) = result {
            push_apply_errors(&mut errors, errs);
            continue;
        }
        match remove_if_unchanged(agent_id, &entry) {
            Ok(true) => {}
            Ok(false) => errors.push(format!(
                "{}: disabled snapshot changed concurrently and was retained",
                agent_id
            )),
            Err(error) => errors.push(format!("{}: remove disabled snapshot: {}", agent_id, error)),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
    let mut errors = Vec::new();
    let ts = backup_timestamp();
    for agent_id in agents {
        let Some(def) = defs.get(agent_id) else {
            errors.push(format!("{agent_id}: unknown Agent"));
            continue;
        };
        if target_file(def, scope, project_dir).is_none() {
            errors.push(format!(
                "{agent_id}: {scope} config path is unavailable; disabled snapshot retained"
            ));
            continue;
        }
        let mut may_purge_snapshot = true;
        if let Err(errs) = remove_one(agent_id, def, server_name, scope, project_dir, &ts) {
            push_apply_errors(&mut errors, errs);
            may_purge_snapshot = false;
        }
        if may_purge_snapshot {
            if let Err(error) = purge(agent_id, server_name, transport, scope) {
                errors.push(format!("{}: purge disabled snapshot: {}", agent_id, error));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
