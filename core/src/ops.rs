//! Install / uninstall / discovery orchestration, shared by the desktop (Tauri
//! commands) and the CLI. Tauri-free — plain functions over the core stores.

use crate::adapter::get_adapter;
use crate::agents::load_agents;
use crate::applier::{apply_diffs, ApplyError};
use crate::differ::{DiffAction, DiffEntry};
use crate::effective::effective_config;
use crate::r#override::OverridePatch;
use crate::paths::{backup_timestamp, backups_dir};
use crate::registry::{read_registry, write_discovered_entry};
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
