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
    pub name: String,
    pub format: String,
    pub key: String,
    pub has_global: bool,
    pub has_project: bool,
    pub enabled: bool,
    pub global: Option<String>,
    pub project: Option<String>,
    pub supported_transports: Vec<&'static str>,
    pub docs: Option<String>,
    pub note: Option<String>,
    pub category: String,
    pub evidence: String,
    pub verified_at: Option<String>,
    pub builtin: bool,
}

pub fn supports_transport(agent_id: &str, transport: &str) -> bool {
    load_agents()
        .get(agent_id)
        .is_some_and(|definition| definition_supports_transport(definition, transport))
}

fn definition_supports_transport(definition: &AgentDefinition, transport: &str) -> bool {
    definition
        .transports
        .as_ref()
        .map(|transports| transports.iter().any(|item| item == transport))
        .unwrap_or_else(|| matches!(transport, "stdio" | "http"))
}

fn supported_transports(definition: &AgentDefinition) -> Vec<&'static str> {
    ["stdio", "http"]
        .into_iter()
        .filter(|transport| definition_supports_transport(definition, transport))
        .collect()
}

/// 内置 agent 定义：编译期内嵌 root agents.json（与 TS CLI 共用的单一数据源）
const BUILTIN_AGENTS_JSON: &str = include_str!("../../data/agents.json");
const CATALOG_AGENTS_JSON: &str = include_str!("../../data/agent-catalog.json");

fn audited_agents() -> BTreeMap<String, AgentDefinition> {
    serde_json::from_str(BUILTIN_AGENTS_JSON).expect("agents.json must be valid")
}

pub fn builtin_agents() -> BTreeMap<String, AgentDefinition> {
    let mut catalog: BTreeMap<String, AgentDefinition> =
        serde_json::from_str(CATALOG_AGENTS_JSON).expect("agent-catalog.json must be valid");
    catalog.extend(audited_agents());
    catalog
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
    let audited = audited_agents();
    for (id, current) in builtins {
        let Some(saved) = stored.get_mut(&id) else {
            stored.insert(id, current);
            continue;
        };
        // A user-created target may legitimately share an id with a broad
        // discovery-only record. Deep-audited definitions, however, own their
        // ids and wire schemas regardless of stale/malformed persisted flags.
        if saved.builtin != Some(true) && !audited.contains_key(&id) {
            continue;
        }
        let enabled = saved.enabled;
        let global = migrated_builtin_global(&id, saved, &current);
        // Definitions are audited data, while enabled/global-path
        // customizations are user state. Replacing from the current definition
        // prevents an old settings snapshot from retaining an obsolete codec,
        // project path, or a writable path for a now read-only catalog item.
        *saved = current;
        saved.enabled = enabled;
        saved.global = global;
    }
    stored
}

fn migrated_builtin_global(
    id: &str,
    saved: &AgentDefinition,
    current: &AgentDefinition,
) -> Option<String> {
    // No audited writer means no path override. This keeps catalog-only targets
    // read-only even when old settings claimed a path for them.
    current.global.as_ref()?;
    // QoderWork was read-only before its user-level contract was verified. Do
    // not turn a stale guessed path into a writable override during promotion.
    if id == "qoderwork" && (saved.format == "unknown" || saved.key.is_empty()) {
        return current.global.clone();
    }
    let stale = matches!(
        (id, saved.global.as_deref()),
        ("qoder", Some("~/.qoder/settings.json"))
            | ("amazon-q", Some("~/.aws/amazonq/mcp.json"))
            | (
                "cline",
                Some(
                    "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"
                )
            )
            | ("continue", Some("~/.continue/config.json"))
            | ("qoderwork", Some("~/.qoderwork/settings.json"))
    );
    if stale {
        current.global.clone()
    } else {
        saved.global.clone().or_else(|| current.global.clone())
    }
}

/// 将完整 agent map 写入 settings.agents（保留其它分区不动）。
pub fn save_agents(map: &BTreeMap<String, AgentDefinition>) -> std::io::Result<()> {
    let overrides = agent_overrides(map);
    mutate_settings(|s| {
        s.agents = (!overrides.is_empty()).then_some(overrides);
    })
}

fn agent_overrides(map: &BTreeMap<String, AgentDefinition>) -> BTreeMap<String, AgentDefinition> {
    let builtins = builtin_agents();
    map.iter()
        .filter(|(id, definition)| builtins.get(*id) != Some(*definition))
        .map(|(id, definition)| (id.clone(), definition.clone()))
        .collect()
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
    let mut agents = load_agents();
    if !allow_overwrite && agents.contains_key(&id) {
        return Err(format!("agent 已存在: {}", id));
    }
    if let Some(existing) = agents.get(&id) {
        if existing.builtin == Some(true) {
            if existing.global.is_none() {
                return Err("该 Agent 尚无可写的全局配置定义".into());
            }
            // Built-in wire schemas are audited product contracts. All callers,
            // not just the UI, may override only global path and enabled state.
            def.project = existing.project.clone();
            def.format = existing.format.clone();
            def.key = existing.key.clone();
        }
        copy_internal_metadata(&mut def, existing);
    } else {
        def.builtin = Some(false);
        def.name.get_or_insert_with(|| id.clone());
        def.category.get_or_insert_with(|| "custom".into());
        def.evidence.get_or_insert_with(|| "custom".into());
        def.codec.get_or_insert_with(|| "standard".into());
        def.layout.get_or_insert_with(|| "map".into());
        def.transports
            .get_or_insert_with(|| vec!["stdio".into(), "http".into()]);
    }
    if def.key.trim().is_empty() {
        return Err("配置 key 不能为空".into());
    }
    if !matches!(def.format.as_str(), "json" | "toml" | "yaml") {
        return Err("配置格式仅支持 JSON、TOML 或 YAML".into());
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
    if def.layout.as_deref() == Some("list") && def.identity_field.is_none() {
        return Err("列表型配置必须指定 identity_field".into());
    }
    agents.insert(id, def);
    save_agents(&agents).map_err(|e| e.to_string())
}

/// List all agent definitions as `AgentInfo` view rows.
pub fn list_infos() -> Vec<AgentInfo> {
    load_agents()
        .into_iter()
        .map(|(id, d)| AgentInfo {
            supported_transports: supported_transports(&d),
            name: d.name.clone().unwrap_or_else(|| id.clone()),
            id,
            format: d.format,
            key: d.key,
            has_global: d.global.is_some(),
            has_project: d.project.is_some(),
            enabled: d.enabled,
            global: d.global,
            project: d.project,
            docs: d.docs,
            note: d.note,
            category: d.category.unwrap_or_else(|| "custom".into()),
            evidence: d.evidence.unwrap_or_else(|| "custom".into()),
            verified_at: d.verified_at,
            builtin: d.builtin == Some(true),
        })
        .collect()
}

fn copy_internal_metadata(definition: &mut AgentDefinition, existing: &AgentDefinition) {
    definition.builtin = existing.builtin;
    definition.name = existing.name.clone();
    definition.docs = existing.docs.clone();
    definition.note = existing.note.clone();
    definition.category = existing.category.clone();
    definition.evidence = existing.evidence.clone();
    definition.verified_at = existing.verified_at.clone();
    definition.codec = existing.codec.clone();
    definition.layout = existing.layout.clone();
    definition.identity_field = existing.identity_field.clone();
    definition.transports = existing.transports.clone();
    definition.root_defaults = existing.root_defaults.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_catalog_and_transport_metadata_load() {
        let a = builtin_agents();
        assert!(a.len() >= 170);
        assert_eq!(a["claude-code"].key, "mcpServers");
        assert_eq!(a["codex"].format, "toml");
        assert!(!definition_supports_transport(&a["claude-desktop"], "http"));
        assert!(definition_supports_transport(&a["claude-desktop"], "stdio"));
        assert!(definition_supports_transport(&a["claude-code"], "http"));
    }

    #[test]
    fn stale_builtin_paths_migrate_without_touching_custom_agents() {
        let mut stored = builtin_agents();
        stored.get_mut("qoder").unwrap().global = Some("~/.qoder/settings.json".into());
        stored.get_mut("amazon-q").unwrap().global = Some("~/.aws/amazonq/mcp.json".into());
        stored.get_mut("amazon-q").unwrap().project = Some(".amazonq/mcp.json".into());
        stored.get_mut("cline").unwrap().global = Some(
            "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json".into(),
        );
        stored.get_mut("cline").unwrap().enabled = false;
        stored.get_mut("continue").unwrap().global = Some("~/.continue/config.json".into());
        stored.get_mut("continue").unwrap().project = Some(".continue/config.json".into());
        stored.get_mut("qoderwork").unwrap().global = Some("~/.custom/qoderwork.json".into());
        stored.get_mut("qoderwork").unwrap().format = "unknown".into();
        stored.get_mut("qoderwork").unwrap().key.clear();
        let custom = AgentDefinition {
            global: Some("~/.custom/mcp.json".into()),
            project: None,
            format: "json".into(),
            key: "mcpServers".into(),
            enabled: false,
            builtin: Some(false),
            ..Default::default()
        };
        stored.insert("custom".into(), custom.clone());

        let merged = merge_builtin_updates(stored);

        assert_eq!(
            merged["qoder"].global.as_deref(),
            Some("~/Library/Application Support/Qoder/SharedClientCache/mcp.json")
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
        assert_eq!(
            merged["continue"].global.as_deref(),
            Some("~/.continue/config.yaml")
        );
        assert!(merged["continue"].project.is_none());
        assert_eq!(
            merged["qoderwork"].global.as_deref(),
            Some("~/.qoderwork/mcp.json")
        );
        assert_eq!(merged["custom"], custom);
    }

    #[test]
    fn stale_qoder_ide_definition_migrates_from_cli_path() {
        let mut stored = builtin_agents();
        stored.get_mut("qoder").unwrap().global = Some("~/.qoder/settings.json".into());

        let merged = merge_builtin_updates(stored);

        assert_eq!(
            merged["qoder"].global.as_deref(),
            Some("~/Library/Application Support/Qoder/SharedClientCache/mcp.json")
        );

        let mut customized = builtin_agents();
        customized.get_mut("qoder").unwrap().global = Some("~/.custom/qoder-mcp.json".into());
        assert_eq!(
            merge_builtin_updates(customized)["qoder"].global.as_deref(),
            Some("~/.custom/qoder-mcp.json")
        );
    }

    #[test]
    fn stale_qoderwork_settings_path_migrates_to_mcp_file() {
        let mut stored = builtin_agents();
        stored.get_mut("qoderwork").unwrap().global = Some("~/.qoderwork/settings.json".into());

        let merged = merge_builtin_updates(stored);

        assert_eq!(
            merged["qoderwork"].global.as_deref(),
            Some("~/.qoderwork/mcp.json")
        );
    }

    #[test]
    fn audited_schema_wins_but_custom_catalog_collision_survives() {
        let mut stored = builtin_agents();
        stored.insert(
            "codex".into(),
            AgentDefinition {
                global: Some("~/.custom/codex.toml".into()),
                project: Some("unsafe-project.yaml".into()),
                format: "yaml".into(),
                key: "unsafe".into(),
                enabled: false,
                builtin: Some(false),
                ..Default::default()
            },
        );
        let custom_chatgpt = AgentDefinition {
            global: Some("~/.custom/chatgpt.json".into()),
            project: None,
            format: "json".into(),
            key: "mcpServers".into(),
            enabled: true,
            builtin: Some(false),
            ..Default::default()
        };
        stored.insert("chatgpt".into(), custom_chatgpt.clone());

        let merged = merge_builtin_updates(stored);
        assert_eq!(merged["codex"].format, "toml");
        assert_eq!(merged["codex"].key, "mcp_servers");
        assert_eq!(
            merged["codex"].project.as_deref(),
            Some(".codex/config.toml")
        );
        assert_eq!(
            merged["codex"].global.as_deref(),
            Some("~/.custom/codex.toml")
        );
        assert!(!merged["codex"].enabled);
        assert_eq!(merged["codex"].builtin, Some(true));
        assert_eq!(merged["chatgpt"], custom_chatgpt);
    }

    #[test]
    fn persistence_keeps_only_builtin_overrides_and_custom_agents() {
        let mut agents = builtin_agents();
        assert!(agent_overrides(&agents).is_empty());

        agents.get_mut("codex").unwrap().enabled = false;
        agents.insert(
            "custom".into(),
            AgentDefinition {
                global: Some("~/.custom/mcp.json".into()),
                project: None,
                format: "json".into(),
                key: "mcpServers".into(),
                enabled: true,
                builtin: Some(false),
                ..Default::default()
            },
        );

        let overrides = agent_overrides(&agents);
        assert_eq!(overrides.len(), 2);
        assert!(overrides.contains_key("codex"));
        assert!(overrides.contains_key("custom"));
    }

    #[test]
    fn put_locks_builtin_schema_and_rejects_catalog_only_targets() {
        let _home = crate::testenv::TestHome::new("agent-schema-lock");
        let mut codex = builtin_agents()["codex"].clone();
        codex.global = Some("~/.custom-codex.toml".into());
        codex.project = Some(".unsafe/project.yaml".into());
        codex.format = "yaml".into();
        codex.key = "unsafe".into();
        codex.codec = Some("goose".into());
        codex.layout = Some("list".into());
        codex.identity_field = Some("unsafe".into());
        codex.transports = Some(vec!["stdio".into()]);
        codex.builtin = Some(false);

        put("codex".into(), codex, true).unwrap();
        let stored = load_agents();
        let codex = &stored["codex"];
        assert_eq!(codex.global.as_deref(), Some("~/.custom-codex.toml"));
        assert_eq!(codex.project.as_deref(), Some(".codex/config.toml"));
        assert_eq!(codex.format, "toml");
        assert_eq!(codex.key, "mcp_servers");
        assert_eq!(codex.codec.as_deref(), Some("codex"));
        assert_eq!(codex.layout.as_deref(), Some("map"));
        assert_eq!(codex.builtin, Some(true));
        assert!(definition_supports_transport(codex, "http"));

        let attempted_devin = AgentDefinition {
            global: Some("~/.devin/mcp.json".into()),
            project: None,
            format: "json".into(),
            key: "mcpServers".into(),
            enabled: true,
            builtin: Some(false),
            ..Default::default()
        };
        assert!(put("devin".into(), attempted_devin, true).is_err());
    }
}
