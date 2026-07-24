//! Recoverable cross-domain asset transactions.

use super::inventory::list_consumption_inventory;
use super::lifecycle::{clear_pending_payload, pending_payload, PendingAssetPayload};
use super::planner::{
    hash_file, hash_mcp_catalog, hash_targets, load_operation, operation_root, CredentialAction,
    LifecycleBinding, PersistedAssetOperation, SkillMigrationEntry,
};
use super::types::{
    AssetCommitRequest, AssetOperationPlan, AssetRef, ConsumptionInventory, ConsumptionStatus,
    DomainPlan, McpConsumptionRecord, ModelAgentSelection, SkillConsumptionRecord,
};
use crate::paths::settings_file;
use crate::resources::mcp::ops;
use crate::resources::mcp::r#override::OverridePatch;
use crate::resources::mcp::registry::{
    delete_discovered_entry, delete_registry_entry, read_registry, read_registry_all,
    write_manual_entry,
};
use crate::resources::model::{
    apply_credential_update, apply_profile, apply_profile_consumption,
    apply_profile_consumption_with_credential_presence, clear_credential_rollback,
    clear_profile_consumption, credential_present, credential_rollback_snapshot,
    credential_snapshot, delete_profile_metadata, persist_credential_rollback,
    restore_credential_snapshot, save_profile,
};
use crate::resources::skill::{
    cancel_operation as cancel_skill_operation, commit_assignment, plan_assignment,
    PlanAssignmentRequest,
};
use crate::safe_write::{
    acquire_settings_lock, begin_transaction_write_tracking, load_transaction_write_states,
    record_transaction_removal, record_transaction_symlink, remove_bytes_if_unchanged,
    remove_symlink_if_unchanged, write_bytes_if_unchanged, write_symlink_if_unchanged,
    TransactionPathState,
};
use crate::settings::{load_settings_strict, mutate_settings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use zeroize::Zeroizing;

static COMMIT_LOCK: Mutex<()> = Mutex::new(());

pub fn commit_asset_operation(request: AssetCommitRequest) -> Result<ConsumptionInventory, String> {
    let _guard = COMMIT_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // Every cooperating MUX process already uses this lock for settings/source
    // mutations. Holding it across verify + apply closes the catalog and Agent
    // writer TOCTOU; nested settings mutations are reentrant on this thread.
    let _filesystem_guard = acquire_settings_lock(&settings_file())?;
    let persisted = load_operation(&request.operation_id)?;
    verify_request(&persisted, &request)?;
    if !persisted.plan.can_commit {
        return Err("asset_operation_blocked: resolve drift or conflict before commit".into());
    }
    if persisted.plan.requires_conflict_confirmation
        && request.conflict_confirmation.as_deref() != Some(persisted.plan.candidate_hash.as_str())
    {
        return Err(
            "confirmation_required: explicitly confirm replacement of the reviewed drifted targets"
                .into(),
        );
    }
    verify_preconditions(&persisted)?;

    let settings_path = settings_file();
    let target_paths = persisted
        .plan
        .target_files
        .iter()
        .map(|path| crate::resources::mcp::scanner::expand_tilde(path))
        .collect::<BTreeSet<_>>();
    let mut snapshots = vec![PathSnapshot::capture(&settings_path)?];
    let skill_link_targets = matches!(
        persisted.plan.domain_plan,
        DomainPlan::Skill { .. }
            | DomainPlan::AgentConfiguration { .. }
            | DomainPlan::AgentCapabilities { .. }
    );
    for path in target_paths {
        if path == settings_path {
            continue;
        }
        snapshots.push(if skill_link_targets {
            PathSnapshot::capture_link(&path)?
        } else {
            PathSnapshot::capture(&path)?
        });
    }
    let tracked_paths = snapshots
        .iter()
        .map(|snapshot| snapshot.path.clone())
        .collect::<Vec<_>>();
    let credential_backups = lifecycle_profile_ids(persisted.lifecycle.as_ref())
        .into_iter()
        .map(|profile_id| {
            let credential = credential_snapshot(&profile_id);
            (profile_id, credential)
        })
        .collect::<Vec<_>>();
    for (profile_id, credential) in &credential_backups {
        persist_credential_rollback(&request.operation_id, profile_id, credential.as_deref())?;
    }
    if let Err(error) = persist_rollback_snapshots(&request.operation_id, &snapshots) {
        for (profile_id, _) in &credential_backups {
            clear_credential_rollback(&request.operation_id, profile_id).map_err(|cleanup| {
                format!(
                    "failed to persist rollback snapshots ({error}); Keychain rollback cleanup failed: {cleanup}"
                )
            })?;
        }
        return Err(error);
    }

    let write_tracker = begin_transaction_write_tracking(
        &transaction_write_evidence_dir(&request.operation_id),
        &tracked_paths,
    )?;
    let applied = apply_operation(&persisted)
        .and_then(|_| verify_operation(&persisted))
        .and_then(|_| mark_operation_committed(&request.operation_id));
    if let Err(error) = applied {
        let written_states = write_tracker.states();
        drop(write_tracker);
        let mut rollback_errors = restore_snapshots_if_unchanged(&snapshots, &written_states);
        // Keep the file/settings and credential rollback domains together. If
        // file ownership cannot be proven, leave the current credential and its
        // durable backup untouched for startup recovery/manual resolution.
        if rollback_errors.is_empty() {
            for (profile_id, credential) in &credential_backups {
                if let Err(rollback) =
                    restore_credential_snapshot(profile_id, credential.as_deref())
                {
                    rollback_errors.push(format!("failed to restore Model credential: {rollback}"));
                }
            }
        }
        if rollback_errors.is_empty() {
            for (profile_id, _) in &credential_backups {
                if let Err(cleanup) = clear_credential_rollback(&request.operation_id, profile_id) {
                    rollback_errors.push(format!(
                        "failed to clear durable Model credential rollback: {cleanup}"
                    ));
                }
            }
        }
        if rollback_errors.is_empty() {
            if let Err(cleanup) = fs::remove_dir_all(operation_root(&request.operation_id)) {
                rollback_errors.push(format!("failed to clean rolled-back operation: {cleanup}"));
            } else {
                clear_pending_payload(&request.operation_id);
            }
        }
        if rollback_errors.is_empty() {
            return Err(format!(
                "asset operation failed and was rolled back: {error}"
            ));
        }
        return Err(format!(
            "asset operation failed ({error}); recovery required: {}",
            rollback_errors.join("; ")
        ));
    }
    drop(write_tracker);

    for (profile_id, _) in &credential_backups {
        clear_credential_rollback(&request.operation_id, profile_id).map_err(|error| {
            format!("asset operation committed but Keychain rollback cleanup failed: {error}")
        })?;
    }
    fs::remove_dir_all(operation_root(&request.operation_id)).map_err(|error| {
        format!("asset operation committed but staging cleanup failed: {error}")
    })?;
    clear_pending_payload(&request.operation_id);
    list_consumption_inventory()
}

/// Recover every operation that had begun mutating durable state. Reviewed but
/// uncommitted plans have no rollback manifest and are safely cancelled after a
/// restart because secret-bearing drafts intentionally live only in memory.
pub fn recover_pending_asset_operations() -> Result<Vec<String>, String> {
    let _guard = COMMIT_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _filesystem_guard = acquire_settings_lock(&settings_file())?;
    let root = crate::paths::mux_dir().join("staging/consumption");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("recovery_required: {error}")),
    };
    let mut recovered = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("recovery_required: {error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("recovery_required: {error}"))?
            .is_dir()
        {
            continue;
        }
        let operation_id = entry.file_name().to_string_lossy().into_owned();
        let persisted =
            load_operation(&operation_id).map_err(|error| format!("recovery_required: {error}"))?;
        let profile_ids = lifecycle_profile_ids(persisted.lifecycle.as_ref());
        let Some(snapshots) = load_rollback_snapshots(&operation_id)? else {
            for profile_id in &profile_ids {
                clear_credential_rollback(&operation_id, profile_id)
                    .map_err(|error| format!("recovery_required: {error}"))?;
            }
            fs::remove_dir_all(entry.path())
                .map_err(|error| format!("recovery_required: {error}"))?;
            clear_pending_payload(&operation_id);
            recovered.push(operation_id);
            continue;
        };

        if operation_commit_marker(&operation_id).is_file() {
            for profile_id in &profile_ids {
                clear_credential_rollback(&operation_id, profile_id)
                    .map_err(|error| format!("recovery_required: {error}"))?;
            }
            fs::remove_dir_all(entry.path())
                .map_err(|error| format!("recovery_required: {error}"))?;
            clear_pending_payload(&operation_id);
            recovered.push(operation_id);
            continue;
        }

        // A Model operation cannot prove that a same-presence credential was
        // replaced after a crash. Validate its Keychain rollback item before
        // touching files, then conservatively restore the complete transaction.
        let mut credential_backups = Vec::new();
        for profile_id in &profile_ids {
            let snapshot = credential_rollback_snapshot(&operation_id, profile_id)
                .map_err(|error| format!("recovery_required: {error}"))?
                .ok_or_else(|| {
                    "recovery_required: Model credential rollback item is missing".to_string()
                })?;
            credential_backups.push((profile_id.clone(), snapshot.map(Zeroizing::new)));
        }
        if !profile_ids.is_empty() || verify_operation(&persisted).is_err() {
            let written_states =
                load_transaction_write_states(&transaction_write_evidence_dir(&operation_id))?;
            let rollback_errors = restore_snapshots_if_unchanged(&snapshots, &written_states);
            if !rollback_errors.is_empty() {
                return Err(format!("recovery_required: {}", rollback_errors.join("; ")));
            }
            for (profile_id, credential) in &credential_backups {
                restore_credential_snapshot(
                    profile_id,
                    credential.as_ref().map(|value| value.as_slice()),
                )
                .map_err(|error| format!("recovery_required: {error}"))?;
            }
        }
        for profile_id in &profile_ids {
            clear_credential_rollback(&operation_id, profile_id)
                .map_err(|error| format!("recovery_required: {error}"))?;
        }
        fs::remove_dir_all(entry.path()).map_err(|error| format!("recovery_required: {error}"))?;
        clear_pending_payload(&operation_id);
        recovered.push(operation_id);
    }
    Ok(recovered)
}

pub(crate) fn pending_recovery_error() -> Option<String> {
    let root = crate::paths::mux_dir().join("staging/consumption");
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        if entry.path().join("rollback/manifest.json").is_file() {
            return Some("检测到未完成的中央资产事务；MUX 将保持只读，直到启动恢复成功。".into());
        }
    }
    None
}

pub fn cancel_asset_operation(operation_id: &str) -> Result<(), String> {
    let _guard = COMMIT_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _filesystem_guard = acquire_settings_lock(&settings_file())?;
    uuid::Uuid::parse_str(operation_id).map_err(|_| "invalid asset operation id".to_string())?;
    if !operation_root(operation_id).exists() {
        clear_pending_payload(operation_id);
        return Ok(());
    }
    let operation = load_operation(operation_id)?;
    if operation.plan.operation_id != operation_id {
        return Err("asset operation identity mismatch".into());
    }
    if operation_has_recovery_evidence(operation_id, &operation)? {
        return Err(
            "recovery_required: the asset operation has started committing; recover it before cancelling"
                .into(),
        );
    }
    match fs::remove_dir_all(operation_root(operation_id)) {
        Ok(()) => {
            clear_pending_payload(operation_id);
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            clear_pending_payload(operation_id);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn operation_has_recovery_evidence(
    operation_id: &str,
    operation: &PersistedAssetOperation,
) -> Result<bool, String> {
    for path in [
        operation_root(operation_id).join("rollback"),
        operation_commit_marker(operation_id),
    ] {
        match fs::symlink_metadata(&path) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => {
                return Err("recovery_required: failed to inspect asset operation evidence".into());
            }
        }
    }
    for profile_id in lifecycle_profile_ids(operation.lifecycle.as_ref()) {
        match credential_rollback_snapshot(operation_id, &profile_id) {
            Ok(Some(_)) => return Ok(true),
            Ok(None) => {}
            Err(_) => {
                return Err(
                    "recovery_required: failed to inspect Model credential rollback evidence"
                        .into(),
                );
            }
        }
    }
    Ok(false)
}

fn lifecycle_profile_ids(lifecycle: Option<&LifecycleBinding>) -> Vec<String> {
    match lifecycle {
        Some(LifecycleBinding::ModelUpsert { profile_id, .. })
        | Some(LifecycleBinding::ModelAdopt { profile_id, .. })
        | Some(LifecycleBinding::ModelDelete { profile_id }) => vec![profile_id.clone()],
        Some(LifecycleBinding::ModelSchemaV2 { id_map, .. }) => id_map
            .iter()
            .flat_map(|(old_id, new_id)| [old_id.clone(), new_id.clone()])
            .collect(),
        _ => Vec::new(),
    }
}

fn operation_commit_marker(operation_id: &str) -> PathBuf {
    operation_root(operation_id).join("commit-complete")
}

fn transaction_write_evidence_dir(operation_id: &str) -> PathBuf {
    operation_root(operation_id).join("rollback/post")
}

fn mark_operation_committed(operation_id: &str) -> Result<(), String> {
    write_private_file(&operation_commit_marker(operation_id), b"committed\n")
}

fn apply_operation(persisted: &PersistedAssetOperation) -> Result<(), String> {
    let Some(lifecycle) = &persisted.lifecycle else {
        return apply_domain_plan(
            &persisted.plan.domain_plan,
            persisted.plan.requires_conflict_confirmation,
        );
    };
    match lifecycle {
        LifecycleBinding::McpUpsert { key, draft_hash } => {
            let PendingAssetPayload::McpUpsert { entry } =
                require_pending_payload(&persisted.plan.operation_id)?
            else {
                return Err(
                    "asset_operation_expired: central MCP draft is unavailable; reopen the editor"
                        .into(),
                );
            };
            verify_payload_hash(&entry, draft_hash)?;
            if entry.key() != *key {
                return Err("asset_operation_stale: MCP draft identity changed".into());
            }
            write_manual_entry(&entry).map_err(|error| error.to_string())?;
            reapply_mcp_consumers(&persisted.plan.domain_plan, key)
        }
        LifecycleBinding::McpAdopt {
            key,
            draft_hash,
            enabled,
        } => {
            if let Some(draft_hash) = draft_hash {
                let PendingAssetPayload::McpUpsert { entry } =
                    require_pending_payload(&persisted.plan.operation_id)?
                else {
                    return Err(
                        "asset_operation_expired: imported MCP config is unavailable; reopen migration"
                            .into(),
                    );
                };
                verify_payload_hash(&entry, draft_hash)?;
                if entry.key() != *key {
                    return Err("asset_operation_stale: MCP migration identity changed".into());
                }
                write_manual_entry(&entry).map_err(|error| error.to_string())?;
            }
            apply_mcp_adoption(&persisted.plan.domain_plan, key, enabled)
        }
        LifecycleBinding::McpDelete {
            key,
            source_id,
            fallback_exists,
            effective_before,
        } => {
            delete_mcp_source_copy(key, source_id)?;
            if !effective_before {
                return Ok(());
            }
            if *fallback_exists {
                reapply_mcp_consumers(&persisted.plan.domain_plan, key)
            } else {
                apply_domain_plan(
                    &persisted.plan.domain_plan,
                    persisted.plan.requires_conflict_confirmation,
                )
            }
        }
        LifecycleBinding::McpEnabled {
            agent_id,
            asset_key,
            after,
            ..
        } => apply_mcp_enabled(agent_id, asset_key, *after),
        LifecycleBinding::SkillEnabled {
            name,
            target_id,
            affected_agent_ids,
            after,
            ..
        } => apply_skill_enabled(name, target_id, affected_agent_ids, *after),
        LifecycleBinding::McpReapply { key } => {
            reapply_mcp_consumers(&persisted.plan.domain_plan, key)
        }
        LifecycleBinding::ModelUpsert {
            profile_id,
            draft_hash,
            credential_action,
        } => {
            let PendingAssetPayload::ModelUpsert {
                profile,
                credential,
            } = require_pending_payload(&persisted.plan.operation_id)?
            else {
                return Err("asset_operation_expired: Model draft or credential is unavailable; reopen the editor".into());
            };
            verify_payload_hash(&profile, draft_hash)?;
            if profile.id != *profile_id || credential_action_for(&credential) != *credential_action
            {
                return Err(
                    "asset_operation_stale: Model draft no longer matches the reviewed plan".into(),
                );
            }
            let desired_credential_present = match credential_action {
                CredentialAction::Keep => credential_present(profile_id),
                CredentialAction::Set => true,
                CredentialAction::Clear => false,
            };
            save_profile(*profile, None)?;
            reapply_model_consumers(
                &persisted.plan.domain_plan,
                profile_id,
                desired_credential_present,
            )?;
            // Keychain mutation is deliberately last. A crash before this line
            // leaves the old credential intact and can roll files/settings back;
            // a crash after it has a fully verifiable committed state.
            apply_credential_update(profile_id, credential.as_deref().map(String::as_str))
        }
        LifecycleBinding::ModelAdopt {
            profile_id,
            draft_hash,
            credential_action,
        } => {
            let PendingAssetPayload::ModelUpsert {
                profile,
                credential,
            } = require_pending_payload(&persisted.plan.operation_id)?
            else {
                return Err("asset_operation_expired: Model adoption payload is unavailable; reopen migration review".into());
            };
            verify_payload_hash(&profile, draft_hash)?;
            if profile.id != *profile_id || credential_action_for(&credential) != *credential_action
            {
                return Err("asset_operation_stale: Model adoption payload changed".into());
            }
            let desired_credential_present = matches!(credential_action, CredentialAction::Set);
            save_profile(*profile, None)?;
            reapply_model_consumers(
                &persisted.plan.domain_plan,
                profile_id,
                desired_credential_present,
            )?;
            let DomainPlan::Model { after, .. } = &persisted.plan.domain_plan else {
                return Err("asset operation domain mismatch".into());
            };
            mutate_settings(|settings| {
                for (agent_id, selection) in after {
                    settings.set_model_selection(agent_id, selection.clone());
                }
            })
            .map_err(|error| error.to_string())?;
            apply_credential_update(profile_id, credential.as_deref().map(String::as_str))
        }
        LifecycleBinding::ModelDelete { profile_id } => {
            apply_domain_plan(
                &persisted.plan.domain_plan,
                persisted.plan.requires_conflict_confirmation,
            )?;
            delete_profile_metadata(profile_id)?;
            apply_credential_update(profile_id, Some(""))
        }
        LifecycleBinding::ModelSchemaV2 {
            id_map,
            draft_hash,
            credential_profile_ids,
        } => {
            let PendingAssetPayload::ModelSchemaV2 { profiles } =
                require_pending_payload(&persisted.plan.operation_id)?
            else {
                return Err(
                    "asset_operation_expired: Model migration payload is unavailable; restart MUX"
                        .into(),
                );
            };
            verify_payload_hash(&profiles, draft_hash)?;
            if profiles.keys().cloned().collect::<BTreeSet<_>>()
                != id_map.values().cloned().collect::<BTreeSet<_>>()
            {
                return Err("asset_operation_stale: Model migration identities changed".into());
            }
            mutate_settings(|settings| {
                settings
                    .model_profiles
                    .get_or_insert_default()
                    .extend(profiles.clone());
            })
            .map_err(|error| error.to_string())?;
            for (old_id, new_id) in id_map {
                if credential_profile_ids.contains(old_id) {
                    let credential = credential_snapshot(old_id).ok_or_else(|| {
                        format!("model_schema_migration_credential_missing: {old_id}")
                    })?;
                    restore_credential_snapshot(new_id, Some(&credential))?;
                }
            }
            apply_domain_plan(
                &persisted.plan.domain_plan,
                persisted.plan.requires_conflict_confirmation,
            )?;
            mutate_settings(|settings| {
                settings.model_profiles = Some(profiles.clone());
                settings.version = Some(crate::settings::SETTINGS_VERSION);
                if let DomainPlan::Model { after, .. } = &persisted.plan.domain_plan {
                    for (agent_id, selection) in after {
                        settings.set_model_selection(agent_id, selection.clone());
                    }
                }
            })
            .map_err(|error| error.to_string())?;
            for old_id in id_map.keys() {
                apply_credential_update(old_id, Some(""))?;
            }
            Ok(())
        }
        LifecycleBinding::AgentConfiguration {
            agent_id,
            after,
            skill_assignments_after,
            skill_migration,
        } => {
            for entry in skill_migration {
                if let Some(source) = &entry.source {
                    create_skill_migration_link(source, &entry.destination)?;
                }
            }
            crate::agents::apply_configuration(agent_id, after, skill_assignments_after.clone())
        }
        LifecycleBinding::AgentCapabilities {
            agent_id,
            after,
            skill_assignments_after,
            skill_migration,
        } => {
            for entry in skill_migration {
                if let Some(source) = &entry.source {
                    create_skill_migration_link(source, &entry.destination)?;
                }
            }
            crate::agents::apply_configuration_patch(
                agent_id,
                after,
                skill_assignments_after.clone(),
            )
        }
    }
}

fn verify_operation(persisted: &PersistedAssetOperation) -> Result<(), String> {
    verify_postcondition(&persisted.plan)?;
    let Some(lifecycle) = &persisted.lifecycle else {
        return Ok(());
    };
    match lifecycle {
        LifecycleBinding::McpUpsert { key, draft_hash } => {
            let entry = read_registry()
                .into_iter()
                .find(|entry| entry.key() == *key)
                .ok_or_else(|| "MCP central post-commit verification failed".to_string())?;
            // Provenance is normalized by the central writer and is not part of
            // the reviewed draft hash.
            let mut entry = entry;
            entry.origin = None;
            verify_payload_hash(&entry, draft_hash)
        }
        LifecycleBinding::McpAdopt {
            key,
            draft_hash,
            enabled,
        } => {
            if let Some(draft_hash) = draft_hash {
                let entry = read_registry()
                    .into_iter()
                    .find(|entry| entry.key() == *key)
                    .ok_or_else(|| "MCP central migration verification failed".to_string())?;
                let mut entry = entry;
                entry.origin = None;
                verify_payload_hash(&entry, draft_hash)?;
            }
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            for (agent_id, expected) in enabled {
                let actual = settings
                    .mcp_consumptions
                    .as_ref()
                    .and_then(|records| records.get(agent_id))
                    .and_then(|records| records.get(key))
                    .map(|record| record.enabled);
                if actual != Some(*expected) {
                    return Err("MCP migration enabled state verification failed".into());
                }
            }
            Ok(())
        }
        LifecycleBinding::McpDelete {
            key,
            source_id,
            fallback_exists,
            effective_before,
        } => {
            if mcp_source_copy_exists(key, source_id) {
                return Err("MCP source copy still exists after deletion".into());
            }
            let effective_exists = read_registry().iter().any(|entry| entry.key() == *key);
            let expected_effective = !*effective_before || *fallback_exists;
            if effective_exists != expected_effective {
                return Err("MCP fallback state did not match the reviewed deletion".into());
            }
            Ok(())
        }
        LifecycleBinding::McpEnabled {
            agent_id,
            asset_key,
            after,
            ..
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let desired = settings
                .mcp_consumptions
                .as_ref()
                .and_then(|records| records.get(agent_id))
                .and_then(|records| records.get(asset_key))
                .map(|record| record.enabled);
            let (name, transport) = split_mcp_key(asset_key)?;
            let observed = ops::scan_installed(None).into_iter().find(|item| {
                item.agent == *agent_id
                    && item.scope == "global"
                    && item.name == name
                    && item.transport == transport
            });
            if desired != Some(*after) || observed.is_none_or(|item| item.enabled != *after) {
                return Err("MCP enabled state did not match the reviewed change".into());
            }
            Ok(())
        }
        LifecycleBinding::SkillEnabled {
            name,
            target_id,
            affected_agent_ids,
            after,
            ..
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let desired = settings
                .skill_assignments
                .as_ref()
                .and_then(|assignments| assignments.get(name))
                .is_some_and(|targets| targets.contains(target_id));
            let enabled = settings
                .skill_consumptions
                .as_ref()
                .and_then(|skills| skills.get(name))
                .and_then(|targets| targets.get(target_id))
                .map(|record| record.enabled);
            if !desired || enabled != Some(*after) {
                return Err("Skill enabled state did not match the reviewed change".into());
            }
            let inventory = list_consumption_inventory()?;
            for agent_id in affected_agent_ids {
                let row = inventory.consumptions.iter().find(|item| {
                    item.agent_id == *agent_id
                        && item.asset == (AssetRef::Skill { name: name.clone() })
                        && item
                            .target
                            .as_ref()
                            .is_some_and(|target| target.target_id == *target_id)
                });
                if row.is_none_or(|item| {
                    !item.desired
                        || item.enabled != Some(*after)
                        || item.observed != *after
                        || item.status != ConsumptionStatus::Synced
                }) {
                    return Err("Skill physical state did not match the reviewed change".into());
                }
            }
            Ok(())
        }
        LifecycleBinding::McpReapply { .. } => Ok(()),
        LifecycleBinding::ModelUpsert {
            profile_id,
            draft_hash,
            credential_action,
        } => {
            let profile = load_settings_strict()
                .map_err(|error| error.to_string())?
                .model_profiles
                .and_then(|profiles| profiles.get(profile_id).cloned())
                .ok_or_else(|| "Model Profile missing after commit".to_string())?;
            verify_payload_hash(&profile, draft_hash)?;
            match credential_action {
                CredentialAction::Keep => {}
                CredentialAction::Set if !credential_present(profile_id) => {
                    return Err("Model credential was not saved after commit".into())
                }
                CredentialAction::Clear if credential_present(profile_id) => {
                    return Err("Model credential was not cleared after commit".into())
                }
                CredentialAction::Set | CredentialAction::Clear => {}
            }
            Ok(())
        }
        LifecycleBinding::ModelAdopt {
            profile_id,
            draft_hash,
            credential_action,
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let profile = settings
                .model_profiles
                .as_ref()
                .and_then(|profiles| profiles.get(profile_id))
                .ok_or_else(|| "Model Profile missing after adoption".to_string())?;
            verify_payload_hash(profile, draft_hash)?;
            if matches!(credential_action, CredentialAction::Set) != credential_present(profile_id)
            {
                return Err("Model credential presence did not match adoption plan".into());
            }
            Ok(())
        }
        LifecycleBinding::ModelDelete { profile_id } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            if settings
                .model_profiles
                .as_ref()
                .is_some_and(|profiles| profiles.contains_key(profile_id))
                || settings
                    .model_assignments
                    .as_ref()
                    .is_some_and(|assignments| assignments.values().any(|id| id == profile_id))
                || settings
                    .model_consumptions
                    .as_ref()
                    .is_some_and(|consumptions| {
                        consumptions
                            .values()
                            .any(|records| records.contains_key(profile_id))
                    })
            {
                return Err("Model Profile deletion postcondition failed".into());
            }
            if credential_present(profile_id) {
                return Err("Model credential still exists after deletion".into());
            }
            Ok(())
        }
        LifecycleBinding::ModelSchemaV2 {
            id_map,
            draft_hash,
            credential_profile_ids,
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            if settings.version != Some(crate::settings::SETTINGS_VERSION) {
                return Err("Model schema version was not updated".into());
            }
            let profiles = settings.model_profiles.unwrap_or_default();
            verify_payload_hash(&profiles, draft_hash)?;
            for (old_id, new_id) in id_map {
                if profiles.contains_key(old_id) || !profiles.contains_key(new_id) {
                    return Err("Model Profile identity migration postcondition failed".into());
                }
                if credential_present(old_id)
                    || credential_present(new_id) != credential_profile_ids.contains(old_id)
                {
                    return Err("Model credential migration postcondition failed".into());
                }
            }
            Ok(())
        }
        LifecycleBinding::AgentConfiguration {
            agent_id,
            after,
            skill_migration,
            ..
        } => {
            if crate::agents::current_configuration(agent_id)? != *after {
                return Err("Agent configuration postcondition failed".into());
            }
            for entry in skill_migration {
                let actual = skill_content_hash(&entry.destination)?;
                if actual.as_deref() != Some(entry.content_hash.as_str()) {
                    return Err("Skills path migration postcondition failed".into());
                }
            }
            Ok(())
        }
        LifecycleBinding::AgentCapabilities {
            agent_id,
            after,
            skill_migration,
            ..
        } => {
            if crate::agents::current_configuration_patch(agent_id)? != *after {
                return Err("Agent capability configuration postcondition failed".into());
            }
            for entry in skill_migration {
                let actual = skill_content_hash(&entry.destination)?;
                if actual.as_deref() != Some(entry.content_hash.as_str()) {
                    return Err("Skills path migration postcondition failed".into());
                }
            }
            Ok(())
        }
    }
}

fn require_pending_payload(operation_id: &str) -> Result<PendingAssetPayload, String> {
    pending_payload(operation_id).ok_or_else(|| {
        "asset_operation_expired: sensitive central draft was not persisted; reopen the editor"
            .to_string()
    })
}

fn verify_payload_hash<T: serde::Serialize>(value: &T, expected: &str) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let actual = hex::encode(Sha256::digest(bytes));
    if actual == expected {
        Ok(())
    } else {
        Err("asset_operation_stale: central draft changed after review".into())
    }
}

fn credential_action_for(credential: &Option<Zeroizing<String>>) -> CredentialAction {
    match credential.as_deref().map(String::as_str) {
        None => CredentialAction::Keep,
        Some("") => CredentialAction::Clear,
        Some(_) => CredentialAction::Set,
    }
}

fn delete_mcp_source_copy(key: &str, source_id: &str) -> Result<(), String> {
    let (name, transport) = split_mcp_key(key)?;
    match source_id {
        "manual" => delete_registry_entry(name, transport),
        "discovered" => delete_discovered_entry(name, transport),
        _ => return Err("asset_read_only: MCP source copy is not user-owned".into()),
    }
    .map_err(|error| error.to_string())
}

fn mcp_source_copy_exists(key: &str, source_id: &str) -> bool {
    read_registry_all().into_iter().any(|item| {
        item.entry.key() == key
            && item.entry.origin.as_ref().is_some_and(|origin| {
                origin.source.as_deref() == Some(source_id) || origin.kind == source_id
            })
    })
}

fn reapply_mcp_consumers(plan: &DomainPlan, key: &str) -> Result<(), String> {
    let DomainPlan::Mcp { after, .. } = plan else {
        return Err("asset operation domain mismatch".into());
    };
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let (name, transport) = split_mcp_key(key)?;
    for (agent_id, desired) in after {
        if !desired.iter().any(|candidate| candidate == key) {
            continue;
        }
        let patch = settings
            .mcp_consumptions
            .as_ref()
            .and_then(|records| records.get(agent_id))
            .and_then(|records| records.get(key))
            .map(|record| record.overrides.clone())
            .unwrap_or_default();
        ops::install(
            name,
            transport,
            "global",
            std::slice::from_ref(agent_id),
            None,
            &HashMap::from([(agent_id.clone(), patch)]),
        )
        .map_err(|errors| errors.join("; "))?;
        let enabled = settings
            .mcp_consumptions
            .as_ref()
            .and_then(|records| records.get(agent_id))
            .and_then(|records| records.get(key))
            .is_none_or(|record| record.enabled);
        if !enabled {
            // Refresh the remembered snapshot from the reviewed central
            // configuration, then return the Agent to its desired disabled
            // state instead of accidentally enabling it.
            ops::disable(
                name,
                transport,
                "global",
                std::slice::from_ref(agent_id),
                None,
            )
            .map_err(|errors| errors.join("; "))?;
        }
    }
    Ok(())
}

/// Adopt exact observed MCP copies without rewriting Agent files. The planner
/// already bound every target byte and verified that all copies match one
/// central config. Disabled observations remain in the existing snapshot store
/// and are recorded as disabled desired relationships.
fn apply_mcp_adoption(
    plan: &DomainPlan,
    key: &str,
    enabled: &BTreeMap<String, bool>,
) -> Result<(), String> {
    let DomainPlan::Mcp { after, .. } = plan else {
        return Err("asset operation domain mismatch".into());
    };
    mutate_settings(|settings| {
        let all = settings.mcp_consumptions.get_or_insert_default();
        for (agent_id, desired) in after {
            if !desired.iter().any(|candidate| candidate == key) {
                continue;
            }
            let records = all.entry(agent_id.clone()).or_default();
            let mut record = records.remove(key).unwrap_or(McpConsumptionRecord {
                asset_key: key.to_string(),
                enabled: true,
                overrides: OverridePatch::default(),
            });
            record.enabled = enabled.get(agent_id).copied().unwrap_or(true);
            records.insert(key.to_string(), record);
        }
    })
    .map_err(|error| error.to_string())
}

fn reapply_model_consumers(
    plan: &DomainPlan,
    profile_id: &str,
    credential_present: bool,
) -> Result<(), String> {
    let DomainPlan::Model { after, .. } = plan else {
        return Err("asset operation domain mismatch".into());
    };
    for (agent_id, desired) in after {
        if desired
            .profiles
            .get(profile_id)
            .is_some_and(|record| record.enabled)
        {
            apply_profile_consumption_with_credential_presence(
                agent_id,
                profile_id,
                credential_present,
                desired.active_profile_id.as_deref() == Some(profile_id),
            )?;
        }
    }
    Ok(())
}

fn verify_request(
    persisted: &PersistedAssetOperation,
    request: &AssetCommitRequest,
) -> Result<(), String> {
    if persisted.plan.operation_id != request.operation_id
        || persisted.plan.candidate_hash != request.candidate_hash
    {
        return Err("asset_operation_stale: confirmation does not match the reviewed plan".into());
    }
    Ok(())
}

fn verify_preconditions(persisted: &PersistedAssetOperation) -> Result<(), String> {
    if hash_file(&settings_file()) != persisted.settings_hash {
        return Err("asset_operation_stale: MUX settings changed after review".into());
    }
    if hash_targets(&persisted.plan.target_files) != persisted.target_hashes {
        return Err("asset_operation_stale: an Agent target changed after review".into());
    }
    if let Some(expected) = &persisted.mcp_catalog_hash {
        if &hash_mcp_catalog()? != expected {
            return Err("asset_operation_stale: central MCP catalog changed after review".into());
        }
    }
    match &persisted.lifecycle {
        Some(LifecycleBinding::AgentConfiguration {
            skill_migration, ..
        })
        | Some(LifecycleBinding::AgentCapabilities {
            skill_migration, ..
        }) => verify_skill_migration_preconditions(skill_migration)?,
        _ => {}
    }
    let current = load_settings_strict().map_err(|error| error.to_string())?;
    match &persisted.plan.domain_plan {
        DomainPlan::Mcp { before, .. } => {
            for (agent_id, expected) in before {
                let actual: Vec<String> = current
                    .mcp_consumptions
                    .as_ref()
                    .and_then(|records| records.get(agent_id))
                    .map(|records| records.keys().cloned().collect())
                    .unwrap_or_default();
                if &actual != expected {
                    return Err("asset_operation_stale: MCP relationships changed".into());
                }
            }
        }
        DomainPlan::Model { before, .. } => {
            for (agent_id, expected) in before {
                let actual = current.model_selection(agent_id);
                if &actual != expected {
                    return Err("asset_operation_stale: Model relationship changed".into());
                }
            }
        }
        DomainPlan::Skill { .. } => {
            // Physical link and assignment preconditions are rechecked by the
            // existing Skills planner for every step.
        }
        DomainPlan::AgentConfiguration {
            agent_id, before, ..
        } => {
            let actual = crate::agents::current_configuration(agent_id)?;
            if actual != **before {
                return Err("asset_operation_stale: Agent configuration changed".into());
            }
        }
        DomainPlan::AgentCapabilities {
            agent_id, before, ..
        } => {
            let actual = crate::agents::current_configuration_patch(agent_id)?;
            if actual != **before {
                return Err("asset_operation_stale: Agent capability configuration changed".into());
            }
        }
    }
    Ok(())
}

fn verify_skill_migration_preconditions(entries: &[SkillMigrationEntry]) -> Result<(), String> {
    for entry in entries {
        match &entry.source {
            Some(source) => {
                if skill_content_hash(source)?.as_deref() != Some(entry.content_hash.as_str()) {
                    return Err("asset_operation_stale: a source Skill changed after review".into());
                }
                if fs::symlink_metadata(expand_tilde_path(&entry.destination)).is_ok() {
                    return Err(
                        "asset_operation_stale: a Skills migration destination appeared after review"
                            .into(),
                    );
                }
            }
            None => {
                if skill_content_hash(&entry.destination)?.as_deref()
                    != Some(entry.content_hash.as_str())
                {
                    return Err(
                        "asset_operation_stale: a destination Skill changed after review".into(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn create_skill_migration_link(source: &str, destination: &str) -> Result<(), String> {
    let source = expand_tilde_path(source);
    let destination = expand_tilde_path(destination);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err("Skills migration destination already exists".into());
    }
    let source = fs::canonicalize(source)
        .map_err(|_| "Skills migration source is unavailable".to_string())?;
    if !source.is_dir() {
        return Err("Skills migration source is not a directory".into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&source, &destination).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&source, &destination).map_err(|error| error.to_string())?;
    record_transaction_symlink(&destination, &source)
}

fn skill_content_hash(path: &str) -> Result<Option<String>, String> {
    let path = expand_tilde_path(path);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
        Ok(_) => {}
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| "Skills migration path could not be resolved".to_string())?;
    if !canonical.is_dir() {
        return Err("Skills migration path is not a directory".into());
    }
    crate::resources::skill::hash_tree(&canonical)
        .map(Some)
        .map_err(|error| format!("{error:?}"))
}

fn expand_tilde_path(path: &str) -> PathBuf {
    crate::resources::mcp::scanner::expand_tilde(path)
}

fn apply_domain_plan(plan: &DomainPlan, replace_model_conflict: bool) -> Result<(), String> {
    match plan {
        DomainPlan::Mcp { before, after } => apply_mcp(before, after),
        DomainPlan::Model { before, after } => apply_model(before, after, replace_model_conflict),
        DomainPlan::Skill { before, after } => apply_skill(before, after),
        DomainPlan::AgentConfiguration { .. } | DomainPlan::AgentCapabilities { .. } => {
            Err("asset operation requires a configuration lifecycle".into())
        }
    }
}

fn apply_mcp(
    before: &BTreeMap<String, Vec<String>>,
    after: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let exact_observed: BTreeSet<(String, String)> = ops::scan_installed(None)
        .into_iter()
        .filter(|item| item.scope == "global" && item.enabled && !item.customized)
        .map(|item| (item.agent, format!("{}::{}", item.name, item.transport)))
        .collect();
    for agent_id in union_keys(before, after) {
        let left: BTreeSet<String> = before
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let right: BTreeSet<String> = after
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        for key in left.difference(&right) {
            let (name, transport) = split_mcp_key(key)?;
            ops::delete(
                name,
                transport,
                "global",
                std::slice::from_ref(agent_id),
                None,
            )
            .map_err(|errors| errors.join("; "))?;
        }
        for key in right.difference(&left) {
            // Adopting an exact external observation only records desired state;
            // rewriting identical Agent bytes would turn discovery into an
            // implicit mutation and needlessly disturb comments or formatting.
            if exact_observed.contains(&(agent_id.clone(), key.clone())) {
                continue;
            }
            let (name, transport) = split_mcp_key(key)?;
            let patch = settings
                .mcp_consumptions
                .as_ref()
                .and_then(|records| records.get(agent_id))
                .and_then(|records| records.get(key))
                .map(|record| record.overrides.clone())
                .unwrap_or_default();
            ops::install(
                name,
                transport,
                "global",
                std::slice::from_ref(agent_id),
                None,
                &HashMap::from([(agent_id.clone(), patch)]),
            )
            .map_err(|errors| errors.join("; "))?;
        }
    }

    mutate_settings(|settings| {
        let all = settings.mcp_consumptions.get_or_insert_default();
        for agent_id in union_keys(before, after) {
            let existing = all.remove(agent_id).unwrap_or_default();
            let mut desired = BTreeMap::new();
            for key in after.get(agent_id).into_iter().flatten() {
                desired.insert(
                    key.clone(),
                    existing.get(key).cloned().unwrap_or(McpConsumptionRecord {
                        asset_key: key.clone(),
                        enabled: true,
                        overrides: OverridePatch::default(),
                    }),
                );
            }
            if !desired.is_empty() {
                all.insert(agent_id.clone(), desired);
            }
        }
    })
    .map_err(|error| error.to_string())
}

fn apply_mcp_enabled(agent_id: &str, asset_key: &str, enabled: bool) -> Result<(), String> {
    let (name, transport) = split_mcp_key(asset_key)?;
    let agents = [agent_id.to_string()];
    if enabled {
        ops::enable(name, transport, "global", &agents, None)
    } else {
        ops::disable(name, transport, "global", &agents, None)
    }
    .map_err(|errors| errors.join("; "))?;

    let updated = mutate_settings(|settings| {
        let Some(record) = settings
            .mcp_consumptions
            .as_mut()
            .and_then(|records| records.get_mut(agent_id))
            .and_then(|records| records.get_mut(asset_key))
        else {
            return false;
        };
        record.enabled = enabled;
        true
    })
    .map_err(|error| error.to_string())?;
    if updated {
        Ok(())
    } else {
        Err("MCP consumption disappeared during enabled-state update".into())
    }
}

fn apply_skill_enabled(
    name: &str,
    target_id: &str,
    affected_agent_ids: &[String],
    enabled: bool,
) -> Result<(), String> {
    let plan = plan_assignment(PlanAssignmentRequest {
        skill_name: name.to_string(),
        agent_ids: affected_agent_ids.to_vec(),
        enabled,
    })
    .map_err(|error| format!("{error:?}"))?;
    if let Err(error) = commit_assignment(plan.confirmation()) {
        let _ = cancel_skill_operation(&plan.operation_id);
        return Err(format!("{error:?}"));
    }
    let central = crate::paths::mux_dir().join("skills").join(name);
    for target in &plan.targets {
        let target_path = expand_tilde_path(&target.global_dir).join(name);
        if enabled {
            record_transaction_symlink(&target_path, &central)?;
        } else {
            record_transaction_removal(&target_path)?;
        }
    }
    mutate_settings(|settings| {
        settings
            .skill_assignments
            .get_or_insert_default()
            .entry(name.to_string())
            .or_default()
            .insert(target_id.to_string());
        settings
            .skill_consumptions
            .get_or_insert_default()
            .entry(name.to_string())
            .or_default()
            .insert(
                target_id.to_string(),
                SkillConsumptionRecord {
                    name: name.to_string(),
                    target_id: target_id.to_string(),
                    enabled,
                },
            );
    })
    .map_err(|error| error.to_string())
}

fn apply_model(
    before: &BTreeMap<String, ModelAgentSelection>,
    after: &BTreeMap<String, ModelAgentSelection>,
    replace_conflict: bool,
) -> Result<(), String> {
    for agent_id in union_keys(before, after) {
        let left = before.get(agent_id).cloned().unwrap_or_default();
        let right = after.get(agent_id).cloned().unwrap_or_default();
        let removed_or_disabled: Vec<String> = left
            .profiles
            .iter()
            .filter(|(profile_id, record)| {
                record.enabled
                    && !right
                        .profiles
                        .get(*profile_id)
                        .is_some_and(|next| next.enabled)
            })
            .map(|(profile_id, _)| profile_id.clone())
            .collect();
        for profile_id in removed_or_disabled {
            if let Err(error) = clear_profile_consumption(
                agent_id,
                &profile_id,
                left.active_profile_id.as_deref() == Some(profile_id.as_str()),
            ) {
                let replaceable = replace_conflict
                    && right.active_profile_id.is_some()
                    && (error.starts_with("model_owned_fields_drift:")
                        || error.starts_with("model_target_conflicted:"));
                if !replaceable {
                    return Err(error);
                }
            }
        }
        for (profile_id, record) in &right.profiles {
            let was_enabled = left
                .profiles
                .get(profile_id)
                .is_some_and(|previous| previous.enabled);
            if record.enabled && (!was_enabled || replace_conflict) {
                apply_profile_consumption(
                    agent_id,
                    profile_id,
                    right.active_profile_id.as_deref() == Some(profile_id),
                )?;
            }
        }
        if left.active_profile_id != right.active_profile_id
            && right.active_profile_id.as_ref().is_some_and(|profile_id| {
                left.profiles
                    .get(profile_id)
                    .is_some_and(|record| record.enabled)
            })
        {
            let profile_id = right.active_profile_id.as_deref().expect("checked above");
            apply_profile(agent_id, profile_id)?;
        }
    }
    mutate_settings(|settings| {
        for agent_id in union_keys(before, after) {
            settings
                .set_model_selection(agent_id, after.get(agent_id).cloned().unwrap_or_default());
        }
    })
    .map_err(|error| error.to_string())
}

fn apply_skill(
    before: &BTreeMap<String, Vec<String>>,
    after: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    for agent_id in union_keys(before, after) {
        let left: BTreeSet<String> = before
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let right: BTreeSet<String> = after
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        for (name, enabled) in left
            .difference(&right)
            .map(|name| (name, false))
            .chain(right.difference(&left).map(|name| (name, true)))
        {
            let plan = plan_assignment(PlanAssignmentRequest {
                skill_name: name.clone(),
                agent_ids: vec![agent_id.clone()],
                enabled,
            })
            .map_err(|error| format!("{error:?}"))?;
            if let Err(error) = commit_assignment(plan.confirmation()) {
                let _ = cancel_skill_operation(&plan.operation_id);
                return Err(format!("{error:?}"));
            }
            let central = crate::paths::mux_dir().join("skills").join(name);
            for target in &plan.targets {
                let target_path = expand_tilde_path(&target.global_dir).join(name);
                if enabled {
                    record_transaction_symlink(&target_path, &central)?;
                } else {
                    record_transaction_removal(&target_path)?;
                }
            }
        }
    }
    Ok(())
}

fn verify_postcondition(plan: &AssetOperationPlan) -> Result<(), String> {
    let inventory = list_consumption_inventory()?;
    match &plan.domain_plan {
        DomainPlan::Mcp { after, .. } => {
            for (agent_id, expected) in after {
                verify_desired_many(&inventory, agent_id, expected, |asset| match asset {
                    AssetRef::Mcp { key } => Some(key.as_str()),
                    _ => None,
                })?;
            }
        }
        DomainPlan::Model { after, .. } => {
            for (agent_id, expected) in after {
                let actual: BTreeMap<_, _> = inventory
                    .consumptions
                    .iter()
                    .filter(|item| item.agent_id == *agent_id && item.desired)
                    .filter_map(|item| match &item.asset {
                        AssetRef::Model { profile_id } => Some((profile_id, item)),
                        _ => None,
                    })
                    .collect();
                if actual.len() != expected.profiles.len() {
                    return Err(format!(
                        "model post-commit verification failed: expected {} Profiles for {agent_id}, observed {}",
                        expected.profiles.len(),
                        actual.len()
                    ));
                }
                for (profile_id, record) in &expected.profiles {
                    let Some(item) = actual.get(profile_id) else {
                        return Err(format!(
                            "model post-commit verification failed: {profile_id} is missing for {agent_id}"
                        ));
                    };
                    let expected_active =
                        expected.active_profile_id.as_deref() == Some(profile_id.as_str());
                    let state_changed = plan.model_state_changes.iter().any(|change| {
                        change.agent_id == *agent_id && change.profile_id == *profile_id
                    });
                    let invalid = item.enabled != Some(record.enabled)
                        || (state_changed
                            && (item.status != ConsumptionStatus::Synced
                                || item.active != Some(expected_active)));
                    if invalid {
                        return Err(format!(
                            "model post-commit verification failed: {profile_id} for {agent_id} is {:?} with enabled {:?} and active {:?}",
                            item.status, item.enabled, item.active
                        ));
                    }
                }
            }
        }
        DomainPlan::Skill { after, .. } => {
            for (agent_id, expected) in after {
                verify_desired_many(&inventory, agent_id, expected, |asset| match asset {
                    AssetRef::Skill { name } => Some(name.as_str()),
                    _ => None,
                })?;
            }
        }
        DomainPlan::AgentConfiguration {
            agent_id,
            after,
            skills_after,
            ..
        } => {
            if crate::agents::current_configuration(agent_id)? != **after {
                return Err("Agent configuration post-commit verification failed".into());
            }
            for (affected_agent, expected) in skills_after {
                verify_desired_many(&inventory, affected_agent, expected, |asset| match asset {
                    AssetRef::Skill { name } => Some(name.as_str()),
                    _ => None,
                })?;
            }
        }
        DomainPlan::AgentCapabilities {
            agent_id,
            after,
            skills_after,
            ..
        } => {
            if crate::agents::current_configuration_patch(agent_id)? != **after {
                return Err(
                    "Agent capability configuration post-commit verification failed".into(),
                );
            }
            for (affected_agent, expected) in skills_after {
                verify_desired_many(&inventory, affected_agent, expected, |asset| match asset {
                    AssetRef::Skill { name } => Some(name.as_str()),
                    _ => None,
                })?;
            }
        }
    }

    for ((agent_id, asset), desired) in expected_effects(plan) {
        let consumption = inventory
            .consumptions
            .iter()
            .find(|item| item.agent_id == agent_id && item.asset == asset && item.desired);
        if desired {
            if !consumption.is_some_and(|item| item.status == ConsumptionStatus::Synced) {
                return Err("asset post-commit verification failed".into());
            }
        } else if consumption.is_some()
            || inventory
                .external
                .iter()
                .any(|item| external_remains_after_removal(&agent_id, &asset, item))
        {
            return Err("asset removal post-commit verification failed".into());
        }
    }
    Ok(())
}

fn verify_desired_many<'a, F>(
    inventory: &'a ConsumptionInventory,
    agent_id: &str,
    expected: &[String],
    identity: F,
) -> Result<(), String>
where
    F: Fn(&'a AssetRef) -> Option<&'a str>,
{
    let actual: BTreeSet<&str> = inventory
        .consumptions
        .iter()
        .filter(|item| item.agent_id == agent_id && item.desired)
        .filter_map(|item| identity(&item.asset))
        .collect();
    let expected: BTreeSet<&str> = expected.iter().map(String::as_str).collect();
    if actual != expected {
        return Err("asset post-commit verification failed".into());
    }
    Ok(())
}

fn expected_effects(plan: &AssetOperationPlan) -> BTreeMap<(String, AssetRef), bool> {
    let mut effects = BTreeMap::new();
    for change in &plan.relationship_changes {
        effects.insert(
            (change.agent_id.clone(), change.asset.clone()),
            asset_desired_after(&plan.domain_plan, &change.agent_id, &change.asset),
        );
    }
    let agents = union_plan_agents(&plan.domain_plan);
    for change in &plan.central_changes {
        for agent_id in &agents {
            effects.insert(
                (agent_id.clone(), change.asset.clone()),
                asset_desired_after(&plan.domain_plan, agent_id, &change.asset),
            );
        }
    }
    effects
}

fn union_plan_agents(plan: &DomainPlan) -> BTreeSet<String> {
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

fn external_remains_after_removal(
    agent_id: &str,
    asset: &AssetRef,
    external: &super::types::ConsumptionView,
) -> bool {
    if external.agent_id != agent_id {
        return false;
    }
    match (asset, &external.asset) {
        (AssetRef::Mcp { key }, AssetRef::Mcp { key: external_key }) => key == external_key,
        // Model writers clear the exact Profile-owned fields before desired
        // state is updated. The external projection is deliberately
        // identity-free, so it may describe an unrelated native model that the
        // clear correctly preserved and cannot prove that this Profile remains.
        (AssetRef::Model { .. }, AssetRef::Model { .. }) => false,
        (
            AssetRef::Skill { name },
            AssetRef::Skill {
                name: external_name,
            },
        ) => name == external_name,
        _ => false,
    }
}

fn split_mcp_key(key: &str) -> Result<(&str, &str), String> {
    key.rsplit_once("::")
        .filter(|(name, transport)| !name.is_empty() && matches!(*transport, "stdio" | "http"))
        .ok_or_else(|| format!("invalid MCP asset key: {key}"))
}

fn union_keys<'a, T>(
    left: &'a BTreeMap<String, T>,
    right: &'a BTreeMap<String, T>,
) -> BTreeSet<&'a String> {
    left.keys().chain(right.keys()).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SnapshotKind {
    Missing,
    File { bytes: Vec<u8>, mode: Option<u32> },
    Symlink { target: PathBuf },
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathSnapshot {
    path: PathBuf,
    kind: SnapshotKind,
}

#[derive(Debug, Serialize, Deserialize)]
struct DurableSnapshotManifest {
    snapshots: Vec<DurableSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DurableSnapshot {
    path: PathBuf,
    kind: DurableSnapshotKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum DurableSnapshotKind {
    Missing,
    File {
        backup: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<u32>,
    },
    Symlink {
        target: PathBuf,
    },
    Directory,
}

impl PathSnapshot {
    /// Ordinary settings/configuration targets must never be symlinks: their
    /// writers follow links, while a link-only snapshot cannot restore the
    /// destination content.
    fn capture(path: &Path) -> Result<Self, String> {
        let snapshot = Self::capture_any(path)?;
        match &snapshot.kind {
            SnapshotKind::Missing | SnapshotKind::File { .. } => Ok(snapshot),
            SnapshotKind::Symlink { .. } => Err(format!(
                "asset_target_unsafe: refusing to snapshot symlinked configuration target: {}",
                path.display()
            )),
            SnapshotKind::Directory => Err(format!(
                "asset_target_unsafe: refusing to snapshot directory transaction target: {}",
                path.display()
            )),
        }
    }

    /// Managed Skill assignment/migration destinations are the sole link
    /// targets supported by the outer transaction.
    fn capture_link(path: &Path) -> Result<Self, String> {
        let snapshot = Self::capture_any(path)?;
        match &snapshot.kind {
            SnapshotKind::Missing | SnapshotKind::Symlink { .. } => Ok(snapshot),
            SnapshotKind::File { .. } | SnapshotKind::Directory => Err(format!(
                "asset_target_unsafe: refusing non-link Skill transaction target: {}",
                path.display()
            )),
        }
    }

    fn capture_any(path: &Path) -> Result<Self, String> {
        let kind = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => SnapshotKind::Symlink {
                target: fs::read_link(path).map_err(|error| error.to_string())?,
            },
            Ok(metadata) if metadata.is_file() => {
                #[cfg(unix)]
                let mode = {
                    use std::os::unix::fs::PermissionsExt;
                    Some(metadata.permissions().mode())
                };
                #[cfg(not(unix))]
                let mode = None;
                SnapshotKind::File {
                    bytes: fs::read(path).map_err(|error| error.to_string())?,
                    mode,
                }
            }
            Ok(metadata) if metadata.is_dir() => SnapshotKind::Directory,
            Ok(_) => return Err(format!("unsupported target type: {}", path.display())),
            Err(error) if error.kind() == ErrorKind::NotFound => SnapshotKind::Missing,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self {
            path: path.to_path_buf(),
            kind,
        })
    }

    fn from_transaction_state(path: &Path, state: &TransactionPathState) -> Self {
        let kind = match state {
            TransactionPathState::Missing => SnapshotKind::Missing,
            TransactionPathState::File { bytes, mode } => SnapshotKind::File {
                bytes: bytes.clone(),
                mode: *mode,
            },
            TransactionPathState::Symlink { target } => SnapshotKind::Symlink {
                target: target.clone(),
            },
        };
        Self {
            path: path.to_path_buf(),
            kind,
        }
    }

    fn restore_if_owned(&self, expected: Option<&Self>) -> Result<(), String> {
        if !self.validate_owned(expected)? {
            return Ok(());
        }
        let expected = expected.expect("validated changed state has write evidence");
        match (&self.kind, &expected.kind) {
            (SnapshotKind::Missing, SnapshotKind::Missing) => Ok(()),
            (
                SnapshotKind::Missing,
                SnapshotKind::File {
                    bytes,
                    mode: expected_mode,
                },
            ) => remove_bytes_if_unchanged(&self.path, bytes, *expected_mode),
            (SnapshotKind::Missing, SnapshotKind::Symlink { target }) => {
                remove_symlink_if_unchanged(&self.path, target)
            }
            (
                SnapshotKind::File { bytes, mode },
                SnapshotKind::File {
                    bytes: expected,
                    mode: expected_mode,
                },
            ) => {
                write_bytes_if_unchanged(&self.path, Some((expected, *expected_mode)), bytes, *mode)
            }
            (SnapshotKind::File { bytes, mode }, SnapshotKind::Missing) => {
                write_bytes_if_unchanged(&self.path, None, bytes, *mode)
            }
            (SnapshotKind::Symlink { target }, SnapshotKind::Missing) => {
                write_symlink_if_unchanged(&self.path, None, target)
            }
            (
                SnapshotKind::Symlink { target },
                SnapshotKind::Symlink {
                    target: expected_target,
                },
            ) => write_symlink_if_unchanged(
                &self.path,
                Some(expected_target.as_path()),
                target.as_path(),
            ),
            (_, SnapshotKind::Directory) | (SnapshotKind::Directory, _) => Err(format!(
                "refusing to roll back directory transaction target: {}",
                self.path.display()
            )),
            _ => Err(format!(
                "refusing to roll back {}: target type changed",
                self.path.display()
            )),
        }
    }

    fn validate_owned(&self, expected: Option<&Self>) -> Result<bool, String> {
        let current = Self::capture_any(&self.path)?;
        if current.kind == self.kind {
            return Ok(false);
        }
        let expected = expected.ok_or_else(|| {
            format!(
                "refusing to roll back {}: no transaction write evidence matches the changed target",
                self.path.display()
            )
        })?;
        if self.path != expected.path || current.kind != expected.kind {
            return Err(format!(
                "refusing to roll back {}: target changed after MUX wrote it",
                self.path.display()
            ));
        }
        if matches!(&self.kind, SnapshotKind::Directory)
            || matches!(&expected.kind, SnapshotKind::Directory)
        {
            return Err(format!(
                "refusing to roll back directory transaction target: {}",
                self.path.display()
            ));
        }
        Ok(true)
    }
}

fn restore_snapshots_if_unchanged(
    snapshots: &[PathSnapshot],
    written_states: &BTreeMap<PathBuf, TransactionPathState>,
) -> Vec<String> {
    let expected = snapshots
        .iter()
        .map(|snapshot| {
            written_states
                .get(&snapshot.path)
                .map(|state| PathSnapshot::from_transaction_state(&snapshot.path, state))
        })
        .collect::<Vec<_>>();
    let preflight_errors = snapshots
        .iter()
        .zip(&expected)
        .filter_map(|(snapshot, expected)| snapshot.validate_owned(expected.as_ref()).err())
        .collect::<Vec<_>>();
    if !preflight_errors.is_empty() {
        return preflight_errors;
    }

    let mut errors = Vec::new();
    for index in (0..snapshots.len()).rev() {
        if let Err(error) = snapshots[index].restore_if_owned(expected[index].as_ref()) {
            errors.push(error);
        }
    }
    errors
}

fn persist_rollback_snapshots(
    operation_id: &str,
    snapshots: &[PathSnapshot],
) -> Result<(), String> {
    let root = operation_root(operation_id).join("rollback");
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    set_private_dir(&root)?;
    let mut durable = Vec::with_capacity(snapshots.len());
    for (index, snapshot) in snapshots.iter().enumerate() {
        let kind = match &snapshot.kind {
            SnapshotKind::Missing => DurableSnapshotKind::Missing,
            SnapshotKind::Directory => DurableSnapshotKind::Directory,
            SnapshotKind::Symlink { target } => DurableSnapshotKind::Symlink {
                target: target.clone(),
            },
            SnapshotKind::File { bytes, mode } => {
                let backup = format!("{index}.bin");
                let path = root.join(&backup);
                write_private_file(&path, bytes)?;
                DurableSnapshotKind::File {
                    backup,
                    mode: *mode,
                }
            }
        };
        durable.push(DurableSnapshot {
            path: snapshot.path.clone(),
            kind,
        });
    }
    let manifest = serde_json::to_vec_pretty(&DurableSnapshotManifest { snapshots: durable })
        .map_err(|error| error.to_string())?;
    // Manifest is written last: its presence proves every referenced backup is
    // durable before the first target mutation begins.
    write_private_file(&root.join("manifest.json"), &manifest)
}

fn load_rollback_snapshots(operation_id: &str) -> Result<Option<Vec<PathSnapshot>>, String> {
    let root = operation_root(operation_id).join("rollback");
    let manifest = match fs::read(root.join("manifest.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("recovery_required: {error}")),
    };
    let manifest: DurableSnapshotManifest = serde_json::from_slice(&manifest)
        .map_err(|_| "recovery_required: invalid asset rollback manifest".to_string())?;
    let mut snapshots = Vec::with_capacity(manifest.snapshots.len());
    for snapshot in manifest.snapshots {
        let kind = match snapshot.kind {
            DurableSnapshotKind::Missing => SnapshotKind::Missing,
            DurableSnapshotKind::Directory => SnapshotKind::Directory,
            DurableSnapshotKind::Symlink { target } => SnapshotKind::Symlink { target },
            DurableSnapshotKind::File { backup, mode } => SnapshotKind::File {
                bytes: fs::read(root.join(backup))
                    .map_err(|error| format!("recovery_required: {error}"))?,
                mode,
            },
        };
        snapshots.push(PathSnapshot {
            path: snapshot.path,
            kind,
        });
    }
    Ok(Some(snapshots))
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn set_private_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{
        plan_set_agent_consumption, plan_set_mcp_enabled, plan_update_central_asset,
        AgentConsumptionSelection, CentralAssetDraft, PlanSetAgentConsumptionRequest,
        PlanSetMcpEnabledRequest, PlanUpdateCentralAssetRequest,
    };
    use crate::domain::types::{
        ModelProfile, ModelProtocol, RegistryConfig, RegistryEntry, StdioConfig,
    };
    use crate::resources::mcp::registry::write_manual_entry;
    use crate::resources::model::save_profile;
    use crate::testenv::TestHome;

    fn model(model: &str) -> ModelProfile {
        ModelProfile {
            id: "work".into(),
            name: "Work".into(),
            provider: "custom".into(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://example.invalid/v1".into(),
            model: model.into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        }
    }

    #[test]
    fn transaction_snapshot_refuses_a_directory_without_deleting_it() {
        let home = TestHome::new("transaction-directory-snapshot");
        let directory = home.home.join("must-remain");
        fs::create_dir_all(&directory).unwrap();
        let sentinel = directory.join("sentinel.txt");
        fs::write(&sentinel, "keep").unwrap();

        let error = PathSnapshot::capture(&directory).unwrap_err();

        assert!(error.contains("refusing to snapshot directory"), "{error}");
        assert_eq!(fs::read_to_string(sentinel).unwrap(), "keep");
    }

    #[cfg(unix)]
    #[test]
    fn transaction_snapshot_refuses_a_symlinked_configuration() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("transaction-config-symlink");
        let destination = home.home.join("settings-target.json");
        let link = home.home.join("settings.json");
        fs::write(&destination, "must remain untouched").unwrap();
        symlink(&destination, &link).unwrap();

        let error = PathSnapshot::capture(&link).unwrap_err();

        assert!(error.contains("symlinked configuration"), "{error}");
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            "must remain untouched"
        );
    }

    #[test]
    fn rollback_preserves_an_external_edit_before_rollback_preparation() {
        let home = TestHome::new("transaction-rollback-cas");
        let target = home.home.join("config.json");
        fs::write(&target, "original").unwrap();
        let original = PathSnapshot::capture(&target).unwrap();
        let evidence = home.home.join("write-evidence");
        let tracker =
            begin_transaction_write_tracking(&evidence, std::slice::from_ref(&target)).unwrap();
        crate::safe_write::write_if_unchanged(&target, Some("original"), "mux-partial").unwrap();
        fs::write(&target, "external-edit").unwrap();
        let states = tracker.states();
        drop(tracker);
        let expected = states
            .get(&target)
            .map(|state| PathSnapshot::from_transaction_state(&target, state));

        let error = original.restore_if_owned(expected.as_ref()).unwrap_err();

        assert!(error.contains("changed after MUX wrote it"), "{error}");
        assert_eq!(fs::read_to_string(target).unwrap(), "external-edit");
    }

    #[test]
    fn rollback_restores_a_file_when_the_cas_state_matches() {
        let home = TestHome::new("transaction-rollback-success");
        let target = home.home.join("config.json");
        fs::write(&target, "original").unwrap();
        let original = PathSnapshot::capture(&target).unwrap();
        let evidence = home.home.join("write-evidence");
        let tracker =
            begin_transaction_write_tracking(&evidence, std::slice::from_ref(&target)).unwrap();
        crate::safe_write::write_if_unchanged(&target, Some("original"), "mux-partial").unwrap();
        let states = tracker.states();
        drop(tracker);
        let expected = states
            .get(&target)
            .map(|state| PathSnapshot::from_transaction_state(&target, state));

        original.restore_if_owned(expected.as_ref()).unwrap();

        assert_eq!(fs::read_to_string(target).unwrap(), "original");
    }

    #[test]
    fn rollback_preflights_every_path_before_restoring_any_file() {
        let home = TestHome::new("transaction-rollback-preflight");
        let externally_edited = home.home.join("first.json");
        let still_owned = home.home.join("second.json");
        fs::write(&externally_edited, "first-original").unwrap();
        fs::write(&still_owned, "second-original").unwrap();
        let snapshots = vec![
            PathSnapshot::capture(&externally_edited).unwrap(),
            PathSnapshot::capture(&still_owned).unwrap(),
        ];
        let tracked_paths = vec![externally_edited.clone(), still_owned.clone()];
        let tracker =
            begin_transaction_write_tracking(&home.home.join("write-evidence"), &tracked_paths)
                .unwrap();
        crate::safe_write::write_if_unchanged(
            &externally_edited,
            Some("first-original"),
            "first-mux",
        )
        .unwrap();
        crate::safe_write::write_if_unchanged(&still_owned, Some("second-original"), "second-mux")
            .unwrap();
        fs::write(&externally_edited, "external-edit").unwrap();
        let states = tracker.states();
        drop(tracker);

        let errors = restore_snapshots_if_unchanged(&snapshots, &states);

        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            fs::read_to_string(externally_edited).unwrap(),
            "external-edit"
        );
        assert_eq!(fs::read_to_string(still_owned).unwrap(), "second-mux");
    }

    #[test]
    fn mcp_commit_updates_target_and_relationship_together() {
        let _home = TestHome::new("consume-commit");
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
        let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();
        let inventory = commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
        })
        .unwrap();
        assert!(inventory.consumptions.iter().any(|item| {
            item.agent_id == "claude-code"
                && item.asset
                    == (AssetRef::Mcp {
                        key: "local::stdio".into(),
                    })
                && item.status == ConsumptionStatus::Synced
        }));
    }

    #[test]
    fn mcp_enabled_toggle_preserves_relationship_and_restores_snapshot() {
        let _home = TestHome::new("consume-enabled-toggle");
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "local-server".into(),
                    args: Some(vec!["--keep-me".into()]),
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        let added = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();
        commit_asset_operation(AssetCommitRequest {
            operation_id: added.operation_id,
            candidate_hash: added.candidate_hash,
            conflict_confirmation: None,
        })
        .unwrap();

        for enabled in [false, true] {
            let plan = plan_set_mcp_enabled(PlanSetMcpEnabledRequest {
                agent_id: "claude-code".into(),
                asset_key: "local::stdio".into(),
                enabled,
            })
            .unwrap();
            let inventory = commit_asset_operation(AssetCommitRequest {
                operation_id: plan.operation_id,
                candidate_hash: plan.candidate_hash,
                conflict_confirmation: None,
            })
            .unwrap();
            assert!(inventory.consumptions.iter().any(|item| {
                item.agent_id == "claude-code"
                    && item.asset
                        == (AssetRef::Mcp {
                            key: "local::stdio".into(),
                        })
                    && item.desired
                    && item.enabled == Some(enabled)
                    && item.status == ConsumptionStatus::Synced
            }));
        }
    }

    #[test]
    fn startup_recovery_rolls_back_a_partial_asset_operation() {
        let home = TestHome::new("consume-recovery-rollback");
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
        let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();
        let target = home.home.join(".claude.json");
        let settings_before = fs::read(settings_file()).unwrap();
        let snapshots = vec![
            PathSnapshot::capture(&settings_file()).unwrap(),
            PathSnapshot::capture(&target).unwrap(),
        ];
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        let tracked_paths = snapshots
            .iter()
            .map(|snapshot| snapshot.path.clone())
            .collect::<Vec<_>>();
        let tracker = begin_transaction_write_tracking(
            &transaction_write_evidence_dir(&plan.operation_id),
            &tracked_paths,
        )
        .unwrap();
        crate::safe_write::write_if_unchanged(&target, None, "partial mutation").unwrap();
        mutate_settings(|settings| {
            settings.mcp_consumptions.get_or_insert_default().insert(
                "claude-code".into(),
                BTreeMap::from([(
                    "local::stdio".into(),
                    McpConsumptionRecord {
                        asset_key: "local::stdio".into(),
                        enabled: true,
                        overrides: Default::default(),
                    },
                )]),
            );
        })
        .unwrap();
        drop(tracker);

        let recovered = recover_pending_asset_operations().unwrap();
        assert_eq!(recovered, vec![plan.operation_id]);
        assert_eq!(fs::read(settings_file()).unwrap(), settings_before);
        assert!(!target.exists());
    }

    #[test]
    fn cancellation_preserves_started_transaction_recovery_evidence() {
        let _home = TestHome::new("consume-cancel-recovery-evidence");
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: None,
                entry: Box::new(RegistryEntry {
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
                }),
            },
        })
        .unwrap();
        let settings_path = settings_file();
        let snapshots = vec![PathSnapshot::capture(&settings_path).unwrap()];
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        let tracker = begin_transaction_write_tracking(
            &transaction_write_evidence_dir(&plan.operation_id),
            std::slice::from_ref(&settings_path),
        )
        .unwrap();
        mutate_settings(|settings| settings.imported = Some("partial".into())).unwrap();
        drop(tracker);

        let error = cancel_asset_operation(&plan.operation_id).unwrap_err();

        assert!(error.starts_with("recovery_required:"), "{error}");
        let root = operation_root(&plan.operation_id);
        assert!(root.join("rollback/manifest.json").is_file());
        assert!(
            fs::read_dir(root.join("rollback/post"))
                .unwrap()
                .next()
                .is_some(),
            "post-write ownership evidence must remain available"
        );
        assert!(pending_recovery_error().is_some());

        assert_eq!(
            recover_pending_asset_operations().unwrap(),
            vec![plan.operation_id]
        );
        assert!(!settings_path.exists());
    }

    #[test]
    fn startup_recovery_finalizes_an_already_verified_commit() {
        let home = TestHome::new("consume-recovery-finalize");
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
        let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();
        let persisted = load_operation(&plan.operation_id).unwrap();
        let target = home.home.join(".claude.json");
        let snapshots = vec![
            PathSnapshot::capture(&settings_file()).unwrap(),
            PathSnapshot::capture(&target).unwrap(),
        ];
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        apply_operation(&persisted).unwrap();
        verify_operation(&persisted).unwrap();

        recover_pending_asset_operations().unwrap();
        assert!(target.exists());
        assert!(
            load_settings_strict().unwrap().mcp_consumptions.unwrap()["claude-code"]
                .contains_key("local::stdio")
        );
        assert!(!operation_root(&plan.operation_id).exists());
    }

    #[test]
    fn model_recovery_restores_the_old_keychain_value_before_commit_marker() {
        let _home = TestHome::new("consume-model-keychain-rollback");
        save_profile(model("old-model"), Some("old-secret".into())).unwrap();
        let settings_before = fs::read(settings_file()).unwrap();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some("work".into()),
                profile: Box::new(model("new-model")),
                credential: Some("new-secret".into()),
            },
        })
        .unwrap();
        let persisted = load_operation(&plan.operation_id).unwrap();
        let snapshots = vec![PathSnapshot::capture(&settings_file()).unwrap()];
        let old_credential = credential_snapshot("work");
        persist_credential_rollback(&plan.operation_id, "work", old_credential.as_deref()).unwrap();
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        let tracked_paths = snapshots
            .iter()
            .map(|snapshot| snapshot.path.clone())
            .collect::<Vec<_>>();
        let tracker = begin_transaction_write_tracking(
            &transaction_write_evidence_dir(&plan.operation_id),
            &tracked_paths,
        )
        .unwrap();
        apply_operation(&persisted).unwrap();
        drop(tracker);
        assert_eq!(credential_snapshot("work").unwrap(), b"new-secret");

        recover_pending_asset_operations().unwrap();
        assert_eq!(fs::read(settings_file()).unwrap(), settings_before);
        assert_eq!(credential_snapshot("work").unwrap(), b"old-secret");
        assert!(!operation_root(&plan.operation_id).exists());
    }

    #[test]
    fn model_recovery_finalizes_only_after_the_durable_commit_marker() {
        let _home = TestHome::new("consume-model-keychain-finalize");
        save_profile(model("old-model"), Some("old-secret".into())).unwrap();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some("work".into()),
                profile: Box::new(model("new-model")),
                credential: Some("new-secret".into()),
            },
        })
        .unwrap();
        let persisted = load_operation(&plan.operation_id).unwrap();
        let snapshots = vec![PathSnapshot::capture(&settings_file()).unwrap()];
        let old_credential = credential_snapshot("work");
        persist_credential_rollback(&plan.operation_id, "work", old_credential.as_deref()).unwrap();
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        apply_operation(&persisted).unwrap();
        verify_operation(&persisted).unwrap();
        mark_operation_committed(&plan.operation_id).unwrap();

        recover_pending_asset_operations().unwrap();
        assert_eq!(credential_snapshot("work").unwrap(), b"new-secret");
        assert_eq!(
            load_settings_strict().unwrap().model_profiles.unwrap()["work"].model,
            "new-model"
        );
        assert!(!operation_root(&plan.operation_id).exists());
        assert!(credential_rollback_snapshot(&plan.operation_id, "work")
            .unwrap()
            .is_none());
    }
}
