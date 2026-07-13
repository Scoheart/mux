use crate::scanner::collapse_home;
use crate::settings::{load_settings, mutate_settings};
use crate::types::AgentDefinition;
use serde::Serialize;
use std::collections::BTreeMap;

/// An agent definition as surfaced to a UI: its stored config plus derived
/// has-path flags. `global`/`project` keep the raw stored `~/…` paths so the UI
/// can display and prefill the path editor.
#[derive(Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub format: String,
    pub key: String,
    pub has_global: bool,
    pub has_project: bool,
    pub enabled: bool,
    pub global: Option<String>,
    pub project: Option<String>,
    pub supported_transports: Vec<&'static str>,
}

pub fn supports_transport(agent_id: &str, transport: &str) -> bool {
    match transport {
        "stdio" => true,
        "http" => agent_id != "claude-desktop",
        _ => false,
    }
}

fn supported_transports(agent_id: &str) -> Vec<&'static str> {
    if supports_transport(agent_id, "http") {
        vec!["stdio", "http"]
    } else {
        vec!["stdio"]
    }
}

/// 内置 agent 定义：编译期内嵌 root agents.json（与 TS CLI 共用的单一数据源）
const BUILTIN_AGENTS_JSON: &str = include_str!("../../data/agents.json");

pub fn builtin_agents() -> BTreeMap<String, AgentDefinition> {
    serde_json::from_str(BUILTIN_AGENTS_JSON).expect("agents.json must be valid")
}

/// 优先读 settings.agents（与 CLI 共用），缺失或为空时用内置。
pub fn load_agents() -> BTreeMap<String, AgentDefinition> {
    match load_settings().agents {
        Some(map) if !map.is_empty() => merge_builtin_updates(map),
        _ => builtin_agents(),
    }
}

fn merge_builtin_updates(
    mut stored: BTreeMap<String, AgentDefinition>,
) -> BTreeMap<String, AgentDefinition> {
    let builtins = builtin_agents();
    for (id, current) in builtins {
        let Some(saved) = stored.get_mut(&id) else {
            stored.insert(id, current);
            continue;
        };
        if saved.builtin != Some(true) {
            continue;
        }
        match id.as_str() {
            "qoder"
                if saved.global.as_deref()
                    == Some("~/Library/Application Support/Qoder/SharedClientCache/mcp.json") =>
            {
                saved.global = current.global;
            }
            "amazon-q" if saved.global.as_deref() == Some("~/.aws/amazonq/mcp.json") => {
                saved.global = current.global;
                if saved.project.as_deref() == Some(".amazonq/mcp.json") {
                    saved.project = current.project;
                }
            }
            "cline"
                if saved.global.as_deref()
                    == Some(
                        "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
                    ) =>
            {
                saved.global = current.global;
            }
            "continue" if saved.global.as_deref() == Some("~/.continue/config.json") => {
                saved.global = current.global;
                if saved.project.as_deref() == Some(".continue/config.json") {
                    saved.project = current.project;
                }
            }
            "qoderwork" if saved.global.as_deref() == Some("~/.qoderwork/mcp.json") => {
                saved.global = current.global;
            }
            _ => {}
        }
    }
    stored
}

/// 将完整 agent map 写入 settings.agents（保留其它分区不动）。
pub fn save_agents(map: &BTreeMap<String, AgentDefinition>) -> std::io::Result<()> {
    mutate_settings(|s| {
        s.agents = Some(map.clone());
    })
}

/// Validate + normalize an agent definition, then persist it (merged over
/// builtin/existing defs in `settings.agents`). `allow_overwrite` distinguishes
/// create (errors on an existing id) from edit (replaces in place). Global paths
/// are collapsed to `~/…`. The legacy project field is retained for backward
/// compatibility, but every usable definition must have a global path.
pub fn put(id: String, mut def: AgentDefinition, allow_overwrite: bool) -> Result<(), String> {
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
    if def.global.is_none() {
        return Err("全局配置路径不能为空".into());
    }
    let mut agents = load_agents();
    if !allow_overwrite && agents.contains_key(&id) {
        return Err(format!("agent 已存在: {}", id));
    }
    agents.insert(id, def);
    save_agents(&agents).map_err(|e| e.to_string())
}

/// List all agent definitions as `AgentInfo` view rows.
pub fn list_infos() -> Vec<AgentInfo> {
    load_agents()
        .into_iter()
        .map(|(id, d)| AgentInfo {
            supported_transports: supported_transports(&id),
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
mod tests {
    use super::*;
    #[test]
    fn builtin_loads_18_plus() {
        let a = builtin_agents();
        assert!(a.len() >= 18);
        assert_eq!(a["claude-code"].key, "mcpServers");
        assert_eq!(a["codex"].format, "toml");
        assert!(!supports_transport("claude-desktop", "http"));
        assert!(supports_transport("claude-desktop", "stdio"));
        assert!(supports_transport("claude-code", "http"));
    }

    #[test]
    fn stale_builtin_paths_migrate_without_touching_custom_agents() {
        let mut stored = builtin_agents();
        stored.get_mut("qoder").unwrap().global =
            Some("~/Library/Application Support/Qoder/SharedClientCache/mcp.json".into());
        stored.get_mut("amazon-q").unwrap().global = Some("~/.aws/amazonq/mcp.json".into());
        stored.get_mut("amazon-q").unwrap().project = Some(".amazonq/mcp.json".into());
        stored.get_mut("cline").unwrap().global = Some(
            "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json".into(),
        );
        stored.get_mut("cline").unwrap().enabled = false;
        stored.get_mut("continue").unwrap().global = Some("~/.continue/config.json".into());
        stored.get_mut("continue").unwrap().project = Some(".continue/config.json".into());
        stored.get_mut("qoderwork").unwrap().global = Some("~/.qoderwork/mcp.json".into());
        let custom = AgentDefinition {
            global: Some("~/.custom/mcp.json".into()),
            project: None,
            format: "json".into(),
            key: "mcpServers".into(),
            enabled: false,
            builtin: Some(false),
        };
        stored.insert("custom".into(), custom.clone());

        let merged = merge_builtin_updates(stored);

        assert_eq!(
            merged["qoder"].global.as_deref(),
            Some("~/.qoder/settings.json")
        );
        assert_eq!(
            merged["amazon-q"].global.as_deref(),
            Some("~/.aws/amazonq/default.json")
        );
        assert_eq!(
            merged["amazon-q"].project.as_deref(),
            Some(".amazonq/default.json")
        );
        assert_eq!(
            merged["cline"].global.as_deref(),
            Some("~/.cline/data/settings/cline_mcp_settings.json")
        );
        assert!(!merged["cline"].enabled);
        assert!(merged["continue"].global.is_none());
        assert!(merged["continue"].project.is_none());
        assert!(merged["qoderwork"].global.is_none());
        assert_eq!(merged["custom"], custom);
    }
}
