use crate::scanner::collapse_home;
use crate::settings::{
    load_settings, mutate_settings, mutate_settings_checked, AgentConfigPathOverride,
};
use crate::types::AgentDefinition;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Error, ErrorKind};

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
    pub skills_global_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfigurationInput {
    pub mcp_path: String,
    #[serde(default)]
    pub model_paths: Vec<String>,
    pub skills_global_dir: Option<String>,
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
const VERIFIED_SKILL_AGENT_IDS: &[&str] = &[
    "claude-code",
    "codex",
    "copilot-cli",
    "cursor",
    "gemini",
    "opencode",
];

fn audited_agents() -> BTreeMap<String, AgentDefinition> {
    serde_json::from_str(BUILTIN_AGENTS_JSON).expect("agents.json must be valid")
}

pub fn builtin_agents() -> BTreeMap<String, AgentDefinition> {
    let catalog: BTreeMap<String, AgentDefinition> =
        serde_json::from_str(CATALOG_AGENTS_JSON).expect("agent-catalog.json must be valid");
    merge_builtin_definitions(catalog, audited_agents())
        .expect("builtin Agent Skills capabilities must be valid")
}

fn merge_builtin_definitions(
    mut catalog: BTreeMap<String, AgentDefinition>,
    audited: BTreeMap<String, AgentDefinition>,
) -> Result<BTreeMap<String, AgentDefinition>, String> {
    for definition in catalog.values_mut() {
        definition.skills = None;
    }
    validate_audited_skill_capabilities(&audited)?;
    catalog.extend(audited);
    Ok(catalog)
}

fn validate_audited_skill_capabilities(
    audited: &BTreeMap<String, AgentDefinition>,
) -> Result<(), String> {
    let capability_ids: Vec<&str> = audited
        .iter()
        .filter_map(|(id, definition)| definition.skills.as_ref().map(|_| id.as_str()))
        .collect();
    if capability_ids != VERIFIED_SKILL_AGENT_IDS {
        return Err(format!(
            "audited Skills capability IDs must be {:?}, found {:?}",
            VERIFIED_SKILL_AGENT_IDS, capability_ids
        ));
    }
    validate_skill_capabilities(audited)
}

fn validate_skill_capabilities(agents: &BTreeMap<String, AgentDefinition>) -> Result<(), String> {
    let mut paths_by_target = BTreeMap::<String, String>::new();
    let mut targets_by_path = BTreeMap::<String, String>::new();

    for (agent_id, definition) in agents {
        let Some(capability) = definition.skills.as_ref() else {
            continue;
        };
        if capability.evidence != "official" {
            return Err(format!(
                "Skills capability for {agent_id} requires official evidence"
            ));
        }
        if capability.docs.trim().is_empty() {
            return Err(format!(
                "Skills capability for {agent_id} requires documentation"
            ));
        }
        if capability.verified_at.trim().is_empty() {
            return Err(format!(
                "Skills capability for {agent_id} requires a verification date"
            ));
        }
        if capability.probes.is_empty() {
            return Err(format!(
                "Skills capability for {agent_id} requires an install probe"
            ));
        }
        let directories = std::iter::once((
            capability.target_id.as_str(),
            capability.global_dir.as_str(),
        ))
        .chain(
            capability
                .aliases
                .iter()
                .map(|alias| (alias.target_id.as_str(), alias.global_dir.as_str())),
        );

        for (target_id, path) in directories {
            validate_skill_directory(path)
                .map_err(|reason| format!("invalid Skills target for {agent_id}: {reason}"))?;

            if let Some(existing) = paths_by_target.get(target_id) {
                if existing != path {
                    return Err(format!(
                        "Skills target {target_id} maps to both {existing} and {path}"
                    ));
                }
            } else {
                paths_by_target.insert(target_id.to_string(), path.to_string());
            }

            if let Some(existing) = targets_by_path.get(path) {
                if existing != target_id {
                    return Err(format!(
                        "Skills path {path} maps to both {existing} and {target_id}"
                    ));
                }
            } else {
                targets_by_path.insert(path.to_string(), target_id.to_string());
            }
        }
    }

    Ok(())
}

fn validate_skill_directory(path: &str) -> Result<(), String> {
    if !path.starts_with("~/") || !path.ends_with("/skills") {
        return Err(format!("{path} must be a ~/.../skills path"));
    }
    let components: Vec<&str> = path[2..].split('/').collect();
    if components
        .iter()
        .any(|component| component.is_empty() || matches!(*component, "." | ".."))
    {
        return Err(format!("{path} contains an unsafe path component"));
    }
    if components.first() == Some(&".mux") {
        return Err(format!("{path} is inside MUX-managed storage"));
    }
    Ok(())
}

/// 优先读 settings.agents（与 CLI 共用），缺失或为空时用内置。
pub fn load_agents() -> BTreeMap<String, AgentDefinition> {
    load_agents_from_settings(&load_settings())
}

pub(crate) fn load_agents_from_settings(
    settings: &crate::settings::Settings,
) -> BTreeMap<String, AgentDefinition> {
    let mut agents = match settings.agents.clone() {
        Some(map) if !map.is_empty() => merge_builtin_updates(map),
        _ => builtin_agents(),
    };
    for (agent_id, path_override) in settings.agent_config_paths.iter().flatten() {
        let Some(global_dir) = path_override.skills_global_dir.as_ref() else {
            continue;
        };
        if let Some(capability) = agents
            .get_mut(agent_id)
            .and_then(|definition| definition.skills.as_mut())
        {
            if capability.global_dir != *global_dir {
                capability.global_dir = global_dir.clone();
                // A configured primary directory is a runtime declaration of
                // its own. Reusing the audited target id would make one id
                // point to both the catalog path and the override path when
                // another Agent still declares the catalog target.
                capability.target_id = format!("{agent_id}-configured");
            }
        }
    }
    agents
}

fn merge_builtin_updates(
    mut stored: BTreeMap<String, AgentDefinition>,
) -> BTreeMap<String, AgentDefinition> {
    let builtins = builtin_agents();
    let audited = audited_agents();
    for (id, definition) in &mut stored {
        if !audited.contains_key(id) {
            definition.skills = None;
        }
    }
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
        if existing.builtin != Some(true) {
            def.skills = None;
        }
    } else {
        def.builtin = Some(false);
        def.skills = None;
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

/// Update every configurable write location from one user action. Model and
/// Skills locations are validated before the settings transaction starts, so
/// the command cannot persist only a subset of the requested paths.
pub fn update_configuration(id: String, input: AgentConfigurationInput) -> Result<(), String> {
    let normalized = normalize_configuration(&id, input)?;
    apply_configuration(&id, &normalized, None)
}

pub(crate) fn current_configuration(id: &str) -> Result<AgentConfigurationInput, String> {
    let agents = load_agents();
    let agent = agents
        .get(id)
        .ok_or_else(|| format!("agent 不存在: {id}"))?;
    let model_paths = crate::models::list_agents()
        .into_iter()
        .find(|agent| agent.id == id)
        .map(|agent| agent.config_paths)
        .unwrap_or_default();
    Ok(AgentConfigurationInput {
        mcp_path: agent
            .global
            .clone()
            .ok_or_else(|| "该 Agent 尚无可写的 MCP 配置".to_string())?,
        model_paths,
        skills_global_dir: agent
            .skills
            .as_ref()
            .map(|capability| capability.global_dir.clone()),
    })
}

pub(crate) fn normalize_configuration(
    id: &str,
    input: AgentConfigurationInput,
) -> Result<AgentConfigurationInput, String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("agent id 不能为空".into());
    }

    let current_agents = load_agents();
    let current = current_agents
        .get(&id)
        .ok_or_else(|| format!("agent 不存在: {id}"))?;
    if current.global.is_none() {
        return Err("该 Agent 尚无可写的 MCP 配置".into());
    }

    let mcp_path = input.mcp_path.trim();
    if mcp_path.is_empty() {
        return Err("MCP 配置路径不能为空".into());
    }
    let mcp_path = collapse_home(mcp_path);

    let model_defaults = crate::models::default_config_paths(&id);
    let model_paths = match model_defaults.as_ref() {
        Some(defaults) => {
            crate::models::normalize_config_paths(&input.model_paths, defaults.len())?
        }
        None if input.model_paths.iter().all(|path| path.trim().is_empty()) => Vec::new(),
        None => return Err("该 Agent 尚未接入 Model writer".into()),
    };

    let skills_default = builtin_agents()
        .get(&id)
        .and_then(|definition| definition.skills.as_ref())
        .map(|capability| capability.global_dir.clone());
    let skills_global_dir = match (skills_default.as_ref(), input.skills_global_dir) {
        (Some(_), Some(path)) => {
            let path = collapse_home(path.trim());
            validate_skill_directory(&path)
                .map_err(|reason| format!("Skills 配置路径无效: {reason}"))?;
            Some(path)
        }
        (Some(_), None) => return Err("Skills 配置路径不能为空".into()),
        (None, Some(path)) if !path.trim().is_empty() => {
            return Err("该 Agent 尚未接入 Skills writer".into())
        }
        (None, _) => None,
    };

    Ok(AgentConfigurationInput {
        mcp_path,
        model_paths,
        skills_global_dir,
    })
}

pub(crate) fn apply_configuration(
    id: &str,
    input: &AgentConfigurationInput,
    skill_assignments: Option<BTreeMap<String, BTreeSet<String>>>,
) -> Result<(), String> {
    let id = id.to_string();
    let mcp_path = input.mcp_path.clone();
    let model_paths = input.model_paths.clone();
    let skills_global_dir = input.skills_global_dir.clone();
    let model_defaults = crate::models::default_config_paths(&id);
    let skills_default = builtin_agents()
        .get(&id)
        .and_then(|definition| definition.skills.as_ref())
        .map(|capability| capability.global_dir.clone());

    mutate_settings_checked(|settings| {
        let mut agents = load_agents_from_settings(settings);
        let definition = agents
            .get_mut(&id)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("agent 不存在: {id}")))?;
        definition.global = Some(mcp_path.clone());

        // Skills runtime paths are overlaid from `agent_config_paths`; keep the
        // persisted Agent definition equal to its audited catalog metadata.
        let builtins = builtin_agents();
        for (agent_id, definition) in &mut agents {
            if let Some(builtin) = builtins.get(agent_id) {
                definition.skills = builtin.skills.clone();
            }
        }
        let overrides = agent_overrides(&agents);
        settings.agents = (!overrides.is_empty()).then_some(overrides);

        let path_overrides = settings.agent_config_paths.get_or_insert_default();
        let path_override = path_overrides.entry(id.clone()).or_default();
        path_override.model_paths = model_defaults
            .as_ref()
            .is_some_and(|defaults| defaults != &model_paths)
            .then_some(model_paths.clone());
        path_override.skills_global_dir = skills_default
            .as_ref()
            .zip(skills_global_dir.as_ref())
            .and_then(|(default, current)| (default != current).then_some(current.clone()));
        if path_override == &AgentConfigPathOverride::default() {
            path_overrides.remove(&id);
        }
        if path_overrides.is_empty() {
            settings.agent_config_paths = None;
        }
        if let Some(assignments) = &skill_assignments {
            settings.skill_assignments = (!assignments.is_empty()).then_some(assignments.clone());
        }
        Ok(())
    })
    .map_err(|error| error.to_string())
}

/// List all agent definitions as `AgentInfo` view rows.
pub fn list_infos() -> Vec<AgentInfo> {
    load_agents()
        .into_iter()
        .map(|(id, d)| {
            let skills_global_dir = d
                .skills
                .as_ref()
                .map(|capability| capability.global_dir.clone());
            AgentInfo {
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
                skills_global_dir,
            }
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
    definition.skills = existing.skills.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentInstallProbe, AgentSkillsCapability, AgentSkillsDirectory};

    fn skills_capability(target_id: &str, global_dir: &str) -> AgentSkillsCapability {
        AgentSkillsCapability {
            target_id: target_id.into(),
            global_dir: global_dir.into(),
            aliases: Vec::new(),
            docs: "https://example.com/skills".into(),
            evidence: "official".into(),
            verified_at: "2026-07-16".into(),
            probes: vec![AgentInstallProbe::Path {
                path: "/Applications/Example.app".into(),
            }],
        }
    }

    #[test]
    fn builtin_catalog_and_transport_metadata_load() {
        let a = builtin_agents();
        assert_eq!(audited_agents().len(), 42);
        let catalog: BTreeMap<String, AgentDefinition> =
            serde_json::from_str(CATALOG_AGENTS_JSON).unwrap();
        assert_eq!(catalog.len(), 175);
        assert_eq!(a.len(), 194);
        assert_eq!(a["claude-code"].key, "mcpServers");
        assert_eq!(a["codex"].format, "toml");
        assert!(!definition_supports_transport(&a["claude-desktop"], "http"));
        assert!(definition_supports_transport(&a["claude-desktop"], "stdio"));
        assert!(definition_supports_transport(&a["claude-code"], "http"));
    }

    #[test]
    fn amazon_q_ide_and_cli_remain_distinct_catalog_surfaces() {
        let agents = builtin_agents();
        let ide = &agents["amazon-q"];
        assert_eq!(ide.name.as_deref(), Some("Amazon Q Developer IDE"));
        assert_eq!(ide.category.as_deref(), Some("ide"));
        assert_eq!(ide.global.as_deref(), Some("~/.aws/amazonq/default.json"));
        assert_eq!(
            ide.docs.as_deref(),
            Some("https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/mcp-ide.html")
        );

        let cli = &agents["amazon-q-cli"];
        assert_eq!(cli.name.as_deref(), Some("Amazon Q CLI"));
        assert!(cli.global.is_none());
        assert!(cli.transports.as_ref().is_some_and(Vec::is_empty));
    }

    #[test]
    fn verified_skill_capabilities_are_data_driven() {
        let agents = builtin_agents();
        let capability_ids: Vec<&str> = agents
            .iter()
            .filter_map(|(id, definition)| definition.skills.as_ref().map(|_| id.as_str()))
            .collect();
        assert_eq!(
            capability_ids,
            [
                "claude-code",
                "codex",
                "copilot-cli",
                "cursor",
                "gemini",
                "opencode",
            ]
        );

        let codex = agents["codex"].skills.as_ref().unwrap();
        assert_eq!(codex.target_id, "agents-user");
        assert_eq!(codex.global_dir, "~/.agents/skills");
        assert!(codex.aliases.is_empty());

        let cursor = agents["cursor"].skills.as_ref().unwrap();
        assert_eq!(cursor.global_dir, "~/.cursor/skills");
        assert_eq!(cursor.aliases[0].target_id, "agents-user");
        assert_eq!(cursor.aliases[0].global_dir, "~/.agents/skills");

        for id in [
            "claude-code",
            "codex",
            "cursor",
            "gemini",
            "opencode",
            "copilot-cli",
        ] {
            let capability = agents[id].skills.as_ref().unwrap();
            assert!(!capability.docs.is_empty());
            assert_eq!(capability.evidence, "official");
            assert!(!capability.probes.is_empty());
        }
    }

    #[test]
    fn agent_info_projects_only_trusted_primary_skills_directories() {
        let _home = crate::testenv::TestHome::new("agent-info-skills-path");
        let infos = list_infos();
        let codex = infos.iter().find(|agent| agent.id == "codex").unwrap();
        let claude_desktop = infos
            .iter()
            .find(|agent| agent.id == "claude-desktop")
            .unwrap();

        assert_eq!(codex.skills_global_dir.as_deref(), Some("~/.agents/skills"));
        assert_eq!(claude_desktop.skills_global_dir, None);
    }

    #[test]
    fn unified_configuration_updates_all_supported_paths() {
        let _home = crate::testenv::TestHome::new("agent-unified-configuration");

        update_configuration(
            "codex".into(),
            AgentConfigurationInput {
                mcp_path: "~/.custom/codex-mcp.toml".into(),
                model_paths: vec!["~/.custom/codex-model.toml".into()],
                skills_global_dir: Some("~/.custom/codex/skills".into()),
            },
        )
        .unwrap();

        let agents = load_agents();
        assert_eq!(
            agents["codex"].global.as_deref(),
            Some("~/.custom/codex-mcp.toml")
        );
        assert_eq!(
            agents["codex"]
                .skills
                .as_ref()
                .map(|capability| capability.global_dir.as_str()),
            Some("~/.custom/codex/skills")
        );
        let model = crate::models::list_agents()
            .into_iter()
            .find(|agent| agent.id == "codex")
            .unwrap();
        assert_eq!(model.config_paths, ["~/.custom/codex-model.toml"]);
    }

    #[test]
    fn skill_capabilities_require_complete_official_evidence() {
        let mut unofficial = skills_capability("unofficial-user", "~/.unofficial/skills");
        unofficial.evidence = "catalog".into();
        let mut undocumented = skills_capability("undocumented-user", "~/.undocumented/skills");
        undocumented.docs = "  ".into();
        let mut unverified = skills_capability("unverified-user", "~/.unverified/skills");
        unverified.verified_at = "".into();
        let mut unprobed = skills_capability("unprobed-user", "~/.unprobed/skills");
        unprobed.probes.clear();

        for (id, capability) in [
            ("unofficial", unofficial),
            ("undocumented", undocumented),
            ("unverified", unverified),
            ("unprobed", unprobed),
        ] {
            let agents = BTreeMap::from([(
                id.into(),
                AgentDefinition {
                    skills: Some(capability),
                    ..Default::default()
                },
            )]);
            assert!(
                validate_skill_capabilities(&agents).is_err(),
                "accepted incomplete Skills evidence for {id}"
            );
        }
    }

    #[test]
    fn catalog_only_skill_capability_cannot_survive_merge() {
        let catalog = BTreeMap::from([(
            "catalog-forged".into(),
            AgentDefinition {
                builtin: Some(false),
                skills: Some(skills_capability(
                    "catalog-forged-user",
                    "~/.catalog-forged/skills",
                )),
                ..Default::default()
            },
        )]);

        let merged = merge_builtin_definitions(catalog, audited_agents()).unwrap();

        assert!(merged["catalog-forged"].skills.is_none());
    }

    #[test]
    fn audited_skill_capabilities_must_match_approved_agent_set() {
        let mut missing = audited_agents();
        missing.get_mut("codex").unwrap().skills = None;
        assert!(merge_builtin_definitions(BTreeMap::new(), missing).is_err());

        let mut extra = audited_agents();
        extra.insert(
            "extra-audited".into(),
            AgentDefinition {
                builtin: Some(true),
                skills: Some(skills_capability(
                    "extra-audited-user",
                    "~/.extra-audited/skills",
                )),
                ..Default::default()
            },
        );
        assert!(merge_builtin_definitions(BTreeMap::new(), extra).is_err());
    }

    #[test]
    fn skill_target_validation_rejects_unsafe_paths_and_contradictions() {
        for path in [
            "/tmp/skills",
            "~/.agents/not-skills",
            "~/.agents//skills",
            "~/.agents/./skills",
            "~/.agents/../skills",
            "~/.mux/skills",
        ] {
            let agents = BTreeMap::from([(
                "unsafe".into(),
                AgentDefinition {
                    skills: Some(skills_capability("unsafe-user", path)),
                    ..Default::default()
                },
            )]);
            assert!(
                validate_skill_capabilities(&agents).is_err(),
                "accepted unsafe Skills path: {path}"
            );
        }

        let mut unsafe_alias = skills_capability("safe-user", "~/.safe/skills");
        unsafe_alias.aliases.push(AgentSkillsDirectory {
            target_id: "unsafe-alias".into(),
            global_dir: "~/.mux/skills".into(),
        });
        let agents = BTreeMap::from([(
            "unsafe-alias".into(),
            AgentDefinition {
                skills: Some(unsafe_alias),
                ..Default::default()
            },
        )]);
        assert!(validate_skill_capabilities(&agents).is_err());

        let conflicting_target = BTreeMap::from([
            (
                "one".into(),
                AgentDefinition {
                    skills: Some(skills_capability("shared-user", "~/.one/skills")),
                    ..Default::default()
                },
            ),
            (
                "two".into(),
                AgentDefinition {
                    skills: Some(skills_capability("shared-user", "~/.two/skills")),
                    ..Default::default()
                },
            ),
        ]);
        assert!(validate_skill_capabilities(&conflicting_target).is_err());

        let conflicting_path = BTreeMap::from([
            (
                "one".into(),
                AgentDefinition {
                    skills: Some(skills_capability("one-user", "~/.shared/skills")),
                    ..Default::default()
                },
            ),
            (
                "two".into(),
                AgentDefinition {
                    skills: Some(skills_capability("two-user", "~/.shared/skills")),
                    ..Default::default()
                },
            ),
        ]);
        assert!(validate_skill_capabilities(&conflicting_path).is_err());
    }

    #[test]
    fn custom_agents_cannot_persist_or_load_skill_capabilities() {
        let forged = skills_capability("forged-user", "~/.forged/skills");
        let mut stored = builtin_agents();
        stored.insert(
            "forged".into(),
            AgentDefinition {
                global: Some("~/.forged/mcp.json".into()),
                format: "json".into(),
                key: "mcpServers".into(),
                enabled: true,
                builtin: Some(false),
                skills: Some(forged.clone()),
                ..Default::default()
            },
        );
        assert!(merge_builtin_updates(stored)["forged"].skills.is_none());

        let _home = crate::testenv::TestHome::new("agent-skills-lock");
        put(
            "forged".into(),
            AgentDefinition {
                global: Some("~/.forged/mcp.json".into()),
                format: "json".into(),
                key: "mcpServers".into(),
                enabled: true,
                skills: Some(forged),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert!(load_settings().agents.unwrap()["forged"].skills.is_none());
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
