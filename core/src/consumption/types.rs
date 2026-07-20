use crate::agents::AgentConfigurationInput;
use crate::r#override::OverridePatch;
use crate::types::{ModelProfile, RegistryEntry};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Stable reference to one central asset. The tagged representation is shared
/// by Rust, Tauri, and the desktop; validation happens while decoding so an
/// invalid identity never reaches a domain adapter.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "domain", rename_all = "kebab-case")]
pub enum AssetRef {
    Mcp { key: String },
    Model { profile_id: String },
    Skill { name: String },
}

#[derive(Deserialize)]
#[serde(tag = "domain", rename_all = "kebab-case", deny_unknown_fields)]
enum UncheckedAssetRef {
    Mcp { key: String },
    Model { profile_id: String },
    Skill { name: String },
}

impl AssetRef {
    pub fn validate(&self) -> Result<(), SelectionError> {
        match self {
            Self::Mcp { key } => validate_mcp_asset_key(key),
            Self::Model { profile_id } => validate_nonempty("profile_id", profile_id),
            Self::Skill { name } => validate_nonempty("name", name),
        }
    }
}

impl<'de> Deserialize<'de> for AssetRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = UncheckedAssetRef::deserialize(deserializer)?;
        let asset = match unchecked {
            UncheckedAssetRef::Mcp { key } => Self::Mcp { key },
            UncheckedAssetRef::Model { profile_id } => Self::Model { profile_id },
            UncheckedAssetRef::Skill { name } => Self::Skill { name },
        };
        asset.validate().map_err(serde::de::Error::custom)?;
        Ok(asset)
    }
}

fn validate_nonempty(field: &'static str, value: &str) -> Result<(), SelectionError> {
    if value.trim().is_empty() {
        return Err(SelectionError::InvalidIdentity {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

pub fn validate_mcp_asset_key(key: &str) -> Result<(), SelectionError> {
    let Some((name, transport)) = key.rsplit_once("::") else {
        return Err(SelectionError::InvalidMcpAssetKey(key.to_string()));
    };
    if name.trim().is_empty() || !matches!(transport, "stdio" | "http") {
        return Err(SelectionError::InvalidMcpAssetKey(key.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConsumptionStatus {
    Synced,
    Pending,
    Drifted,
    Conflicted,
    Unsupported,
    External,
}

/// Physical destination behind a Skill relationship. Several Agents may read
/// the same target, so the target identity is carried separately from the
/// projected per-Agent rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumptionTarget {
    pub target_id: String,
    pub global_dir: String,
}

/// Read projection. `desired` and `observed` remain explicit so missing and
/// external states cannot disappear from the inventory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumptionView {
    pub agent_id: String,
    pub asset: AssetRef,
    pub desired: bool,
    pub observed: bool,
    /// Domain-specific enabled state. Present for MCP consumptions and external
    /// MCP observations; Model and Skill relationships do not have an off state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Whether this Model Profile is the Agent's current primary model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    pub status: ConsumptionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub affected_agent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ConsumptionTarget>,
}

/// Desired relationships and read-only external observations are separated so
/// callers cannot accidentally treat discovery as ownership.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsumptionInventory {
    #[serde(default)]
    pub consumptions: Vec<ConsumptionView>,
    #[serde(default)]
    pub external: Vec<ConsumptionView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpConsumptionRecord {
    pub asset_key: String,
    pub enabled: bool,
    #[serde(default)]
    pub overrides: OverridePatch,
}

/// One MUX-owned Model Profile installed for an Agent. Installation and the
/// Agent's active/default model are intentionally separate: an Agent may keep
/// several enabled profiles while only one is current.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelConsumptionRecord {
    pub profile_id: String,
    pub enabled: bool,
    /// RFC3339 timestamp updated only when this Profile becomes current. It is
    /// used to choose a deterministic fallback when the current Profile is
    /// disabled or removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_selected_at: Option<String>,
}

/// Complete desired Model state for one Agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModelAgentSelection {
    #[serde(default)]
    pub profiles: BTreeMap<String, ModelConsumptionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
}

impl ModelAgentSelection {
    /// Keep the current pointer valid, falling back to the most recently used
    /// enabled Profile and then to a stable profile id.
    pub fn normalize_active(&mut self) {
        let active_available = self.active_profile_id.as_ref().is_some_and(|active| {
            self.profiles
                .get(active)
                .is_some_and(|record| record.enabled)
        });
        if active_available {
            return;
        }
        self.active_profile_id = self
            .profiles
            .values()
            .filter(|record| record.enabled)
            .max_by(|left, right| {
                left.last_selected_at
                    .cmp(&right.last_selected_at)
                    .then_with(|| right.profile_id.cmp(&left.profile_id))
            })
            .map(|record| record.profile_id.clone());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AssetOperationKind {
    SetConsumption,
    UpdateAsset,
    DeleteAsset,
    Adopt,
    UpdateConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CentralAssetAction {
    Create,
    Update,
    Delete,
}

/// Secret-free projection of the central half of an asset operation. Domain
/// payloads stay with their adapter and are never copied into the review plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CentralAssetChange {
    pub asset: AssetRef,
    pub action: CentralAssetAction,
    #[serde(default)]
    pub summary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipChange {
    pub agent_id: String,
    pub asset: AssetRef,
    pub action: RelationshipAction,
}

/// Domain-specific desired sets remain typed. The common coordinator owns
/// lifecycle and review, not MCP/Model/Skill payloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DomainPlan {
    Mcp {
        before: BTreeMap<String, Vec<String>>,
        after: BTreeMap<String, Vec<String>>,
    },
    Model {
        before: BTreeMap<String, ModelAgentSelection>,
        after: BTreeMap<String, ModelAgentSelection>,
    },
    Skill {
        before: BTreeMap<String, Vec<String>>,
        after: BTreeMap<String, Vec<String>>,
    },
    AgentConfiguration {
        agent_id: String,
        before: AgentConfigurationInput,
        after: AgentConfigurationInput,
        skills_before: BTreeMap<String, Vec<String>>,
        skills_after: BTreeMap<String, Vec<String>>,
        #[serde(default)]
        affected_agent_ids: Vec<String>,
        #[serde(default)]
        migrated_skill_names: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetOperationPlan {
    pub operation_id: String,
    pub kind: AssetOperationKind,
    pub domain_plan: DomainPlan,
    #[serde(default)]
    pub central_changes: Vec<CentralAssetChange>,
    #[serde(default)]
    pub relationship_changes: Vec<RelationshipChange>,
    #[serde(default)]
    pub target_files: Vec<String>,
    #[serde(default)]
    pub affected_agent_ids: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub can_commit: bool,
    #[serde(default)]
    pub requires_conflict_confirmation: bool,
    pub candidate_hash: String,
}

/// Drafts are accepted only by the central asset workspaces. MCP configuration
/// may contain headers or environment values, and Model credentials are secret,
/// so the planner binds them by hash and keeps the values out of persisted plans.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "domain", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CentralAssetDraft {
    Mcp {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        existing_key: Option<String>,
        entry: Box<RegistryEntry>,
    },
    Model {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        existing_id: Option<String>,
        profile: ModelProfile,
        /// `None` keeps an existing credential, `Some("")` clears it, and a
        /// non-empty value replaces it. The value is never persisted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        credential: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanUpdateCentralAssetRequest {
    pub draft: CentralAssetDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanDeleteCentralAssetRequest {
    pub asset: AssetRef,
    /// MCP Registry can contain several source copies of one stable asset key.
    /// Deletion is bound to the reviewed user-owned source copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanSetAgentConsumptionRequest {
    pub agent_id: String,
    pub selection: AgentConsumptionSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanSetMcpEnabledRequest {
    pub agent_id: String,
    pub asset_key: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanSetModelEnabledRequest {
    pub agent_id: String,
    pub profile_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanSetActiveModelRequest {
    pub agent_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanUpdateAgentConfigurationRequest {
    pub agent_id: String,
    pub configuration: AgentConfigurationInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanSetAssetConsumersRequest {
    pub asset: AssetRef,
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssetCommitRequest {
    pub operation_id: String,
    pub candidate_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_confirmation: Option<String>,
}

impl McpConsumptionRecord {
    pub fn validate(&self) -> Result<(), SelectionError> {
        validate_mcp_asset_key(&self.asset_key)
    }
}

impl ModelConsumptionRecord {
    pub fn validate(&self) -> Result<(), SelectionError> {
        validate_nonempty("profile_id", &self.profile_id)
    }
}

/// Complete desired selection for one Agent and one domain. Empty selections
/// mean unassign. Model accepts several installed Profiles; the current Profile
/// is changed through the explicit active-model operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AgentConsumptionSelection {
    Mcp { asset_keys: Vec<String> },
    Model { profile_ids: Vec<String> },
    Skill { names: Vec<String> },
}

impl AgentConsumptionSelection {
    pub fn normalize(self) -> Result<Self, SelectionError> {
        match self {
            Self::Mcp { asset_keys } => {
                for key in &asset_keys {
                    validate_mcp_asset_key(key)?;
                }
                Ok(Self::Mcp {
                    asset_keys: dedup_sorted(asset_keys),
                })
            }
            Self::Model { profile_ids } => {
                for profile_id in &profile_ids {
                    validate_nonempty("profile_id", profile_id)?;
                }
                Ok(Self::Model {
                    profile_ids: dedup_sorted(profile_ids),
                })
            }
            Self::Skill { names } => {
                for name in &names {
                    validate_nonempty("name", name)?;
                }
                Ok(Self::Skill {
                    names: dedup_sorted(names),
                })
            }
        }
    }
}

fn dedup_sorted(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    InvalidMcpAssetKey(String),
    InvalidIdentity { field: &'static str, value: String },
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMcpAssetKey(key) => write!(
                f,
                "invalid MCP asset key {key:?}; expected name::stdio or name::http"
            ),
            Self::InvalidIdentity { field, .. } => write!(f, "{field} must not be empty"),
        }
    }
}

impl std::error::Error for SelectionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn asset_refs_use_stable_tagged_json() {
        let cases = [
            (
                AssetRef::Mcp {
                    key: "github::stdio".into(),
                },
                json!({"domain":"mcp","key":"github::stdio"}),
            ),
            (
                AssetRef::Model {
                    profile_id: "work".into(),
                },
                json!({"domain":"model","profile_id":"work"}),
            ),
            (
                AssetRef::Skill {
                    name: "review-changes".into(),
                },
                json!({"domain":"skill","name":"review-changes"}),
            ),
        ];

        for (asset, expected) in cases {
            assert_eq!(serde_json::to_value(&asset).unwrap(), expected);
            assert_eq!(serde_json::from_value::<AssetRef>(expected).unwrap(), asset);
        }
    }

    #[test]
    fn asset_ref_decode_rejects_invalid_identities() {
        for invalid in [
            json!({"domain":"mcp","key":"github"}),
            json!({"domain":"mcp","key":"github::websocket"}),
            json!({"domain":"mcp","key":"::stdio"}),
            json!({"domain":"model","profile_id":"  "}),
            json!({"domain":"skill","name":""}),
        ] {
            assert!(serde_json::from_value::<AssetRef>(invalid).is_err());
        }
    }

    #[test]
    fn many_selections_are_deduplicated_and_sorted() {
        assert_eq!(
            AgentConsumptionSelection::Mcp {
                asset_keys: vec!["z::http".into(), "a::stdio".into(), "z::http".into()]
            }
            .normalize()
            .unwrap(),
            AgentConsumptionSelection::Mcp {
                asset_keys: vec!["a::stdio".into(), "z::http".into()]
            }
        );
        assert_eq!(
            AgentConsumptionSelection::Skill {
                names: vec!["z".into(), "a".into(), "z".into()]
            }
            .normalize()
            .unwrap(),
            AgentConsumptionSelection::Skill {
                names: vec!["a".into(), "z".into()]
            }
        );
    }

    #[test]
    fn model_selection_accepts_multiple_profiles_and_deduplicates() {
        assert_eq!(
            AgentConsumptionSelection::Model {
                profile_ids: vec!["work".into(), "personal".into(), "work".into()],
            }
            .normalize()
            .unwrap(),
            AgentConsumptionSelection::Model {
                profile_ids: vec!["personal".into(), "work".into()],
            }
        );
    }
}
