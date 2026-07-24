//! Central asset lifecycle planning.

use super::planner::{finalize_plan_with, CredentialAction, LifecycleBinding};
use super::types::{
    AssetCommitRequest, AssetOperationKind, AssetOperationPlan, AssetRef, CentralAssetAction,
    CentralAssetChange, CentralAssetDraft, DomainPlan, ModelAgentSelection, ModelConsumptionRecord,
    PlanDeleteCentralAssetRequest, PlanUpdateCentralAssetRequest,
};
use crate::domain::types::{ModelProfile, RegistryEntry};
use crate::paths::local_sources_dir;
use crate::resources::mcp::registry::{read_registry, read_registry_all};
use crate::resources::model::{
    credential_present, migrated_profiles_v2, model_agent_capability, prepare_profile_draft,
    profile_credential_issue,
};
use crate::settings::{load_settings_strict, Settings};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) enum PendingAssetPayload {
    McpUpsert {
        entry: Box<RegistryEntry>,
    },
    ModelUpsert {
        profile: Box<ModelProfile>,
        credential: Option<Zeroizing<String>>,
    },
    ModelSchemaV2 {
        profiles: BTreeMap<String, ModelProfile>,
    },
}

static PENDING_PAYLOADS: LazyLock<Mutex<BTreeMap<String, PendingAssetPayload>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

pub(crate) fn pending_payload(operation_id: &str) -> Option<PendingAssetPayload> {
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(operation_id)
        .cloned()
}

pub(crate) fn clear_pending_payload(operation_id: &str) {
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(operation_id);
}

pub(crate) fn store_pending_mcp_entry(operation_id: &str, entry: RegistryEntry) {
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(
            operation_id.to_string(),
            PendingAssetPayload::McpUpsert {
                entry: Box::new(entry),
            },
        );
}

pub(crate) fn store_pending_model_profile(
    operation_id: &str,
    profile: ModelProfile,
    credential: Option<String>,
) {
    store_pending_model_profile_secret(operation_id, profile, credential.map(Zeroizing::new));
}

pub(crate) fn store_pending_model_profile_secret(
    operation_id: &str,
    profile: ModelProfile,
    credential: Option<Zeroizing<String>>,
) {
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(
            operation_id.to_string(),
            PendingAssetPayload::ModelUpsert {
                profile: Box::new(profile),
                credential,
            },
        );
}

fn remap_model_selection(
    selection: ModelAgentSelection,
    id_map: &BTreeMap<String, String>,
) -> Result<ModelAgentSelection, String> {
    let profiles = selection
        .profiles
        .into_iter()
        .map(|(old_id, record)| {
            let new_id = id_map
                .get(&old_id)
                .ok_or_else(|| format!("model_schema_migration_missing_profile: {old_id}"))?
                .clone();
            Ok((
                new_id.clone(),
                ModelConsumptionRecord {
                    profile_id: new_id,
                    enabled: record.enabled,
                    last_selected_at: record.last_selected_at,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let active_profile_id = selection
        .active_profile_id
        .map(|old_id| {
            id_map
                .get(&old_id)
                .cloned()
                .ok_or_else(|| format!("model_schema_migration_missing_profile: {old_id}"))
        })
        .transpose()?;
    Ok(ModelAgentSelection {
        profiles,
        active_profile_id,
    })
}

pub fn plan_model_schema_v2_migration() -> Result<Option<AssetOperationPlan>, String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    if settings.version.unwrap_or_default() >= crate::settings::SETTINGS_VERSION {
        return Ok(None);
    }
    let (id_map, profiles) = migrated_profiles_v2(&settings)?;
    let agent_ids = settings
        .model_consumptions
        .iter()
        .flatten()
        .map(|(agent_id, _)| agent_id.clone())
        .chain(
            settings
                .model_assignments
                .iter()
                .flatten()
                .map(|(agent_id, _)| agent_id.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for agent_id in agent_ids {
        let selection = settings.model_selection(&agent_id);
        before.insert(agent_id.clone(), selection.clone());
        after.insert(agent_id, remap_model_selection(selection, &id_map)?);
    }
    let draft_hash = hash_serializable(&profiles)?;
    let credential_profile_ids = id_map
        .keys()
        .filter(|profile_id| credential_present(profile_id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let central_changes = id_map
        .iter()
        .map(|(old_id, new_id)| CentralAssetChange {
            asset: AssetRef::Model {
                profile_id: new_id.clone(),
            },
            action: CentralAssetAction::Update,
            summary: model_schema_migration_summary(
                before
                    .values()
                    .filter(|selection| selection.profiles.contains_key(old_id))
                    .count(),
                credential_profile_ids.contains(old_id),
            ),
        })
        .collect();
    let domain_plan = DomainPlan::Model { before, after };
    let plan = finalize_plan_with(
        AssetOperationKind::UpdateAsset,
        domain_plan,
        central_changes,
        Vec::new(),
        Some(LifecycleBinding::ModelSchemaV2 {
            id_map,
            draft_hash,
            credential_profile_ids,
        }),
    )?;
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(
            plan.operation_id.clone(),
            PendingAssetPayload::ModelSchemaV2 { profiles },
        );
    Ok(Some(plan))
}

pub fn migrate_model_profiles_v2_if_needed() -> Result<bool, String> {
    let Some(plan) = plan_model_schema_v2_migration()? else {
        return Ok(false);
    };
    if !plan.can_commit || plan.requires_conflict_confirmation {
        let _ = super::transaction::cancel_asset_operation(&plan.operation_id);
        return Err(
            "model_schema_migration_blocked: existing Model config requires manual review".into(),
        );
    }
    super::transaction::commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id.clone(),
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: None,
    })?;
    Ok(true)
}

pub fn plan_update_central_asset(
    request: PlanUpdateCentralAssetRequest,
) -> Result<AssetOperationPlan, String> {
    match request.draft {
        CentralAssetDraft::Mcp {
            existing_key,
            entry,
        } => plan_mcp_upsert(existing_key, *entry),
        CentralAssetDraft::Model {
            existing_id,
            profile,
            credential,
        } => plan_model_upsert(existing_id, *profile, credential),
    }
}

pub fn plan_delete_central_asset(
    request: PlanDeleteCentralAssetRequest,
) -> Result<AssetOperationPlan, String> {
    request
        .asset
        .validate()
        .map_err(|error| error.to_string())?;
    match request.asset {
        AssetRef::Mcp { key } => plan_mcp_delete(key, request.source_id),
        AssetRef::Model { profile_id } => {
            if request.source_id.is_some() {
                return Err(
                    "invalid_asset_source: Model assets do not have Registry sources".into(),
                );
            }
            plan_model_delete(profile_id)
        }
        AssetRef::Skill { .. } => Err(
            "unsupported_asset_lifecycle: Skills use the verified Skill update/remove transaction"
                .into(),
        ),
    }
}

fn plan_mcp_upsert(
    existing_key: Option<String>,
    mut entry: RegistryEntry,
) -> Result<AssetOperationPlan, String> {
    validate_mcp_entry(&entry)?;
    let key = entry.key();
    let effective = read_registry();
    let action = match existing_key {
        Some(existing_key) => {
            if existing_key != key {
                return Err(
                    "asset_identity_change: MCP name and transport cannot change during an edit; create a new asset instead"
                        .into(),
                );
            }
            let existing = effective
                .iter()
                .find(|candidate| candidate.key() == key)
                .ok_or_else(|| {
                    "asset_operation_stale: central MCP asset no longer exists".to_string()
                })?;
            if !is_user_owned(existing) {
                return Err(
                    "asset_read_only: source-owned MCP assets must be edited at their source"
                        .into(),
                );
            }
            CentralAssetAction::Update
        }
        None => {
            if effective.iter().any(|candidate| candidate.key() == key) {
                return Err("asset_identity_conflict: a central MCP with this name and transport already exists".into());
            }
            CentralAssetAction::Create
        }
    };
    // The central writer owns provenance. Keeping a stale discovered origin in
    // the pending payload would make review hashes depend on a field it replaces.
    entry.origin = None;

    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let domain_plan = mcp_unchanged_consumers(&settings, &key);
    let consumer_count = domain_agent_count(&domain_plan);
    let draft_hash = hash_serializable(&entry)?;
    let lifecycle = LifecycleBinding::McpUpsert {
        key: key.clone(),
        draft_hash,
    };
    let summary = central_upsert_summary("中央 MCP 配置", &action, consumer_count);
    let plan = finalize_plan_with(
        AssetOperationKind::UpdateAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Mcp { key },
            action,
            summary,
        }],
        vec![display_path(&local_sources_dir().join("manual.json"))],
        Some(lifecycle),
    )?;
    store_pending_mcp_entry(&plan.operation_id, entry);
    Ok(plan)
}

fn plan_model_upsert(
    existing_id: Option<String>,
    profile: ModelProfile,
    credential: Option<String>,
) -> Result<AssetOperationPlan, String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let profile = prepare_profile_draft(&settings, existing_id.as_deref(), profile)?;
    let existing = settings
        .model_profiles
        .as_ref()
        .and_then(|profiles| profiles.get(&profile.id));
    let action = match existing_id {
        Some(_) => {
            if existing.is_none() {
                return Err("asset_operation_stale: Model Profile no longer exists".into());
            }
            CentralAssetAction::Update
        }
        None => {
            if existing.is_some() {
                return Err(
                    "asset_identity_conflict: a Model Profile with this id already exists".into(),
                );
            }
            CentralAssetAction::Create
        }
    };

    let credential_action = match credential.as_deref() {
        None => CredentialAction::Keep,
        Some("") => CredentialAction::Clear,
        Some(_) => CredentialAction::Set,
    };
    let desired_credential_present = match credential_action {
        CredentialAction::Keep => credential_present(&profile.id),
        CredentialAction::Set => true,
        CredentialAction::Clear => false,
    };
    let domain_plan = model_unchanged_consumers(&settings, &profile, desired_credential_present)?;
    let consumer_count = domain_agent_count(&domain_plan);
    let draft_hash = hash_serializable(&profile)?;
    let lifecycle = LifecycleBinding::ModelUpsert {
        profile_id: profile.id.clone(),
        draft_hash,
        credential_action: credential_action.clone(),
    };
    let summary = model_upsert_summary(&action, &credential_action, consumer_count);
    let plan = finalize_plan_with(
        AssetOperationKind::UpdateAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Model {
                profile_id: profile.id.clone(),
            },
            action,
            summary,
        }],
        Vec::new(),
        Some(lifecycle),
    )?;
    store_pending_model_profile(&plan.operation_id, profile, credential);
    Ok(plan)
}

fn plan_mcp_delete(key: String, source_id: Option<String>) -> Result<AssetOperationPlan, String> {
    let source_id = source_id.ok_or_else(|| {
        "invalid_asset_source: deleting an MCP source copy requires source_id".to_string()
    })?;
    if !matches!(source_id.as_str(), "manual" | "discovered") {
        return Err(
            "asset_read_only: source-owned MCP assets must be removed from their source".into(),
        );
    }
    let copies: Vec<_> = read_registry_all()
        .into_iter()
        .filter(|item| item.entry.key() == key)
        .collect();
    let target = copies
        .iter()
        .find(|item| source_id_for(&item.entry) == source_id)
        .ok_or_else(|| {
            "asset_operation_stale: the reviewed MCP source copy no longer exists".to_string()
        })?;
    let effective_before = target.in_effect;
    let fallback_exists = effective_before
        && copies
            .iter()
            .any(|item| source_id_for(&item.entry) != source_id);
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let domain_plan = if effective_before {
        mcp_delete_consumers(&settings, &key, fallback_exists)
    } else {
        DomainPlan::Mcp {
            before: BTreeMap::new(),
            after: BTreeMap::new(),
        }
    };
    let consumer_count = domain_agent_count(&domain_plan);
    let summary = mcp_delete_summary(effective_before, fallback_exists, consumer_count);
    finalize_plan_with(
        AssetOperationKind::DeleteAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Mcp { key: key.clone() },
            action: CentralAssetAction::Delete,
            summary,
        }],
        vec![display_path(
            &local_sources_dir().join(format!("{source_id}.json")),
        )],
        Some(LifecycleBinding::McpDelete {
            key,
            source_id,
            fallback_exists,
            effective_before,
        }),
    )
}

fn plan_model_delete(profile_id: String) -> Result<AssetOperationPlan, String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    if !settings
        .model_profiles
        .as_ref()
        .is_some_and(|profiles| profiles.contains_key(&profile_id))
    {
        return Err("asset_operation_stale: Model Profile no longer exists".into());
    }
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    let agent_ids: BTreeSet<String> = settings
        .model_consumptions
        .iter()
        .flatten()
        .map(|(agent_id, _)| agent_id.clone())
        .chain(
            settings
                .model_assignments
                .iter()
                .flatten()
                .map(|(id, _)| id.clone()),
        )
        .collect();
    for agent_id in agent_ids {
        let existing = settings.model_selection(&agent_id);
        if !existing.profiles.contains_key(&profile_id) {
            continue;
        }
        let mut desired = existing.clone();
        desired.profiles.remove(&profile_id);
        desired.normalize_active();
        before.insert(agent_id.clone(), existing);
        after.insert(agent_id, desired);
    }
    let domain_plan = DomainPlan::Model { before, after };
    let consumer_count = domain_agent_count(&domain_plan);
    let summary = model_delete_summary(consumer_count);
    finalize_plan_with(
        AssetOperationKind::DeleteAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Model {
                profile_id: profile_id.clone(),
            },
            action: CentralAssetAction::Delete,
            summary,
        }],
        Vec::new(),
        Some(LifecycleBinding::ModelDelete { profile_id }),
    )
}

fn validate_mcp_entry(entry: &RegistryEntry) -> Result<(), String> {
    if entry.name.trim().is_empty() {
        return Err("invalid_asset: MCP name is required".into());
    }
    match (&entry.config.stdio, &entry.config.http) {
        (Some(config), None) if !config.command.trim().is_empty() => Ok(()),
        (None, Some(config))
            if !config.url.trim().is_empty()
                && (config.url.starts_with("https://") || config.url.starts_with("http://")) =>
        {
            Ok(())
        }
        (Some(_), Some(_)) => Err("invalid_asset: MCP must contain exactly one transport".into()),
        _ => Err("invalid_asset: MCP transport configuration is incomplete".into()),
    }
}

fn is_user_owned(entry: &RegistryEntry) -> bool {
    matches!(
        entry.origin.as_ref().map(|origin| origin.kind.as_str()),
        Some("manual" | "discovered")
    )
}

fn source_id_for(entry: &RegistryEntry) -> String {
    entry
        .origin
        .as_ref()
        .and_then(|origin| origin.source.clone())
        .or_else(|| entry.origin.as_ref().map(|origin| origin.kind.clone()))
        .unwrap_or_default()
}

fn mcp_unchanged_consumers(settings: &Settings, key: &str) -> DomainPlan {
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for (agent_id, records) in settings.mcp_consumptions.iter().flatten() {
        if records.contains_key(key) {
            let selection: Vec<String> = records.keys().cloned().collect();
            before.insert(agent_id.clone(), selection.clone());
            after.insert(agent_id.clone(), selection);
        }
    }
    DomainPlan::Mcp { before, after }
}

fn mcp_delete_consumers(settings: &Settings, key: &str, keep_relationships: bool) -> DomainPlan {
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for (agent_id, records) in settings.mcp_consumptions.iter().flatten() {
        if !records.contains_key(key) {
            continue;
        }
        let selection: Vec<String> = records.keys().cloned().collect();
        let desired = if keep_relationships {
            selection.clone()
        } else {
            selection
                .iter()
                .filter(|candidate| candidate.as_str() != key)
                .cloned()
                .collect()
        };
        before.insert(agent_id.clone(), selection);
        after.insert(agent_id.clone(), desired);
    }
    DomainPlan::Mcp { before, after }
}

fn model_unchanged_consumers(
    settings: &Settings,
    profile: &ModelProfile,
    desired_credential_present: bool,
) -> Result<DomainPlan, String> {
    let mut candidate_settings = settings.clone();
    candidate_settings
        .model_profiles
        .get_or_insert_default()
        .insert(profile.id.clone(), profile.clone());
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    let agent_ids: BTreeSet<String> = settings
        .model_consumptions
        .iter()
        .flatten()
        .map(|(agent_id, _)| agent_id.clone())
        .chain(
            settings
                .model_assignments
                .iter()
                .flatten()
                .map(|(id, _)| id.clone()),
        )
        .collect();
    for agent_id in agent_ids {
        let selection = settings.model_selection(&agent_id);
        if !selection.profiles.contains_key(&profile.id) {
            continue;
        }
        let capability = model_agent_capability(&agent_id).ok_or_else(|| {
            format!("model_agent_unsupported: {agent_id} has no managed Model writer")
        })?;
        if capability.mode != "managed"
            || !capability.supported_protocols.contains(&profile.protocol)
        {
            return Err(format!(
                "model_protocol_unsupported: the edited Profile is incompatible with {agent_id}"
            ));
        }
        if let Some((code, message)) =
            profile_credential_issue(&agent_id, profile, desired_credential_present)
        {
            return Err(format!("{code}: {message}"));
        }
        super::planner::validate_model_selection_contract(
            &candidate_settings,
            &agent_id,
            &selection,
        )?;
        before.insert(agent_id.clone(), selection.clone());
        after.insert(agent_id, selection);
    }
    Ok(DomainPlan::Model { before, after })
}

fn domain_agent_count(plan: &DomainPlan) -> usize {
    match plan {
        DomainPlan::Mcp { before, after } | DomainPlan::Skill { before, after } => before
            .keys()
            .chain(after.keys())
            .collect::<BTreeSet<_>>()
            .len(),
        DomainPlan::Model { before, after } => before
            .keys()
            .chain(after.keys())
            .collect::<BTreeSet<_>>()
            .len(),
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
            .map(String::as_str)
            .chain(affected_agent_ids.iter().map(String::as_str))
            .chain(std::iter::once(agent_id.as_str()))
            .collect::<BTreeSet<_>>()
            .len(),
    }
}

fn central_upsert_summary(
    central_label: &str,
    action: &CentralAssetAction,
    consumer_count: usize,
) -> Vec<String> {
    let verb = match action {
        CentralAssetAction::Create => "创建",
        CentralAssetAction::Update => "更新",
        CentralAssetAction::Delete => "删除",
    };
    let prefix = if consumer_count == 0 { "仅" } else { "" };
    vec![
        format!("{prefix}{verb}{central_label}"),
        agent_sync_summary(consumer_count),
    ]
}

fn model_upsert_summary(
    action: &CentralAssetAction,
    credential_action: &CredentialAction,
    consumer_count: usize,
) -> Vec<String> {
    let mut summary = central_upsert_summary("中央模型配置", action, consumer_count);
    if let Some(credential_summary) = match credential_action {
        CredentialAction::Keep => None,
        CredentialAction::Set if matches!(action, CentralAssetAction::Create) => {
            Some("将 API Key 保存到钥匙串")
        }
        CredentialAction::Set => Some("更新钥匙串中的 API Key"),
        CredentialAction::Clear => Some("清除钥匙串中的 API Key"),
    } {
        summary.insert(1, credential_summary.into());
    }
    summary
}

fn model_schema_migration_summary(consumer_count: usize, credential_present: bool) -> Vec<String> {
    let mut summary = vec!["升级模型配置".into()];
    if credential_present {
        summary.push("保留钥匙串中的 API Key".into());
    }
    summary.push(agent_sync_summary(consumer_count));
    summary
}

fn model_delete_summary(consumer_count: usize) -> Vec<String> {
    let mut summary = vec![
        "删除中央模型配置".into(),
        "同时清理钥匙串中的 API Key（如有）".into(),
    ];
    summary.push(if consumer_count == 0 {
        "当前没有已关联的 Agent，只删除中央配置".into()
    } else {
        format!("将从 {consumer_count} 个已关联 Agent 移除")
    });
    summary
}

fn mcp_delete_summary(
    effective_before: bool,
    fallback_exists: bool,
    consumer_count: usize,
) -> Vec<String> {
    if !effective_before {
        return vec![
            "删除这份 MCP 配置".into(),
            "当前生效的 MCP 配置与 Agent 关联不会改变".into(),
        ];
    }
    if fallback_exists {
        return vec![
            "删除当前使用的 MCP 配置".into(),
            "改用同名的其他 MCP 配置".into(),
            agent_sync_summary(consumer_count),
        ];
    }
    vec![
        "删除中央 MCP 配置".into(),
        if consumer_count == 0 {
            "当前没有已关联的 Agent，只删除中央配置".into()
        } else {
            format!("将从 {consumer_count} 个已关联 Agent 移除并解除关联")
        },
    ]
}

fn agent_sync_summary(consumer_count: usize) -> String {
    if consumer_count == 0 {
        "当前没有已关联的 Agent，无需同步".into()
    } else {
        format!("将同步到 {consumer_count} 个已关联 Agent")
    }
}

fn hash_serializable<T: serde::Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn display_path(path: &Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.display().to_string();
    };
    path.strip_prefix(home)
        .map(|relative| format!("~/{}", relative.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{ModelProtocol, RegistryConfig, StdioConfig};
    use crate::resources::mcp::registry::write_manual_entry;
    use crate::settings::mutate_settings;
    use crate::testenv::TestHome;

    fn mcp(command: &str) -> RegistryEntry {
        RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: command.into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        }
    }

    #[test]
    fn persisted_mcp_plan_contains_no_draft_values() {
        let home = TestHome::new("lifecycle-mcp-secret");
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: None,
                entry: Box::new(mcp("secret-command-value")),
            },
        })
        .unwrap();
        assert_eq!(
            plan.central_changes[0].summary,
            vec!["仅创建中央 MCP 配置", "当前没有已关联的 Agent，无需同步"]
        );
        let persisted = std::fs::read_to_string(
            home.home
                .join(".mux/staging/consumption")
                .join(plan.operation_id)
                .join("plan.json"),
        )
        .unwrap();
        assert!(!persisted.contains("secret-command-value"));
    }

    #[test]
    fn model_edit_keeps_every_consumer_in_the_plan() {
        let _home = TestHome::new("lifecycle-model-consumers");
        let profile = ModelProfile {
            id: "work".into(),
            name: "Work".into(),
            provider: "custom".into(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://old.invalid".into(),
            model: "old".into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        };
        mutate_settings(|settings| {
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(profile.id.clone(), profile.clone());
            settings
                .model_assignments
                .get_or_insert_default()
                .insert("codex".into(), profile.id.clone());
        })
        .unwrap();
        let mut edited = profile;
        edited.model = "new".into();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some("work".into()),
                profile: Box::new(edited),
                credential: None,
            },
        })
        .unwrap();
        assert_eq!(plan.affected_agent_ids, vec!["codex"]);
        assert!(plan.relationship_changes.is_empty());
        assert_eq!(plan.kind, AssetOperationKind::UpdateAsset);
        assert_eq!(
            plan.central_changes[0].summary,
            vec!["更新中央模型配置", "将同步到 1 个已关联 Agent"]
        );
    }

    #[test]
    fn model_review_copy_explains_credentials_and_zero_agent_scope() {
        assert_eq!(
            model_upsert_summary(&CentralAssetAction::Create, &CredentialAction::Set, 0),
            vec![
                "仅创建中央模型配置",
                "将 API Key 保存到钥匙串",
                "当前没有已关联的 Agent，无需同步",
            ]
        );
        assert_eq!(
            model_upsert_summary(&CentralAssetAction::Update, &CredentialAction::Clear, 2),
            vec![
                "更新中央模型配置",
                "清除钥匙串中的 API Key",
                "将同步到 2 个已关联 Agent",
            ]
        );
        assert_eq!(
            model_delete_summary(0),
            vec![
                "删除中央模型配置",
                "同时清理钥匙串中的 API Key（如有）",
                "当前没有已关联的 Agent，只删除中央配置",
            ]
        );
    }

    #[test]
    fn mcp_delete_review_copy_distinguishes_fallback_and_inactive_copies() {
        assert_eq!(
            mcp_delete_summary(true, true, 2),
            vec![
                "删除当前使用的 MCP 配置",
                "改用同名的其他 MCP 配置",
                "将同步到 2 个已关联 Agent",
            ]
        );
        assert_eq!(
            mcp_delete_summary(false, false, 0),
            vec![
                "删除这份 MCP 配置",
                "当前生效的 MCP 配置与 Agent 关联不会改变",
            ]
        );
        assert_eq!(
            mcp_delete_summary(true, false, 0),
            vec![
                "删除中央 MCP 配置",
                "当前没有已关联的 Agent，只删除中央配置",
            ]
        );
    }

    #[test]
    fn grok_model_edit_rejects_a_new_credential_without_env_key() {
        let _home = TestHome::new("lifecycle-grok-model-credential");
        let profile = ModelProfile {
            id: "work".into(),
            name: "Work".into(),
            provider: "openrouter".into(),
            model_vendor: Some("provider".into()),
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiCompletions,
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "provider/model".into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        };
        mutate_settings(|settings| {
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(profile.id.clone(), profile.clone());
            settings
                .model_assignments
                .get_or_insert_default()
                .insert("grok-build".into(), profile.id.clone());
        })
        .unwrap();

        let error = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some(profile.id.clone()),
                profile: Box::new(profile),
                credential: Some("test-credential".into()),
            },
        })
        .unwrap_err();

        assert!(error.starts_with("grok_build_env_key_required:"));
    }

    #[test]
    fn deleting_mcp_plans_relationship_cleanup() {
        let _home = TestHome::new("lifecycle-mcp-delete");
        write_manual_entry(&mcp("local-server")).unwrap();
        mutate_settings(|settings| {
            settings
                .mcp_consumptions
                .get_or_insert_default()
                .entry("claude-code".into())
                .or_default()
                .insert(
                    "local::stdio".into(),
                    crate::assets::McpConsumptionRecord {
                        asset_key: "local::stdio".into(),
                        enabled: true,
                        overrides: Default::default(),
                    },
                );
        })
        .unwrap();
        let plan = plan_delete_central_asset(PlanDeleteCentralAssetRequest {
            asset: AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            source_id: Some("manual".into()),
        })
        .unwrap();
        assert_eq!(plan.relationship_changes.len(), 1);
        assert_eq!(plan.kind, AssetOperationKind::DeleteAsset);
    }

    #[test]
    fn model_schema_v2_migrates_identity_metadata_and_keychain_atomically() {
        let _home = TestHome::new("model-schema-v2-central");
        let legacy = ModelProfile {
            id: "openrouter-free".into(),
            name: "OpenRouter Free".into(),
            provider: String::new(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiCompletions,
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "openrouter/free".into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        };
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings
                .extra
                .insert("future".into(), serde_json::json!({"keep": true}));
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(legacy.id.clone(), legacy.clone());
        })
        .unwrap();
        crate::resources::model::apply_credential_update(&legacy.id, Some("test-secret")).unwrap();

        assert!(migrate_model_profiles_v2_if_needed().unwrap());

        let settings = load_settings_strict().unwrap();
        assert_eq!(settings.version, Some(crate::settings::SETTINGS_VERSION));
        assert_eq!(settings.extra["future"]["keep"], true);
        let profiles = settings.model_profiles.unwrap();
        assert_eq!(profiles.len(), 1);
        let profile = profiles.values().next().unwrap();
        assert_ne!(profile.id, legacy.id);
        assert!(profile.id.starts_with("openrouter-openrouter-free-"));
        assert_eq!(profile.provider, "openrouter");
        assert_eq!(profile.model_vendor.as_deref(), Some("openrouter"));
        assert!(!crate::resources::model::credential_present(&legacy.id));
        assert!(crate::resources::model::credential_present(&profile.id));
    }

    #[test]
    fn model_schema_v2_rewrites_a_managed_agent_provider_identity() {
        let _home = TestHome::new("model-schema-v2-consumer");
        let legacy = ModelProfile {
            id: "legacy-router".into(),
            name: "Legacy Router".into(),
            provider: String::new(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiCompletions,
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "openrouter/free".into(),
            env_key: Some("OPENROUTER_API_KEY".into()),
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        };
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(legacy.id.clone(), legacy.clone());
        })
        .unwrap();
        crate::resources::model::apply_profile("grok-build", &legacy.id).unwrap();

        assert!(migrate_model_profiles_v2_if_needed().unwrap());

        let settings = load_settings_strict().unwrap();
        let profile = settings
            .model_profiles
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_ne!(profile.id, legacy.id);
        assert_eq!(
            settings
                .model_selection("grok-build")
                .active_profile_id
                .as_deref(),
            Some(profile.id.as_str())
        );
        assert_eq!(
            crate::resources::model::observe_profile("grok-build", profile).unwrap(),
            crate::resources::model::ModelObservedState::Synced
        );
    }
}
