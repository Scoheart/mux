use crate::adapter::get_adapter;
use crate::types::{AgentDefinition, McpConfig};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ScannedMcp {
    pub name: String,
    pub config: McpConfig,
    pub agent: String,
    pub scope: String, // "global" | "project"
    pub file_path: String,
}

pub fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(p)
}

fn read_section(format: &str, key: &str, path: &Path) -> BTreeMap<String, McpConfig> {
    get_adapter(format, key).read(path)
}

pub fn scan_agents(
    agents: &BTreeMap<String, AgentDefinition>,
    project_dir: Option<&Path>,
    scan_all: bool,
) -> Vec<ScannedMcp> {
    let mut out = Vec::new();
    for (name, def) in agents {
        if !scan_all && !def.enabled {
            continue;
        }
        if let Some(g) = &def.global {
            let path = expand_tilde(g);
            for (mcp_name, cfg) in read_section(&def.format, &def.key, &path) {
                out.push(ScannedMcp {
                    name: mcp_name, config: cfg, agent: name.clone(),
                    scope: "global".into(), file_path: path.display().to_string(),
                });
            }
        }
        if let (Some(proj), Some(base)) = (&def.project, project_dir) {
            let path = base.join(proj);
            for (mcp_name, cfg) in read_section(&def.format, &def.key, &path) {
                out.push(ScannedMcp {
                    name: mcp_name, config: cfg, agent: name.clone(),
                    scope: "project".into(), file_path: path.display().to_string(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentDefinition;

    #[test]
    fn scans_project_json_config() {
        let mut base = std::env::temp_dir();
        base.push(format!("mux-scan-project-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("mcp.json"),
            r#"{"mcpServers":{"git":{"command":"npx"}}}"#).unwrap();
        let mut agents = BTreeMap::new();
        agents.insert("test".to_string(), AgentDefinition {
            global: None, project: Some("mcp.json".into()),
            format: "json".into(), key: "mcpServers".into(),
            enabled: true, builtin: Some(true),
        });
        let found = scan_agents(&agents, Some(&base), false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "git");
        assert_eq!(found[0].scope, "project");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn skips_disabled_unless_scan_all() {
        let mut agents = BTreeMap::new();
        agents.insert("off".to_string(), AgentDefinition {
            global: Some("~/nope.json".into()), project: None,
            format: "json".into(), key: "mcpServers".into(),
            enabled: false, builtin: None,
        });
        assert_eq!(scan_agents(&agents, None, false).len(), 0);
    }
}
