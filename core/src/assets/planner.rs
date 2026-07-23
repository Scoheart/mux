//! Cross-domain asset planning.

use super::compatibility::compatibility_for;
use super::inventory::list_consumption_inventory;
use super::types::{
    AgentConsumptionSelection, AssetOperationKind, AssetOperationPlan, AssetRef,
    CentralAssetChange, ConsumptionStatus, DomainPlan, ModelConsumptionRecord, ModelStateChange,
    ModelStateSnapshot, PlanEnsureAgentConsumptionRequest, PlanReapplyMcpRequest,
    PlanSetActiveModelRequest, PlanSetAgentConsumptionRequest, PlanSetAssetConsumersRequest,
    PlanSetMcpEnabledRequest, PlanSetModelEnabledRequest, PlanUpdateAgentCapabilitiesRequest,
    PlanUpdateAgentConfigurationRequest, PlanUpdateAssetConsumersRequest, RelationshipAction,
    RelationshipChange,
};
use crate::agents::{
    builtin_agents, current_configuration_patch, load_agents, normalize_configuration,
    normalize_configuration_patch, AgentConfigurationInput, AgentConfigurationPatch,
    McpConfigurationPatch, ModelConfigurationPatch, SkillConfigurationPatch,
};
use crate::paths::{mux_dir, settings_file};
use crate::resources::mcp::scanner::{collapse_home, expand_tilde};
use crate::resources::skill::{
    canonical_skill_assignments, canonical_skill_target_path, hash_tree,
    list_inventory as list_skills_inventory, list_inventory_for_settings,
    normalize_agent_selection, skill_agent_capability_for_settings, InventoryState, SkillLocation,
    SkillsInventory,
};
use crate::settings::{load_settings_strict, AgentConfigPathOverride, Settings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const OPERATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PersistedAssetOperation {
    pub schema_version: u32,
    pub plan: AssetOperationPlan,
    pub settings_hash: String,
    pub target_hashes: BTreeMap<String, String>,
    /// Canonical effective/source catalog projection used by MCP plans. This
    /// binds consumer writes to the exact central configuration that was
    /// reviewed, even when a source cache changes outside settings.json.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_catalog_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "kebab-case")]
pub(crate) enum LifecycleBinding {
    McpUpsert {
        key: String,
        draft_hash: String,
    },
    McpAdopt {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft_hash: Option<String>,
        enabled: BTreeMap<String, bool>,
    },
    McpDelete {
        key: String,
        source_id: String,
        fallback_exists: bool,
        effective_before: bool,
    },
    McpEnabled {
        agent_id: String,
        asset_key: String,
        before: bool,
        after: bool,
    },
    McpReapply {
        key: String,
    },
    ModelUpsert {
        profile_id: String,
        draft_hash: String,
        credential_action: CredentialAction,
    },
    ModelAdopt {
        profile_id: String,
        draft_hash: String,
        credential_action: CredentialAction,
    },
    ModelDelete {
        profile_id: String,
    },
    ModelSchemaV2 {
        id_map: BTreeMap<String, String>,
        draft_hash: String,
        #[serde(default)]
        credential_profile_ids: BTreeSet<String>,
    },
    AgentConfiguration {
        agent_id: String,
        after: AgentConfigurationInput,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill_assignments_after: Option<BTreeMap<String, BTreeSet<String>>>,
        #[serde(default)]
        skill_migration: Vec<SkillMigrationEntry>,
    },
    AgentCapabilities {
        agent_id: String,
        after: AgentConfigurationPatch,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill_assignments_after: Option<BTreeMap<String, BTreeSet<String>>>,
        #[serde(default)]
        skill_migration: Vec<SkillMigrationEntry>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SkillMigrationEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub destination: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CredentialAction {
    Keep,
    Set,
    Clear,
}

pub fn plan_set_agent_consumption(
    request: PlanSetAgentConsumptionRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    let selection = request
        .selection
        .normalize()
        .map_err(|error| error.to_string())?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let domain_plan = match selection {
        AgentConsumptionSelection::Mcp { asset_keys } => {
            validate_unique_mcp_names(&asset_keys)?;
            for key in &asset_keys {
                require_compatible(&request.agent_id, &AssetRef::Mcp { key: key.clone() })?;
            }
            let before = current_mcp_selection(&settings, &request.agent_id);
            DomainPlan::Mcp {
                before: BTreeMap::from([(request.agent_id.clone(), before)]),
                after: BTreeMap::from([(request.agent_id, asset_keys)]),
            }
        }
        AgentConsumptionSelection::Model { profile_ids } => {
            for profile_id in &profile_ids {
                require_compatible(
                    &request.agent_id,
                    &AssetRef::Model {
                        profile_id: profile_id.clone(),
                    },
                )?;
            }
            let before = settings.model_selection(&request.agent_id);
            let desired: BTreeSet<_> = profile_ids.into_iter().collect();
            let mut after = before.clone();
            after
                .profiles
                .retain(|profile_id, _| desired.contains(profile_id));
            for profile_id in desired {
                after
                    .profiles
                    .entry(profile_id.clone())
                    .or_insert(ModelConsumptionRecord {
                        profile_id,
                        enabled: true,
                        last_selected_at: None,
                    });
            }
            after.normalize_active();
            validate_model_selection_contract(&settings, &request.agent_id, &after)?;
            DomainPlan::Model {
                before: BTreeMap::from([(request.agent_id.clone(), before)]),
                after: BTreeMap::from([(request.agent_id, after)]),
            }
        }
        AgentConsumptionSelection::Skill { names } => {
            for name in &names {
                require_compatible(&request.agent_id, &AssetRef::Skill { name: name.clone() })?;
            }
            let before = current_skill_selection(&request.agent_id)?;
            skill_plan_for_agent(&request.agent_id, before, names)?
        }
    };
    finalize_plan(domain_plan)
}

pub fn plan_ensure_agent_consumption(
    request: PlanEnsureAgentConsumptionRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    let requested = request
        .selection
        .normalize()
        .map_err(|error| error.to_string())?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let selection = match requested {
        AgentConsumptionSelection::Mcp { asset_keys } => {
            let mut desired: BTreeSet<String> = current_mcp_selection(&settings, &request.agent_id)
                .into_iter()
                .collect();
            desired.extend(asset_keys);
            AgentConsumptionSelection::Mcp {
                asset_keys: desired.into_iter().collect(),
            }
        }
        AgentConsumptionSelection::Model { profile_ids } => {
            let mut desired: BTreeSet<String> = settings
                .model_selection(&request.agent_id)
                .profiles
                .into_keys()
                .collect();
            desired.extend(profile_ids);
            AgentConsumptionSelection::Model {
                profile_ids: desired.into_iter().collect(),
            }
        }
        AgentConsumptionSelection::Skill { names } => {
            let mut desired: BTreeSet<String> = current_skill_selection(&request.agent_id)?
                .into_iter()
                .collect();
            desired.extend(names);
            AgentConsumptionSelection::Skill {
                names: desired.into_iter().collect(),
            }
        }
    };
    plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: request.agent_id,
        selection,
    })
}

pub fn plan_set_model_enabled(
    request: PlanSetModelEnabledRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let before = settings.model_selection(&request.agent_id);
    let mut after = before.clone();
    let record = after
        .profiles
        .get_mut(&request.profile_id)
        .ok_or_else(|| "model_consumption_missing: Model is not added to this Agent".to_string())?;
    if record.enabled == request.enabled {
        return Err("model_enabled_unchanged: Model already has the requested state".into());
    }
    record.enabled = request.enabled;
    after.normalize_active();
    validate_model_selection_contract(&settings, &request.agent_id, &after)?;
    finalize_plan(DomainPlan::Model {
        before: BTreeMap::from([(request.agent_id.clone(), before)]),
        after: BTreeMap::from([(request.agent_id, after)]),
    })
}

pub fn plan_set_active_model(
    request: PlanSetActiveModelRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let before = settings.model_selection(&request.agent_id);
    let mut after = before.clone();
    validate_model_selection_contract(&settings, &request.agent_id, &after)?;
    let record = after
        .profiles
        .get(&request.profile_id)
        .ok_or_else(|| "model_consumption_missing: Model is not added to this Agent".to_string())?;
    if !record.enabled {
        return Err("model_consumption_disabled: enable the Model before selecting it".into());
    }
    require_compatible(
        &request.agent_id,
        &AssetRef::Model {
            profile_id: request.profile_id.clone(),
        },
    )?;
    if before.active_profile_id.as_deref() == Some(request.profile_id.as_str()) {
        return Err("active_model_unchanged: Model is already current".into());
    }
    after.active_profile_id = Some(request.profile_id.clone());
    if let Some(record) = after.profiles.get_mut(&request.profile_id) {
        record.last_selected_at =
            Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    }
    finalize_plan(DomainPlan::Model {
        before: BTreeMap::from([(request.agent_id.clone(), before)]),
        after: BTreeMap::from([(request.agent_id, after)]),
    })
}

/// Plan an MCP on/off transition without removing its desired relationship.
/// The unchanged DomainPlan keeps the asset assigned to the Agent; the bound
/// lifecycle mutation snapshots/restores the actual config during commit.
pub fn plan_set_mcp_enabled(
    request: PlanSetMcpEnabledRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    super::types::validate_mcp_asset_key(&request.asset_key).map_err(|error| error.to_string())?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let records = settings
        .mcp_consumptions
        .as_ref()
        .and_then(|consumptions| consumptions.get(&request.agent_id))
        .ok_or_else(|| "mcp_consumption_missing: MCP is not assigned to this Agent".to_string())?;
    let record = records
        .get(&request.asset_key)
        .ok_or_else(|| "mcp_consumption_missing: MCP is not assigned to this Agent".to_string())?;
    if record.enabled == request.enabled {
        return Err("mcp_enabled_unchanged: MCP already has the requested state".into());
    }
    let selection: Vec<String> = records.keys().cloned().collect();
    let domain_plan = DomainPlan::Mcp {
        before: BTreeMap::from([(request.agent_id.clone(), selection.clone())]),
        after: BTreeMap::from([(request.agent_id.clone(), selection)]),
    };
    finalize_plan_with(
        AssetOperationKind::SetConsumption,
        domain_plan,
        Vec::new(),
        Vec::new(),
        Some(LifecycleBinding::McpEnabled {
            agent_id: request.agent_id,
            asset_key: request.asset_key,
            before: record.enabled,
            after: request.enabled,
        }),
    )
}

pub fn plan_reapply_mcp(request: PlanReapplyMcpRequest) -> Result<AssetOperationPlan, String> {
    request
        .asset_key
        .rsplit_once("::")
        .filter(|(name, transport)| {
            !name.trim().is_empty() && matches!(*transport, "stdio" | "http")
        })
        .ok_or_else(|| "invalid MCP asset key".to_string())?;
    if !crate::resources::mcp::registry::read_registry()
        .iter()
        .any(|entry| entry.key() == request.asset_key)
    {
        return Err("asset_operation_stale: central MCP asset no longer exists".into());
    }
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let domain_plan = {
        let consumers = current_mcp_consumers(&settings, &request.asset_key);
        DomainPlan::Mcp {
            before: consumers.clone(),
            after: consumers,
        }
    };
    finalize_plan_with(
        AssetOperationKind::UpdateAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Mcp {
                key: request.asset_key.clone(),
            },
            action: crate::domain::assets::CentralAssetAction::Update,
            summary: vec!["重新传播中央 MCP；中央资产内容保持不变".into()],
        }],
        Vec::new(),
        Some(LifecycleBinding::McpReapply {
            key: request.asset_key,
        }),
    )
}

fn current_mcp_consumers(settings: &Settings, asset_key: &str) -> BTreeMap<String, Vec<String>> {
    settings
        .mcp_consumptions
        .iter()
        .flatten()
        .filter(|(_, records)| records.contains_key(asset_key))
        .map(|(agent_id, _)| (agent_id.clone(), current_mcp_selection(settings, agent_id)))
        .collect()
}

pub fn plan_update_agent_configuration(
    request: PlanUpdateAgentConfigurationRequest,
) -> Result<AssetOperationPlan, String> {
    let configuration = normalize_configuration(&request.agent_id, request.configuration)?;
    let patch = AgentConfigurationPatch {
        mcp: Some(McpConfigurationPatch {
            path: configuration.mcp_path,
            key: configuration.mcp_key,
        }),
        model: crate::resources::model::default_config_paths(&request.agent_id).map(|_| {
            ModelConfigurationPatch {
                paths: configuration.model_paths,
            }
        }),
        skill: configuration
            .skills_global_dir
            .map(|global_dir| SkillConfigurationPatch {
                global_dir,
                alias_dirs: configuration.skills_alias_dirs,
            }),
    };
    plan_update_agent_capabilities(PlanUpdateAgentCapabilitiesRequest {
        agent_id: request.agent_id,
        patch,
    })
}

pub fn plan_update_agent_capabilities(
    request: PlanUpdateAgentCapabilitiesRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    let before = current_configuration_patch(&request.agent_id)?;
    let normalized = normalize_configuration_patch(&request.agent_id, request.patch)?;
    let mut after = before.clone();
    if normalized.mcp.is_some() {
        after.mcp = normalized.mcp;
    }
    if normalized.model.is_some() {
        after.model = normalized.model;
    }
    if normalized.skill.is_some() {
        after.skill = normalized.skill;
    }
    if before == after {
        return Err("agent_configuration_unchanged: 配置没有变化".into());
    }

    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    if before.mcp != after.mcp
        && settings
            .mcp_consumptions
            .as_ref()
            .and_then(|consumptions| consumptions.get(&request.agent_id))
            .is_some_and(|records| !records.is_empty())
    {
        return Err(
            "agent_configuration_in_use: MCP capability 已有 desired resources；当前版本不支持安全迁移 MCP writer 路径或 key"
                .into(),
        );
    }
    if before.model != after.model
        && !settings
            .model_selection(&request.agent_id)
            .profiles
            .is_empty()
    {
        return Err(
            "agent_configuration_in_use: Model capability 已有 desired resources；当前版本不支持安全迁移 Model writer 路径"
                .into(),
        );
    }
    let mut skills_before = BTreeMap::new();
    let mut skills_after = BTreeMap::new();
    let mut skill_assignments_after = None;
    let mut skill_migration = Vec::new();
    let mut target_files = Vec::new();
    let mut configuration_affected_agents = BTreeSet::from([request.agent_id.clone()]);

    if before.skill != after.skill {
        let before_skill = before
            .skill
            .as_ref()
            .ok_or_else(|| "skill_target_unavailable: 当前 Agent 没有 Skills 目标".to_string())?;
        let after_skill = after
            .skill
            .as_ref()
            .ok_or_else(|| "skill_target_unavailable: 新 Skills 目标不可用".to_string())?;
        let old_capability = skill_agent_capability_for_settings(&settings, &request.agent_id)
            .map_err(|error| format!("{error:?}"))?
            .ok_or_else(|| "skill_target_unavailable: 当前 Agent 没有 Skills 目标".to_string())?;
        let old_inventory =
            list_inventory_for_settings(&settings).map_err(|error| format!("{error:?}"))?;
        let mut prospective = settings.clone();
        set_prospective_skill_paths(&mut prospective, &request.agent_id, after_skill)?;
        prospective.skill_assignments = None;
        let new_capability = skill_agent_capability_for_settings(&prospective, &request.agent_id)
            .map_err(|error| format!("{error:?}"))?
            .ok_or_else(|| "skill_target_unavailable: 新 Skills 目标不可用".to_string())?;
        configuration_affected_agents.extend(old_capability.affected_agent_ids.iter().cloned());
        configuration_affected_agents.extend(new_capability.affected_agent_ids.iter().cloned());
        let prospective_targets =
            list_inventory_for_settings(&prospective).map_err(|error| format!("{error:?}"))?;

        let old_path = canonical_skill_target_path(&before_skill.global_dir)
            .map_err(|error| format!("{error:?}"))?;
        let new_path = canonical_skill_target_path(&after_skill.global_dir)
            .map_err(|error| format!("{error:?}"))?;
        if old_path != new_path {
            skill_migration = plan_skill_target_merge(
                &old_inventory,
                &old_capability.target_id,
                &new_capability.target_id,
                &after_skill.global_dir,
                &prospective_targets,
            )?;
            target_files.extend(
                skill_migration
                    .iter()
                    .filter(|entry| entry.source.is_some())
                    .map(|entry| entry.destination.clone()),
            );
        }

        let previous_assignments =
            canonical_skill_assignments(&settings).map_err(|error| format!("{error:?}"))?;
        let mut assignments = BTreeMap::new();
        for (name, target_ids) in &previous_assignments {
            let mut remapped = BTreeSet::new();
            for target_id in target_ids {
                let old_target = old_inventory
                    .targets
                    .iter()
                    .find(|target| &target.target_id == target_id)
                    .ok_or_else(|| {
                        "skill_target_unavailable: 现有 Skills 目标已不可用".to_string()
                    })?;
                let target_path = canonical_skill_target_path(&old_target.global_dir)
                    .map_err(|error| format!("{error:?}"))?;
                if target_id == &old_capability.target_id && old_path != new_path {
                    remapped.insert(new_capability.target_id.clone());
                }
                if let Some(target) = prospective_targets.targets.iter().find(|target| {
                    canonical_skill_target_path(&target.global_dir)
                        .ok()
                        .as_ref()
                        == Some(&target_path)
                }) {
                    remapped.insert(target.target_id.clone());
                } else if target_id != &old_capability.target_id || old_path == new_path {
                    return Err(format!(
                        "skill_path_migration_conflict: {name} 仍分配到即将移除的 Skills 目录"
                    ));
                }
            }
            if !remapped.is_empty() {
                assignments.insert(name.clone(), remapped);
            }
        }
        prospective.skill_assignments = (!assignments.is_empty()).then_some(assignments.clone());
        let after_inventory =
            list_inventory_for_settings(&prospective).map_err(|error| format!("{error:?}"))?;
        skills_before = projected_skill_relationships(&previous_assignments, &old_inventory);
        skills_after = projected_skill_relationships(&assignments, &after_inventory);
        skill_assignments_after = Some(assignments);
    }

    let domain_plan = DomainPlan::AgentCapabilities {
        agent_id: request.agent_id.clone(),
        before: Box::new(before),
        after: Box::new(after.clone()),
        skills_before,
        skills_after,
        affected_agent_ids: configuration_affected_agents.into_iter().collect(),
        migrated_skill_names: skill_migration
            .iter()
            .filter_map(|entry| {
                Path::new(&entry.destination)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect(),
    };
    finalize_plan_with(
        AssetOperationKind::UpdateConfiguration,
        domain_plan,
        Vec::new(),
        target_files,
        Some(LifecycleBinding::AgentCapabilities {
            agent_id: request.agent_id,
            after,
            skill_assignments_after,
            skill_migration,
        }),
    )
}

fn set_prospective_skill_paths(
    settings: &mut Settings,
    agent_id: &str,
    configuration: &SkillConfigurationPatch,
) -> Result<(), String> {
    let defaults = builtin_agents()
        .get(agent_id)
        .and_then(|definition| definition.skills.as_ref())
        .cloned();
    let global_dir = configuration.global_dir.as_str();
    let default_aliases = defaults
        .as_ref()
        .map(|capability| {
            capability
                .aliases
                .iter()
                .map(|alias| alias.global_dir.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let overrides = settings.agent_config_paths.get_or_insert_default();
    let entry = overrides.entry(agent_id.to_string()).or_default();
    entry.skills_global_dir = (defaults
        .as_ref()
        .map(|capability| capability.global_dir.as_str())
        != Some(global_dir))
    .then(|| global_dir.to_string());
    entry.skills_alias_dirs =
        (default_aliases != configuration.alias_dirs).then(|| configuration.alias_dirs.clone());
    if entry == &AgentConfigPathOverride::default() {
        overrides.remove(agent_id);
    }
    if overrides.is_empty() {
        settings.agent_config_paths = None;
    }
    Ok(())
}

fn plan_skill_target_merge(
    old_inventory: &SkillsInventory,
    old_target_id: &str,
    new_target_id: &str,
    new_global_dir: &str,
    new_inventory: &SkillsInventory,
) -> Result<Vec<SkillMigrationEntry>, String> {
    let old_items: Vec<_> = old_inventory
        .items
        .iter()
        .filter(|item| {
            matches!(
                &item.location,
                SkillLocation::AgentTarget { target_id, .. } if target_id == old_target_id
            )
        })
        .collect();
    let new_items: BTreeMap<_, _> = new_inventory
        .items
        .iter()
        .filter_map(|item| match &item.location {
            SkillLocation::AgentTarget { target_id, .. } if target_id == new_target_id => {
                Some((item.name.as_str(), item))
            }
            _ => None,
        })
        .collect();
    let mut migration = Vec::new();
    for old_item in old_items {
        validate_migration_name(old_inventory, &old_item.name)?;
        validate_migration_item(old_item)?;
        let old_source = item_physical_path(old_item)?;
        let old_hash = hash_tree(&old_source).map_err(|error| format!("{error:?}"))?;
        let destination = format!("{new_global_dir}/{}", old_item.name);
        if let Some(new_item) = new_items.get(old_item.name.as_str()) {
            validate_migration_name(new_inventory, &old_item.name)?;
            validate_migration_item(new_item)?;
            let new_source = item_physical_path(new_item)?;
            let new_hash = hash_tree(&new_source).map_err(|error| format!("{error:?}"))?;
            if new_hash != old_hash {
                return Err(format!(
                    "skill_path_migration_conflict: {} 在新旧目录内容不同",
                    old_item.name
                ));
            }
            migration.push(SkillMigrationEntry {
                source: None,
                destination,
                content_hash: old_hash,
            });
        } else {
            migration.push(SkillMigrationEntry {
                source: Some(collapse_home(&old_source.to_string_lossy())),
                destination,
                content_hash: old_hash,
            });
        }
    }
    migration.sort_by(|left, right| left.destination.cmp(&right.destination));
    Ok(migration)
}

fn validate_migration_name(inventory: &SkillsInventory, name: &str) -> Result<(), String> {
    for item in inventory.items.iter().filter(|item| item.name == name) {
        validate_migration_item(item)?;
    }
    Ok(())
}

fn validate_migration_item(
    item: &crate::resources::skill::SkillInventoryItem,
) -> Result<(), String> {
    let blocked = [
        InventoryState::BrokenLink,
        InventoryState::ConflictingLink,
        InventoryState::Missing,
        InventoryState::LocallyModified,
    ]
    .iter()
    .find(|state| item.states.contains(state));
    if let Some(state) = blocked {
        return Err(format!(
            "skill_path_migration_conflict: {} 当前状态为 {:?}",
            item.name, state
        ));
    }
    Ok(())
}

fn item_physical_path(
    item: &crate::resources::skill::SkillInventoryItem,
) -> Result<PathBuf, String> {
    let SkillLocation::AgentTarget { global_dir, .. } = &item.location else {
        return Err("skill_path_migration_conflict: Skill 不在 Agent 目标中".into());
    };
    let path = expand_tilde(&format!("{global_dir}/{}", item.name));
    let canonical = fs::canonicalize(path)
        .map_err(|_| format!("skill_path_migration_conflict: {} 无法安全读取", item.name))?;
    if !canonical.is_dir() {
        return Err(format!(
            "skill_path_migration_conflict: {} 不是目录",
            item.name
        ));
    }
    Ok(canonical)
}

fn projected_skill_relationships(
    assignments: &BTreeMap<String, BTreeSet<String>>,
    inventory: &SkillsInventory,
) -> BTreeMap<String, Vec<String>> {
    let targets: BTreeMap<_, _> = inventory
        .targets
        .iter()
        .map(|target| (target.target_id.as_str(), target))
        .collect();
    let mut projected = BTreeMap::<String, BTreeSet<String>>::new();
    for (name, target_ids) in assignments {
        for target_id in target_ids {
            let Some(target) = targets.get(target_id.as_str()) else {
                continue;
            };
            let agents = if target.affected_agent_ids.is_empty() {
                &target.primary_agent_ids
            } else {
                &target.affected_agent_ids
            };
            for agent_id in agents {
                projected
                    .entry(agent_id.clone())
                    .or_default()
                    .insert(name.clone());
            }
        }
    }
    projected
        .into_iter()
        .map(|(agent_id, names)| (agent_id, names.into_iter().collect()))
        .collect()
}

pub fn plan_set_asset_consumers(
    request: PlanSetAssetConsumersRequest,
) -> Result<AssetOperationPlan, String> {
    request
        .asset
        .validate()
        .map_err(|error| error.to_string())?;
    let selected: BTreeSet<String> = request.agent_ids.into_iter().collect();
    for agent_id in &selected {
        validate_agent_id(agent_id)?;
        require_compatible(agent_id, &request.asset)?;
    }
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let inventory = list_consumption_inventory()?;
    let current: BTreeSet<String> = inventory
        .consumptions
        .iter()
        .filter(|item| item.asset == request.asset && item.desired)
        .map(|item| item.agent_id.clone())
        .collect();
    let affected: BTreeSet<String> = current.union(&selected).cloned().collect();

    let domain_plan = match &request.asset {
        AssetRef::Mcp { key } => {
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for agent_id in affected {
                let existing = current_mcp_selection(&settings, &agent_id);
                let mut desired: BTreeSet<String> = existing.iter().cloned().collect();
                if selected.contains(&agent_id) {
                    desired.insert(key.clone());
                } else {
                    desired.remove(key);
                }
                validate_unique_mcp_names(&desired.iter().cloned().collect::<Vec<_>>())?;
                before.insert(agent_id.clone(), existing);
                after.insert(agent_id, desired.into_iter().collect());
            }
            DomainPlan::Mcp { before, after }
        }
        AssetRef::Model { profile_id } => {
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for agent_id in affected {
                let existing = settings.model_selection(&agent_id);
                let mut desired = existing.clone();
                if selected.contains(&agent_id) {
                    desired
                        .profiles
                        .entry(profile_id.clone())
                        .or_insert(ModelConsumptionRecord {
                            profile_id: profile_id.clone(),
                            enabled: true,
                            last_selected_at: None,
                        });
                } else {
                    desired.profiles.remove(profile_id);
                }
                desired.normalize_active();
                validate_model_selection_contract(&settings, &agent_id, &desired)?;
                before.insert(agent_id.clone(), existing);
                after.insert(agent_id, desired);
            }
            DomainPlan::Model { before, after }
        }
        AssetRef::Skill { name } => {
            validate_closed_skill_consumers(name, &selected)?;
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for agent_id in affected {
                let existing = current_skill_selection(&agent_id)?;
                let mut desired: BTreeSet<String> = existing.iter().cloned().collect();
                if selected.contains(&agent_id) {
                    desired.insert(name.clone());
                } else {
                    desired.remove(name);
                }
                before.insert(agent_id.clone(), existing);
                after.insert(agent_id, desired.into_iter().collect());
            }
            DomainPlan::Skill { before, after }
        }
    };
    finalize_plan(domain_plan)
}

/// Plan an additive/removal consumer change from the current authoritative
/// relationship state. Keeping the read and plan inside one core call avoids
/// frontends accidentally replacing a concurrent consumer with a stale full
/// selection.
pub fn plan_update_asset_consumers(
    request: PlanUpdateAssetConsumersRequest,
) -> Result<AssetOperationPlan, String> {
    request
        .asset
        .validate()
        .map_err(|error| error.to_string())?;
    let additions: BTreeSet<String> = request.add_agent_ids.into_iter().collect();
    let removals: BTreeSet<String> = request.remove_agent_ids.into_iter().collect();
    if additions.is_empty() && removals.is_empty() {
        return Err("asset_consumer_delta_empty: at least one consumer change is required".into());
    }
    if let Some(agent_id) = additions.intersection(&removals).next() {
        return Err(format!(
            "asset_consumer_delta_conflict: Agent cannot be both added and removed: {agent_id}"
        ));
    }
    for agent_id in additions.union(&removals) {
        validate_agent_id(agent_id)?;
    }

    let inventory = list_consumption_inventory()?;
    let mut selected: BTreeSet<String> = inventory
        .consumptions
        .iter()
        .filter(|item| item.asset == request.asset && item.desired)
        .map(|item| item.agent_id.clone())
        .collect();
    selected.extend(additions);
    for agent_id in removals {
        selected.remove(&agent_id);
    }
    plan_set_asset_consumers(PlanSetAssetConsumersRequest {
        asset: request.asset,
        agent_ids: selected.into_iter().collect(),
    })
}

pub(crate) fn validate_model_selection_contract(
    settings: &crate::settings::Settings,
    agent_id: &str,
    selection: &crate::domain::assets::ModelAgentSelection,
) -> Result<(), String> {
    let capability = crate::resources::model::model_agent_capability(agent_id)
        .ok_or_else(|| format!("unsupported model Agent: {agent_id}"))?;
    if !capability.supports_multiple && selection.profiles.len() > 1 {
        return Err(format!(
            "model_single_profile_only: {} supports one MUX Model Profile",
            capability.name
        ));
    }
    let mut identities = BTreeMap::<String, String>::new();
    for (profile_id, record) in &selection.profiles {
        if !record.enabled {
            continue;
        }
        let profile = settings
            .model_profiles
            .as_ref()
            .and_then(|profiles| profiles.get(profile_id))
            .ok_or_else(|| format!("model_profile_missing: {profile_id}"))?;
        let identity = match agent_id {
            "qwen-code" => format!(
                "{}::{}",
                match profile.protocol {
                    crate::domain::types::ModelProtocol::AnthropicMessages => "anthropic",
                    _ => "openai",
                },
                profile.model
            ),
            "factory-droid" => profile.model.clone(),
            _ => profile_id.clone(),
        };
        if let Some(existing) = identities.insert(identity.clone(), profile_id.clone()) {
            return Err(format!(
                "model_native_identity_conflict: {agent_id} cannot distinguish Profiles {existing} and {profile_id} ({identity})"
            ));
        }
    }
    Ok(())
}

fn validate_unique_mcp_names(keys: &[String]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for key in keys {
        let name = key
            .rsplit_once("::")
            .map(|(name, _)| name)
            .ok_or_else(|| format!("invalid MCP asset key: {key}"))?;
        if !names.insert(name) {
            return Err(format!(
                "mcp_identity_conflict: one Agent cannot consume two transport variants named {name}"
            ));
        }
    }
    Ok(())
}

fn skill_plan_for_agent(
    agent_id: &str,
    current: Vec<String>,
    desired: Vec<String>,
) -> Result<DomainPlan, String> {
    let current_set: BTreeSet<String> = current.iter().cloned().collect();
    let desired_set: BTreeSet<String> = desired.iter().cloned().collect();
    let changed: Vec<String> = current_set
        .symmetric_difference(&desired_set)
        .cloned()
        .collect();
    let mut affected = BTreeSet::from([agent_id.to_string()]);
    for name in &changed {
        let compatibility = compatibility_for(agent_id, &AssetRef::Skill { name: name.clone() })?;
        affected.extend(compatibility.affected_agent_ids);
    }
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for affected_agent in affected {
        let existing = current_skill_selection(&affected_agent)?;
        let mut next: BTreeSet<String> = existing.iter().cloned().collect();
        for name in &changed {
            if desired_set.contains(name) {
                next.insert(name.clone());
            } else {
                next.remove(name);
            }
        }
        before.insert(affected_agent.clone(), existing);
        after.insert(affected_agent, next.into_iter().collect());
    }
    Ok(DomainPlan::Skill { before, after })
}

fn validate_closed_skill_consumers(name: &str, selected: &BTreeSet<String>) -> Result<(), String> {
    for agent_id in selected {
        let compatibility = compatibility_for(
            agent_id,
            &AssetRef::Skill {
                name: name.to_string(),
            },
        )?;
        let missing: Vec<_> = compatibility
            .affected_agent_ids
            .into_iter()
            .filter(|affected| !selected.contains(affected))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "skill_shared_target_conflict: {agent_id} shares one physical Skill target with {}",
                missing.join(", ")
            ));
        }
    }
    Ok(())
}

fn require_compatible(agent_id: &str, asset: &AssetRef) -> Result<(), String> {
    let view = compatibility_for(agent_id, asset)?;
    if view.compatible {
        Ok(())
    } else {
        let reason = view.reason.expect("incompatible view has reason");
        Err(format!("{}: {}", reason.code, reason.message))
    }
}

fn validate_agent_id(agent_id: &str) -> Result<(), String> {
    if agent_id.trim().is_empty() || !load_agents().contains_key(agent_id) {
        return Err(format!("unknown Agent: {agent_id}"));
    }
    Ok(())
}

fn current_mcp_selection(settings: &Settings, agent_id: &str) -> Vec<String> {
    settings
        .mcp_consumptions
        .as_ref()
        .and_then(|consumptions| consumptions.get(agent_id))
        .map(|records| records.keys().cloned().collect())
        .unwrap_or_default()
}

fn current_skill_selection(agent_id: &str) -> Result<Vec<String>, String> {
    let names: BTreeSet<String> = list_consumption_inventory()?
        .consumptions
        .into_iter()
        .filter_map(|item| match item.asset {
            AssetRef::Skill { name } if item.agent_id == agent_id && item.desired => Some(name),
            _ => None,
        })
        .collect();
    Ok(names.iter().cloned().collect())
}

fn finalize_plan(domain_plan: DomainPlan) -> Result<AssetOperationPlan, String> {
    finalize_plan_with(
        AssetOperationKind::SetConsumption,
        domain_plan,
        Vec::new(),
        Vec::new(),
        None,
    )
}

pub(crate) fn finalize_plan_with(
    kind: AssetOperationKind,
    domain_plan: DomainPlan,
    central_changes: Vec<CentralAssetChange>,
    extra_target_files: Vec<String>,
    lifecycle: Option<LifecycleBinding>,
) -> Result<AssetOperationPlan, String> {
    if let Some(error) = super::transaction::pending_recovery_error() {
        return Err(format!("recovery_required: {error}"));
    }
    let relationship_changes = relationship_changes(&domain_plan);
    let model_state_changes = model_state_changes(&domain_plan);
    let affected_agent_ids: Vec<String> = agents_for_plan(&domain_plan).into_iter().collect();
    let mut target_files = target_files(&domain_plan)?;
    target_files.extend(extra_target_files);
    target_files.sort();
    target_files.dedup();
    validate_transaction_targets(&domain_plan, &target_files)?;
    let current_inventory = list_consumption_inventory()?;
    let effects = effect_assets(&domain_plan, &central_changes, &relationship_changes);
    let mut blocked: Vec<_> = current_inventory
        .consumptions
        .iter()
        .filter(|item| {
            effects.contains(&(item.agent_id.clone(), item.asset.clone()))
                && matches!(
                    item.status,
                    ConsumptionStatus::Drifted | ConsumptionStatus::Conflicted
                )
        })
        .map(|item| {
            format!(
                "{}: {}",
                item.agent_id,
                item.reason.as_deref().unwrap_or("unresolved_drift")
            )
        })
        .collect();
    for (agent_id, asset) in &effects {
        if !asset_desired_after(&domain_plan, agent_id, asset) {
            continue;
        }
        if kind == AssetOperationKind::UpdateConfiguration
            && matches!(asset, AssetRef::Skill { .. })
        {
            // The configuration migration already compared the old and new
            // physical Skill content by hash. A matching external observation
            // at the destination is the merge target, not an unmanaged clash.
            continue;
        }
        for item in current_inventory.external.iter().filter(|item| {
            kind != AssetOperationKind::Adopt && external_blocks_selection(agent_id, asset, item)
        }) {
            blocked.push(format!(
                "{}: {}",
                item.agent_id,
                item.reason.as_deref().unwrap_or("external_asset_conflict")
            ));
        }
    }
    blocked.sort();
    blocked.dedup();
    let warnings = blocked.clone();
    let replaceable_model_conflict = blocked.iter().all(|warning| {
        [
            "model_external_unmanaged",
            "model_owned_fields_drift",
            "model_target_missing",
        ]
        .iter()
        .any(|reason| warning.ends_with(reason))
    });
    let model_replacement = !blocked.is_empty()
        && replaceable_model_conflict
        && kind == AssetOperationKind::SetConsumption
        && matches!(
            &domain_plan,
            DomainPlan::Model { after, .. }
                if after.values().all(|selection| selection.active_profile_id.is_some())
        );
    let requires_conflict_confirmation =
        !blocked.is_empty() && (kind == AssetOperationKind::UpdateAsset || model_replacement);
    let can_commit = blocked.is_empty() || requires_conflict_confirmation;
    let settings_hash = hash_file(&settings_file());
    let target_hashes = hash_targets(&target_files);
    let mcp_catalog_hash = if matches!(&domain_plan, DomainPlan::Mcp { .. }) {
        Some(hash_mcp_catalog()?)
    } else {
        None
    };
    let operation_id = Uuid::new_v4().hyphenated().to_string();
    let candidate_material = serde_json::to_vec(&(
        &kind,
        &domain_plan,
        &central_changes,
        &relationship_changes,
        &model_state_changes,
        &target_files,
        &affected_agent_ids,
        &warnings,
        can_commit,
        requires_conflict_confirmation,
        &settings_hash,
        &target_hashes,
        &mcp_catalog_hash,
        &lifecycle,
    ))
    .map_err(|error| error.to_string())?;
    let candidate_hash = hex::encode(Sha256::digest(candidate_material));
    let plan = AssetOperationPlan {
        operation_id,
        kind,
        domain_plan,
        central_changes,
        relationship_changes,
        model_state_changes,
        target_files,
        affected_agent_ids,
        warnings,
        can_commit,
        requires_conflict_confirmation,
        candidate_hash,
    };
    persist_operation(&PersistedAssetOperation {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan: plan.clone(),
        settings_hash,
        target_hashes,
        mcp_catalog_hash,
        lifecycle,
    })?;
    Ok(plan)
}

fn validate_transaction_targets(plan: &DomainPlan, targets: &[String]) -> Result<(), String> {
    let skill_links_are_managed = matches!(plan, DomainPlan::Skill { .. });
    for target in targets {
        let path = expand_tilde(target);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                return Err(format!(
                    "asset_target_unsafe: refusing to use directory as a transaction file: {}",
                    path.display()
                ));
            }
            Ok(metadata) if metadata.file_type().is_symlink() && !skill_links_are_managed => {
                return Err(format!(
                    "asset_target_unsafe: symlink-backed configuration files require explicit migration: {}",
                    path.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "asset_target_unreadable: failed to inspect {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn model_state_changes(plan: &DomainPlan) -> Vec<ModelStateChange> {
    let DomainPlan::Model { before, after } = plan else {
        return Vec::new();
    };
    let mut changes = Vec::new();
    for agent_id in union_keys(before, after) {
        let left = before.get(agent_id).cloned().unwrap_or_default();
        let right = after.get(agent_id).cloned().unwrap_or_default();
        let profile_ids: BTreeSet<String> = left
            .profiles
            .keys()
            .chain(right.profiles.keys())
            .cloned()
            .collect();
        for profile_id in profile_ids {
            let snapshot = |selection: &super::types::ModelAgentSelection| {
                let record = selection.profiles.get(&profile_id);
                ModelStateSnapshot {
                    added: record.is_some(),
                    enabled: record.is_some_and(|record| record.enabled),
                    active: selection.active_profile_id.as_deref() == Some(profile_id.as_str()),
                }
            };
            let before_state = snapshot(&left);
            let after_state = snapshot(&right);
            if before_state == after_state {
                continue;
            }
            let reason = if !before_state.added && after_state.added {
                "model_added"
            } else if before_state.added && !after_state.added {
                "model_removed"
            } else if before_state.enabled != after_state.enabled {
                if after_state.enabled {
                    "model_enabled"
                } else {
                    "model_disabled"
                }
            } else if before_state.active != after_state.active {
                if after_state.active {
                    "model_activated"
                } else {
                    "model_deactivated"
                }
            } else {
                "model_state_updated"
            };
            let fallback_profile_id = (before_state.active && !after_state.active)
                .then(|| right.active_profile_id.clone())
                .flatten();
            changes.push(ModelStateChange {
                agent_id: agent_id.clone(),
                profile_id,
                before: before_state,
                after: after_state,
                fallback_profile_id,
                reason: reason.into(),
            });
        }
    }
    changes
}

fn relationship_changes(plan: &DomainPlan) -> Vec<RelationshipChange> {
    let mut changes = Vec::new();
    match plan {
        DomainPlan::Mcp { before, after } => {
            for agent_id in union_keys(before, after) {
                diff_many(
                    agent_id,
                    before.get(agent_id).cloned().unwrap_or_default(),
                    after.get(agent_id).cloned().unwrap_or_default(),
                    |key| AssetRef::Mcp { key },
                    &mut changes,
                );
            }
        }
        DomainPlan::Skill { before, after } => {
            for agent_id in union_keys(before, after) {
                diff_many(
                    agent_id,
                    before.get(agent_id).cloned().unwrap_or_default(),
                    after.get(agent_id).cloned().unwrap_or_default(),
                    |name| AssetRef::Skill { name },
                    &mut changes,
                );
            }
        }
        DomainPlan::Model { before, after } => {
            for agent_id in union_keys(before, after) {
                let left = before.get(agent_id).cloned().unwrap_or_default();
                let right = after.get(agent_id).cloned().unwrap_or_default();
                diff_many(
                    agent_id,
                    left.profiles.keys().cloned().collect(),
                    right.profiles.keys().cloned().collect(),
                    |profile_id| AssetRef::Model { profile_id },
                    &mut changes,
                );
            }
        }
        DomainPlan::AgentConfiguration {
            skills_before,
            skills_after,
            ..
        }
        | DomainPlan::AgentCapabilities {
            skills_before,
            skills_after,
            ..
        } => {
            for agent_id in union_keys(skills_before, skills_after) {
                diff_many(
                    agent_id,
                    skills_before.get(agent_id).cloned().unwrap_or_default(),
                    skills_after.get(agent_id).cloned().unwrap_or_default(),
                    |name| AssetRef::Skill { name },
                    &mut changes,
                );
            }
        }
    }
    changes.sort_by(|left, right| {
        left.agent_id
            .cmp(&right.agent_id)
            .then_with(|| left.asset.cmp(&right.asset))
            .then_with(|| format!("{:?}", left.action).cmp(&format!("{:?}", right.action)))
    });
    changes
}

fn diff_many<F>(
    agent_id: &str,
    before: Vec<String>,
    after: Vec<String>,
    asset: F,
    out: &mut Vec<RelationshipChange>,
) where
    F: Fn(String) -> AssetRef,
{
    let before: BTreeSet<String> = before.into_iter().collect();
    let after: BTreeSet<String> = after.into_iter().collect();
    for identity in before.difference(&after) {
        out.push(RelationshipChange {
            agent_id: agent_id.to_owned(),
            asset: asset(identity.clone()),
            action: RelationshipAction::Remove,
        });
    }
    for identity in after.difference(&before) {
        out.push(RelationshipChange {
            agent_id: agent_id.to_owned(),
            asset: asset(identity.clone()),
            action: RelationshipAction::Add,
        });
    }
}

fn union_keys<'a, T>(
    left: &'a BTreeMap<String, T>,
    right: &'a BTreeMap<String, T>,
) -> BTreeSet<&'a String> {
    left.keys().chain(right.keys()).collect()
}

fn agents_for_plan(plan: &DomainPlan) -> BTreeSet<String> {
    match plan {
        DomainPlan::Mcp { before, after } | DomainPlan::Skill { before, after } => {
            before.keys().chain(after.keys()).cloned().collect()
        }
        DomainPlan::Model { before, after } => before.keys().chain(after.keys()).cloned().collect(),
        DomainPlan::AgentConfiguration {
            agent_id,
            skills_before,
            skills_after,
            affected_agent_ids,
            ..
        }
        | DomainPlan::AgentCapabilities {
            agent_id,
            skills_before,
            skills_after,
            affected_agent_ids,
            ..
        } => skills_before
            .keys()
            .chain(skills_after.keys())
            .cloned()
            .chain(affected_agent_ids.iter().cloned())
            .chain(std::iter::once(agent_id.clone()))
            .collect(),
    }
}

fn domain_matches(plan: &DomainPlan, asset: &AssetRef) -> bool {
    matches!(
        (plan, asset),
        (DomainPlan::Mcp { .. }, AssetRef::Mcp { .. })
            | (DomainPlan::Model { .. }, AssetRef::Model { .. })
            | (DomainPlan::Skill { .. }, AssetRef::Skill { .. })
            | (
                DomainPlan::AgentConfiguration { .. },
                AssetRef::Skill { .. }
            )
            | (DomainPlan::AgentCapabilities { .. }, AssetRef::Skill { .. })
    )
}

fn effect_assets(
    plan: &DomainPlan,
    central_changes: &[CentralAssetChange],
    relationship_changes: &[RelationshipChange],
) -> BTreeSet<(String, AssetRef)> {
    let mut effects: BTreeSet<(String, AssetRef)> = relationship_changes
        .iter()
        .map(|change| (change.agent_id.clone(), change.asset.clone()))
        .collect();
    // Re-selecting an assigned Model is the Agent-scoped repair path. Include
    // unchanged desired profiles so drift/conflicts are reviewed and bound to
    // an explicit confirmation instead of producing an invisible no-op.
    if let DomainPlan::Model { after, .. } = plan {
        effects.extend(after.iter().flat_map(|(agent_id, selection)| {
            selection
                .profiles
                .keys()
                .cloned()
                .map(|profile_id| (agent_id.clone(), AssetRef::Model { profile_id }))
        }));
    }
    let agents = agents_for_plan(plan);
    for change in central_changes {
        if !domain_matches(plan, &change.asset) {
            continue;
        }
        effects.extend(
            agents
                .iter()
                .cloned()
                .map(|agent_id| (agent_id, change.asset.clone())),
        );
    }
    effects
}

fn asset_desired_after(plan: &DomainPlan, agent_id: &str, asset: &AssetRef) -> bool {
    match (plan, asset) {
        (DomainPlan::Mcp { after, .. }, AssetRef::Mcp { key }) => {
            after.get(agent_id).is_some_and(|keys| keys.contains(key))
        }
        (DomainPlan::Model { after, .. }, AssetRef::Model { profile_id }) => after
            .get(agent_id)
            .is_some_and(|selection| selection.profiles.contains_key(profile_id)),
        (DomainPlan::Skill { after, .. }, AssetRef::Skill { name }) => after
            .get(agent_id)
            .is_some_and(|names| names.contains(name)),
        (DomainPlan::AgentConfiguration { skills_after, .. }, AssetRef::Skill { name }) => {
            skills_after
                .get(agent_id)
                .is_some_and(|names| names.contains(name))
        }
        (DomainPlan::AgentCapabilities { skills_after, .. }, AssetRef::Skill { name }) => {
            skills_after
                .get(agent_id)
                .is_some_and(|names| names.contains(name))
        }
        _ => false,
    }
}

fn external_blocks_selection(
    agent_id: &str,
    asset: &AssetRef,
    external: &super::types::ConsumptionView,
) -> bool {
    if external.agent_id != agent_id {
        return false;
    }
    match (asset, &external.asset) {
        (AssetRef::Mcp { key }, AssetRef::Mcp { key: external_key }) => {
            key == external_key && external.reason.as_deref() != Some("mcp_adoptable")
        }
        (AssetRef::Model { .. }, AssetRef::Model { .. }) => true,
        (
            AssetRef::Skill { name },
            AssetRef::Skill {
                name: external_name,
            },
        ) => name == external_name,
        _ => false,
    }
}

fn target_files(plan: &DomainPlan) -> Result<Vec<String>, String> {
    let mut files = BTreeSet::new();
    match plan {
        DomainPlan::Mcp { .. } => {
            let agents = load_agents();
            for agent_id in agents_for_plan(plan) {
                if let Some(path) = agents.get(&agent_id).and_then(|agent| agent.global.clone()) {
                    files.insert(path);
                }
            }
        }
        DomainPlan::Model { before, after } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            for agent_id in agents_for_plan(plan) {
                let paths =
                    crate::resources::model::configured_path_strings_checked(&settings, &agent_id)?
                        .ok_or_else(|| format!("unsupported model Agent: {agent_id}"))?;
                let profile_ids: BTreeSet<String> = before
                    .get(&agent_id)
                    .into_iter()
                    .chain(after.get(&agent_id))
                    .flat_map(|selection| selection.profiles.keys().cloned())
                    .collect();
                for path in crate::resources::model::adapters::target_files(
                    &agent_id,
                    &paths,
                    &profile_ids.into_iter().collect::<Vec<_>>(),
                    settings.model_profiles.as_ref().unwrap_or(&BTreeMap::new()),
                ) {
                    files.insert(path);
                }
            }
        }
        DomainPlan::Skill { before, after } => {
            let skills = list_skills_inventory().map_err(|error| format!("{error:?}"))?;
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let assignments =
                canonical_skill_assignments(&settings).map_err(|error| format!("{error:?}"))?;
            let changed_names = changed_skill_names(before, after);

            for name in changed_names {
                let desired_agents = after
                    .iter()
                    .filter(|(_, names)| names.contains(&name))
                    .map(|(agent_id, _)| agent_id.clone())
                    .collect::<Vec<_>>();
                let mut touched_target_ids = assignments.get(&name).cloned().unwrap_or_default();
                touched_target_ids.extend(
                    normalize_agent_selection(&desired_agents)
                        .map_err(|error| format!("{error:?}"))?,
                );
                touched_target_ids.extend(
                    skills
                        .items
                        .iter()
                        .filter(|item| item.name == name)
                        .flat_map(|item| item.assigned_target_ids.iter().cloned()),
                );

                for target in &skills.targets {
                    if touched_target_ids.contains(&target.target_id) {
                        files.insert(format!("{}/{}", target.global_dir, name));
                    }
                }
            }
        }
        DomainPlan::AgentConfiguration { .. } | DomainPlan::AgentCapabilities { .. } => {}
    }
    Ok(files.into_iter().collect())
}

fn changed_skill_names(
    before: &BTreeMap<String, Vec<String>>,
    after: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .flat_map(|agent_id| {
            let left: BTreeSet<String> = before
                .get(agent_id)
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            let right: BTreeSet<String> =
                after.get(agent_id).into_iter().flatten().cloned().collect();
            left.symmetric_difference(&right)
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn hash_targets(targets: &[String]) -> BTreeMap<String, String> {
    targets
        .iter()
        .map(|target| (target.clone(), hash_path(&expand_tilde(target))))
        .collect()
}

pub(crate) fn hash_file(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => hex::encode(Sha256::digest(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => "missing".into(),
        Err(error) => format!("error:{:?}", error.kind()),
    }
}

pub(crate) fn hash_mcp_catalog() -> Result<String, String> {
    // Converting through Value canonicalizes object keys that originated in
    // HashMaps (headers/env) before hashing.
    let catalog = serde_json::to_value(crate::resources::mcp::registry::read_registry_all())
        .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&catalog).map_err(|error| error.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn hash_path(path: &Path) -> String {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::read_link(path)
            .map(|target| hex::encode(Sha256::digest(target.as_os_str().as_encoded_bytes())))
            .unwrap_or_else(|error| format!("error:{:?}", error.kind())),
        Ok(metadata) if metadata.is_file() => hash_file(path),
        Ok(metadata) if metadata.is_dir() => "directory".into(),
        Ok(_) => "other".into(),
        Err(error) if error.kind() == ErrorKind::NotFound => "missing".into(),
        Err(error) => format!("error:{:?}", error.kind()),
    }
}

pub(crate) fn operation_root(operation_id: &str) -> PathBuf {
    mux_dir().join("staging/consumption").join(operation_id)
}

fn persist_operation(operation: &PersistedAssetOperation) -> Result<(), String> {
    let root = operation_root(&operation.plan.operation_id);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    let path = root.join("plan.json");
    let bytes = serde_json::to_vec_pretty(operation).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn load_operation(operation_id: &str) -> Result<PersistedAssetOperation, String> {
    Uuid::parse_str(operation_id).map_err(|_| "invalid asset operation id".to_string())?;
    let bytes = fs::read(operation_root(operation_id).join("plan.json"))
        .map_err(|_| "asset operation is unavailable or expired".to_string())?;
    let operation: PersistedAssetOperation = serde_json::from_slice(&bytes)
        .map_err(|_| "asset operation plan is invalid".to_string())?;
    if operation.schema_version != OPERATION_SCHEMA_VERSION
        || operation.plan.operation_id != operation_id
    {
        return Err("asset operation plan is incompatible".into());
    }
    Ok(operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::current_configuration;
    use crate::assets::{commit_asset_operation, AssetCommitRequest};
    use crate::domain::assets::{McpConsumptionRecord, ModelAgentSelection};
    use crate::domain::types::{
        HttpConfig, ModelProfile, ModelProtocol, RegistryConfig, RegistryEntry, StdioConfig,
    };
    use crate::resources::mcp::registry::write_manual_entry;
    use crate::settings::mutate_settings;
    use crate::testenv::TestHome;

    fn write_external_skill(root: &Path, name: &str, description: &str) {
        let skill = root.join(name);
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n"),
        )
        .unwrap();
    }

    fn write_local_mcp_for_target_safety() {
        write_manual_entry(&RegistryEntry {
            name: "target-safety".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "target-safety-server".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
    }

    #[test]
    fn mcp_plan_rejects_a_directory_target_without_touching_its_contents() {
        let home = TestHome::new("consume-directory-target");
        write_local_mcp_for_target_safety();
        let target = home.home.join(".claude.json");
        fs::create_dir_all(&target).unwrap();
        let sentinel = target.join("keep.txt");
        fs::write(&sentinel, "keep").unwrap();

        let error = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["target-safety::stdio".into()],
            },
        })
        .unwrap_err();

        assert!(error.contains("refusing to use directory"), "{error}");
        assert_eq!(fs::read_to_string(sentinel).unwrap(), "keep");
    }

    #[cfg(unix)]
    #[test]
    fn mcp_plan_rejects_a_symlink_backed_configuration_file() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("consume-symlink-target");
        write_local_mcp_for_target_safety();
        let destination = home.home.join("real-claude.json");
        let target = home.home.join(".claude.json");
        fs::write(&destination, "{\"untouched\":true}").unwrap();
        symlink(&destination, &target).unwrap();

        let error = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["target-safety::stdio".into()],
            },
        })
        .unwrap_err();

        assert!(error.contains("symlink-backed"), "{error}");
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            "{\"untouched\":true}"
        );
    }

    #[test]
    fn mcp_plan_has_stable_typed_diff_and_private_persistence() {
        let home = TestHome::new("consume-plan");
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "local-server".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        mutate_settings(|settings| settings.mcp_consumptions = None).unwrap();

        let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();

        assert!(plan.can_commit);
        assert_eq!(plan.relationship_changes.len(), 1);
        assert_eq!(plan.relationship_changes[0].action, RelationshipAction::Add);
        let persisted = fs::read_to_string(
            home.home
                .join(".mux/staging/consumption")
                .join(&plan.operation_id)
                .join("plan.json"),
        )
        .unwrap();
        assert!(!persisted.contains(home.home.to_string_lossy().as_ref()));
    }

    #[test]
    fn agent_and_asset_entrypoints_produce_the_same_candidate() {
        let _home = TestHome::new("consume-plan-equivalent");
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "local-server".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        let agent = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();
        let asset = plan_set_asset_consumers(PlanSetAssetConsumersRequest {
            asset: AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            agent_ids: vec!["claude-code".into()],
        })
        .unwrap();
        assert_eq!(agent.domain_plan, asset.domain_plan);
        assert_eq!(agent.relationship_changes, asset.relationship_changes);
        assert_eq!(agent.target_files, asset.target_files);
        assert_eq!(agent.candidate_hash, asset.candidate_hash);
    }

    #[test]
    fn consumer_delta_preserves_existing_consumers() {
        let _home = TestHome::new("consume-plan-delta");
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "local-server".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        mutate_settings(|settings| {
            settings
                .mcp_consumptions
                .get_or_insert_default()
                .entry("codex".into())
                .or_default()
                .insert(
                    "local::stdio".into(),
                    McpConsumptionRecord {
                        asset_key: "local::stdio".into(),
                        enabled: true,
                        overrides: Default::default(),
                    },
                );
        })
        .unwrap();

        let plan = plan_update_asset_consumers(PlanUpdateAssetConsumersRequest {
            asset: AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            add_agent_ids: vec!["claude-code".into()],
            remove_agent_ids: Vec::new(),
        })
        .unwrap();

        let DomainPlan::Mcp { after, .. } = plan.domain_plan else {
            panic!("expected MCP plan");
        };
        assert!(after["codex"].contains(&"local::stdio".to_string()));
        assert!(after["claude-code"].contains(&"local::stdio".to_string()));
    }

    #[test]
    fn ensure_agent_consumption_preserves_unrelated_assets() {
        let _home = TestHome::new("consume-plan-ensure");
        for name in ["first", "second"] {
            write_manual_entry(&RegistryEntry {
                name: name.into(),
                description: String::new(),
                tags: Vec::new(),
                config: RegistryConfig {
                    stdio: Some(StdioConfig {
                        command: format!("{name}-server"),
                        args: None,
                        env: None,
                        cwd: None,
                    }),
                    http: None,
                },
                origin: None,
                repo: None,
            })
            .unwrap();
        }
        mutate_settings(|settings| {
            settings
                .mcp_consumptions
                .get_or_insert_default()
                .entry("codex".into())
                .or_default()
                .insert(
                    "first::stdio".into(),
                    McpConsumptionRecord {
                        asset_key: "first::stdio".into(),
                        enabled: true,
                        overrides: Default::default(),
                    },
                );
        })
        .unwrap();

        let plan = plan_ensure_agent_consumption(PlanEnsureAgentConsumptionRequest {
            agent_id: "codex".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["second::stdio".into()],
            },
        })
        .unwrap();

        let DomainPlan::Mcp { after, .. } = plan.domain_plan else {
            panic!("expected MCP plan");
        };
        assert_eq!(
            after["codex"],
            vec!["first::stdio".to_string(), "second::stdio".to_string()]
        );
    }

    #[test]
    fn capability_plan_rejects_in_use_mcp_writer_path_changes() {
        let _home = TestHome::new("capability-plan-mcp-path-in-use");
        mutate_settings(|settings| {
            settings
                .mcp_consumptions
                .get_or_insert_default()
                .entry("codex".into())
                .or_default()
                .insert(
                    "fixture::stdio".into(),
                    McpConsumptionRecord {
                        asset_key: "fixture::stdio".into(),
                        enabled: true,
                        overrides: Default::default(),
                    },
                );
        })
        .unwrap();

        let error = plan_update_agent_capabilities(PlanUpdateAgentCapabilitiesRequest {
            agent_id: "codex".into(),
            patch: AgentConfigurationPatch {
                mcp: Some(McpConfigurationPatch {
                    path: "~/.codex/moved.toml".into(),
                    key: None,
                }),
                ..AgentConfigurationPatch::default()
            },
        })
        .unwrap_err();

        assert!(error.starts_with("agent_configuration_in_use:"), "{error}");
    }

    #[test]
    fn capability_plan_rejects_in_use_model_writer_path_changes() {
        let _home = TestHome::new("capability-plan-model-path-in-use");
        mutate_settings(|settings| {
            settings.set_model_selection(
                "grok-build",
                ModelAgentSelection {
                    profiles: BTreeMap::from([(
                        "fixture".into(),
                        ModelConsumptionRecord {
                            profile_id: "fixture".into(),
                            enabled: true,
                            last_selected_at: None,
                        },
                    )]),
                    active_profile_id: Some("fixture".into()),
                },
            );
        })
        .unwrap();

        let error = plan_update_agent_capabilities(PlanUpdateAgentCapabilitiesRequest {
            agent_id: "grok-build".into(),
            patch: AgentConfigurationPatch {
                model: Some(ModelConfigurationPatch {
                    paths: vec!["~/.grok/moved.toml".into()],
                }),
                ..AgentConfigurationPatch::default()
            },
        })
        .unwrap_err();

        assert!(error.starts_with("agent_configuration_in_use:"), "{error}");
    }

    #[test]
    fn one_agent_cannot_select_two_transports_with_the_same_mcp_name() {
        let _home = TestHome::new("consume-mcp-name-conflict");
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "local-server".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: None,
                http: Some(HttpConfig {
                    kind: "http".into(),
                    url: "https://example.invalid/mcp".into(),
                    headers: None,
                }),
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        let error = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into(), "local::http".into()],
            },
        })
        .unwrap_err();
        assert!(error.starts_with("mcp_identity_conflict:"));
    }

    #[test]
    fn asset_entrypoint_cannot_add_a_second_profile_to_a_single_model_agent() {
        let _home = TestHome::new("consume-model-single-profile");
        let profile = |id: &str| ModelProfile {
            id: id.into(),
            name: id.into(),
            provider: "custom".into(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::AnthropicMessages,
            base_url: format!("https://{id}.example.invalid/v1"),
            model: format!("{id}-model"),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: false,
        };
        mutate_settings(|settings| {
            settings.model_profiles.get_or_insert_default().extend([
                ("work".into(), profile("work")),
                ("other".into(), profile("other")),
            ]);
            settings.set_model_selection(
                "claude-code",
                crate::assets::ModelAgentSelection {
                    profiles: BTreeMap::from([(
                        "work".into(),
                        ModelConsumptionRecord {
                            profile_id: "work".into(),
                            enabled: true,
                            last_selected_at: None,
                        },
                    )]),
                    active_profile_id: Some("work".into()),
                },
            );
        })
        .unwrap();

        let error = plan_set_asset_consumers(PlanSetAssetConsumersRequest {
            asset: AssetRef::Model {
                profile_id: "other".into(),
            },
            agent_ids: vec!["claude-code".into()],
        })
        .unwrap_err();

        assert!(error.starts_with("model_single_profile_only:"), "{error}");
    }

    #[test]
    fn qwen_rejects_profiles_with_the_same_native_selection_identity() {
        let _home = TestHome::new("consume-model-qwen-identity");
        let profile = |id: &str| ModelProfile {
            id: id.into(),
            name: id.into(),
            provider: "custom".into(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiCompletions,
            base_url: format!("https://{id}.example.invalid/v1"),
            model: "shared-model".into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: false,
        };
        mutate_settings(|settings| {
            settings.model_profiles.get_or_insert_default().extend([
                ("first".into(), profile("first")),
                ("second".into(), profile("second")),
            ]);
        })
        .unwrap();

        let error = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "qwen-code".into(),
            selection: AgentConsumptionSelection::Model {
                profile_ids: vec!["first".into(), "second".into()],
            },
        })
        .unwrap_err();

        assert!(
            error.starts_with("model_native_identity_conflict:"),
            "{error}"
        );
    }

    #[test]
    fn configuration_plan_migrates_external_skills_before_switching_the_path() {
        let home = TestHome::new("configuration-skill-migration");
        fs::create_dir_all(home.home.join(".codex")).unwrap();
        write_external_skill(
            &home.home.join(".agents/skills"),
            "shared-notes",
            "Shared notes",
        );
        mutate_settings(|settings| {
            settings.skill_assignments = Some(
                [(
                    "shared-notes".into(),
                    ["agents-user".into()].into_iter().collect(),
                )]
                .into_iter()
                .collect(),
            );
        })
        .unwrap();
        let mut configuration = current_configuration("codex").unwrap();
        configuration.skills_global_dir = Some("~/.codex-private/skills".into());

        let plan = plan_update_agent_configuration(PlanUpdateAgentConfigurationRequest {
            agent_id: "codex".into(),
            configuration,
        })
        .unwrap();
        assert_eq!(plan.kind, AssetOperationKind::UpdateConfiguration);
        assert_eq!(plan.affected_agent_ids, vec!["codex"]);
        assert_eq!(
            plan.target_files,
            vec!["~/.codex-private/skills/shared-notes"]
        );

        commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
        })
        .unwrap();

        let migrated = home.home.join(".codex-private/skills/shared-notes");
        assert!(fs::symlink_metadata(&migrated)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            crate::agents::load_agents()["codex"]
                .skills
                .as_ref()
                .unwrap()
                .target_id,
            "codex-configured"
        );
        assert_eq!(
            load_settings_strict().unwrap().skill_assignments.unwrap()["shared-notes"],
            ["codex-configured".into()].into_iter().collect()
        );
    }

    #[test]
    fn configuration_plan_adds_a_skills_compatibility_directory_without_moving_primary_content() {
        let home = TestHome::new("configuration-skill-alias");
        fs::create_dir_all(home.home.join(".codex")).unwrap();
        let mut configuration = current_configuration("codex").unwrap();
        configuration.skills_alias_dirs = vec!["~/.codex/skills".into()];

        let plan = plan_update_agent_configuration(PlanUpdateAgentConfigurationRequest {
            agent_id: "codex".into(),
            configuration,
        })
        .unwrap();
        assert!(plan.target_files.is_empty());
        let DomainPlan::AgentCapabilities { before, after, .. } = &plan.domain_plan else {
            panic!("expected Agent capability plan");
        };
        assert!(before.skill.as_ref().unwrap().alias_dirs.is_empty());
        assert_eq!(
            after.skill.as_ref().unwrap().alias_dirs,
            ["~/.codex/skills"]
        );

        commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
        })
        .unwrap();

        assert_eq!(
            crate::agents::load_agents()["codex"]
                .skills
                .as_ref()
                .unwrap()
                .aliases[0]
                .global_dir,
            "~/.codex/skills"
        );
    }

    #[test]
    fn configuration_plan_commits_a_key_only_override_without_touching_mcp_files() {
        let _home = TestHome::new("configuration-mcp-key-only");
        let mut configuration = current_configuration("codex").unwrap();
        configuration.mcp_key = Some("custom.mcpServers".into());

        let plan = plan_update_agent_configuration(PlanUpdateAgentConfigurationRequest {
            agent_id: "codex".into(),
            configuration,
        })
        .unwrap();

        assert!(plan.target_files.is_empty());
        let DomainPlan::AgentCapabilities { before, after, .. } = &plan.domain_plan else {
            panic!("expected Agent capability plan");
        };
        let before_mcp = before.mcp.as_ref().unwrap();
        let after_mcp = after.mcp.as_ref().unwrap();
        assert_eq!(before_mcp.key.as_deref(), Some("mcp_servers"));
        assert_eq!(after_mcp.key.as_deref(), Some("custom.mcpServers"));
        assert_eq!(before_mcp.path, after_mcp.path);

        commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
        })
        .unwrap();

        assert_eq!(
            crate::agents::load_agents()["codex"].key,
            "custom.mcpServers"
        );
    }

    #[test]
    fn configuration_plan_blocks_same_name_different_skill_content() {
        let home = TestHome::new("configuration-skill-conflict");
        fs::create_dir_all(home.home.join(".codex")).unwrap();
        fs::create_dir_all(home.home.join("Library/Application Support/Cursor")).unwrap();
        write_external_skill(
            &home.home.join(".cursor/skills"),
            "clash",
            "Private version",
        );
        write_external_skill(&home.home.join(".agents/skills"), "clash", "Shared version");
        let mut configuration = current_configuration("cursor").unwrap();
        configuration.skills_global_dir = Some("~/.agents/skills".into());

        let error = plan_update_agent_configuration(PlanUpdateAgentConfigurationRequest {
            agent_id: "cursor".into(),
            configuration,
        })
        .unwrap_err();

        assert!(
            error.starts_with("skill_path_migration_conflict:"),
            "{error}"
        );
        assert_eq!(
            crate::agents::load_agents()["cursor"]
                .skills
                .as_ref()
                .unwrap()
                .global_dir,
            "~/.cursor/skills"
        );
    }
}
