//! Recoverable central commits with target-scoped convergence incidents.

use super::inventory::list_consumption_inventory;
use super::lifecycle::{clear_pending_payload, pending_payload, PendingAssetPayload};
use super::planner::{
    hash_file, hash_mcp_catalog, hash_settings_target, hash_skill_target_graph, hash_target,
    hash_targets, load_operation, operation_root, CredentialAction, LifecycleBinding,
    PersistedAssetOperation, SkillMigrationEntry,
};
use super::store::AssetStateStore;
use super::types::{
    AssetCapability, AssetCommitRequest, AssetOperationKind, AssetOperationPlan, AssetRef,
    ConsumptionInventory, DomainPlan, McpConsumptionRecord, ModelAgentSelection,
    RelationshipAction, SkillConsumptionRecord, TargetIncident,
};
use crate::paths::{mcp_catalog_file, model_catalog_file, settings_file, skill_catalog_file};
use crate::resources::mcp::ops;
use crate::resources::mcp::r#override::OverridePatch;
use crate::resources::mcp::registry::{
    delete_discovered_entry, delete_registry_entry, read_registry, read_registry_all,
    write_manual_entry,
};
use crate::resources::model::{
    agent_has_configured_models, apply_credential_update, apply_profile,
    apply_profile_consumption, clear_all_configured_models_for_targets,
    apply_profile_consumption_with_credential_presence, clear_credential_rollback,
    clear_profile_consumption, credential_present, credential_rollback_snapshot,
    credential_snapshot, delete_profile, delete_provider, persist_credential_rollback,
    provider_credential_present, provider_credential_subject, restore_credential_snapshot,
    save_profile, save_provider_bundle,
};
use crate::domain::agents::ModelStorageAuthority;
use crate::resources::skill::{
    acquire_skills_lock, cancel_operation_in_asset_transaction, canonical_skill_assignments,
    commit_assignment_in_asset_transaction, declared_targets_for_agents, normalize_agent_selection,
    plan_assignment, reapply_assignment_safely, release_assignment_safely, PlanAssignmentRequest,
    SkillsOperationLock, SkillsPaths,
};
use crate::safe_write::{
    acquire_settings_lock, anchored_states_match, begin_transaction_write_tracking_with_states,
    capture_parent_directory, create_transaction_symlink_if_missing,
    ensure_no_transaction_mutation_intents, fingerprint_anchored_path_state,
    load_transaction_write_states, read_path_state_anchored, recover_global_mutation_intents,
    recover_transaction_mutation_intents, remove_bytes_if_unchanged_in_parent,
    remove_symlink_if_unchanged_in_parent, resume_transaction_write_tracking,
    write_bytes_if_unchanged_in_parent, write_symlink_if_unchanged_in_parent, AnchoredPathState,
    ParentDirectorySnapshot, PathIdentity, TransactionPathState,
};
#[cfg(test)]
use crate::safe_write::{begin_transaction_write_tracking, set_transaction_symlink};
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
const ROLLBACK_MANIFEST_VERSION: u32 = 2;
const TARGET_INCIDENT_MARKER: &str = "target-incident";

fn plan_capability(plan: &DomainPlan) -> AssetCapability {
    match plan {
        DomainPlan::Mcp { .. } => AssetCapability::Mcp,
        DomainPlan::Model { .. } => AssetCapability::Model,
        DomainPlan::Skill { .. } | DomainPlan::AgentCapabilities { .. } => AssetCapability::Skill,
    }
}

fn incident_id(capability: AssetCapability, target_path: &str) -> String {
    let material = format!("{capability:?}:{target_path}");
    format!(
        "target-{}",
        hex::encode(Sha256::digest(material.as_bytes()))
    )
}

fn target_paths_for_agent(plan: &AssetOperationPlan, agent_id: &str) -> Vec<String> {
    if plan.affected_agent_ids.len() <= 1 {
        return plan.target_files.clone();
    }
    let settings = load_settings_strict().ok();
    let agents = crate::agents::load_agents();
    let skill_targets = crate::resources::skill::list_inventory()
        .ok()
        .map(|inventory| inventory.targets)
        .unwrap_or_default();
    let mut matched = plan
        .target_files
        .iter()
        .filter(|target| match &plan.domain_plan {
            DomainPlan::Mcp { .. } => agents
                .get(agent_id)
                .and_then(|agent| agent.global.as_ref())
                .is_some_and(|path| path == *target),
            DomainPlan::Model { .. } => settings
                .as_ref()
                .and_then(|settings| {
                    crate::resources::model::configured_path_strings_checked(settings, agent_id)
                        .ok()
                        .flatten()
                })
                .is_some_and(|configured| {
                    let target = crate::resources::mcp::scanner::expand_tilde(target);
                    configured.into_iter().any(|candidate| {
                        let candidate = crate::resources::mcp::scanner::expand_tilde(&candidate);
                        target == candidate || target.parent() == candidate.parent()
                    })
                }),
            DomainPlan::Skill { .. } | DomainPlan::AgentCapabilities { .. } => {
                skill_targets.iter().any(|skill_target| {
                    skill_target
                        .affected_agent_ids
                        .iter()
                        .any(|id| id == agent_id)
                        && target.starts_with(&skill_target.global_dir)
                })
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    if matched.is_empty() {
        matched = plan.target_files.clone();
    }
    matched.sort();
    matched.dedup();
    matched
}

fn record_target_incident(
    plan: &AssetOperationPlan,
    agent_id: &str,
    code: &str,
) -> Result<(), String> {
    let capability = plan_capability(&plan.domain_plan);
    let targets = target_paths_for_agent(plan, agent_id);
    mutate_settings(|settings| {
        let incidents = settings.target_incidents.get_or_insert_default();
        for target_path in &targets {
            let id = incident_id(capability, target_path);
            let mut affected_agent_ids = vec![agent_id.to_string()];
            if let Some(existing) = incidents.get(&id) {
                affected_agent_ids.extend(existing.affected_agent_ids.iter().cloned());
            }
            affected_agent_ids.sort();
            affected_agent_ids.dedup();
            incidents.insert(
                id.clone(),
                TargetIncident {
                    id: id.clone(),
                    operation_id: plan.operation_id.clone(),
                    capability,
                    target_id: id,
                    target_path: target_path.clone(),
                    affected_agent_ids,
                    code: code.to_string(),
                    retryable: true,
                },
            );
        }
    })
    .map_err(|error| error.to_string())
}

fn clear_target_incidents(plan: &AssetOperationPlan, agent_id: &str) -> Result<(), String> {
    let capability = plan_capability(&plan.domain_plan);
    let target_ids = target_paths_for_agent(plan, agent_id)
        .into_iter()
        .map(|target| incident_id(capability, &target))
        .collect::<BTreeSet<_>>();
    mutate_settings(|settings| {
        if let Some(incidents) = settings.target_incidents.as_mut() {
            incidents.retain(|id, _| !target_ids.contains(id));
            if incidents.is_empty() {
                settings.target_incidents = None;
            }
        }
    })
    .map_err(|error| error.to_string())
}

fn incident_operation_ids_for_plan(plan: &AssetOperationPlan) -> BTreeSet<String> {
    let capability = plan_capability(&plan.domain_plan);
    let affected_agents = plan
        .affected_agent_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    load_settings_strict()
        .ok()
        .and_then(|settings| settings.target_incidents)
        .into_iter()
        .flat_map(|incidents| incidents.into_values())
        .filter(|incident| {
            incident.capability == capability
                && incident
                    .affected_agent_ids
                    .iter()
                    .any(|agent_id| affected_agents.contains(agent_id))
        })
        .map(|incident| incident.operation_id)
        .collect()
}

fn cleanup_resolved_incident_operations(operation_ids: &BTreeSet<String>, current: &str) {
    let still_referenced = load_settings_strict()
        .ok()
        .and_then(|settings| settings.target_incidents)
        .into_iter()
        .flat_map(|incidents| incidents.into_values())
        .map(|incident| incident.operation_id)
        .collect::<BTreeSet<_>>();
    for operation_id in operation_ids {
        if operation_id != current && !still_referenced.contains(operation_id) {
            let canonical = uuid::Uuid::parse_str(operation_id)
                .ok()
                .map(|parsed| parsed.hyphenated().to_string());
            if canonical.as_deref() != Some(operation_id.as_str()) {
                continue;
            }
            let root = operation_root(operation_id);
            if fs::symlink_metadata(&root).is_ok_and(|metadata| {
                metadata.file_type().is_dir() && !metadata.file_type().is_symlink()
            }) {
                // A resolved target incident may still own a durable guard or
                // temp from an older rename-based writer. Reconcile and retire
                // that exact evidence before removing its operation journal;
                // otherwise the sibling artifacts would be orphaned forever.
                let reconciled = load_rollback_snapshots(operation_id)
                    .ok()
                    .flatten()
                    .map(|snapshots| {
                        snapshots
                            .iter()
                            .map(|snapshot| (snapshot.path.clone(), snapshot.parent.clone()))
                            .collect::<BTreeMap<_, _>>()
                    })
                    .is_some_and(|parents| {
                        recover_transaction_mutation_intents(
                            &transaction_mutation_intent_dir(operation_id),
                            &parents,
                        )
                        .and_then(|_| {
                            ensure_no_transaction_mutation_intents(
                                &transaction_mutation_intent_dir(operation_id),
                            )
                        })
                        .is_ok()
                    });
                if !reconciled {
                    continue;
                }
                let _ = fs::remove_dir_all(&root);
                clear_pending_payload(operation_id);
            }
        }
    }
}

pub fn commit_asset_operation(request: AssetCommitRequest) -> Result<ConsumptionInventory, String> {
    commit_asset_operation_with_hook(request, || Ok(()))
}

fn commit_asset_operation_with_hook<F>(
    request: AssetCommitRequest,
    after_preconditions: F,
) -> Result<ConsumptionInventory, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let _guard = COMMIT_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // All Asset commits take locks in the same order as standalone Skills
    // operations: Skills -> settings -> safe-write journal. This both removes
    // the historical lock inversion and lets Skill links be owned directly by
    // the outer Asset transaction.
    let skills_guard = acquire_asset_skills_lock()?;
    // Every cooperating MUX process already uses this lock for settings/source
    // mutations. Holding it across verify + apply closes the catalog and Agent
    // writer TOCTOU; nested settings mutations are reentrant on this thread.
    let _filesystem_guard = acquire_settings_lock(&settings_file())?;
    recover_global_mutation_intents()?;
    let persisted = load_operation(&request.operation_id)?;
    verify_request(&persisted, &request)?;
    if !persisted.plan.can_commit {
        return Err("asset_operation_blocked: resolve drift or conflict before commit".into());
    }
    verify_preconditions(&persisted)?;
    after_preconditions()?;
    let incident_operation_ids = incident_operation_ids_for_plan(&persisted.plan);

    let settings_path = settings_file();
    let target_paths = persisted
        .plan
        .target_files
        .iter()
        .map(|path| crate::resources::mcp::scanner::expand_tilde(path))
        .collect::<BTreeSet<_>>();
    let catalog_paths = [
        mcp_catalog_file(),
        model_catalog_file(),
        skill_catalog_file(),
    ];
    let mut snapshots = vec![PathSnapshot::capture(&settings_path)?];
    for path in &catalog_paths {
        snapshots.push(PathSnapshot::capture(path)?);
    }
    let skill_link_targets = matches!(
        persisted.plan.domain_plan,
        DomainPlan::Skill { .. } | DomainPlan::AgentCapabilities { .. }
    );
    let preserving_external_skill_targets =
        matches!(persisted.plan.domain_plan, DomainPlan::Skill { .. })
            && !persisted.plan.relationship_changes.is_empty()
            && persisted
                .plan
                .relationship_changes
                .iter()
                .all(|change| change.action == RelationshipAction::Remove);
    for path in target_paths {
        if path == settings_path || catalog_paths.contains(&path) {
            continue;
        }
        snapshots.push(if preserving_external_skill_targets {
            PathSnapshot::capture_any(&path)?
        } else if skill_link_targets {
            PathSnapshot::capture_link(&path)?
        } else {
            PathSnapshot::capture(&path)?
        });
    }
    verify_captured_snapshots(&persisted, &snapshots)?;
    let tracked_paths = snapshots
        .iter()
        .map(|snapshot| snapshot.path.clone())
        .collect::<Vec<_>>();
    let tracked_parent_snapshots = snapshots
        .iter()
        .map(|snapshot| (snapshot.path.clone(), snapshot.parent.clone()))
        .collect::<BTreeMap<_, _>>();
    let reviewed_states = snapshots
        .iter()
        .map(|snapshot| (snapshot.path.clone(), snapshot.anchored_state()))
        .collect::<BTreeMap<_, _>>();
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

    let write_tracker = begin_transaction_write_tracking_with_states(
        &transaction_write_evidence_dir(&request.operation_id),
        &tracked_paths,
        &tracked_parent_snapshots,
        &reviewed_states,
    )?;
    // A prior target incident is observable state, not a write prohibition.
    // Every new operation gets one real convergence attempt for its own target;
    // failures remain scoped incidents instead of silently skipping Agent I/O.
    let blocked_agents = BTreeSet::new();
    let applied = apply_operation(&persisted, &skills_guard, &blocked_agents)
        .and_then(|_| verify_operation(&persisted))
        .and_then(|_| mark_operation_committed(&request.operation_id));
    if let Err(error) = applied {
        if let Err(recovery) = recover_transaction_mutation_intents(
            &transaction_mutation_intent_dir(&request.operation_id),
            &tracked_parent_snapshots,
        ) {
            drop(write_tracker);
            if localize_runtime_recovery_failure(&persisted.plan) {
                return Err(format!(
                    "target_recovery_required: asset operation failed ({error}); target claim recovery failed: {recovery}"
                ));
            }
            return Err(format!(
                "recovery_required: asset operation failed ({error}); claim recovery failed: {recovery}"
            ));
        }
        // Claim recovery can restore the previous state of a target that was
        // written more than once in this transaction. Capture evidence only
        // after that reconciliation so rollback CAS uses the actual leaf now
        // present in the reviewed namespace.
        let written_states = write_tracker.states();
        let mut rollback_errors = restore_snapshots_if_unchanged(&snapshots, &written_states);
        if let Err(recovery) = ensure_no_transaction_mutation_intents(
            &transaction_mutation_intent_dir(&request.operation_id),
        ) {
            rollback_errors.push(recovery);
        }
        drop(write_tracker);
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
        if localize_runtime_recovery_failure(&persisted.plan) {
            return Err(format!(
                "target_recovery_required: asset operation failed ({error}); target rollback failed: {}",
                rollback_errors.join("; ")
            ));
        }
        return Err(format!(
            "recovery_required: asset operation failed ({error}); rollback failed: {}",
            rollback_errors.join("; ")
        ));
    }
    drop(write_tracker);
    ensure_no_transaction_mutation_intents(&transaction_mutation_intent_dir(
        &request.operation_id,
    ))?;

    for (profile_id, _) in &credential_backups {
        clear_credential_rollback(&request.operation_id, profile_id).map_err(|error| {
            format!("asset operation committed but Keychain rollback cleanup failed: {error}")
        })?;
    }
    fs::remove_dir_all(operation_root(&request.operation_id)).map_err(|error| {
        format!("asset operation committed but staging cleanup failed: {error}")
    })?;
    clear_pending_payload(&request.operation_id);
    cleanup_resolved_incident_operations(&incident_operation_ids, &request.operation_id);
    list_consumption_inventory()
}

/// Recover every operation that had begun mutating durable state. Reviewed but
/// uncommitted plans have no rollback manifest and are safely cancelled after a
/// restart because secret-bearing drafts intentionally live only in memory.
pub fn recover_pending_asset_operations() -> Result<Vec<String>, String> {
    let _guard = COMMIT_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _skills_guard = acquire_asset_skills_lock()?;
    let _filesystem_guard = acquire_settings_lock(&settings_file())?;
    recover_global_mutation_intents()?;
    let root = crate::paths::mux_dir().join("staging/consumption");
    let root_before = match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_dir() => metadata,
        Ok(_) => return Err("recovery_required: the Asset operation root is unsafe".into()),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("recovery_required: {error}")),
    };
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) => return Err(format!("recovery_required: {error}")),
    };
    let mut recovered = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("recovery_required: {error}"))?;
        let entry_type = entry
            .file_type()
            .map_err(|error| format!("recovery_required: {error}"))?;
        if entry_type.is_symlink() {
            return Err("recovery_required: an Asset operation root is unsafe".into());
        }
        if !entry_type.is_dir() {
            continue;
        }
        let operation_id = entry.file_name().to_string_lossy().into_owned();
        let persisted =
            load_operation(&operation_id).map_err(|error| format!("recovery_required: {error}"))?;
        if target_incident_marker_exists(&operation_id)? {
            record_recovery_incidents(&persisted.plan)?;
            continue;
        }
        match recover_pending_asset_operation(&persisted, &entry.path()) {
            Ok(()) => {
                recovered.push(operation_id);
            }
            Err(error) if !persisted.plan.target_files.is_empty() => {
                // A target that changed after MUX wrote it is a local convergence
                // incident. Preserve the journal and rollback evidence, record
                // the exact affected target(s), and continue recovering peers.
                load_settings_strict().map_err(|settings_error| {
                    format!(
                        "recovery_required: target recovery failed ({error}); central settings are unavailable: {settings_error}"
                    )
                })?;
                record_recovery_incidents(&persisted.plan)?;
                let _ = write_private_file(
                    &operation_root(&persisted.plan.operation_id).join(TARGET_INCIDENT_MARKER),
                    b"recorded\n",
                );
            }
            Err(error) => return Err(error),
        }
    }
    let root_after = fs::symlink_metadata(&root).map_err(|error| {
        format!("recovery_required: failed to recheck Asset operations: {error}")
    })?;
    if !root_after.file_type().is_dir() || !same_directory_identity(&root_before, &root_after) {
        return Err("recovery_required: the Asset operation root changed during recovery".into());
    }
    Ok(recovered)
}

fn recover_pending_asset_operation(
    persisted: &PersistedAssetOperation,
    operation_path: &Path,
) -> Result<(), String> {
    let operation_id = &persisted.plan.operation_id;
    let profile_ids = lifecycle_profile_ids(persisted.lifecycle.as_ref());
    let Some(snapshots) = load_rollback_snapshots(operation_id)? else {
        ensure_no_transaction_mutation_intents(&transaction_mutation_intent_dir(operation_id))?;
        for profile_id in &profile_ids {
            clear_credential_rollback(operation_id, profile_id)
                .map_err(|error| format!("recovery_required: {error}"))?;
        }
        fs::remove_dir_all(operation_path)
            .map_err(|error| format!("recovery_required: {error}"))?;
        clear_pending_payload(operation_id);
        return Ok(());
    };

    let parent_snapshots = snapshots
        .iter()
        .map(|snapshot| (snapshot.path.clone(), snapshot.parent.clone()))
        .collect::<BTreeMap<_, _>>();
    recover_transaction_mutation_intents(
        &transaction_mutation_intent_dir(operation_id),
        &parent_snapshots,
    )?;
    ensure_no_transaction_mutation_intents(&transaction_mutation_intent_dir(operation_id))?;

    if operation_commit_marker_exists(operation_id)? {
        for profile_id in &profile_ids {
            clear_credential_rollback(operation_id, profile_id)
                .map_err(|error| format!("recovery_required: {error}"))?;
        }
        fs::remove_dir_all(operation_path)
            .map_err(|error| format!("recovery_required: {error}"))?;
        clear_pending_payload(operation_id);
        return Ok(());
    }

    let written_states =
        load_transaction_write_states(&transaction_write_evidence_dir(operation_id))?;
    let mut credential_backups = Vec::new();
    for profile_id in &profile_ids {
        let snapshot = credential_rollback_snapshot(operation_id, profile_id)
            .map_err(|error| format!("recovery_required: {error}"))?
            .ok_or_else(|| {
                "recovery_required: Model credential rollback item is missing".to_string()
            })?;
        credential_backups.push((profile_id.clone(), snapshot.map(Zeroizing::new)));
    }
    // The commit marker is the only durable completion boundary. Desired-state
    // verification alone cannot prove that every physical target was written,
    // so every unmarked operation is rolled back when ownership still matches.
    let tracked_paths = snapshots
        .iter()
        .map(|snapshot| snapshot.path.clone())
        .collect::<Vec<_>>();
    let reviewed_states = snapshots
        .iter()
        .map(|snapshot| (snapshot.path.clone(), snapshot.anchored_state()))
        .collect::<BTreeMap<_, _>>();
    let write_tracker = resume_transaction_write_tracking(
        &transaction_write_evidence_dir(operation_id),
        &tracked_paths,
        &parent_snapshots,
        &reviewed_states,
        &written_states,
    )?;
    let mut rollback_errors = restore_snapshots_if_unchanged(&snapshots, &written_states);
    if let Err(error) =
        ensure_no_transaction_mutation_intents(&transaction_mutation_intent_dir(operation_id))
    {
        rollback_errors.push(error);
    }
    drop(write_tracker);
    if !rollback_errors.is_empty() {
        return Err(format!(
            "target_recovery_required: {}",
            rollback_errors.join("; ")
        ));
    }
    for (profile_id, credential) in &credential_backups {
        restore_credential_snapshot(
            profile_id,
            credential.as_ref().map(|value| value.as_slice()),
        )
        .map_err(|error| format!("target_recovery_required: {error}"))?;
    }
    ensure_no_transaction_mutation_intents(&transaction_mutation_intent_dir(operation_id))?;
    for profile_id in &profile_ids {
        clear_credential_rollback(operation_id, profile_id)
            .map_err(|error| format!("target_recovery_required: {error}"))?;
    }
    fs::remove_dir_all(operation_path)
        .map_err(|error| format!("target_recovery_required: {error}"))?;
    clear_pending_payload(operation_id);
    Ok(())
}

fn target_incident_marker_exists(operation_id: &str) -> Result<bool, String> {
    let marker = operation_root(operation_id).join(TARGET_INCIDENT_MARKER);
    match fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err("recovery_required: Asset target incident marker is unsafe".into()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "recovery_required: failed to inspect Asset target incident marker: {error}"
        )),
    }
}

fn operation_commit_marker_exists(operation_id: &str) -> Result<bool, String> {
    let marker = operation_commit_marker(operation_id);
    match fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err("recovery_required: Asset commit marker is unsafe".into()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "recovery_required: failed to inspect Asset commit marker: {error}"
        )),
    }
}

fn record_recovery_incidents(plan: &AssetOperationPlan) -> Result<(), String> {
    for agent_id in &plan.affected_agent_ids {
        record_target_incident(plan, agent_id, "target_recovery_required")?;
    }
    Ok(())
}

fn localize_runtime_recovery_failure(plan: &AssetOperationPlan) -> bool {
    if plan.target_files.is_empty() || plan.affected_agent_ids.is_empty() {
        return false;
    }
    if load_settings_strict().is_err() || record_recovery_incidents(plan).is_err() {
        return false;
    }
    let _ = write_private_file(
        &operation_root(&plan.operation_id).join(TARGET_INCIDENT_MARKER),
        b"recorded\n",
    );
    true
}

fn acquire_asset_skills_lock() -> Result<SkillsOperationLock, String> {
    let paths = SkillsPaths::resolve_from_env().map_err(|error| format!("{error:?}"))?;
    paths
        .ensure_mux_root()
        .map_err(|error| format!("{error:?}"))?;
    acquire_skills_lock(&paths).map_err(|error| format!("{error:?}"))
}

#[cfg(unix)]
fn same_directory_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_directory_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
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
        Some(LifecycleBinding::ModelProviderUpsert { provider_id, .. })
        | Some(LifecycleBinding::ModelProviderDelete { provider_id }) => {
            vec![provider_credential_subject(provider_id)]
        }
        _ => Vec::new(),
    }
}

fn operation_commit_marker(operation_id: &str) -> PathBuf {
    operation_root(operation_id).join("commit-complete")
}

fn transaction_write_evidence_dir(operation_id: &str) -> PathBuf {
    operation_root(operation_id).join("rollback/post")
}

fn transaction_mutation_intent_dir(operation_id: &str) -> PathBuf {
    operation_root(operation_id).join("rollback/claims")
}

fn mark_operation_committed(operation_id: &str) -> Result<(), String> {
    mark_operation_committed_with_barrier_hook(operation_id, |_| Ok(()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitMarkerDurabilityBarrier {
    File,
    OperationDirectory,
    ParentDirectory,
}

fn mark_operation_committed_with_barrier_hook<F>(
    operation_id: &str,
    mut after_barrier: F,
) -> Result<(), String>
where
    F: FnMut(CommitMarkerDurabilityBarrier) -> Result<(), String>,
{
    let operation_directory = operation_root(operation_id);
    write_private_file(&operation_commit_marker(operation_id), b"committed\n")?;
    after_barrier(CommitMarkerDurabilityBarrier::File)?;

    sync_directory(&operation_directory)?;
    after_barrier(CommitMarkerDurabilityBarrier::OperationDirectory)?;

    let parent = operation_directory
        .parent()
        .ok_or_else(|| "asset operation directory has no parent".to_string())?;
    sync_directory(parent)?;
    after_barrier(CommitMarkerDurabilityBarrier::ParentDirectory)
}

/// A pure relationship removal releases MUX's desired ownership. Central
/// lifecycle operations and enabled-state edits do not get this exemption:
/// they still own and must reconcile their Agent bytes.
fn releases_relationship_ownership(plan: &AssetOperationPlan) -> bool {
    plan.kind == AssetOperationKind::SetConsumption
        && plan.central_changes.is_empty()
        && !plan.relationship_changes.is_empty()
        && plan
            .relationship_changes
            .iter()
            .all(|change| change.action == RelationshipAction::Remove)
}

fn apply_operation(
    persisted: &PersistedAssetOperation,
    skills_lock: &SkillsOperationLock,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let Some(lifecycle) = &persisted.lifecycle else {
        return apply_domain_plan(
            &persisted.plan,
            releases_relationship_ownership(&persisted.plan),
            skills_lock,
            blocked_agents,
        );
    };
    match lifecycle {
        LifecycleBinding::McpClear { agent_id } => {
            mutate_settings(|settings| {
                if let Some(consumptions) = settings.mcp_consumptions.as_mut() {
                    consumptions.remove(agent_id);
                    if consumptions.is_empty() {
                        settings.mcp_consumptions = None;
                    }
                }
                if let Some(disabled) = settings.disabled.as_mut() {
                    disabled.remove(agent_id);
                    if disabled.is_empty() {
                        settings.disabled = None;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
            match crate::resources::mcp::ops::clear_agent(agent_id).and_then(|_| {
                if crate::resources::mcp::ops::agent_has_entries(agent_id)? {
                    Err("target_convergence_failed: MCP entries reappeared after clear".into())
                } else {
                    Ok(())
                }
            }) {
                Ok(()) => clear_target_incidents(&persisted.plan, agent_id),
                Err(_) => {
                    record_target_incident(&persisted.plan, agent_id, "target_convergence_failed")
                }
            }
        }
        LifecycleBinding::ModelClear {
            agent_id,
            storage_authority,
            ..
        } => {
            let before = match &persisted.plan.domain_plan {
                DomainPlan::Model { before, .. } => before.get(agent_id).cloned().unwrap_or_default(),
                _ => return Err("Model clear requires a Model domain plan".into()),
            };
            mutate_settings(|settings| {
                if let Some(consumptions) = settings.model_consumptions.as_mut() {
                    consumptions.remove(agent_id);
                    if consumptions.is_empty() {
                        settings.model_consumptions = None;
                    }
                }
                if let Some(assignments) = settings.model_assignments.as_mut() {
                    assignments.remove(agent_id);
                    if assignments.is_empty() {
                        settings.model_assignments = None;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
            let result = match storage_authority {
                ModelStorageAuthority::NativeRegistry => clear_all_configured_models_for_targets(
                    agent_id,
                    &persisted.plan.target_files,
                )
                    .and_then(|_| {
                        if agent_has_configured_models(agent_id)? {
                            Err("target_convergence_failed: Model entries reappeared after clear".into())
                        } else {
                            Ok(())
                        }
                    }),
                ModelStorageAuthority::MuxMapping => {
                    let mut result = Ok(());
                    for (profile_id, _) in &before.profiles {
                        let active = before.active_profile_id.as_deref() == Some(profile_id.as_str());
                        if let Err(error) = clear_profile_consumption(agent_id, profile_id, active) {
                            result = Err(error);
                            break;
                        }
                    }
                    result
                }
                ModelStorageAuthority::Guided => Err("model_agent_guided".into()),
            };
            match result {
                Ok(()) => clear_target_incidents(&persisted.plan, agent_id),
                Err(_) => record_target_incident(
                    &persisted.plan,
                    agent_id,
                    "target_convergence_failed",
                ),
            }
        }
        LifecycleBinding::McpUpsert {
            key,
            draft_hash,
            previous_key,
            previous_source_id,
        } => {
            let PendingAssetPayload::Mcp { entry } =
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
            if let Some(previous_key) = previous_key {
                let previous_source_id = previous_source_id.as_deref().ok_or_else(|| {
                    "asset_operation_stale: MCP rename source is unavailable".to_string()
                })?;
                migrate_mcp_consumption_records(previous_key, key)?;
                apply_domain_plan(
                    &persisted.plan,
                    releases_relationship_ownership(&persisted.plan),
                    skills_lock,
                    blocked_agents,
                )?;
                delete_mcp_source_copy(previous_key, previous_source_id)
            } else {
                reapply_mcp_consumers(&persisted.plan, key, blocked_agents)
            }
        }
        LifecycleBinding::McpAdopt {
            key,
            draft_hash,
            enabled,
        } => {
            if let Some(draft_hash) = draft_hash {
                let PendingAssetPayload::Mcp { entry } =
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
            apply_mcp_adoption(&persisted.plan, key, enabled)
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
                reapply_mcp_consumers(&persisted.plan, key, blocked_agents)
            } else {
                apply_domain_plan(
                    &persisted.plan,
                    releases_relationship_ownership(&persisted.plan),
                    skills_lock,
                    blocked_agents,
                )
            }
        }
        LifecycleBinding::McpEnabled {
            agent_id,
            asset_key,
            after,
            ..
        } => apply_mcp_enabled(&persisted.plan, agent_id, asset_key, *after, blocked_agents),
        LifecycleBinding::McpEnabledBulk {
            agent_id,
            before,
            after,
        } => {
            for asset_key in before
                .iter()
                .filter_map(|(key, enabled)| (*enabled != *after).then_some(key))
            {
                apply_mcp_enabled(&persisted.plan, agent_id, asset_key, *after, blocked_agents)?;
            }
            Ok(())
        }
        LifecycleBinding::SkillEnabled {
            name,
            target_id,
            affected_agent_ids,
            after,
            ..
        } => apply_skill_enabled(
            &persisted.plan,
            name,
            target_id,
            affected_agent_ids,
            *after,
            skills_lock,
            blocked_agents,
        ),
        LifecycleBinding::McpReapply { key, agent_ids } => {
            reapply_mcp_consumers_for_agents(&persisted.plan, key, agent_ids, blocked_agents)
        }
        LifecycleBinding::ModelReapply {
            agent_id,
            profile_id,
            enabled,
            active,
            credential_present,
        } => {
            if blocked_agents.contains(agent_id) {
                return Ok(());
            }
            let result = if *enabled {
                apply_profile_consumption_with_credential_presence(
                    agent_id,
                    profile_id,
                    *credential_present,
                    *active,
                )
                .map(|_| ())
            } else {
                clear_profile_consumption(agent_id, profile_id, *active)
            };
            match result {
                Ok(()) => clear_target_incidents(&persisted.plan, agent_id),
                Err(_) => {
                    record_target_incident(&persisted.plan, agent_id, "target_convergence_failed")
                }
            }
        }
        LifecycleBinding::SkillReapply {
            name,
            target_id,
            enabled,
            ..
        } => {
            let _ = enabled;
            let result = reapply_assignment_safely(name, target_id, skills_lock)
                .map_err(|error| format!("{error:?}"));
            for agent_id in &persisted.plan.affected_agent_ids {
                if result.is_ok() {
                    clear_target_incidents(&persisted.plan, agent_id)?;
                } else {
                    record_target_incident(&persisted.plan, agent_id, "target_convergence_failed")?;
                }
            }
            Ok(())
        }
        LifecycleBinding::SkillAdoptObserved { name, record } => {
            let inventory =
                crate::resources::skill::list_inventory().map_err(|error| format!("{error:?}"))?;
            let current = inventory.items.iter().find(|item| {
                item.name == *name
                    && matches!(
                        item.location,
                        crate::resources::skill::SkillLocation::Central
                    )
            });
            if current.and_then(|item| item.content_hash.as_ref()) != Some(&record.content_hash) {
                return Err(
                    "asset_operation_stale: Skill content changed after convergence review".into(),
                );
            }
            mutate_settings(|settings| {
                settings
                    .managed_skills
                    .get_or_insert_default()
                    .insert(name.clone(), record.as_ref().clone());
            })
            .map_err(|error| error.to_string())?;
            for agent_id in &persisted.plan.affected_agent_ids {
                clear_target_incidents(&persisted.plan, agent_id)?;
            }
            Ok(())
        }
        LifecycleBinding::ModelUpsert {
            profile_id,
            draft_hash,
            credential_action,
        } => {
            let PendingAssetPayload::Model {
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
                &persisted.plan,
                profile_id,
                desired_credential_present,
                blocked_agents,
            )?;
            // Keychain mutation is deliberately last. A crash before this line
            // leaves the old credential intact and can roll files/settings back;
            // a crash after it has a fully verifiable committed state.
            apply_credential_update(profile_id, credential.as_deref().map(String::as_str))
        }
        LifecycleBinding::ModelProviderUpsert {
            provider_id,
            profile_ids,
            draft_hash,
            credential_action,
        } => {
            let PendingAssetPayload::ModelProvider {
                provider,
                profiles,
                credential,
            } = require_pending_payload(&persisted.plan.operation_id)?
            else {
                return Err("asset_operation_expired: Model Provider draft or credential is unavailable; reopen the editor".into());
            };
            verify_payload_hash(&(provider.as_ref(), &profiles), draft_hash)?;
            if provider.id != *provider_id
                || profiles.keys().cloned().collect::<BTreeSet<_>>() != *profile_ids
                || credential_action_for(&credential) != *credential_action
            {
                return Err(
                    "asset_operation_stale: Model Provider draft no longer matches the reviewed plan"
                        .into(),
                );
            }
            let desired_credential_present = match credential_action {
                CredentialAction::Keep => provider_credential_present(provider_id),
                CredentialAction::Set => true,
                CredentialAction::Clear => false,
            };
            save_provider_bundle(*provider, profiles)?;
            for profile_id in profile_ids {
                reapply_model_consumers(
                    &persisted.plan,
                    profile_id,
                    desired_credential_present,
                    blocked_agents,
                )?;
            }
            apply_credential_update(
                &provider_credential_subject(provider_id),
                credential.as_deref().map(String::as_str),
            )
        }
        LifecycleBinding::ModelProviderDelete { provider_id } => delete_provider(provider_id),
        LifecycleBinding::ModelAdopt {
            profile_id,
            draft_hash,
            credential_action,
        } => {
            let PendingAssetPayload::Model {
                profile,
                credential,
            } = require_pending_payload(&persisted.plan.operation_id)?
            else {
                return Err("asset_operation_expired: Model adoption payload is unavailable; refresh the observed state".into());
            };
            verify_payload_hash(&profile, draft_hash)?;
            if profile.id != *profile_id || credential_action_for(&credential) != *credential_action
            {
                return Err("asset_operation_stale: Model adoption payload changed".into());
            }
            save_profile(*profile, None)?;
            // Adoption changes MUX authority to match the reviewed Agent state.
            // It must never normalize or rewrite that state; doing so can erase
            // unrelated external models from a shared Agent configuration.
            let DomainPlan::Model { after, .. } = &persisted.plan.domain_plan else {
                return Err("asset operation domain mismatch".into());
            };
            mutate_settings(|settings| {
                for (agent_id, selection) in after {
                    settings.set_model_selection(agent_id, selection.clone());
                }
            })
            .map_err(|error| error.to_string())?;
            apply_credential_update(profile_id, credential.as_deref().map(String::as_str))?;
            for agent_id in &persisted.plan.affected_agent_ids {
                clear_target_incidents(&persisted.plan, agent_id)?;
            }
            Ok(())
        }
        LifecycleBinding::ModelDelete { profile_id } => {
            apply_domain_plan(
                &persisted.plan,
                releases_relationship_ownership(&persisted.plan),
                skills_lock,
                blocked_agents,
            )?;
            delete_profile(profile_id)
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
    verify_postcondition(&persisted.plan, persisted.lifecycle.as_ref())?;
    let Some(lifecycle) = &persisted.lifecycle else {
        return Ok(());
    };
    match lifecycle {
        LifecycleBinding::McpClear { agent_id } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let has_consumptions = settings
                .mcp_consumptions
                .as_ref()
                .and_then(|records| records.get(agent_id))
                .is_some_and(|records| !records.is_empty());
            let has_disabled = settings
                .disabled
                .as_ref()
                .and_then(|records| records.get(agent_id))
                .is_some_and(|records| !records.is_empty());
            if has_consumptions || has_disabled {
                return Err("MCP clear desired state verification failed".into());
            }
            Ok(())
        }
        LifecycleBinding::ModelClear { agent_id, .. } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let has_consumptions = settings
                .model_consumptions
                .as_ref()
                .and_then(|records| records.get(agent_id))
                .is_some_and(|records| !records.is_empty());
            let has_assignment = settings
                .model_assignments
                .as_ref()
                .is_some_and(|records| records.contains_key(agent_id));
            if has_consumptions || has_assignment {
                return Err("Model clear desired state verification failed".into());
            }
            Ok(())
        }
        LifecycleBinding::McpUpsert {
            key,
            draft_hash,
            previous_key,
            previous_source_id,
        } => {
            let entry = read_registry()
                .into_iter()
                .find(|entry| entry.key() == *key)
                .ok_or_else(|| "MCP central post-commit verification failed".to_string())?;
            // Provenance is normalized by the central writer and is not part of
            // the reviewed draft hash.
            let mut entry = entry;
            entry.origin = None;
            verify_payload_hash(&entry, draft_hash)?;
            if let Some(previous_key) = previous_key {
                let previous_source_id = previous_source_id
                    .as_deref()
                    .ok_or_else(|| "MCP rename source verification is unavailable".to_string())?;
                if mcp_source_copy_exists(previous_key, previous_source_id)
                    || read_registry()
                        .iter()
                        .any(|entry| entry.key() == *previous_key)
                {
                    return Err("old MCP identity still exists after rename".into());
                }
                let settings = load_settings_strict().map_err(|error| error.to_string())?;
                if settings
                    .mcp_consumptions
                    .iter()
                    .flatten()
                    .map(|(_, records)| records)
                    .any(|records| {
                        records.contains_key(previous_key)
                            || records
                                .get(key)
                                .is_some_and(|record| record.asset_key != *key)
                    })
                {
                    return Err("old MCP consumption identity still exists after rename".into());
                }
            }
            Ok(())
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
            if desired != Some(*after) {
                return Err("MCP enabled state did not match the reviewed change".into());
            }
            Ok(())
        }
        LifecycleBinding::McpEnabledBulk {
            agent_id,
            before,
            after,
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let records = settings
                .mcp_consumptions
                .as_ref()
                .and_then(|records| records.get(agent_id))
                .ok_or_else(|| {
                    "MCP consumptions disappeared after bulk enabled-state update".to_string()
                })?;
            if before
                .keys()
                .any(|key| records.get(key).map(|record| record.enabled) != Some(*after))
            {
                return Err("MCP enabled states did not match the reviewed bulk change".into());
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
            let _ = affected_agent_ids;
            Ok(())
        }
        LifecycleBinding::McpReapply { key, agent_ids } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            for agent_id in agent_ids {
                let record = settings
                    .mcp_consumptions
                    .as_ref()
                    .and_then(|records| records.get(agent_id))
                    .and_then(|records| records.get(key))
                    .filter(|record| record.asset_key == *key)
                    .ok_or_else(|| {
                        "MCP reapply consumption identity verification failed".to_string()
                    })?;
                let _ = record;
            }
            Ok(())
        }
        LifecycleBinding::ModelReapply {
            agent_id,
            profile_id,
            enabled,
            active,
            ..
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let selection = settings.model_selection(agent_id);
            if selection
                .profiles
                .get(profile_id)
                .is_none_or(|record| record.enabled != *enabled)
                || (selection.active_profile_id.as_deref() == Some(profile_id)) != *active
            {
                return Err("Model physical state did not match the reviewed reapply".into());
            }
            Ok(())
        }
        LifecycleBinding::SkillReapply {
            agent_id,
            name,
            target_id,
            affected_agent_ids,
            enabled,
        } => {
            if !affected_agent_ids
                .iter()
                .any(|affected| affected == agent_id)
            {
                return Err("Skill reapply omitted the selected Agent".into());
            }
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let assigned = settings
                .skill_assignments
                .as_ref()
                .and_then(|assignments| assignments.get(name))
                .is_some_and(|targets| targets.contains(target_id));
            let desired_enabled = settings
                .skill_consumptions
                .as_ref()
                .and_then(|skills| skills.get(name))
                .and_then(|targets| targets.get(target_id))
                .map(|record| record.enabled)
                .unwrap_or(true);
            if !assigned || desired_enabled != *enabled {
                return Err("Skill desired state did not match the reviewed reapply".into());
            }
            Ok(())
        }
        LifecycleBinding::SkillAdoptObserved { name, record } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            if settings
                .managed_skills
                .as_ref()
                .and_then(|records| records.get(name))
                != Some(record.as_ref())
            {
                return Err("Skill adopted baseline was not persisted".into());
            }
            let inventory =
                crate::resources::skill::list_inventory().map_err(|error| format!("{error:?}"))?;
            let current = inventory.items.iter().find(|item| {
                item.name == *name
                    && matches!(
                        item.location,
                        crate::resources::skill::SkillLocation::Central
                    )
            });
            if current.is_none_or(|item| {
                item.content_hash.as_ref() != Some(&record.content_hash)
                    || item
                        .states
                        .contains(&crate::resources::skill::InventoryState::LocallyModified)
            }) {
                return Err("Skill adopted baseline verification failed".into());
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
        LifecycleBinding::ModelProviderUpsert {
            provider_id,
            profile_ids,
            draft_hash,
            credential_action,
        } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            let provider = settings
                .model_providers
                .as_ref()
                .and_then(|providers| providers.get(provider_id))
                .ok_or_else(|| "Model Provider missing after commit".to_string())?;
            let profiles = profile_ids
                .iter()
                .map(|profile_id| {
                    settings
                        .model_profiles
                        .as_ref()
                        .and_then(|profiles| profiles.get(profile_id))
                        .cloned()
                        .map(|profile| (profile_id.clone(), profile))
                        .ok_or_else(|| "Provider child Model missing after commit".to_string())
                })
                .collect::<Result<BTreeMap<_, _>, String>>()?;
            verify_payload_hash(&(provider, &profiles), draft_hash)?;
            match credential_action {
                CredentialAction::Keep => {}
                CredentialAction::Set if !provider_credential_present(provider_id) => {
                    return Err("Provider credential was not saved after commit".into())
                }
                CredentialAction::Clear if provider_credential_present(provider_id) => {
                    return Err("Provider credential was not cleared after commit".into())
                }
                CredentialAction::Set | CredentialAction::Clear => {}
            }
            Ok(())
        }
        LifecycleBinding::ModelProviderDelete { provider_id } => {
            let settings = load_settings_strict().map_err(|error| error.to_string())?;
            if settings
                .model_providers
                .as_ref()
                .is_some_and(|providers| providers.contains_key(provider_id))
            {
                return Err("Model Provider deletion postcondition failed".into());
            }
            if provider_credential_present(provider_id) {
                return Err("Model Provider credential still exists after deletion".into());
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
            match credential_action {
                CredentialAction::Keep => {}
                CredentialAction::Set if !credential_present(profile_id) => {
                    return Err("Model credential was not saved after adoption".into())
                }
                CredentialAction::Clear if credential_present(profile_id) => {
                    return Err("Model credential was not cleared after adoption".into())
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

fn reapply_mcp_consumers(
    operation: &AssetOperationPlan,
    key: &str,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let DomainPlan::Mcp { after, .. } = &operation.domain_plan else {
        return Err("asset operation domain mismatch".into());
    };
    let agent_ids = after
        .iter()
        .filter(|(_, desired)| desired.iter().any(|candidate| candidate == key))
        .map(|(agent_id, _)| agent_id.clone())
        .collect::<Vec<_>>();
    reapply_mcp_consumers_for_agents(operation, key, &agent_ids, blocked_agents)
}

fn reapply_mcp_consumers_for_agents(
    operation: &AssetOperationPlan,
    key: &str,
    agent_ids: &[String],
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let (name, transport) = split_mcp_key(key)?;
    for agent_id in agent_ids {
        if blocked_agents.contains(agent_id) {
            continue;
        }
        let patch = settings
            .mcp_consumptions
            .as_ref()
            .and_then(|records| records.get(agent_id))
            .and_then(|records| records.get(key))
            .map(|record| record.overrides.clone())
            .unwrap_or_default();
        let result = (|| {
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
                ops::disable(
                    name,
                    transport,
                    "global",
                    std::slice::from_ref(agent_id),
                    None,
                )
                .map_err(|errors| errors.join("; "))?;
            }
            Ok::<_, String>(())
        })();
        if result.is_ok() {
            clear_target_incidents(operation, agent_id)?;
        } else {
            record_target_incident(operation, agent_id, "target_convergence_failed")?;
        }
    }
    Ok(())
}

fn migrate_mcp_consumption_records(old_key: &str, new_key: &str) -> Result<(), String> {
    mutate_settings(|settings| {
        for records in settings
            .mcp_consumptions
            .iter_mut()
            .flatten()
            .map(|(_, records)| records)
        {
            if records.contains_key(new_key) {
                return Err(format!(
                    "asset_identity_conflict: MCP consumption already contains {new_key}"
                ));
            }
            let Some(mut record) = records.remove(old_key) else {
                continue;
            };
            record.asset_key = new_key.to_string();
            records.insert(new_key.to_string(), record);
        }
        Ok(())
    })
    .map_err(|error| error.to_string())??;
    Ok(())
}

/// Adopt exact observed MCP copies without rewriting Agent files. The planner
/// already bound every target byte and verified that all copies match one
/// central config. Disabled observations remain in the existing snapshot store
/// and are recorded as disabled desired relationships.
fn apply_mcp_adoption(
    operation: &AssetOperationPlan,
    key: &str,
    enabled: &BTreeMap<String, bool>,
) -> Result<(), String> {
    let DomainPlan::Mcp { after, .. } = &operation.domain_plan else {
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
    .map_err(|error| error.to_string())?;
    for agent_id in after.keys() {
        clear_target_incidents(operation, agent_id)?;
    }
    Ok(())
}

fn reapply_model_consumers(
    operation: &AssetOperationPlan,
    profile_id: &str,
    credential_present: bool,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let DomainPlan::Model { after, .. } = &operation.domain_plan else {
        return Err("asset operation domain mismatch".into());
    };
    for (agent_id, desired) in after {
        if blocked_agents.contains(agent_id) {
            continue;
        }
        if desired
            .profiles
            .get(profile_id)
            .is_some_and(|record| record.enabled)
        {
            let result = apply_profile_consumption_with_credential_presence(
                agent_id,
                profile_id,
                credential_present,
                desired.active_profile_id.as_deref() == Some(profile_id),
            );
            if result.is_ok() {
                clear_target_incidents(operation, agent_id)?;
            } else {
                record_target_incident(operation, agent_id, "target_convergence_failed")?;
            }
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
    if persisted.schema_version == 2 {
        let current_hash = hash_file(&settings_file());
        if persisted.settings_hash.as_deref() != Some(current_hash.as_str()) {
            return Err("asset_operation_stale: MUX settings changed after review".into());
        }
    } else {
        AssetStateStore::load()?.verify(&persisted.state_preconditions)?;
    }
    let settings_target_hash = if persisted.schema_version == 2 {
        hash_target(&settings_file())
    } else {
        hash_settings_target(&settings_file())
    };
    if settings_target_hash != persisted.settings_target_hash {
        return Err("asset_operation_stale: MUX settings target changed after review".into());
    }
    if hash_targets(&persisted.plan.target_files) != persisted.target_hashes {
        return Err("asset_operation_stale: an Agent target changed after review".into());
    }
    if persisted.schema_version == 2 {
        if let Some(expected) = &persisted.mcp_catalog_hash {
            if &hash_mcp_catalog()? != expected {
                return Err(
                    "asset_operation_stale: central MCP catalog changed after review".into(),
                );
            }
        }
        if let Some(expected) = &persisted.skill_target_graph_hash {
            if &hash_skill_target_graph()? != expected {
                return Err(
                    "asset_operation_stale: Skill target graph changed after review".into(),
                );
            }
        }
    }
    if let Some(LifecycleBinding::ModelReapply {
        profile_id,
        credential_present: reviewed,
        ..
    }) = &persisted.lifecycle
    {
        if credential_present(profile_id) != *reviewed {
            return Err(
                "asset_operation_stale: Model credential state changed after review".into(),
            );
        }
    }
    if let Some(LifecycleBinding::AgentCapabilities {
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

fn verify_captured_snapshots(
    persisted: &PersistedAssetOperation,
    snapshots: &[PathSnapshot],
) -> Result<(), String> {
    let settings_path = settings_file();
    let settings = snapshots
        .iter()
        .find(|snapshot| snapshot.path == settings_path)
        .ok_or_else(|| "asset_operation_stale: settings snapshot is missing".to_string())?;
    let settings_target_changed = if persisted.schema_version == 2 {
        persisted.settings_hash.as_deref() != Some(settings.path_hash().as_str())
            || settings.target_fingerprint()? != persisted.settings_target_hash
    } else {
        hash_settings_target(&settings_path) != persisted.settings_target_hash
    };
    if settings_target_changed {
        return Err("asset_operation_stale: MUX settings changed while preparing commit".into());
    }
    if persisted.schema_version >= 3 {
        // The settings lock is still held. Recompute the semantic revisions
        // after snapshot capture so a non-cooperating write between the first
        // check and rollback preparation cannot escape review.
        AssetStateStore::load()?.verify(&persisted.state_preconditions)?;
    }

    let snapshots_by_path = snapshots
        .iter()
        .map(|snapshot| (snapshot.path.clone(), snapshot))
        .collect::<BTreeMap<_, _>>();
    for target in &persisted.plan.target_files {
        let path = crate::resources::mcp::scanner::expand_tilde(target);
        let snapshot = snapshots_by_path.get(&path).ok_or_else(|| {
            "asset_operation_stale: reviewed Agent target snapshot is missing".to_string()
        })?;
        let expected = persisted.target_hashes.get(target).ok_or_else(|| {
            "asset_operation_stale: reviewed Agent target fingerprint is missing".to_string()
        })?;
        if snapshot.target_fingerprint()? != *expected {
            return Err(
                "asset_operation_stale: an Agent target changed while preparing commit".into(),
            );
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
    let source = fs::canonicalize(source)
        .map_err(|_| "Skills migration source is unavailable".to_string())?;
    if !source.is_dir() {
        return Err("Skills migration source is not a directory".into());
    }
    create_transaction_symlink_if_missing(&destination, &source)
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

fn apply_domain_plan(
    operation: &AssetOperationPlan,
    release_orphaned_relationships: bool,
    skills_lock: &SkillsOperationLock,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let plan = &operation.domain_plan;
    match plan {
        DomainPlan::Mcp { before, after } => apply_mcp(
            operation,
            before,
            after,
            release_orphaned_relationships,
            blocked_agents,
        ),
        DomainPlan::Model { before, after } => apply_model(
            operation,
            before,
            after,
            &model_relationship_releases(operation),
            &BTreeSet::new(),
            blocked_agents,
        ),
        DomainPlan::Skill { before, after } => apply_skill(
            operation,
            before,
            after,
            release_orphaned_relationships,
            skills_lock,
            blocked_agents,
        ),
        DomainPlan::AgentCapabilities { .. } => {
            Err("asset operation requires a configuration lifecycle".into())
        }
    }
}

/// A pure relationship removal may release ownership when the Agent bytes no
/// longer match MUX authority. Synchronized entries are still removed
/// normally; missing central metadata or drifted bytes are preserved as
/// external observations. Toggle and update operations never enter this set.
fn model_relationship_releases(plan: &AssetOperationPlan) -> BTreeSet<(String, String)> {
    if !releases_relationship_ownership(plan) {
        return BTreeSet::new();
    }
    plan.relationship_changes
        .iter()
        .filter_map(|change| match (&change.asset, &change.action) {
            (AssetRef::Model { profile_id }, RelationshipAction::Remove) => {
                Some((change.agent_id.clone(), profile_id.clone()))
            }
            _ => None,
        })
        .collect()
}

fn apply_mcp(
    operation: &AssetOperationPlan,
    before: &BTreeMap<String, Vec<String>>,
    after: &BTreeMap<String, Vec<String>>,
    release_orphaned_relationships: bool,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
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
    .map_err(|error| error.to_string())?;
    let central_keys: BTreeSet<String> = read_registry()
        .into_iter()
        .map(|entry| entry.key())
        .collect();
    let exact_observed: BTreeSet<(String, String)> = ops::scan_installed(None)
        .into_iter()
        .filter(|item| item.scope == "global" && item.enabled && !item.customized)
        .map(|item| (item.agent, format!("{}::{}", item.name, item.transport)))
        .collect();
    for agent_id in union_keys(before, after) {
        if blocked_agents.contains(agent_id) {
            continue;
        }
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
        let result = (|| {
            for key in left.difference(&right) {
                if release_orphaned_relationships
                    && (!central_keys.contains(key)
                        || !exact_observed.contains(&(agent_id.clone(), key.clone())))
                {
                    continue;
                }
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
            // Reconcile the complete desired set, not only the relationship
            // delta. This lets an ordinary add/update repair a prior local
            // target incident instead of preserving missing historical entries.
            for key in &right {
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
                let enabled = settings
                    .mcp_consumptions
                    .as_ref()
                    .and_then(|records| records.get(agent_id))
                    .and_then(|records| records.get(key))
                    .is_none_or(|record| record.enabled);
                if !enabled {
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
            Ok::<_, String>(())
        })()
        .and_then(|_| {
            verify_agent_mcp_convergence(
                agent_id,
                &left,
                &right,
                &settings,
                release_orphaned_relationships,
            )
        });
        if result.is_ok() {
            clear_target_incidents(operation, agent_id)?;
        } else {
            record_target_incident(operation, agent_id, "target_convergence_failed")?;
        }
    }
    Ok(())
}

fn verify_agent_mcp_convergence(
    agent_id: &str,
    before: &BTreeSet<String>,
    desired: &BTreeSet<String>,
    settings: &crate::settings::Settings,
    release_orphaned_relationships: bool,
) -> Result<(), String> {
    let observed = ops::scan_installed(None)
        .into_iter()
        .filter(|item| item.scope == "global" && item.agent == agent_id)
        .map(|item| {
            (
                format!("{}::{}", item.name, item.transport),
                item.enabled,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for key in desired {
        let expected_enabled = settings
            .mcp_consumptions
            .as_ref()
            .and_then(|records| records.get(agent_id))
            .and_then(|records| records.get(key))
            .is_none_or(|record| record.enabled);
        if observed.get(key) != Some(&expected_enabled) {
            return Err(format!(
                "target_convergence_failed: {agent_id} MCP target does not contain {key} in its desired state"
            ));
        }
    }
    if !release_orphaned_relationships {
        for key in before.difference(desired) {
            if observed.contains_key(key) {
                return Err(format!(
                    "target_convergence_failed: {agent_id} MCP target still contains removed asset {key}"
                ));
            }
        }
    }
    Ok(())
}

fn apply_mcp_enabled(
    operation: &AssetOperationPlan,
    agent_id: &str,
    asset_key: &str,
    enabled: bool,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
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
    if !updated {
        return Err("MCP consumption disappeared during enabled-state update".into());
    }
    if blocked_agents.contains(agent_id) {
        return Ok(());
    }
    let (name, transport) = split_mcp_key(asset_key)?;
    let agents = [agent_id.to_string()];
    let result = if enabled {
        ops::enable(name, transport, "global", &agents, None)
    } else {
        ops::disable(name, transport, "global", &agents, None)
    }
    .and_then(|_| {
        let observed = ops::scan_installed(None).into_iter().find(|item| {
            item.scope == "global"
                && item.agent == agent_id
                && item.name == name
                && item.transport == transport
        });
        if observed.is_some_and(|item| item.enabled == enabled) {
            Ok(())
        } else {
            Err(vec![format!(
                "target_convergence_failed: {agent_id} MCP enabled state was not persisted"
            )])
        }
    });
    if result.is_ok() {
        clear_target_incidents(operation, agent_id)
    } else {
        record_target_incident(operation, agent_id, "target_convergence_failed")
    }
}

fn apply_skill_enabled(
    operation: &AssetOperationPlan,
    name: &str,
    target_id: &str,
    affected_agent_ids: &[String],
    enabled: bool,
    skills_lock: &SkillsOperationLock,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let persist_desired = || {
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
    };
    if affected_agent_ids
        .iter()
        .any(|agent_id| blocked_agents.contains(agent_id))
    {
        return persist_desired();
    }
    let result = (|| {
        let assignment_plan = plan_assignment(PlanAssignmentRequest {
            skill_name: name.to_string(),
            agent_ids: affected_agent_ids.to_vec(),
            enabled,
        })
        .map_err(|error| format!("{error:?}"))?;
        if let Err(error) =
            commit_assignment_in_asset_transaction(assignment_plan.confirmation(), skills_lock)
        {
            let _ =
                cancel_operation_in_asset_transaction(&assignment_plan.operation_id, skills_lock);
            return Err(format!("{error:?}"));
        }
        Ok::<_, String>(())
    })();
    // The Skill link engine also updates assignment settings. Normalize the
    // central desired record after that physical attempt so a failed target
    // still becomes pending convergence rather than losing user intent.
    persist_desired()?;
    for agent_id in affected_agent_ids {
        if result.is_ok() {
            clear_target_incidents(operation, agent_id)?;
        } else {
            record_target_incident(operation, agent_id, "target_convergence_failed")?;
        }
    }
    Ok(())
}

fn apply_model(
    operation: &AssetOperationPlan,
    before: &BTreeMap<String, ModelAgentSelection>,
    after: &BTreeMap<String, ModelAgentSelection>,
    relationship_releases: &BTreeSet<(String, String)>,
    preserve_agent_targets: &BTreeSet<String>,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let central_profile_ids: BTreeSet<String> = settings
        .model_profiles
        .iter()
        .flatten()
        .map(|(profile_id, _)| profile_id.clone())
        .collect();
    mutate_settings(|settings| {
        for agent_id in union_keys(before, after) {
            settings
                .set_model_selection(agent_id, after.get(agent_id).cloned().unwrap_or_default());
        }
    })
    .map_err(|error| error.to_string())?;
    for agent_id in union_keys(before, after) {
        let left = before.get(agent_id).cloned().unwrap_or_default();
        let right = after.get(agent_id).cloned().unwrap_or_default();
        if preserve_agent_targets.contains(agent_id) || blocked_agents.contains(agent_id) {
            continue;
        }
        let result = (|| {
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
                let release_relationship =
                    relationship_releases.contains(&(agent_id.to_string(), profile_id.clone()));
                if release_relationship && !central_profile_ids.contains(&profile_id) {
                    continue;
                }
                if let Err(error) = clear_profile_consumption(
                    agent_id,
                    &profile_id,
                    left.active_profile_id.as_deref() == Some(profile_id.as_str()),
                ) {
                    let released_as_external = release_relationship
                        && (error.starts_with("model_owned_fields_drift:")
                            || error.starts_with("model_target_conflicted:"));
                    if !released_as_external {
                        return Err(error);
                    }
                }
            }
            for (profile_id, record) in &right.profiles {
                let was_enabled = left
                    .profiles
                    .get(profile_id)
                    .is_some_and(|previous| previous.enabled);
                if record.enabled && !was_enabled {
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
            Ok::<_, String>(())
        })();
        if result.is_ok() {
            clear_target_incidents(operation, agent_id)?;
        } else {
            record_target_incident(operation, agent_id, "target_convergence_failed")?;
        }
    }
    Ok(())
}

fn apply_skill(
    operation: &AssetOperationPlan,
    before: &BTreeMap<String, Vec<String>>,
    after: &BTreeMap<String, Vec<String>>,
    release_orphaned_relationships: bool,
    skills_lock: &SkillsOperationLock,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let mut removals = BTreeMap::<String, BTreeSet<String>>::new();
    let mut additions = BTreeMap::<String, BTreeSet<String>>::new();
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
        for name in left.difference(&right) {
            removals
                .entry(name.clone())
                .or_default()
                .insert(agent_id.clone());
        }
        for name in right.difference(&left) {
            additions
                .entry(name.clone())
                .or_default()
                .insert(agent_id.clone());
        }
    }
    for (name, agent_ids) in removals {
        let settings = load_settings_strict().map_err(|error| error.to_string())?;
        if release_orphaned_relationships {
            let assignments =
                canonical_skill_assignments(&settings).map_err(|error| format!("{error:?}"))?;
            let assigned = assignments.get(&name).cloned().unwrap_or_default();
            let declared =
                declared_targets_for_agents(&agent_ids.iter().cloned().collect::<Vec<_>>())
                    .map_err(|error| format!("{error:?}"))?;
            let target_ids: BTreeSet<String> = assigned.intersection(&declared).cloned().collect();
            for target_id in target_ids {
                let target_agents = skill_target_agents(&target_id, &agent_ids);
                if target_agents
                    .iter()
                    .any(|agent_id| blocked_agents.contains(agent_id))
                {
                    continue;
                }
                let result =
                    release_assignment_safely(&name, &BTreeSet::from([target_id]), skills_lock)
                        .map_err(|error| format!("{error:?}"));
                record_skill_target_result(operation, &target_agents, result.is_ok())?;
            }
        } else {
            apply_skill_assignment(
                operation,
                &name,
                agent_ids,
                false,
                skills_lock,
                blocked_agents,
            )?;
        }
    }
    for (name, agent_ids) in additions {
        apply_skill_assignment(
            operation,
            &name,
            agent_ids,
            true,
            skills_lock,
            blocked_agents,
        )?;
    }
    persist_skill_desired(before, after)?;
    Ok(())
}

fn apply_skill_assignment(
    operation: &AssetOperationPlan,
    name: &str,
    agent_ids: BTreeSet<String>,
    enabled: bool,
    skills_lock: &SkillsOperationLock,
    blocked_agents: &BTreeSet<String>,
) -> Result<(), String> {
    let target_ids = normalize_agent_selection(&agent_ids.iter().cloned().collect::<Vec<_>>())
        .map_err(|error| format!("{error:?}"))?;
    for target_id in target_ids {
        let target_agents = skill_target_agents(&target_id, &agent_ids);
        if target_agents
            .iter()
            .any(|agent_id| blocked_agents.contains(agent_id))
        {
            continue;
        }
        let result = (|| {
            let plan = plan_assignment(PlanAssignmentRequest {
                skill_name: name.to_string(),
                agent_ids: target_agents.clone(),
                enabled,
            })
            .map_err(|error| format!("{error:?}"))?;
            if let Err(error) =
                commit_assignment_in_asset_transaction(plan.confirmation(), skills_lock)
            {
                let _ = cancel_operation_in_asset_transaction(&plan.operation_id, skills_lock);
                return Err(format!("{error:?}"));
            }
            Ok::<_, String>(())
        })();
        record_skill_target_result(operation, &target_agents, result.is_ok())?;
    }
    Ok(())
}

fn skill_target_agents(target_id: &str, fallback: &BTreeSet<String>) -> Vec<String> {
    crate::resources::skill::list_inventory()
        .ok()
        .and_then(|inventory| {
            inventory
                .targets
                .into_iter()
                .find(|target| target.target_id == target_id)
        })
        .map(|target| {
            if target.affected_agent_ids.is_empty() {
                target.primary_agent_ids
            } else {
                target.affected_agent_ids
            }
        })
        .filter(|agents| !agents.is_empty())
        .unwrap_or_else(|| fallback.iter().cloned().collect())
}

fn record_skill_target_result(
    operation: &AssetOperationPlan,
    agent_ids: &[String],
    success: bool,
) -> Result<(), String> {
    for agent_id in agent_ids {
        if success {
            clear_target_incidents(operation, agent_id)?;
        } else {
            record_target_incident(operation, agent_id, "target_convergence_failed")?;
        }
    }
    Ok(())
}

fn persist_skill_desired(
    before: &BTreeMap<String, Vec<String>>,
    after: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let touched = before
        .values()
        .flatten()
        .cloned()
        .chain(after.values().flatten().cloned())
        .collect::<BTreeSet<_>>();
    let mut desired = BTreeMap::new();
    for name in touched {
        let agents = after
            .iter()
            .filter(|(_, names)| names.contains(&name))
            .map(|(agent_id, _)| agent_id.clone())
            .collect::<Vec<_>>();
        let target_ids = normalize_agent_selection(&agents)
            .map_err(|error| format!("{error:?}"))?
            .into_iter()
            .collect::<BTreeSet<_>>();
        desired.insert(name, target_ids);
    }
    mutate_settings(|settings| {
        let assignments = settings.skill_assignments.get_or_insert_default();
        let consumptions = settings.skill_consumptions.get_or_insert_default();
        for (name, target_ids) in desired {
            if target_ids.is_empty() {
                assignments.remove(&name);
                consumptions.remove(&name);
                continue;
            }
            assignments.insert(name.clone(), target_ids.clone());
            let existing = consumptions.remove(&name).unwrap_or_default();
            let mut records = BTreeMap::new();
            for target_id in target_ids {
                records.insert(
                    target_id.clone(),
                    existing
                        .get(&target_id)
                        .cloned()
                        .unwrap_or(SkillConsumptionRecord {
                            name: name.clone(),
                            target_id,
                            enabled: true,
                        }),
                );
            }
            consumptions.insert(name, records);
        }
    })
    .map_err(|error| error.to_string())
}

fn verify_postcondition(
    plan: &AssetOperationPlan,
    lifecycle: Option<&LifecycleBinding>,
) -> Result<(), String> {
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
                    let invalid = item.enabled != Some(record.enabled)
                        || item.desired_active != Some(expected_active);
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

    for ((agent_id, asset), desired) in expected_effects(plan, lifecycle) {
        let consumption = inventory
            .consumptions
            .iter()
            .find(|item| item.agent_id == agent_id && item.asset == asset && item.desired);
        if desired {
            if consumption.is_none() {
                return Err(format!(
                    "asset post-commit verification failed: missing desired {asset:?} for {agent_id}"
                ));
            }
        } else if consumption.is_some() {
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
        return Err(format!(
            "asset post-commit verification failed for {agent_id}: expected {expected:?}, observed {actual:?}"
        ));
    }
    Ok(())
}

fn expected_effects(
    plan: &AssetOperationPlan,
    lifecycle: Option<&LifecycleBinding>,
) -> BTreeMap<(String, AssetRef), bool> {
    super::planner::effect_assets(
        &plan.domain_plan,
        &plan.central_changes,
        &plan.relationship_changes,
        &plan.consumption_state_changes,
        lifecycle,
    )
    .into_iter()
    .map(|(agent_id, asset)| {
        let desired = asset_desired_after(&plan.domain_plan, &agent_id, &asset);
        ((agent_id, asset), desired)
    })
    .collect()
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
        (DomainPlan::AgentCapabilities { skills_after, .. }, AssetRef::Skill { name }) => {
            skills_after
                .get(agent_id)
                .is_some_and(|names| names.contains(name))
        }
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
    parent: ParentDirectorySnapshot,
    kind: SnapshotKind,
    identity: PathIdentity,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurableSnapshotManifest {
    version: u32,
    snapshots: Vec<DurableSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DurableSnapshot {
    path: PathBuf,
    parent: ParentDirectorySnapshot,
    identity: PathIdentity,
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
        let parent = capture_parent_directory(path)?;
        let (kind, identity) = match read_path_state_anchored(path, &parent)? {
            AnchoredPathState::Missing => (SnapshotKind::Missing, PathIdentity::unknown()),
            AnchoredPathState::File {
                bytes,
                mode,
                identity,
            } => (SnapshotKind::File { bytes, mode }, identity),
            AnchoredPathState::Symlink { target, identity } => {
                (SnapshotKind::Symlink { target }, identity)
            }
            AnchoredPathState::Directory { identity } => (SnapshotKind::Directory, identity),
            AnchoredPathState::Other { .. } => {
                return Err(format!("unsupported target type: {}", path.display()));
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            parent,
            kind,
            identity,
        })
    }

    fn anchored_state(&self) -> AnchoredPathState {
        match &self.kind {
            SnapshotKind::Missing => AnchoredPathState::Missing,
            SnapshotKind::File { bytes, mode } => AnchoredPathState::File {
                bytes: bytes.clone(),
                mode: *mode,
                identity: self.identity,
            },
            SnapshotKind::Symlink { target } => AnchoredPathState::Symlink {
                target: target.clone(),
                identity: self.identity,
            },
            SnapshotKind::Directory => AnchoredPathState::Directory {
                identity: self.identity,
            },
        }
    }

    fn path_hash(&self) -> String {
        match &self.kind {
            SnapshotKind::Missing => "missing".into(),
            SnapshotKind::File { bytes, .. } => hex::encode(Sha256::digest(bytes)),
            SnapshotKind::Symlink { target } => {
                hex::encode(Sha256::digest(target.as_os_str().as_encoded_bytes()))
            }
            SnapshotKind::Directory => "directory".into(),
        }
    }

    fn target_fingerprint(&self) -> Result<String, String> {
        fingerprint_anchored_path_state(&self.anchored_state(), &self.parent)
    }

    fn from_transaction_state(
        path: &Path,
        parent: &ParentDirectorySnapshot,
        state: &TransactionPathState,
    ) -> Self {
        let kind = match state {
            TransactionPathState::Missing => SnapshotKind::Missing,
            TransactionPathState::File { bytes, mode, .. } => SnapshotKind::File {
                bytes: bytes.clone(),
                mode: *mode,
            },
            TransactionPathState::Symlink { target, .. } => SnapshotKind::Symlink {
                target: target.clone(),
            },
        };
        let identity = match state {
            TransactionPathState::Missing => PathIdentity::unknown(),
            TransactionPathState::File { identity, .. }
            | TransactionPathState::Symlink { identity, .. } => *identity,
        };
        Self {
            path: path.to_path_buf(),
            parent: parent.clone(),
            kind,
            identity,
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
            ) => {
                remove_bytes_if_unchanged_in_parent(&self.path, &self.parent, bytes, *expected_mode)
            }
            (SnapshotKind::Missing, SnapshotKind::Symlink { target }) => {
                remove_symlink_if_unchanged_in_parent(&self.path, &self.parent, target)
            }
            (
                SnapshotKind::File { bytes, mode },
                SnapshotKind::File {
                    bytes: expected,
                    mode: expected_mode,
                },
            ) => write_bytes_if_unchanged_in_parent(
                &self.path,
                &self.parent,
                Some((expected, *expected_mode)),
                bytes,
                *mode,
            ),
            (SnapshotKind::File { bytes, mode }, SnapshotKind::Missing) => {
                write_bytes_if_unchanged_in_parent(&self.path, &self.parent, None, bytes, *mode)
            }
            (SnapshotKind::Symlink { target }, SnapshotKind::Missing) => {
                write_symlink_if_unchanged_in_parent(&self.path, &self.parent, None, target)
            }
            (
                SnapshotKind::Symlink { target },
                SnapshotKind::Symlink {
                    target: expected_target,
                },
            ) => write_symlink_if_unchanged_in_parent(
                &self.path,
                &self.parent,
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
        let current = read_path_state_anchored(&self.path, &self.parent)?;
        if matches!(&current, AnchoredPathState::Other { .. }) {
            return Err(format!(
                "refusing to roll back {}: unsupported target type",
                self.path.display()
            ));
        }
        if anchored_states_match(&self.anchored_state(), &current) {
            return Ok(false);
        }
        let expected = expected.ok_or_else(|| {
            format!(
                "refusing to roll back {}: no transaction write evidence matches the changed target",
                self.path.display()
            )
        })?;
        if self.path != expected.path
            || self.parent != expected.parent
            || !anchored_states_match(&expected.anchored_state(), &current)
        {
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
            written_states.get(&snapshot.path).map(|state| {
                PathSnapshot::from_transaction_state(&snapshot.path, &snapshot.parent, state)
            })
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
            parent: snapshot.parent.clone(),
            identity: snapshot.identity,
            kind,
        });
    }
    let manifest = serde_json::to_vec_pretty(&DurableSnapshotManifest {
        version: ROLLBACK_MANIFEST_VERSION,
        snapshots: durable,
    })
    .map_err(|error| error.to_string())?;
    // Manifest is written last: its presence proves every referenced backup is
    // durable before the first target mutation begins.
    write_private_file(&root.join("manifest.json"), &manifest)?;
    fs::File::open(&root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    if let Some(parent) = root.parent() {
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
    if manifest.version != ROLLBACK_MANIFEST_VERSION {
        return Err("recovery_required: unsupported asset rollback manifest".into());
    }
    let mut snapshots = Vec::with_capacity(manifest.snapshots.len());
    for snapshot in manifest.snapshots {
        #[cfg(unix)]
        if !matches!(&snapshot.kind, DurableSnapshotKind::Missing) && !snapshot.identity.is_exact()
        {
            return Err("recovery_required: rollback snapshot has no exact target identity".into());
        }
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
            parent: snapshot.parent,
            kind,
            identity: snapshot.identity,
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

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync directory {}: {error}", path.display()))
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
    use serde_json::Value;
    use crate::assets::{
        plan_set_active_model, plan_set_agent_consumption, plan_set_all_mcp_enabled,
        plan_set_mcp_enabled, plan_set_model_enabled, plan_update_central_asset,
        AgentConsumptionSelection, CentralAssetAction, CentralAssetDraft,
        PlanSetActiveModelRequest, PlanSetAgentConsumptionRequest, PlanSetAllMcpEnabledRequest,
        PlanSetMcpEnabledRequest, PlanSetModelEnabledRequest, PlanUpdateCentralAssetRequest,
    };
    use crate::domain::assets::ConsumptionStatus;
    use crate::domain::types::{
        ModelProfile, ModelProtocol, ModelProviderConfig, ModelProviderProtocolConfig,
        RegistryConfig, RegistryEntry, SourceDef, StdioConfig,
    };
    use crate::resources::mcp::registry::{read_registry_all, write_manual_entry};
    use crate::resources::mcp::sources::cached_path;
    use crate::resources::model::save_profile;
    use crate::testenv::TestHome;

    #[test]
    fn clear_agent_models_removes_native_pi_registry_even_without_mux_relationships() {
        let home = TestHome::new("clear-agent-models-native-pi");
        let models = home.home.join(".pi/agent/models.json");
        let settings = home.home.join(".pi/agent/settings.json");
        fs::create_dir_all(models.parent().unwrap()).unwrap();
        fs::write(
            &models,
            r#"{"providers":{"manual":{"models":[{"id":"external"}]}}}"#,
        )
        .unwrap();
        fs::write(
            &settings,
            r#"{"theme":"dark","defaultProvider":"manual","defaultModel":"external"}"#,
        )
        .unwrap();

        let plan = crate::assets::plan_clear_agent_models(
            crate::domain::assets::PlanClearAgentModelsRequest { agent_id: "pi".into() },
        )
        .unwrap();
        assert_eq!(plan.kind, AssetOperationKind::ClearModels);
        assert!(plan.can_commit, "{:?}", plan.warnings);
        assert!(plan.warnings.iter().any(|warning| warning.contains("external")));

        let result = commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        })
        .unwrap();
        assert!(result.converged);
        let models: Value = serde_json::from_str(&fs::read_to_string(models).unwrap()).unwrap();
        let settings: Value = serde_json::from_str(&fs::read_to_string(settings).unwrap()).unwrap();
        assert_eq!(models["providers"], serde_json::json!({}));
        assert!(settings.get("defaultProvider").is_none());
        assert!(settings.get("defaultModel").is_none());
        assert_eq!(settings["theme"], "dark");
    }

    fn model(model: &str) -> ModelProfile {
        ModelProfile {
            id: "work".into(),
            provider_id: Some("custom-provider".into()),
            name: "Work".into(),
            provider: "custom".into(),
            model_vendor: None,
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://example.invalid/v1".into(),
            endpoint_path: String::new(),
            model: model.into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        }
    }

    fn shared_provider_models() -> (ModelProviderConfig, ModelProfile, ModelProfile) {
        let provider = ModelProviderConfig {
            id: "shared-provider".into(),
            name: "Shared Provider".into(),
            provider: "custom".into(),
            base_url: "https://old.example.test".into(),
            protocols: BTreeMap::from([
                (
                    ModelProtocol::OpenaiResponses,
                    ModelProviderProtocolConfig {
                        endpoint_path: "/v1/responses".into(),
                    },
                ),
                (
                    ModelProtocol::AnthropicMessages,
                    ModelProviderProtocolConfig {
                        endpoint_path: "/anthropic/v1/messages".into(),
                    },
                ),
            ]),
            env_key: None,
        };
        let first = ModelProfile {
            id: "shared-openai".into(),
            provider_id: Some(provider.id.clone()),
            name: "Shared OpenAI".into(),
            provider: provider.provider.clone(),
            model_vendor: Some("openai".into()),
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: provider.base_url.clone(),
            endpoint_path: provider.protocols[&ModelProtocol::OpenaiResponses]
                .endpoint_path
                .clone(),
            model: "gpt-shared".into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(true),
        };
        let second = ModelProfile {
            id: "shared-anthropic".into(),
            provider_id: Some(provider.id.clone()),
            name: "Shared Anthropic".into(),
            model_vendor: Some("anthropic".into()),
            protocol: ModelProtocol::AnthropicMessages,
            endpoint_path: provider.protocols[&ModelProtocol::AnthropicMessages]
                .endpoint_path
                .clone(),
            model: "claude-shared".into(),
            ..first.clone()
        };
        (provider, first, second)
    }

    fn install_shared_provider_models() -> (ModelProviderConfig, ModelProfile, ModelProfile) {
        let (provider, first, second) = shared_provider_models();
        save_profile(first.clone(), Some("old-secret".into())).unwrap();
        save_profile(second.clone(), Some("old-secret".into())).unwrap();
        (provider, first, second)
    }

    fn parent_snapshots_for_paths(paths: &[PathBuf]) -> BTreeMap<PathBuf, ParentDirectorySnapshot> {
        paths
            .iter()
            .map(|path| (path.clone(), capture_parent_directory(path).unwrap()))
            .collect()
    }

    fn parent_snapshots_for_snapshots(
        snapshots: &[PathSnapshot],
    ) -> BTreeMap<PathBuf, ParentDirectorySnapshot> {
        snapshots
            .iter()
            .map(|snapshot| (snapshot.path.clone(), snapshot.parent.clone()))
            .collect()
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

    #[cfg(unix)]
    #[test]
    fn rollback_refuses_a_replaced_skill_parent_without_touching_the_symlink_destination() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("transaction-skill-parent-swap");
        let parent = home.home.join(".agents/skills");
        fs::create_dir_all(&parent).unwrap();
        let target = parent.join("reviewed-skill");
        let central = home.home.join(".mux/skills/reviewed-skill");
        let original = PathSnapshot::capture_link(&target).unwrap();
        let paths = vec![target.clone()];
        let tracker = begin_transaction_write_tracking(
            &home.home.join("write-evidence"),
            &paths,
            &parent_snapshots_for_paths(&paths),
        )
        .unwrap();
        set_transaction_symlink(&target, Some(&central)).unwrap();

        let retained_parent = home.home.join("retained-skills-parent");
        fs::rename(&parent, &retained_parent).unwrap();
        let outside = home.home.join("outside-skills-parent");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, &parent).unwrap();

        let written_states = tracker.states();
        drop(tracker);
        let errors = restore_snapshots_if_unchanged(&[original], &written_states);

        assert!(!errors.is_empty(), "a swapped parent must require recovery");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("asset_target_unsafe")),
            "{errors:?}"
        );
        assert!(!outside.join("reviewed-skill").exists());
        assert_eq!(
            fs::read_link(retained_parent.join("reviewed-skill")).unwrap(),
            central
        );
    }

    #[cfg(unix)]
    #[test]
    fn forward_skill_link_refuses_a_parent_swap_after_snapshot() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("transaction-forward-skill-parent-swap");
        let parent = home.home.join(".agents/skills");
        fs::create_dir_all(&parent).unwrap();
        let target = parent.join("reviewed-skill");
        let central = home.home.join(".mux/skills/reviewed-skill");
        fs::create_dir_all(&central).unwrap();
        let paths = vec![target.clone()];
        let tracker = begin_transaction_write_tracking(
            &home.home.join("write-evidence"),
            &paths,
            &parent_snapshots_for_paths(&paths),
        )
        .unwrap();

        let retained_parent = home.home.join("retained-skills-parent");
        fs::rename(&parent, &retained_parent).unwrap();
        let outside = home.home.join("outside-skills-parent");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, &parent).unwrap();

        let error = create_transaction_symlink_if_missing(&target, &central).unwrap_err();
        drop(tracker);

        assert!(error.contains("asset_target_unsafe"), "{error}");
        assert!(!outside.join("reviewed-skill").exists());
        assert!(!retained_parent.join("reviewed-skill").exists());
    }

    #[test]
    fn rollback_preserves_an_external_edit_before_rollback_preparation() {
        let home = TestHome::new("transaction-rollback-cas");
        let target = home.home.join("config.json");
        fs::write(&target, "original").unwrap();
        let original = PathSnapshot::capture(&target).unwrap();
        let evidence = home.home.join("write-evidence");
        let paths = std::slice::from_ref(&target);
        let tracker =
            begin_transaction_write_tracking(&evidence, paths, &parent_snapshots_for_paths(paths))
                .unwrap();
        crate::safe_write::write_if_unchanged(&target, Some("original"), "mux-partial").unwrap();
        fs::write(&target, "external-edit").unwrap();
        let states = tracker.states();
        drop(tracker);
        let expected = states
            .get(&target)
            .map(|state| PathSnapshot::from_transaction_state(&target, &original.parent, state));

        let error = original.restore_if_owned(expected.as_ref()).unwrap_err();

        assert!(error.contains("changed after MUX wrote it"), "{error}");
        assert_eq!(fs::read_to_string(target).unwrap(), "external-edit");
    }

    #[test]
    fn settings_edit_after_snapshot_is_never_written_over_or_rolled_back() {
        let home = TestHome::new("transaction-settings-post-snapshot-race");
        mutate_settings(|settings| settings.imported = Some("reviewed".into())).unwrap();
        let path = settings_file();
        let snapshots = vec![PathSnapshot::capture(&path).unwrap()];
        let tracked_paths = vec![path.clone()];
        let tracker = begin_transaction_write_tracking(
            &home.home.join("write-evidence"),
            &tracked_paths,
            &parent_snapshots_for_snapshots(&snapshots),
        )
        .unwrap();
        let external = fs::read_to_string(&path)
            .unwrap()
            .replace("reviewed", "external-update");
        fs::write(&path, &external).unwrap();

        let error = mutate_settings(|settings| settings.state = Some(serde_json::json!("mux")))
            .unwrap_err();
        assert!(
            error.to_string().contains("reviewed target changed"),
            "{error}"
        );
        let states = tracker.states();
        drop(tracker);

        let rollback_errors = restore_snapshots_if_unchanged(&snapshots, &states);
        assert_eq!(rollback_errors.len(), 1, "{rollback_errors:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), external);
    }

    #[test]
    fn rollback_restores_a_file_when_the_cas_state_matches() {
        let home = TestHome::new("transaction-rollback-success");
        let target = home.home.join("config.json");
        fs::write(&target, "original").unwrap();
        let original = PathSnapshot::capture(&target).unwrap();
        let evidence = home.home.join("write-evidence");
        let paths = std::slice::from_ref(&target);
        let tracker =
            begin_transaction_write_tracking(&evidence, paths, &parent_snapshots_for_paths(paths))
                .unwrap();
        crate::safe_write::write_if_unchanged(&target, Some("original"), "mux-partial").unwrap();
        let states = tracker.states();
        drop(tracker);
        let expected = states
            .get(&target)
            .map(|state| PathSnapshot::from_transaction_state(&target, &original.parent, state));

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
        let tracker = begin_transaction_write_tracking(
            &home.home.join("write-evidence"),
            &tracked_paths,
            &parent_snapshots_for_snapshots(&snapshots),
        )
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
    fn commit_rejects_a_leaf_edit_after_precondition_verification() {
        let _home = TestHome::new("consume-post-verify-leaf-race");
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
        let target = crate::resources::mcp::scanner::expand_tilde(&plan.target_files[0]);
        let external = r#"{"mcpServers":{"external":{"command":"must-survive"}}}"#;

        let error = commit_asset_operation_with_hook(
            AssetCommitRequest {
                operation_id: plan.operation_id,
                candidate_hash: plan.candidate_hash,
            },
            || {
                fs::create_dir_all(target.parent().unwrap()).map_err(|error| error.to_string())?;
                fs::write(&target, external).map_err(|error| error.to_string())
            },
        )
        .unwrap_err();

        assert!(error.contains("changed while preparing commit"), "{error}");
        assert_eq!(fs::read_to_string(target).unwrap(), external);
    }

    #[test]
    fn foreign_rollback_manifest_does_not_block_unrelated_asset_commit() {
        let _home = TestHome::new("consume-foreign-recovery");
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: None,
                entry: Box::new(RegistryEntry {
                    name: "blocked-by-recovery".into(),
                    description: String::new(),
                    tags: Vec::new(),
                    config: RegistryConfig {
                        stdio: Some(StdioConfig {
                            command: "blocked-server".into(),
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
        let foreign_id = uuid::Uuid::new_v4().to_string();
        let foreign_rollback = operation_root(&foreign_id).join("rollback");
        fs::create_dir_all(&foreign_rollback).unwrap();
        fs::write(foreign_rollback.join("manifest.json"), b"{}").unwrap();

        commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        })
        .unwrap();

        assert!(
            read_registry()
                .into_iter()
                .any(|entry| entry.name == "blocked-by-recovery"),
            "the unrelated central asset must remain writable"
        );
        assert!(foreign_rollback.join("manifest.json").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_asset_rollback_marker_fails_closed() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("consume-unsafe-recovery");
        let foreign_id = uuid::Uuid::new_v4().to_string();
        let foreign_rollback = operation_root(&foreign_id).join("rollback");
        fs::create_dir_all(&foreign_rollback).unwrap();
        let outside = home.home.join("outside-manifest");
        fs::write(&outside, b"must-not-be-read").unwrap();
        symlink(&outside, foreign_rollback.join("manifest.json")).unwrap();

        let error = recover_pending_asset_operations().unwrap_err();

        assert!(error.starts_with("recovery_required:"), "{error}");
        assert_eq!(fs::read(outside).unwrap(), b"must-not-be-read");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_asset_operation_root_cannot_hide_pending_recovery() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("consume-root-symlink");
        let staging = home.home.join(".mux/staging");
        fs::create_dir_all(&staging).unwrap();
        let outside = home.home.join("outside-consumption");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, staging.join("consumption")).unwrap();

        let error = recover_pending_asset_operations().unwrap_err();

        assert!(error.starts_with("recovery_required:"), "{error}");
        assert!(outside.read_dir().unwrap().next().is_none());
    }

    #[test]
    fn subscribed_mcp_edit_creates_manual_override_without_mutating_source_cache() {
        let _home = TestHome::new("consume-subscription-override");
        let source = SourceDef::new_remote(
            "team-catalog".into(),
            "Team catalog".into(),
            "https://example.invalid/mcp.json".into(),
            "json".into(),
            "2026-07-25T00:00:00Z".into(),
        );
        mutate_settings(|settings| {
            settings
                .sources
                .get_or_insert_default()
                .push(source.clone());
        })
        .unwrap();
        let source_path = cached_path(&source).unwrap();
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        let subscribed = r#"{"mcpServers":{"shared":{"command":"npx","args":["shared-mcp"]}}}"#;
        fs::write(&source_path, subscribed).unwrap();

        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: Some("shared::stdio".into()),
                entry: Box::new(RegistryEntry {
                    name: "shared".into(),
                    description: "Personal credentials".into(),
                    tags: Vec::new(),
                    config: RegistryConfig {
                        stdio: Some(StdioConfig {
                            command: "npx".into(),
                            args: Some(vec!["shared-mcp".into()]),
                            env: Some(HashMap::from([(
                                "SHARED_API_KEY".into(),
                                "user-secret".into(),
                            )])),
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
        assert_eq!(plan.central_changes[0].action, CentralAssetAction::Create);

        commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        })
        .unwrap();

        assert_eq!(fs::read_to_string(source_path).unwrap(), subscribed);
        let copies = read_registry_all()
            .into_iter()
            .filter(|item| item.entry.key() == "shared::stdio")
            .collect::<Vec<_>>();
        assert_eq!(copies.len(), 2);
        let effective = copies.iter().find(|item| item.in_effect).unwrap();
        assert_eq!(effective.entry.origin.as_ref().unwrap().kind, "manual");
        assert_eq!(
            effective
                .entry
                .config
                .stdio
                .as_ref()
                .unwrap()
                .env
                .as_ref()
                .unwrap()
                .get("SHARED_API_KEY")
                .map(String::as_str),
            Some("user-secret"),
        );
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
    fn bulk_mcp_enabled_toggle_updates_every_managed_relationship() {
        let _home = TestHome::new("consume-enabled-toggle-all");
        for name in ["alpha", "beta"] {
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
        let added = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["alpha::stdio".into(), "beta::stdio".into()],
            },
        })
        .unwrap();
        commit_asset_operation(AssetCommitRequest {
            operation_id: added.operation_id,
            candidate_hash: added.candidate_hash,
        })
        .unwrap();

        for enabled in [false, true] {
            let plan = plan_set_all_mcp_enabled(PlanSetAllMcpEnabledRequest {
                agent_id: "claude-code".into(),
                enabled,
            })
            .unwrap();
            assert_eq!(plan.consumption_state_changes.len(), 2);
            let inventory = commit_asset_operation(AssetCommitRequest {
                operation_id: plan.operation_id,
                candidate_hash: plan.candidate_hash,
            })
            .unwrap();
            let rows = inventory
                .consumptions
                .iter()
                .filter(|item| {
                    item.agent_id == "claude-code" && matches!(item.asset, AssetRef::Mcp { .. })
                })
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 2);
            assert!(rows.iter().all(|item| item.enabled == Some(enabled)));
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
            &parent_snapshots_for_snapshots(&snapshots),
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
    fn startup_recovery_localizes_an_externally_changed_target() {
        let home = TestHome::new("consume-target-incident");
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
            &parent_snapshots_for_snapshots(&snapshots),
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
        let external = r#"{"mcpServers":{"external":{"command":"must-survive"}}}"#;
        fs::write(&target, external).unwrap();

        assert!(recover_pending_asset_operations().unwrap().is_empty());

        assert_eq!(fs::read_to_string(&target).unwrap(), external);
        let settings = load_settings_strict().unwrap();
        assert!(settings.mcp_consumptions.unwrap()["claude-code"].contains_key("local::stdio"));
        let incidents = settings.target_incidents.unwrap();
        assert_eq!(incidents.len(), 1);
        let incident = incidents.values().next().unwrap();
        assert_eq!(incident.capability, AssetCapability::Mcp);
        assert_eq!(incident.affected_agent_ids, ["claude-code"]);
        assert_eq!(incident.code, "target_recovery_required");
        assert!(operation_root(&plan.operation_id)
            .join(TARGET_INCIDENT_MARKER)
            .is_file());
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
            &parent_snapshots_for_snapshots(&snapshots),
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
        assert_eq!(
            recover_pending_asset_operations().unwrap(),
            vec![plan.operation_id]
        );
        assert!(!settings_path.exists());
    }

    #[test]
    fn commit_marker_crosses_every_durability_barrier_before_success() {
        let _home = TestHome::new("consume-commit-marker-durability");
        let operation_id = uuid::Uuid::new_v4().to_string();
        fs::create_dir_all(operation_root(&operation_id)).unwrap();
        let marker = operation_commit_marker(&operation_id);
        let mut observed = Vec::new();

        let error = mark_operation_committed_with_barrier_hook(&operation_id, |barrier| {
            assert_eq!(fs::read(&marker).unwrap(), b"committed\n");
            observed.push(barrier);
            if barrier == CommitMarkerDurabilityBarrier::OperationDirectory {
                return Err("simulated parent-directory sync interruption".into());
            }
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error, "simulated parent-directory sync interruption");
        assert_eq!(
            observed,
            vec![
                CommitMarkerDurabilityBarrier::File,
                CommitMarkerDurabilityBarrier::OperationDirectory,
            ]
        );

        observed.clear();
        mark_operation_committed_with_barrier_hook(&operation_id, |barrier| {
            observed.push(barrier);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            observed,
            vec![
                CommitMarkerDurabilityBarrier::File,
                CommitMarkerDurabilityBarrier::OperationDirectory,
                CommitMarkerDurabilityBarrier::ParentDirectory,
            ]
        );
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
        let skills_guard = acquire_asset_skills_lock().unwrap();
        apply_operation(&persisted, &skills_guard, &BTreeSet::new()).unwrap();
        verify_operation(&persisted).unwrap();
        mark_operation_committed(&plan.operation_id).unwrap();
        drop(skills_guard);

        recover_pending_asset_operations().unwrap();
        assert!(target.exists());
        assert!(
            load_settings_strict().unwrap().mcp_consumptions.unwrap()["claude-code"]
                .contains_key("local::stdio")
        );
        assert!(!operation_root(&plan.operation_id).exists());
    }

    #[test]
    fn model_recovery_preserves_the_provider_keychain_value_before_commit_marker() {
        let _home = TestHome::new("consume-model-keychain-rollback");
        save_profile(model("old-model"), Some("old-secret".into())).unwrap();
        let settings_before = fs::read(settings_file()).unwrap();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some("work".into()),
                profile: Box::new(model("new-model")),
                credential: None,
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
            &parent_snapshots_for_snapshots(&snapshots),
        )
        .unwrap();
        let skills_guard = acquire_asset_skills_lock().unwrap();
        apply_operation(&persisted, &skills_guard, &BTreeSet::new()).unwrap();
        drop(tracker);
        drop(skills_guard);
        assert_eq!(credential_snapshot("work").unwrap(), b"old-secret");

        recover_pending_asset_operations().unwrap();
        assert_eq!(fs::read(settings_file()).unwrap(), settings_before);
        assert_eq!(credential_snapshot("work").unwrap(), b"old-secret");
        assert!(!operation_root(&plan.operation_id).exists());
    }

    #[test]
    fn provider_commit_updates_every_child_model_and_credential() {
        let _home = TestHome::new("consume-provider-commit");
        let (mut provider, first, second) = install_shared_provider_models();
        provider.base_url = "https://new.example.test/".into();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::ModelProvider {
                existing_id: Some(provider.id.clone()),
                provider: Box::new(provider.clone()),
                credential: Some("new-secret".into()),
            },
        })
        .unwrap();

        commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        })
        .unwrap();

        let settings = load_settings_strict().unwrap();
        let profiles = settings.model_profiles.unwrap();
        assert_eq!(profiles[&first.id].base_url, "https://new.example.test");
        assert_eq!(profiles[&first.id].endpoint_path, "/v1/responses");
        assert_eq!(profiles[&second.id].base_url, "https://new.example.test");
        assert_eq!(profiles[&second.id].endpoint_path, "/anthropic/v1/messages");
        assert_eq!(credential_snapshot(&first.id).unwrap(), b"new-secret");
        assert_eq!(credential_snapshot(&second.id).unwrap(), b"new-secret");
    }

    #[test]
    fn provider_update_repairs_only_its_reviewed_child_profiles() {
        let home = TestHome::new("consume-provider-scoped-children");
        let (mut provider, mut first, mut second) = install_shared_provider_models();
        provider.env_key = Some("SHARED_PROVIDER_API_KEY".into());
        first.env_key = provider.env_key.clone();
        second.env_key = provider.env_key.clone();
        mutate_settings(|settings| {
            settings
                .model_providers
                .get_or_insert_default()
                .insert(provider.id.clone(), provider.clone());
            let profiles = settings.model_profiles.get_or_insert_default();
            profiles.insert(first.id.clone(), first.clone());
            profiles.insert(second.id.clone(), second.clone());
        })
        .unwrap();
        let mut third = model("third-model");
        third.id = "third-profile".into();
        third.provider_id = Some("other-provider".into());
        third.name = "Third Profile".into();
        save_profile(third.clone(), None).unwrap();

        let assignment = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "grok-build".into(),
            selection: AgentConsumptionSelection::Model {
                profile_ids: vec![first.id.clone(), second.id.clone(), third.id.clone()],
            },
        })
        .unwrap();
        commit_asset_operation(AssetCommitRequest {
            operation_id: assignment.operation_id,
            candidate_hash: assignment.candidate_hash,
        })
        .unwrap();

        let target = home.home.join(".grok/config.toml");
        let drifted = fs::read_to_string(&target)
            .unwrap()
            .replace("gpt-shared", "first-reviewed-drift")
            .replace("third-model", "third-unreviewed-drift");
        fs::write(&target, &drifted).unwrap();

        provider.base_url = "https://new.example.test".into();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::ModelProvider {
                existing_id: Some(provider.id.clone()),
                provider: Box::new(provider),
                credential: None,
            },
        })
        .unwrap();
        assert!(!plan.can_commit, "{:?}", plan.warnings);
        assert!(plan.central_changes.iter().any(|change| change.asset
            == (AssetRef::Model {
                profile_id: first.id.clone(),
            })));
        assert!(plan.central_changes.iter().any(|change| change.asset
            == (AssetRef::Model {
                profile_id: second.id.clone(),
            })));
        assert!(!plan.central_changes.iter().any(|change| change.asset
            == (AssetRef::Model {
                profile_id: third.id.clone(),
            })));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains(&format!("model:{}", first.id))));
        assert!(!plan
            .warnings
            .iter()
            .any(|warning| warning.contains(&format!("model:{}", third.id))));

        let error = commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        })
        .unwrap_err();
        assert!(error.starts_with("asset_operation_blocked:"));
        let updated = fs::read_to_string(target).unwrap();
        assert!(updated.contains("first-reviewed-drift"));
        assert!(updated.contains("third-unreviewed-drift"));
        assert!(!updated.contains("third-model"));
    }

    #[test]
    fn provider_update_skips_a_disabled_child_consumer_and_preserves_its_payload() {
        let home = TestHome::new("consume-provider-disabled-child");
        let (mut provider, mut first, mut second) = install_shared_provider_models();
        provider.env_key = Some("SHARED_PROVIDER_API_KEY".into());
        first.env_key = provider.env_key.clone();
        second.env_key = provider.env_key.clone();
        mutate_settings(|settings| {
            settings
                .model_providers
                .get_or_insert_default()
                .insert(provider.id.clone(), provider.clone());
            let profiles = settings.model_profiles.get_or_insert_default();
            profiles.insert(first.id.clone(), first.clone());
            profiles.insert(second.id.clone(), second.clone());
        })
        .unwrap();

        let assignment = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "grok-build".into(),
            selection: AgentConsumptionSelection::Model {
                profile_ids: vec![first.id.clone(), second.id.clone()],
            },
        })
        .unwrap();
        commit_asset_operation(AssetCommitRequest {
            operation_id: assignment.operation_id,
            candidate_hash: assignment.candidate_hash,
        })
        .unwrap();

        let make_first_active = plan_set_active_model(PlanSetActiveModelRequest {
            agent_id: "grok-build".into(),
            profile_id: first.id.clone(),
        })
        .unwrap();
        commit_asset_operation(AssetCommitRequest {
            operation_id: make_first_active.operation_id,
            candidate_hash: make_first_active.candidate_hash,
        })
        .unwrap();

        let target = home.home.join(".grok/config.toml");
        let managed = fs::read_to_string(&target).unwrap();
        let disable = plan_set_model_enabled(PlanSetModelEnabledRequest {
            agent_id: "grok-build".into(),
            profile_id: second.id.clone(),
            enabled: false,
        })
        .unwrap();
        commit_asset_operation(AssetCommitRequest {
            operation_id: disable.operation_id,
            candidate_hash: disable.candidate_hash,
        })
        .unwrap();
        let disabled_stray = managed.replace("claude-shared", "disabled-child-customization");
        fs::write(&target, &disabled_stray).unwrap();

        provider.base_url = "https://new.example.test".into();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::ModelProvider {
                existing_id: Some(provider.id.clone()),
                provider: Box::new(provider),
                credential: None,
            },
        })
        .unwrap();
        assert!(plan.can_commit, "{:?}", plan.warnings);
        assert!(!plan
            .warnings
            .iter()
            .any(|warning| warning.contains(&format!("model:{}", second.id))));

        let inventory = commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        })
        .unwrap();
        let updated = fs::read_to_string(target).unwrap();
        assert!(updated.contains("disabled-child-customization"));
        assert!(inventory.consumptions.iter().any(|item| {
            item.agent_id == "grok-build"
                && item.asset
                    == (AssetRef::Model {
                        profile_id: second.id.clone(),
                    })
                && item.enabled == Some(false)
                && item.status == ConsumptionStatus::ExternalChanged
                && item.reason.as_deref() == Some("model_disabled_state_drift")
        }));
        let profiles = load_settings_strict().unwrap().model_profiles.unwrap();
        assert_eq!(profiles[&second.id].base_url, "https://new.example.test");
    }

    #[test]
    fn provider_recovery_rolls_back_every_child_model_and_credential() {
        let _home = TestHome::new("consume-provider-keychain-rollback");
        let (mut provider, first, second) = install_shared_provider_models();
        let settings_before = fs::read(settings_file()).unwrap();
        provider.base_url = "https://new.example.test".into();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::ModelProvider {
                existing_id: Some(provider.id.clone()),
                provider: Box::new(provider.clone()),
                credential: Some("new-secret".into()),
            },
        })
        .unwrap();
        let persisted = load_operation(&plan.operation_id).unwrap();
        let snapshots = vec![PathSnapshot::capture(&settings_file()).unwrap()];
        let subject = provider_credential_subject(&provider.id);
        let old_credential = credential_snapshot(&subject);
        persist_credential_rollback(&plan.operation_id, &subject, old_credential.as_deref())
            .unwrap();
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        let tracked_paths = vec![settings_file()];
        let tracker = begin_transaction_write_tracking(
            &transaction_write_evidence_dir(&plan.operation_id),
            &tracked_paths,
            &parent_snapshots_for_snapshots(&snapshots),
        )
        .unwrap();
        let skills_guard = acquire_asset_skills_lock().unwrap();
        apply_operation(&persisted, &skills_guard, &BTreeSet::new()).unwrap();
        drop(tracker);
        drop(skills_guard);
        assert_eq!(credential_snapshot(&first.id).unwrap(), b"new-secret");
        assert_eq!(credential_snapshot(&second.id).unwrap(), b"new-secret");

        recover_pending_asset_operations().unwrap();
        assert_eq!(fs::read(settings_file()).unwrap(), settings_before);
        assert_eq!(credential_snapshot(&first.id).unwrap(), b"old-secret");
        assert_eq!(credential_snapshot(&second.id).unwrap(), b"old-secret");
        assert!(!operation_root(&plan.operation_id).exists());
    }

    #[test]
    fn model_recovery_finalizes_metadata_without_mutating_provider_credential() {
        let _home = TestHome::new("consume-model-keychain-finalize");
        save_profile(model("old-model"), Some("old-secret".into())).unwrap();
        let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some("work".into()),
                profile: Box::new(model("new-model")),
                credential: None,
            },
        })
        .unwrap();
        let persisted = load_operation(&plan.operation_id).unwrap();
        let snapshots = vec![PathSnapshot::capture(&settings_file()).unwrap()];
        let old_credential = credential_snapshot("work");
        persist_credential_rollback(&plan.operation_id, "work", old_credential.as_deref()).unwrap();
        persist_rollback_snapshots(&plan.operation_id, &snapshots).unwrap();
        let skills_guard = acquire_asset_skills_lock().unwrap();
        apply_operation(&persisted, &skills_guard, &BTreeSet::new()).unwrap();
        verify_operation(&persisted).unwrap();
        mark_operation_committed(&plan.operation_id).unwrap();
        drop(skills_guard);

        recover_pending_asset_operations().unwrap();
        assert_eq!(credential_snapshot("work").unwrap(), b"old-secret");
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
