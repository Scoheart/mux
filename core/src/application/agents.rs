//! Agent discovery, capability, and configuration use cases.

pub use crate::domain::agents::{
    AgentCapabilitySet, AgentCapabilityView, AgentConfigurationPatch, AgentIdentityView,
    McpAgentCapabilityView, ModelAgentCapabilityView, SkillAgentCapabilityView,
};
use crate::domain::error::{CoreError, CoreResult};
use std::collections::BTreeMap;

pub use crate::agents::AgentInfo;
pub use crate::domain::agents::{
    AgentConfigurationInput, McpConfigurationPatch, ModelConfigurationPatch,
    SkillConfigurationPatch,
};
pub use crate::domain::types::{
    AgentDefinition, AgentInstallProbe, AgentSkillsCapability, AgentSkillsDirectory,
};

pub fn load_agents() -> BTreeMap<String, AgentDefinition> {
    super::gate::read(crate::agents::load_agents)
}

pub fn list_infos() -> Vec<AgentInfo> {
    super::gate::read(crate::agents::list_infos)
}

pub fn supports_transport(agent_id: &str, transport: &str) -> bool {
    super::gate::read(|| crate::agents::supports_transport(agent_id, transport))
}

pub fn put(id: String, definition: AgentDefinition, allow_overwrite: bool) -> Result<(), String> {
    super::gate::write(|| crate::agents::put(id, definition, allow_overwrite))
}

pub fn set_enabled(agent_id: &str, enabled: bool) -> Result<(), String> {
    super::gate::write(|| crate::agents::set_enabled(agent_id, enabled))
}

/// Project every Agent through one typed capability graph. Frontends no longer
/// need to join MCP-shaped Agent rows with separate Model and Skill catalogs.
pub fn list_capabilities() -> CoreResult<Vec<AgentCapabilityView>> {
    super::gate::read(|| {
        let skill_capabilities = crate::resources::skill::list_skill_agent_capabilities()
            .map_err(super::error::from_skill)?;
        Ok(list_capabilities_with_skills(&skill_capabilities))
    })
}

pub(crate) fn list_capabilities_with_skills(
    skill_capabilities: &[crate::resources::skill::SkillAgentCapabilityView],
) -> Vec<AgentCapabilityView> {
    let infos = crate::agents::list_infos();
    let mut aliases = BTreeMap::new();
    let mut views = BTreeMap::new();

    for info in infos {
        let alias_dirs = info
            .skills_global_dir
            .as_ref()
            .map(|primary| {
                info.skills_global_dirs
                    .iter()
                    .filter(|path| *path != primary)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        aliases.insert(info.id.clone(), alias_dirs);

        let mcp = info.global.clone().map(|config_path| {
            let observed_config =
                crate::resources::mcp::scanner::expand_tilde(&config_path).is_file();
            (
                observed_config,
                McpAgentCapabilityView {
                    writable: true,
                    config_path: Some(config_path),
                    format: info.format.clone(),
                    key: info.key.clone(),
                    supported_transports: info
                        .supported_transports
                        .iter()
                        .map(|transport| (*transport).to_string())
                        .collect(),
                },
            )
        });
        let installed = mcp.as_ref().is_some_and(|(observed, _)| *observed);
        views.insert(
            info.id.clone(),
            AgentCapabilityView {
                identity: AgentIdentityView {
                    id: info.id,
                    name: info.name,
                    enabled: info.enabled,
                    builtin: info.builtin,
                    category: info.category,
                    evidence: info.evidence,
                    docs: info.docs,
                    note: info.note,
                    verified_at: info.verified_at,
                },
                installed,
                capabilities: AgentCapabilitySet {
                    mcp: mcp.map(|(_, capability)| capability),
                    ..AgentCapabilitySet::default()
                },
            },
        );
    }

    for model in crate::resources::model::list_agents() {
        let entry = views
            .entry(model.id.clone())
            .or_insert_with(|| AgentCapabilityView {
                identity: AgentIdentityView {
                    id: model.id.clone(),
                    name: model.name.clone(),
                    enabled: true,
                    builtin: true,
                    category: "model".into(),
                    evidence: "official".into(),
                    docs: Some(model.docs.clone()),
                    note: Some(model.note.clone()),
                    verified_at: None,
                },
                installed: false,
                capabilities: AgentCapabilitySet::default(),
            });
        entry.installed |= model.installed;
        entry.capabilities.model = Some(ModelAgentCapabilityView {
            mode: model.mode,
            installed: model.installed,
            config_paths: model.config_paths,
            assigned_profiles: model.assigned_profiles,
            active_profile: model.active_profile,
            supports_multiple: model.supports_multiple,
            credential_mode: model.credential_mode,
            supported_protocols: model.supported_protocols,
        });
    }

    for skill in skill_capabilities {
        let entry = views
            .entry(skill.id.clone())
            .or_insert_with(|| AgentCapabilityView {
                identity: AgentIdentityView {
                    id: skill.id.clone(),
                    name: skill.id.clone(),
                    enabled: true,
                    builtin: true,
                    category: "skill".into(),
                    evidence: "official".into(),
                    docs: None,
                    note: None,
                    verified_at: None,
                },
                installed: false,
                capabilities: AgentCapabilitySet::default(),
            });
        entry.installed |= skill.installed;
        entry.capabilities.skill = Some(SkillAgentCapabilityView {
            installed: skill.installed,
            target_id: skill.target_id.clone(),
            global_dir: skill.global_dir.clone(),
            alias_dirs: aliases.remove(&skill.id).unwrap_or_default(),
            affected_agent_ids: skill.affected_agent_ids.clone(),
        });
    }

    views.into_values().collect()
}

pub fn get_configuration_patch(agent_id: &str) -> CoreResult<AgentConfigurationPatch> {
    super::gate::read(|| {
        crate::agents::current_configuration_patch(agent_id).map_err(CoreError::from)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{AgentInstallProbe, AgentSkillsCapability};
    use std::collections::BTreeSet;

    #[test]
    fn capability_projection_has_no_duplicate_agent_ids() {
        let views = list_capabilities().unwrap();
        let ids = views
            .iter()
            .map(|view| view.identity.id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), views.len());
        assert!(views.iter().any(|view| view.capabilities.model.is_some()));
        assert!(views.iter().any(|view| view.capabilities.skill.is_some()));
    }

    #[test]
    fn custom_skill_only_agent_projects_without_an_mcp_capability() {
        let _home = crate::testenv::TestHome::new("app-custom-skill");
        put(
            "custom-skill".into(),
            AgentDefinition {
                enabled: true,
                skills: Some(AgentSkillsCapability {
                    target_id: "custom-skill-user".into(),
                    global_dir: "~/.custom-skill/skills".into(),
                    aliases: Vec::new(),
                    docs: "https://example.com/custom-skill".into(),
                    evidence: "official-source".into(),
                    verified_at: "2026-07-23".into(),
                    probes: vec![AgentInstallProbe::Path {
                        path: "/Applications/Custom Skill Missing.app".into(),
                    }],
                }),
                ..AgentDefinition::default()
            },
            false,
        )
        .unwrap();

        let views = list_capabilities().unwrap();
        let custom = views
            .iter()
            .find(|view| view.identity.id == "custom-skill")
            .unwrap();
        assert!(!custom.identity.builtin);
        assert_eq!(custom.identity.category, "custom");
        assert!(custom.capabilities.mcp.is_none());
        assert!(custom.capabilities.model.is_none());
        let skill = custom.capabilities.skill.as_ref().unwrap();
        assert_eq!(skill.target_id, "custom-skill-user");
        assert_eq!(skill.global_dir, "~/.custom-skill/skills");
    }
}
