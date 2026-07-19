use super::planner::{finalize_plan_with, CredentialAction, LifecycleBinding};
use super::types::{
    AssetOperationKind, AssetOperationPlan, AssetRef, CentralAssetAction, CentralAssetChange,
    CentralAssetDraft, DomainPlan, PlanDeleteCentralAssetRequest, PlanUpdateCentralAssetRequest,
};
use crate::models::{model_agent_capability, validate_profile_draft};
use crate::paths::local_sources_dir;
use crate::registry::{read_registry, read_registry_all};
use crate::settings::{load_settings_strict, Settings};
use crate::types::{ModelProfile, RegistryEntry};
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
        profile: ModelProfile,
        credential: Option<Zeroizing<String>>,
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
        } => plan_model_upsert(existing_id, profile, credential),
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
    let plan = finalize_plan_with(
        AssetOperationKind::UpdateAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Mcp { key },
            action,
            summary: vec![
                "中央 MCP 配置与元数据".into(),
                format!("传播到 {consumer_count} 个 desired consumer"),
            ],
        }],
        vec![display_path(&local_sources_dir().join("manual.json"))],
        Some(lifecycle),
    )?;
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(
            plan.operation_id.clone(),
            PendingAssetPayload::McpUpsert {
                entry: Box::new(entry),
            },
        );
    Ok(plan)
}

fn plan_model_upsert(
    existing_id: Option<String>,
    profile: ModelProfile,
    credential: Option<String>,
) -> Result<AssetOperationPlan, String> {
    validate_profile_draft(&profile)?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let existing = settings
        .model_profiles
        .as_ref()
        .and_then(|profiles| profiles.get(&profile.id));
    let action = match existing_id {
        Some(existing_id) => {
            if existing_id != profile.id {
                return Err("asset_identity_change: Model Profile id cannot change".into());
            }
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

    let domain_plan = model_unchanged_consumers(&settings, &profile)?;
    let consumer_count = domain_agent_count(&domain_plan);
    let credential_action = match credential.as_deref() {
        None => CredentialAction::Keep,
        Some("") => CredentialAction::Clear,
        Some(_) => CredentialAction::Set,
    };
    let draft_hash = hash_serializable(&profile)?;
    let lifecycle = LifecycleBinding::ModelUpsert {
        profile_id: profile.id.clone(),
        draft_hash,
        credential_action,
    };
    let plan = finalize_plan_with(
        AssetOperationKind::UpdateAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Model {
                profile_id: profile.id.clone(),
            },
            action,
            summary: vec![
                "Profile metadata 与 Keychain credential presence".into(),
                format!("传播到 {consumer_count} 个 desired consumer"),
            ],
        }],
        Vec::new(),
        Some(lifecycle),
    )?;
    PENDING_PAYLOADS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(
            plan.operation_id.clone(),
            PendingAssetPayload::ModelUpsert {
                profile,
                credential: credential.map(Zeroizing::new),
            },
        );
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
    finalize_plan_with(
        AssetOperationKind::DeleteAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Mcp { key: key.clone() },
            action: CentralAssetAction::Delete,
            summary: vec![
                format!("删除 {source_id} source copy"),
                if fallback_exists {
                    format!("保留关系并将 fallback 传播到 {consumer_count} 个 consumer")
                } else {
                    format!("级联解除 {consumer_count} 个 desired consumer")
                },
            ],
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
    for (agent_id, assigned) in settings.model_assignments.iter().flatten() {
        if assigned == &profile_id {
            before.insert(agent_id.clone(), Some(profile_id.clone()));
            after.insert(agent_id.clone(), None);
        }
    }
    let domain_plan = DomainPlan::Model { before, after };
    let consumer_count = domain_agent_count(&domain_plan);
    finalize_plan_with(
        AssetOperationKind::DeleteAsset,
        domain_plan,
        vec![CentralAssetChange {
            asset: AssetRef::Model {
                profile_id: profile_id.clone(),
            },
            action: CentralAssetAction::Delete,
            summary: vec![
                "删除 Profile metadata 与 Keychain credential".into(),
                format!("级联解除 {consumer_count} 个 desired consumer"),
            ],
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
) -> Result<DomainPlan, String> {
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for (agent_id, assigned) in settings.model_assignments.iter().flatten() {
        if assigned != &profile.id {
            continue;
        }
        let capability = model_agent_capability(agent_id).ok_or_else(|| {
            format!("model_agent_unsupported: {agent_id} has no managed Model writer")
        })?;
        if capability.mode != "managed"
            || !capability.supported_protocols.contains(&profile.protocol)
        {
            return Err(format!(
                "model_protocol_unsupported: the edited Profile is incompatible with {agent_id}"
            ));
        }
        before.insert(agent_id.clone(), Some(profile.id.clone()));
        after.insert(agent_id.clone(), Some(profile.id.clone()));
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
    use crate::registry::write_manual_entry;
    use crate::settings::mutate_settings;
    use crate::testenv::TestHome;
    use crate::types::{ModelProtocol, RegistryConfig, StdioConfig};

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
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://old.invalid".into(),
            model: "old".into(),
            context_window: None,
            max_output_tokens: None,
            reasoning: false,
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
                profile: edited,
                credential: None,
            },
        })
        .unwrap();
        assert_eq!(plan.affected_agent_ids, vec!["codex"]);
        assert!(plan.relationship_changes.is_empty());
        assert_eq!(plan.kind, AssetOperationKind::UpdateAsset);
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
                    crate::consumption::McpConsumptionRecord {
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
}
