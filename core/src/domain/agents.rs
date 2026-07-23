//! Agent identity, capability, and configuration contracts.

use crate::domain::types::ModelProtocol;
use serde::{Deserialize, Serialize};

/// Legacy all-capabilities input retained for the existing desktop wire
/// contract. New integrations should use [`AgentConfigurationPatch`], whose
/// domains are independently optional.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfigurationInput {
    pub mcp_path: String,
    /// `None` keeps the effective key for backward-compatible callers.
    #[serde(default)]
    pub mcp_key: Option<String>,
    #[serde(default)]
    pub model_paths: Vec<String>,
    pub skills_global_dir: Option<String>,
    #[serde(default)]
    pub skills_alias_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpConfigurationPatch {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelConfigurationPatch {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillConfigurationPatch {
    pub global_dir: String,
    #[serde(default)]
    pub alias_dirs: Vec<String>,
}

/// A partial Agent configuration update. An Agent may expose any combination
/// of MCP, Model, and Skill writers; configuring one never requires another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AgentConfigurationPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfigurationPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelConfigurationPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillConfigurationPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentIdentityView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub builtin: bool,
    pub category: String,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpAgentCapabilityView {
    pub writable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub format: String,
    pub key: String,
    #[serde(default)]
    pub supported_transports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAgentCapabilityView {
    pub mode: String,
    pub installed: bool,
    #[serde(default)]
    pub config_paths: Vec<String>,
    #[serde(default)]
    pub assigned_profiles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    pub supports_multiple: bool,
    pub credential_mode: String,
    #[serde(default)]
    pub supported_protocols: Vec<ModelProtocol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillAgentCapabilityView {
    pub installed: bool,
    pub target_id: String,
    pub global_dir: String,
    #[serde(default)]
    pub alias_dirs: Vec<String>,
    #[serde(default)]
    pub affected_agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AgentCapabilitySet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpAgentCapabilityView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelAgentCapabilityView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillAgentCapabilityView>,
}

/// Unified Agent projection used by every frontend. Domain-specific details
/// stay typed instead of occupying MCP-shaped fields at the Agent root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapabilityView {
    pub identity: AgentIdentityView,
    pub installed: bool,
    pub capabilities: AgentCapabilitySet,
}
