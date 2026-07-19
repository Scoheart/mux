use super::inventory::list_consumption_inventory;
use super::lifecycle::{clear_pending_payload, pending_payload, PendingAssetPayload};
use super::planner::{
    hash_file, hash_targets, load_operation, operation_root, CredentialAction, LifecycleBinding,
    PersistedAssetOperation, SkillMigrationEntry,
};
use super::types::{
    AssetCommitRequest, AssetOperationPlan, AssetRef, ConsumptionInventory, ConsumptionStatus,
    DomainPlan, McpConsumptionRecord,
};
use crate::models::{
    apply_credential_update, apply_profile, apply_profile_with_credential_presence,
    clear_credential_rollback, clear_profile, credential_present, credential_rollback_snapshot,
    credential_snapshot, delete_profile_metadata, persist_credential_rollback,
    restore_credential_snapshot, save_profile,
};
use crate::ops;
use crate::paths::settings_file;
use crate::r#override::OverridePatch;
use crate::registry::{
    delete_discovered_entry, delete_registry_entry, read_registry, read_registry_all,
    write_manual_entry,
};
use crate::settings::{load_settings_strict, mutate_settings};
use crate::skills::{
    cancel_operation as cancel_skill_operation, commit_assignment, plan_assignment,
    PlanAssignmentRequest,
};
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

    let mut snapshots = vec![PathSnapshot::capture(&settings_file())?];
    snapshots.extend(
        persisted
            .plan
            .target_files
            .iter()
            .map(|path| PathSnapshot::capture(&crate::scanner::expand_tilde(path)))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let credential_backup = lifecycle_profile_id(persisted.lifecycle.as_ref())
        .map(|profile_id| (profile_id.to_string(), credential_snapshot(profile_id)));
    if let Some((profile_id, credential)) = &credential_backup {
        persist_credential_rollback(&request.operation_id, profile_id, credential.as_deref())?;
    }
    if let Err(error) = persist_rollback_snapshots(&request.operation_id, &snapshots) {
        if let Some((profile_id, _)) = &credential_backup {
            clear_credential_rollback(&request.operation_id, profile_id).map_err(|cleanup| {
                format!(
                    "failed to persist rollback snapshots ({error}); Keychain rollback cleanup failed: {cleanup}"
                )
            })?;
        }
        return Err(error);
    }

    let applied = apply_operation(&persisted)
        .and_then(|_| verify_operation(&persisted))
        .and_then(|_| mark_operation_committed(&request.operation_id));
    if let Err(error) = applied {
        let mut rollback_errors = Vec::new();
        for snapshot in snapshots.iter().rev() {
            if let Err(rollback) = snapshot.restore() {
                rollback_errors.push(rollback);
            }
        }
        if let Some((profile_id, credential)) = &credential_backup {
            if let Err(rollback) = restore_credential_snapshot(profile_id, credential.as_deref()) {
                rollback_errors.push(format!("failed to restore Model credential: {rollback}"));
            }
        }
        if rollback_errors.is_empty() {
            if let Some((profile_id, _)) = &credential_backup {
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

    if let Some((profile_id, _)) = &credential_backup {
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
        let profile_id = lifecycle_profile_id(persisted.lifecycle.as_ref()).map(str::to_string);
        let Some(snapshots) = load_rollback_snapshots(&operation_id)? else {
            if let Some(profile_id) = &profile_id {
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
            if let Some(profile_id) = &profile_id {
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
        let credential_backup = if let Some(profile_id) = &profile_id {
            let snapshot = credential_rollback_snapshot(&operation_id, profile_id)
                .map_err(|error| format!("recovery_required: {error}"))?
                .ok_or_else(|| {
                    "recovery_required: Model credential rollback item is missing".to_string()
                })?;
            Some(snapshot.map(Zeroizing::new))
        } else {
            None
        };
        if profile_id.is_some() || verify_operation(&persisted).is_err() {
            for snapshot in snapshots.iter().rev() {
                snapshot
                    .restore()
                    .map_err(|error| format!("recovery_required: {error}"))?;
            }
            if let (Some(profile_id), Some(credential)) = (&profile_id, &credential_backup) {
                restore_credential_snapshot(
                    profile_id,
                    credential.as_ref().map(|value| value.as_slice()),
                )
                .map_err(|error| format!("recovery_required: {error}"))?;
            }
        }
        if let Some(profile_id) = &profile_id {
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
    let operation = load_operation(operation_id)?;
    if operation.plan.operation_id != operation_id {
        return Err("asset operation identity mismatch".into());
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

fn lifecycle_profile_id(lifecycle: Option<&LifecycleBinding>) -> Option<&str> {
    match lifecycle {
        Some(LifecycleBinding::ModelUpsert { profile_id, .. })
        | Some(LifecycleBinding::ModelDelete { profile_id }) => Some(profile_id),
        _ => None,
    }
}

fn operation_commit_marker(operation_id: &str) -> PathBuf {
    operation_root(operation_id).join("commit-complete")
}

fn mark_operation_committed(operation_id: &str) -> Result<(), String> {
    write_private_file(&operation_commit_marker(operation_id), b"committed\n")
}

fn apply_operation(persisted: &PersistedAssetOperation) -> Result<(), String> {
    let Some(lifecycle) = &persisted.lifecycle else {
        return apply_domain_plan(&persisted.plan.domain_plan);
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
                apply_domain_plan(&persisted.plan.domain_plan)
            }
        }
        LifecycleBinding::McpEnabled {
            agent_id,
            asset_key,
            after,
            ..
        } => apply_mcp_enabled(agent_id, asset_key, *after),
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
            save_profile(profile, None)?;
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
        LifecycleBinding::ModelDelete { profile_id } => {
            apply_domain_plan(&persisted.plan.domain_plan)?;
            delete_profile_metadata(profile_id)?;
            apply_credential_update(profile_id, Some(""))
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
            if desired != Some(*after) || !observed.is_some_and(|item| item.enabled == *after) {
                return Err("MCP enabled state did not match the reviewed change".into());
            }
            Ok(())
        }
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
            {
                return Err("Model Profile deletion postcondition failed".into());
            }
            if credential_present(profile_id) {
                return Err("Model credential still exists after deletion".into());
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
            &[agent_id.clone()],
            None,
            &HashMap::from([(agent_id.clone(), patch)]),
        )
        .map_err(|errors| errors.join("; "))?;
    }
    Ok(())
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
        if desired.as_deref() == Some(profile_id) {
            apply_profile_with_credential_presence(agent_id, profile_id, credential_present)?;
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
    if let Some(LifecycleBinding::AgentConfiguration {
        skill_migration, ..
    }) = &persisted.lifecycle
    {
        verify_skill_migration_preconditions(skill_migration)?;
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
                let actual = current
                    .model_assignments
                    .as_ref()
                    .and_then(|assignments| assignments.get(agent_id))
                    .cloned();
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
            if &actual != before {
                return Err("asset_operation_stale: Agent configuration changed".into());
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
    std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(source, destination).map_err(|error| error.to_string())?;
    Ok(())
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
    crate::skills::hash_tree(&canonical)
        .map(Some)
        .map_err(|error| format!("{error:?}"))
}

fn expand_tilde_path(path: &str) -> PathBuf {
    crate::scanner::expand_tilde(path)
}

fn apply_domain_plan(plan: &DomainPlan) -> Result<(), String> {
    match plan {
        DomainPlan::Mcp { before, after } => apply_mcp(before, after),
        DomainPlan::Model { before, after } => apply_model(before, after),
        DomainPlan::Skill { before, after } => apply_skill(before, after),
        DomainPlan::AgentConfiguration { .. } => {
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
            ops::delete(name, transport, "global", &[agent_id.clone()], None)
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
                &[agent_id.clone()],
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

fn apply_model(
    before: &BTreeMap<String, Option<String>>,
    after: &BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    for agent_id in union_keys(before, after) {
        let left = before.get(agent_id).cloned().flatten();
        let right = after.get(agent_id).cloned().flatten();
        if left == right {
            continue;
        }
        if let Some(profile_id) = left {
            clear_profile(agent_id, &profile_id)?;
        }
        if let Some(profile_id) = right {
            apply_profile(agent_id, &profile_id)?;
        }
    }
    mutate_settings(|settings| {
        let assignments = settings.model_assignments.get_or_insert_default();
        for agent_id in union_keys(before, after) {
            match after.get(agent_id).cloned().flatten() {
                Some(profile_id) => {
                    assignments.insert(agent_id.clone(), profile_id);
                }
                None => {
                    assignments.remove(agent_id);
                }
            }
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
                let actual = inventory.consumptions.iter().find_map(|item| {
                    if item.agent_id != *agent_id || !item.desired {
                        return None;
                    }
                    match &item.asset {
                        AssetRef::Model { profile_id } => Some((profile_id.as_str(), &item.status)),
                        _ => None,
                    }
                });
                match (expected.as_deref(), actual) {
                    (None, None) => {}
                    (Some(expected), Some((actual, _))) if expected == actual => {}
                    _ => return Err("model post-commit verification failed".into()),
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
            if crate::agents::current_configuration(agent_id)? != *after {
                return Err("Agent configuration post-commit verification failed".into());
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
            .and_then(Option::as_deref)
            .is_some_and(|desired| desired == profile_id),
        (DomainPlan::Skill { after, .. }, AssetRef::Skill { name }) => after
            .get(agent_id)
            .is_some_and(|names| names.contains(name)),
        (DomainPlan::AgentConfiguration { skills_after, .. }, AssetRef::Skill { name }) => {
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
        // When a Model assignment has been removed, any remaining model-owned
        // fields are an incomplete clear even though the external projection has
        // no central Profile identity.
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

#[derive(Debug)]
enum SnapshotKind {
    Missing,
    File { bytes: Vec<u8>, mode: Option<u32> },
    Symlink { target: PathBuf },
    Directory,
}

#[derive(Debug)]
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
    fn capture(path: &Path) -> Result<Self, String> {
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

    fn restore(&self) -> Result<(), String> {
        remove_current(&self.path)?;
        match &self.kind {
            SnapshotKind::Missing => Ok(()),
            SnapshotKind::Directory => fs::create_dir_all(&self.path).map_err(|e| e.to_string()),
            SnapshotKind::File { bytes, mode } => {
                if let Some(parent) = self.path.parent() {
                    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                fs::write(&self.path, bytes).map_err(|error| error.to_string())?;
                #[cfg(unix)]
                if let Some(mode) = mode {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&self.path, fs::Permissions::from_mode(*mode))
                        .map_err(|error| error.to_string())?;
                }
                Ok(())
            }
            SnapshotKind::Symlink { target } => {
                if let Some(parent) = self.path.parent() {
                    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, &self.path)
                    .map_err(|error| error.to_string())?;
                #[cfg(windows)]
                std::os::windows::fs::symlink_dir(target, &self.path)
                    .map_err(|error| error.to_string())?;
                Ok(())
            }
        }
    }
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

fn remove_current(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consumption::{
        plan_set_agent_consumption, plan_set_mcp_enabled, plan_update_central_asset,
        AgentConsumptionSelection, CentralAssetDraft, PlanSetAgentConsumptionRequest,
        PlanSetMcpEnabledRequest, PlanUpdateCentralAssetRequest,
    };
    use crate::models::save_profile;
    use crate::registry::write_manual_entry;
    use crate::testenv::TestHome;
    use crate::types::{ModelProfile, ModelProtocol, RegistryConfig, RegistryEntry, StdioConfig};

    fn model(model: &str) -> ModelProfile {
        ModelProfile {
            id: "work".into(),
            name: "Work".into(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://example.invalid/v1".into(),
            model: model.into(),
            context_window: None,
            max_output_tokens: None,
            reasoning: false,
        }
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
        fs::write(&target, b"partial mutation").unwrap();
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

        let recovered = recover_pending_asset_operations().unwrap();
        assert_eq!(recovered, vec![plan.operation_id]);
        assert_eq!(fs::read(settings_file()).unwrap(), settings_before);
        assert!(!target.exists());
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
                profile: model("new-model"),
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
                profile: model("new-model"),
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
