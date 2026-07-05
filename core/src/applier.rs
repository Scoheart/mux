use crate::adapter::get_adapter;
use crate::differ::{DiffAction, DiffEntry};
use crate::scanner::expand_tilde;
use crate::types::{AgentDefinition, McpConfig};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A single failed apply target, collected so one failure does not abort the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyError {
    pub target: String,
    pub error: String,
}

fn target_path(def: &AgentDefinition, scope: &str, project_dir: Option<&Path>) -> Option<PathBuf> {
    if scope == "global" {
        def.global.as_ref().map(|g| expand_tilde(g))
    } else {
        match (&def.project, project_dir) {
            (Some(p), Some(base)) => Some(base.join(p)),
            _ => None,
        }
    }
}

fn backup(path: &Path, backups_dir: &Path, stamp: &str) {
    if !path.exists() { return; }
    let _ = fs::create_dir_all(backups_dir);
    let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let _ = fs::copy(path, backups_dir.join(format!("{}-{}", fname, stamp)));
}

/// Apply diffs to their target files. A single failing target is recorded and
/// does not abort the remaining work; `Ok(())` is returned only if all targets
/// succeed, otherwise `Err` carries every failure (spec §5).
///
/// `configs`: (mcp_name -> already-computed effective config) for Add diffs.
pub fn apply_diffs(
    diffs: &[DiffEntry],
    agents: &BTreeMap<String, AgentDefinition>,
    configs: &BTreeMap<String, McpConfig>,
    backups_dir: &Path,
    project_dir: Option<&Path>,
    timestamp: &str,
) -> Result<(), Vec<ApplyError>> {
    let mut backed_up = std::collections::HashSet::new();
    let mut errors: Vec<ApplyError> = Vec::new();
    for diff in diffs {
        let Some(def) = agents.get(&diff.agent) else { continue };
        let Some(path) = target_path(def, &diff.scope, project_dir) else { continue };
        if !backed_up.contains(&path) && path.exists() {
            backup(&path, backups_dir, timestamp);
            backed_up.insert(path.clone());
        }
        let adapter = get_adapter(&def.format, &def.key);
        let result = match diff.action {
            DiffAction::Add => {
                let Some(cfg) = configs.get(&diff.mcp_name) else { continue };
                // Single-server upsert: never re-serialize the user's other servers.
                adapter.upsert(&path, &diff.mcp_name, cfg)
            }
            DiffAction::Remove => {
                let names = vec![diff.mcp_name.clone()];
                adapter.remove(&path, &names)
            }
        };
        if let Err(e) = result {
            errors.push(ApplyError { target: path.display().to_string(), error: e });
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentDefinition, StdioConfig};

    #[test]
    fn applies_add_and_creates_backup() {
        let mut base = std::env::temp_dir();
        base.push(format!("mux-apply-add-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let cfg_path = base.join("mcp.json");
        std::fs::write(&cfg_path, r#"{"mcpServers":{"old":{"command":"x"}}}"#).unwrap();

        let mut agents = BTreeMap::new();
        agents.insert("test".to_string(), AgentDefinition {
            global: None, project: Some("mcp.json".into()),
            format: "json".into(), key: "mcpServers".into(),
            enabled: true, builtin: None });

        let mut configs = BTreeMap::new();
        configs.insert("git".to_string(), McpConfig::Stdio(StdioConfig {
            command: "npx".into(), args: None, env: None }));

        let diffs = vec![DiffEntry { action: DiffAction::Add,
            mcp_name: "git".into(), agent: "test".into(), scope: "project".into() }];
        let backups = base.join("backups");
        let res = apply_diffs(&diffs, &agents, &configs, &backups, Some(&base), "STAMP");

        assert!(res.is_ok());
        let written = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(written.contains("git"));
        assert!(backups.join("mcp.json-STAMP").exists());
        std::fs::remove_dir_all(&base).ok();
    }
}
