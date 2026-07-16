use super::inventory::{declared_targets_for_agents, normalize_assignment_enable};
use super::source::{load_staged_resolution, stage_private_candidate};
use super::staging::StagingRoot;
use super::transaction::{acquire_skills_lock, validate_operation_id};
use super::{
    audit_skill, diff_trees, execute_transaction, findings_digest, has_pending_recovery, hash_tree,
    io_error, list_inventory, normalize_agent_selection, validate_candidate, DirectoryMutation,
    InventoryState, LinkMutation, LinkState, ManagedSkillRecord, OperationPlan,
    PlanAssignmentRequest, PlanImportRequest, PlanInstallRequest, PlannedLinkState, PlannedSkill,
    PlannedTarget, RiskLevel, SkillCommitRequest, SkillError, SkillOperationKind,
    SkillSettingsSnapshot, SkillSource, SkillSourceResolution, SkillTargetView, SkillUpdateState,
    SkillsInventory, SkillsPaths, TransactionOrder, TransactionSpec,
};
use crate::settings::{load_settings_strict, Settings};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

const PLAN_SCHEMA_VERSION: u32 = 1;
const PLAN_FILE: &str = "plan.json";
const MAX_PLAN_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PersistedPlan {
    schema_version: u32,
    plan: OperationPlan,
    input: PersistedPlanInput,
    expected_central: Vec<ExpectedCentral>,
    expected_links: Vec<ExpectedLink>,
    expected_target_roots: Vec<ExpectedTargetRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedPlanInput {
    Install {
        request: PlanInstallRequest,
        resolution: SkillSourceResolution,
    },
    Import {
        request: PlanImportRequest,
        source_target_id: String,
        source_name: String,
        original_path: String,
        backup_path: String,
    },
    Assignment {
        request: PlanAssignmentRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct ExpectedCentral {
    skill_name: String,
    content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedLink {
    skill_name: String,
    target_id: String,
    state: LinkState,
    desired_managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct ExpectedTargetRoot {
    target_id: String,
    root_path: String,
    anchor_path: String,
    anchor_device: u64,
    anchor_inode: u64,
    anchor_mode: u32,
    remaining_components: Vec<String>,
}

#[derive(Serialize)]
struct CandidateBinding<'a> {
    operation_id: &'a str,
    kind: &'a SkillOperationKind,
    skills: Vec<BoundSkill<'a>>,
    requested_agent_ids: Vec<&'a str>,
    target_ids: Vec<&'a str>,
    replace_conflicts: bool,
    assignment_enabled: Option<bool>,
    expected_central: &'a [ExpectedCentral],
    expected_links: &'a [ExpectedLink],
    expected_target_roots: &'a [ExpectedTargetRoot],
}

#[derive(Serialize)]
struct BoundSkill<'a> {
    name: &'a str,
    source: &'a SkillSource,
    resolved_revision: &'a Option<String>,
    content_hash: &'a str,
    replace_existing: bool,
}

pub fn plan_install(request: PlanInstallRequest) -> Result<OperationPlan, SkillError> {
    sanitize_result(plan_install_inner(request))
}

fn plan_install_inner(request: PlanInstallRequest) -> Result<OperationPlan, SkillError> {
    ensure_recovery_clear()?;
    validate_operation_id(&request.resolution_id)?;
    let paths = SkillsPaths::resolve_from_env()?;
    let resolution = load_staged_resolution(&paths, &request.resolution_id)?;
    let persisted = build_install_plan(request, resolution)?;
    persist_plan(&paths, &persisted)?;
    Ok(persisted.plan)
}

pub fn commit_install(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    sanitize_result(commit_plan(request, SkillOperationKind::Install))
}

pub fn plan_import(request: PlanImportRequest) -> Result<OperationPlan, SkillError> {
    sanitize_result(plan_import_inner(request))
}

fn plan_import_inner(request: PlanImportRequest) -> Result<OperationPlan, SkillError> {
    ensure_recovery_clear()?;
    let paths = SkillsPaths::resolve_from_env()?;
    let inventory = list_inventory()?;
    let external = external_item(&paths, &inventory, &request.identity)?;
    let operation_id = Uuid::new_v4().hyphenated().to_string();
    create_operation_root(&paths, &operation_id)?;
    let result = (|| {
        let candidates = StagingRoot::open(&paths)?
            .open_operation(&operation_id)?
            .create_private_directory("candidates")?;
        let staged = candidates.join(&external.name);
        let before_hash = hash_tree(&external.path)?;
        stage_private_candidate(&external.path, &staged)?;
        let staged_hash = hash_tree(&staged)?;
        if hash_tree(&external.path)? != before_hash || staged_hash != before_hash {
            return Err(SkillError::PlanStale {
                message: "the external Skill changed while it was staged".into(),
            });
        }
        let backup = paths.backups_skills_dir().join(format!(
            "import-{}-{}",
            crate::paths::backup_timestamp(),
            &operation_id[..8]
        ));
        let backup = backup.join(&external.name);
        let persisted = build_import_plan(
            request,
            operation_id.clone(),
            external.target_id,
            external.name,
            collapse_home(&external.path, paths.user_home()),
            collapse_home(&backup, paths.user_home()),
        )?;
        persist_plan(&paths, &persisted)?;
        Ok(persisted.plan)
    })();
    if result.is_err() {
        remove_unjournaled_operation(&paths, &operation_id)?;
    }
    result
}

pub fn commit_import(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    sanitize_result(commit_plan(request, SkillOperationKind::Import))
}

pub fn plan_assignment(request: PlanAssignmentRequest) -> Result<OperationPlan, SkillError> {
    sanitize_result(plan_assignment_inner(request))
}

fn plan_assignment_inner(request: PlanAssignmentRequest) -> Result<OperationPlan, SkillError> {
    ensure_recovery_clear()?;
    let paths = SkillsPaths::resolve_from_env()?;
    let operation_id = Uuid::new_v4().hyphenated().to_string();
    create_operation_root(&paths, &operation_id)?;
    let result = (|| {
        let persisted = build_assignment_plan(request, operation_id.clone())?;
        persist_plan(&paths, &persisted)?;
        Ok(persisted.plan)
    })();
    if result.is_err() {
        remove_unjournaled_operation(&paths, &operation_id)?;
    }
    result
}

pub fn commit_assignment(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    sanitize_result(commit_plan(request, SkillOperationKind::Assignment))
}

pub fn cancel_operation(operation_id: &str) -> Result<(), SkillError> {
    sanitize_result(cancel_operation_inner(operation_id))
}

fn cancel_operation_inner(operation_id: &str) -> Result<(), SkillError> {
    validate_operation_id(operation_id)?;
    let paths = SkillsPaths::resolve_from_env()?;
    let _lock = acquire_skills_lock(&paths)?;
    let journal = paths
        .journals_skills_dir()
        .join(format!("{operation_id}.json"));
    match fs::symlink_metadata(&journal) {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&journal, error)),
        Ok(_) => {
            return Err(SkillError::RecoveryRequired {
                message: "an active Skills journal prevents cancellation".into(),
            })
        }
    }
    StagingRoot::open(&paths)?
        .remove_operation_if_exists(operation_id)
        .map(|_| ())
}

fn build_install_plan(
    request: PlanInstallRequest,
    resolution: SkillSourceResolution,
) -> Result<PersistedPlan, SkillError> {
    if resolution.operation_id != request.resolution_id {
        return invalid_source("the staged resolution does not match the requested operation");
    }
    let paths = SkillsPaths::resolve_from_env()?;
    let settings = current_settings_snapshot()?;
    let inventory = list_inventory()?;
    let selected_names = selected_skill_names(&request.skill_names)?;
    let candidate_summaries = resolution
        .candidates
        .iter()
        .map(|candidate| (candidate.name.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    for name in &selected_names {
        if !candidate_summaries.contains_key(name.as_str()) {
            return invalid_source("a selected Skill is not part of the staged resolution");
        }
    }
    let desired_target_ids = normalize_agent_selection(&request.agent_ids)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut touched_target_ids = desired_target_ids.clone();
    for name in &selected_names {
        touched_target_ids.extend(assigned_target_ids(&settings, name));
    }
    let target_views = selected_target_views(
        &inventory,
        &touched_target_ids.into_iter().collect::<Vec<_>>(),
    )?;
    let mut expected_central = Vec::new();
    let mut expected_links = Vec::new();
    let mut skills = Vec::new();

    for name in selected_names {
        let summary = candidate_summaries[&name.as_str()];
        let candidate = paths
            .staging_skills_dir()
            .join(&resolution.operation_id)
            .join("candidates")
            .join(&name);
        let validated = validate_candidate(&candidate)?;
        if validated.content_hash != summary.content_hash || validated.manifest.name != summary.name
        {
            return stale("a staged Skill candidate changed after resolution");
        }
        let central = paths.central_skill(&name);
        let central_hash = inspect_central(&central)?;
        if central_hash.is_some() && !request.replace_conflicts {
            return conflict("central Skill content already exists");
        }
        expected_central.push(ExpectedCentral {
            skill_name: name.clone(),
            content_hash: central_hash.clone(),
        });
        let mut replaces_target = false;
        for target in &target_views {
            let state = inspect_link(&target_path(&paths, target, &name)?, &central, &paths)?;
            let desired_managed = desired_target_ids.contains(&target.target_id);
            if desired_managed && is_link_conflict(&state) {
                if !request.replace_conflicts {
                    return conflict("an Agent Skill target conflicts with this install");
                }
                replaces_target = true;
            } else if !desired_managed && !is_safe_absent_transition(&state) {
                return conflict(
                    "a prior Agent Skill assignment is no longer an exact managed link",
                );
            }
            expected_links.push(ExpectedLink {
                skill_name: name.clone(),
                target_id: target.target_id.clone(),
                state,
                desired_managed,
            });
        }
        let source = candidate_source(&resolution.source, &summary.relative_path);
        let risk = audit_skill(&candidate)?;
        let files = diff_trees(central_hash.as_ref().map(|_| central.as_path()), &candidate)?;
        skills.push(PlannedSkill {
            manifest: validated.manifest,
            source,
            resolved_revision: resolution.resolved_revision.clone(),
            files,
            risk,
            existing_states: existing_states(&inventory, &name),
            replace_existing: central_hash.is_some() || replaces_target,
            content_hash: validated.content_hash,
        });
    }

    let targets = planned_targets(&target_views, &expected_links);
    let warnings = plan_warnings(&targets, &request.agent_ids);
    let plan = new_plan(
        resolution.operation_id.clone(),
        SkillOperationKind::Install,
        skills,
        targets,
        settings_hash(&settings)?,
        warnings,
    )?;
    finalize_plan(PersistedPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan,
        input: PersistedPlanInput::Install {
            request,
            resolution,
        },
        expected_central,
        expected_links,
        expected_target_roots: Vec::new(),
    })
}

#[derive(Debug)]
struct ExternalItem {
    name: String,
    target_id: String,
    path: PathBuf,
}

fn external_item(
    paths: &SkillsPaths,
    inventory: &SkillsInventory,
    identity: &str,
) -> Result<ExternalItem, SkillError> {
    let item = inventory
        .items
        .iter()
        .find(|item| item.identity == identity)
        .ok_or_else(|| invalid_source_error("the selected external Skill is unavailable"))?;
    if !item.states.contains(&InventoryState::External) {
        return conflict_result("only an external Agent Skill directory can be imported");
    }
    let super::SkillLocation::AgentTarget {
        target_id,
        global_dir,
    } = &item.location
    else {
        return conflict_result("only an external Agent Skill directory can be imported");
    };
    let root = paths
        .expand_user(global_dir)
        .ok_or_else(|| invalid_source_error("the selected Agent target is unavailable"))?;
    let path = root.join(&item.name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| invalid_source_error("the selected external Skill is unavailable"))?;
    if !metadata.file_type().is_dir() {
        return conflict_result("only a real external Skill directory can be imported");
    }
    validate_candidate(&path)?;
    Ok(ExternalItem {
        name: item.name.clone(),
        target_id: target_id.clone(),
        path,
    })
}

fn build_import_plan(
    request: PlanImportRequest,
    operation_id: String,
    source_target_id: String,
    source_name: String,
    original_path: String,
    backup_path: String,
) -> Result<PersistedPlan, SkillError> {
    let paths = SkillsPaths::resolve_from_env()?;
    let settings = current_settings_snapshot()?;
    let inventory = list_inventory()?;
    let external = external_item(&paths, &inventory, &request.identity)?;
    if external.name != source_name || external.target_id != source_target_id {
        return stale("the selected external Skill moved after review");
    }
    if collapse_home(&external.path, paths.user_home()) != original_path {
        return stale("the selected external Skill path changed after review");
    }
    let staged = paths
        .staging_skills_dir()
        .join(&operation_id)
        .join("candidates")
        .join(&source_name);
    let validated = validate_candidate(&staged)?;
    if hash_tree(&external.path)? != validated.content_hash {
        return stale("the external Skill changed after review");
    }
    let mut desired_target_ids = normalize_agent_selection(&request.agent_ids)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    desired_target_ids.insert(source_target_id.clone());
    let mut touched_target_ids = desired_target_ids.clone();
    touched_target_ids.extend(assigned_target_ids(&settings, &source_name));
    let target_views = selected_target_views(
        &inventory,
        &touched_target_ids.into_iter().collect::<Vec<_>>(),
    )?;
    let central = paths.central_skill(&source_name);
    let central_hash = inspect_central(&central)?;
    if central_hash.is_some() && !request.replace_conflicts {
        return conflict_result("central Skill content already exists");
    }
    let mut expected_links = Vec::new();
    let mut replaces_target = false;
    for target in &target_views {
        let state = inspect_link(
            &target_path(&paths, target, &source_name)?,
            &central,
            &paths,
        )?;
        let is_source = target.target_id == source_target_id;
        let desired_managed = desired_target_ids.contains(&target.target_id);
        if is_source && !matches!(state, LinkState::Directory { .. }) {
            return stale("the selected external Skill changed type after review");
        }
        if desired_managed && !is_source && is_link_conflict(&state) {
            if !request.replace_conflicts {
                return conflict_result("an Agent Skill target conflicts with this import");
            }
            replaces_target = true;
        } else if !desired_managed && !is_safe_absent_transition(&state) {
            return conflict_result(
                "a prior Agent Skill assignment is no longer an exact managed link",
            );
        }
        expected_links.push(ExpectedLink {
            skill_name: source_name.clone(),
            target_id: target.target_id.clone(),
            state,
            desired_managed,
        });
    }
    let source = SkillSource::Imported {
        original_path: original_path.clone(),
        backup_path: backup_path.clone(),
    };
    let risk = audit_skill(&staged)?;
    let files = diff_trees(central_hash.as_ref().map(|_| central.as_path()), &staged)?;
    let skill = PlannedSkill {
        manifest: validated.manifest,
        source,
        resolved_revision: None,
        files,
        risk,
        existing_states: existing_states(&inventory, &source_name),
        replace_existing: central_hash.is_some() || replaces_target,
        content_hash: validated.content_hash,
    };
    let targets = planned_targets(&target_views, &expected_links);
    let warnings = plan_warnings(&targets, &request.agent_ids);
    let plan = new_plan(
        operation_id,
        SkillOperationKind::Import,
        vec![skill],
        targets,
        settings_hash(&settings)?,
        warnings,
    )?;
    finalize_plan(PersistedPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan,
        input: PersistedPlanInput::Import {
            request,
            source_target_id,
            source_name: source_name.clone(),
            original_path,
            backup_path,
        },
        expected_central: vec![ExpectedCentral {
            skill_name: source_name,
            content_hash: central_hash,
        }],
        expected_links,
        expected_target_roots: Vec::new(),
    })
}

fn build_assignment_plan(
    request: PlanAssignmentRequest,
    operation_id: String,
) -> Result<PersistedPlan, SkillError> {
    if request.agent_ids.is_empty() {
        return invalid_source("an assignment requires at least one installed Agent");
    }
    let paths = SkillsPaths::resolve_from_env()?;
    let settings = current_settings_snapshot()?;
    let inventory = list_inventory()?;
    let records = settings
        .managed_skills
        .as_ref()
        .ok_or_else(|| invalid_source_error("the managed Skill is unavailable"))?;
    let record = records
        .get(&request.skill_name)
        .ok_or_else(|| invalid_source_error("the managed Skill is unavailable"))?;
    let central = paths.central_skill(&request.skill_name);
    let validated = validate_candidate(&central)?;
    if validated.content_hash != record.content_hash {
        return conflict_result("the managed Skill was locally modified");
    }
    let prior_target_ids = assigned_target_ids(&settings, &request.skill_name);
    let desired_target_ids = if request.enabled {
        normalize_assignment_enable(&request.agent_ids, &prior_target_ids)?
            .into_iter()
            .collect::<BTreeSet<_>>()
    } else {
        let removed = assigned_targets_for_agents(&settings, &request)?;
        prior_target_ids
            .difference(&removed)
            .cloned()
            .collect::<BTreeSet<_>>()
    };
    let touched_target_ids = prior_target_ids
        .union(&desired_target_ids)
        .cloned()
        .collect::<Vec<_>>();
    let target_views = selected_target_views(&inventory, &touched_target_ids)?;
    let mut expected_links = Vec::new();
    for target in &target_views {
        let state = inspect_link(
            &target_path(&paths, target, &request.skill_name)?,
            &central,
            &paths,
        )?;
        let desired_managed = desired_target_ids.contains(&target.target_id);
        if desired_managed && is_link_conflict(&state) {
            return conflict_result("assignment would overwrite an unreviewed Agent Skill target");
        }
        if !desired_managed && !is_safe_absent_transition(&state) {
            return conflict_result("only an exact managed Skill link can be disabled");
        }
        expected_links.push(ExpectedLink {
            skill_name: request.skill_name.clone(),
            target_id: target.target_id.clone(),
            state,
            desired_managed,
        });
    }
    let risk = audit_skill(&central)?;
    let skill = PlannedSkill {
        manifest: validated.manifest,
        source: record.source.clone(),
        resolved_revision: record.resolved_revision.clone(),
        files: Vec::new(),
        risk,
        existing_states: existing_states(&inventory, &request.skill_name),
        replace_existing: false,
        content_hash: validated.content_hash,
    };
    let targets = planned_targets(&target_views, &expected_links);
    let warnings = plan_warnings(&targets, &request.agent_ids);
    let plan = new_plan(
        operation_id,
        SkillOperationKind::Assignment,
        vec![skill],
        targets,
        settings_hash(&settings)?,
        warnings,
    )?;
    finalize_plan(PersistedPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan,
        input: PersistedPlanInput::Assignment { request },
        expected_central: vec![ExpectedCentral {
            skill_name: record.name.clone(),
            content_hash: Some(record.content_hash.clone()),
        }],
        expected_links,
        expected_target_roots: Vec::new(),
    })
}

fn assigned_targets_for_agents(
    settings: &SkillSettingsSnapshot,
    request: &PlanAssignmentRequest,
) -> Result<BTreeSet<String>, SkillError> {
    let declared = declared_targets_for_agents(&request.agent_ids)?;
    Ok(assigned_target_ids(settings, &request.skill_name)
        .intersection(&declared)
        .cloned()
        .collect())
}

fn assigned_target_ids(settings: &SkillSettingsSnapshot, skill_name: &str) -> BTreeSet<String> {
    settings
        .skill_assignments
        .as_ref()
        .and_then(|assignments| assignments.get(skill_name))
        .cloned()
        .unwrap_or_default()
}

fn commit_plan(
    request: SkillCommitRequest,
    expected_kind: SkillOperationKind,
) -> Result<SkillsInventory, SkillError> {
    validate_operation_id(&request.operation_id)?;
    let paths = SkillsPaths::resolve_from_env()?;
    let persisted = load_plan(&paths, &request.operation_id)?;
    if persisted.plan.operation_id != request.operation_id
        || persisted.plan.kind != expected_kind
        || request.candidate_hash != persisted.plan.candidate_hash
    {
        return Err(stale_error(
            "the reviewed Skills plan does not match this commit",
        ));
    }
    let rebuilt = rebuild_plan(&persisted).map_err(revalidation_error)?;
    if rebuilt != persisted {
        return Err(stale_error("the reviewed Skills plan is stale"));
    }
    if persisted.plan.requires_risk_override
        && request.findings_confirmation.as_deref() != Some(persisted.plan.findings_hash.as_str())
    {
        return Err(SkillError::ConfirmationRequired {
            message: "high-risk Skill findings require exact confirmation".into(),
            findings_hash: persisted.plan.findings_hash.clone(),
        });
    }
    let spec = transaction_spec(&paths, &persisted)?;
    execute_transaction(spec)?;
    list_inventory()
}

fn revalidation_error(error: SkillError) -> SkillError {
    match error {
        SkillError::InvalidManifest { .. }
        | SkillError::UnsafePath { .. }
        | SkillError::InvalidSource { .. }
        | SkillError::Conflict { .. } => {
            stale_error("reviewed Skill candidates or targets changed after planning")
        }
        other => other,
    }
}

fn rebuild_plan(persisted: &PersistedPlan) -> Result<PersistedPlan, SkillError> {
    match &persisted.input {
        PersistedPlanInput::Install {
            request,
            resolution,
        } => {
            let paths = SkillsPaths::resolve_from_env()?;
            let reloaded = load_staged_resolution(&paths, &persisted.plan.operation_id)?;
            if &reloaded != resolution {
                return Err(stale_error(
                    "the staged Skill resolution changed after review",
                ));
            }
            build_install_plan(request.clone(), reloaded)
        }
        PersistedPlanInput::Import {
            request,
            source_target_id,
            source_name,
            original_path,
            backup_path,
        } => build_import_plan(
            request.clone(),
            persisted.plan.operation_id.clone(),
            source_target_id.clone(),
            source_name.clone(),
            original_path.clone(),
            backup_path.clone(),
        ),
        PersistedPlanInput::Assignment { request } => {
            build_assignment_plan(request.clone(), persisted.plan.operation_id.clone())
        }
    }
}

fn transaction_spec(
    paths: &SkillsPaths,
    persisted: &PersistedPlan,
) -> Result<TransactionSpec, SkillError> {
    let settings_before = current_settings_snapshot()?;
    if settings_hash(&settings_before)? != persisted.plan.settings_hash {
        return Err(stale_error("Skills settings changed after review"));
    }
    let mut settings_after = settings_before.clone();
    match &persisted.input {
        PersistedPlanInput::Install { .. } | PersistedPlanInput::Import { .. } => {
            install_settings_after(paths, persisted, &mut settings_after)?;
        }
        PersistedPlanInput::Assignment { request } => {
            assignment_settings_after(persisted, request, &mut settings_after);
        }
    }

    let mut directory_mutations = Vec::new();
    if matches!(
        persisted.plan.kind,
        SkillOperationKind::Install | SkillOperationKind::Import
    ) {
        for expected in &persisted.expected_central {
            directory_mutations.push(DirectoryMutation {
                replacement: Some(
                    paths
                        .staging_skills_dir()
                        .join(&persisted.plan.operation_id)
                        .join("candidates")
                        .join(&expected.skill_name),
                ),
                destination: paths.central_skill(&expected.skill_name),
                backup: paths
                    .backups_skills_dir()
                    .join(&persisted.plan.operation_id)
                    .join("central")
                    .join(&expected.skill_name),
                expected_before_hash: expected.content_hash.clone(),
            });
        }
    }

    let target_by_id = persisted
        .plan
        .targets
        .iter()
        .map(|target| (target.target_id.as_str(), target))
        .collect::<BTreeMap<_, _>>();
    let mut link_mutations = Vec::new();
    for expected in &persisted.expected_links {
        let target = target_by_id
            .get(expected.target_id.as_str())
            .ok_or_else(|| invalid_source_error("a planned Skill target is unavailable"))?;
        let path = paths
            .expand_user(&target.global_dir)
            .ok_or_else(|| invalid_source_error("a planned Skill target path is unavailable"))?
            .join(&expected.skill_name);
        let desired_target = if expected.desired_managed {
            Some(paths.central_skill(&expected.skill_name))
        } else {
            None
        };
        let backup = if matches!(expected.state, LinkState::Directory { .. }) {
            Some(import_or_replacement_backup(paths, persisted, expected)?)
        } else {
            None
        };
        link_mutations.push(LinkMutation {
            path,
            expected: expected.state.clone(),
            desired_target,
            backup,
        });
    }
    Ok(TransactionSpec {
        operation_id: persisted.plan.operation_id.clone(),
        order: TransactionOrder::ContentThenLinks,
        directory_mutations,
        link_mutations,
        settings_before,
        settings_after,
    })
}

fn install_settings_after(
    paths: &SkillsPaths,
    persisted: &PersistedPlan,
    settings: &mut SkillSettingsSnapshot,
) -> Result<(), SkillError> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let records = settings.managed_skills.get_or_insert_default();
    let assignments = settings.skill_assignments.get_or_insert_default();
    for skill in &persisted.plan.skills {
        let candidate = paths
            .staging_skills_dir()
            .join(&persisted.plan.operation_id)
            .join("candidates")
            .join(&skill.manifest.name);
        let validated = validate_candidate(&candidate)?;
        if validated.content_hash != skill.content_hash {
            return Err(stale_error(
                "a staged Skill candidate changed before commit",
            ));
        }
        let installed_at = records
            .get(&skill.manifest.name)
            .map(|record| record.installed_at.clone())
            .unwrap_or_else(|| now.clone());
        records.insert(
            skill.manifest.name.clone(),
            ManagedSkillRecord {
                name: skill.manifest.name.clone(),
                description: skill.manifest.description.clone(),
                content_kind: validated.content_kind,
                source: skill.source.clone(),
                resolved_revision: skill.resolved_revision.clone(),
                content_hash: skill.content_hash.clone(),
                installed_at,
                updated_at: now.clone(),
                risk: skill.risk.clone(),
                update: SkillUpdateState::default(),
            },
        );
        let desired_target_ids = persisted
            .expected_links
            .iter()
            .filter(|link| link.skill_name == skill.manifest.name && link.desired_managed)
            .map(|link| link.target_id.clone())
            .collect::<BTreeSet<_>>();
        if desired_target_ids.is_empty() {
            assignments.remove(&skill.manifest.name);
        } else {
            assignments.insert(skill.manifest.name.clone(), desired_target_ids);
        }
    }
    if assignments.is_empty() {
        settings.skill_assignments = None;
    }
    Ok(())
}

fn assignment_settings_after(
    persisted: &PersistedPlan,
    request: &PlanAssignmentRequest,
    settings: &mut SkillSettingsSnapshot,
) {
    let assignments = settings.skill_assignments.get_or_insert_default();
    let desired_target_ids = persisted
        .expected_links
        .iter()
        .filter(|link| link.skill_name == request.skill_name && link.desired_managed)
        .map(|link| link.target_id.clone())
        .collect::<BTreeSet<_>>();
    if desired_target_ids.is_empty() {
        assignments.remove(&request.skill_name);
    } else {
        assignments.insert(request.skill_name.clone(), desired_target_ids);
    }
    if assignments.is_empty() {
        settings.skill_assignments = None;
    }
}

fn import_or_replacement_backup(
    paths: &SkillsPaths,
    persisted: &PersistedPlan,
    expected: &ExpectedLink,
) -> Result<PathBuf, SkillError> {
    if let PersistedPlanInput::Import {
        source_target_id,
        source_name,
        backup_path,
        ..
    } = &persisted.input
    {
        if source_target_id == &expected.target_id && source_name == &expected.skill_name {
            return paths
                .expand_user(backup_path)
                .ok_or_else(|| invalid_source_error("the reviewed import backup path is invalid"));
        }
    }
    Ok(paths
        .backups_skills_dir()
        .join(&persisted.plan.operation_id)
        .join("targets")
        .join(&expected.target_id)
        .join(&expected.skill_name))
}

fn new_plan(
    operation_id: String,
    kind: SkillOperationKind,
    mut skills: Vec<PlannedSkill>,
    mut targets: Vec<PlannedTarget>,
    settings_hash: String,
    mut warnings: Vec<String>,
) -> Result<OperationPlan, SkillError> {
    skills.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    targets.sort_by(|left, right| left.target_id.cmp(&right.target_id));
    warnings.sort();
    let findings_hash = aggregate_findings_hash(&skills)?;
    let requires_risk_override = skills
        .iter()
        .any(|skill| skill.risk.level == RiskLevel::High);
    Ok(OperationPlan {
        operation_id,
        kind,
        skills,
        targets,
        settings_hash,
        candidate_hash: String::new(),
        findings_hash,
        requires_risk_override,
        warnings,
    })
}

fn finalize_plan(mut persisted: PersistedPlan) -> Result<PersistedPlan, SkillError> {
    persisted.expected_central.sort();
    persisted.expected_links.sort_by(|left, right| {
        left.skill_name
            .cmp(&right.skill_name)
            .then(left.target_id.cmp(&right.target_id))
    });
    persisted.expected_target_roots =
        expected_target_roots(&SkillsPaths::resolve_from_env()?, &persisted.plan.targets)?;
    persisted.expected_target_roots.sort();
    persisted.plan.candidate_hash = candidate_hash(&persisted)?;
    Ok(persisted)
}

fn candidate_hash(persisted: &PersistedPlan) -> Result<String, SkillError> {
    let mut requested_agent_ids = match &persisted.input {
        PersistedPlanInput::Install { request, .. } => request.agent_ids.iter(),
        PersistedPlanInput::Import { request, .. } => request.agent_ids.iter(),
        PersistedPlanInput::Assignment { request } => request.agent_ids.iter(),
    }
    .map(String::as_str)
    .collect::<Vec<_>>();
    requested_agent_ids.sort();
    let (replace_conflicts, assignment_enabled) = match &persisted.input {
        PersistedPlanInput::Install { request, .. } => (request.replace_conflicts, None),
        PersistedPlanInput::Import { request, .. } => (request.replace_conflicts, None),
        PersistedPlanInput::Assignment { request } => (false, Some(request.enabled)),
    };
    let binding = CandidateBinding {
        operation_id: &persisted.plan.operation_id,
        kind: &persisted.plan.kind,
        skills: persisted
            .plan
            .skills
            .iter()
            .map(|skill| BoundSkill {
                name: &skill.manifest.name,
                source: &skill.source,
                resolved_revision: &skill.resolved_revision,
                content_hash: &skill.content_hash,
                replace_existing: skill.replace_existing,
            })
            .collect(),
        requested_agent_ids,
        target_ids: persisted
            .plan
            .targets
            .iter()
            .map(|target| target.target_id.as_str())
            .collect(),
        replace_conflicts,
        assignment_enabled,
        expected_central: &persisted.expected_central,
        expected_links: &persisted.expected_links,
        expected_target_roots: &persisted.expected_target_roots,
    };
    canonical_hash(&binding)
}

fn aggregate_findings_hash(skills: &[PlannedSkill]) -> Result<String, SkillError> {
    let digests = skills
        .iter()
        .map(|skill| {
            findings_digest(&skill.risk).map(|digest| (skill.manifest.name.clone(), digest))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let [(.., digest)] = digests.as_slice() {
        return Ok(digest.clone());
    }
    canonical_hash(&digests)
}

fn current_settings_snapshot() -> Result<SkillSettingsSnapshot, SkillError> {
    let settings = load_settings_strict().map_err(|_| SkillError::Io {
        message: "MUX settings could not be read safely".into(),
        path: None,
    })?;
    Ok(snapshot_from_settings(&settings))
}

fn snapshot_from_settings(settings: &Settings) -> SkillSettingsSnapshot {
    SkillSettingsSnapshot {
        managed_skills: settings.managed_skills.clone(),
        skill_assignments: settings.skill_assignments.clone(),
        skill_update_checked_at: settings.skill_update_checked_at.clone(),
    }
}

fn settings_hash(settings: &SkillSettingsSnapshot) -> Result<String, SkillError> {
    canonical_hash(settings)
}

fn canonical_hash<T: Serialize>(value: &T) -> Result<String, SkillError> {
    let bytes = serde_json::to_vec(value).map_err(|_| SkillError::InvalidSource {
        message: "canonical Skills plan data could not be encoded".into(),
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn selected_skill_names(names: &[String]) -> Result<Vec<String>, SkillError> {
    if names.is_empty() {
        return invalid_source("an install requires at least one selected Skill");
    }
    let mut selected = BTreeSet::new();
    for name in names {
        if !valid_skill_name(name) || !selected.insert(name.clone()) {
            return invalid_source("the selected Skill names are invalid or duplicated");
        }
    }
    Ok(selected.into_iter().collect())
}

fn valid_skill_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn candidate_source(source: &SkillSource, relative_path: &str) -> SkillSource {
    let join = |base: &str| match (base.is_empty(), relative_path.is_empty()) {
        (true, _) => relative_path.to_owned(),
        (_, true) => base.to_owned(),
        (false, false) => format!("{base}/{relative_path}"),
    };
    match source {
        SkillSource::Github {
            owner,
            repo,
            subpath,
            requested_ref,
            pinned,
        } => SkillSource::Github {
            owner: owner.clone(),
            repo: repo.clone(),
            subpath: join(subpath),
            requested_ref: requested_ref.clone(),
            pinned: *pinned,
        },
        SkillSource::Local { path, subpath } => SkillSource::Local {
            path: path.clone(),
            subpath: join(subpath),
        },
        SkillSource::Imported { .. } => source.clone(),
    }
}

fn selected_target_views(
    inventory: &SkillsInventory,
    target_ids: &[String],
) -> Result<Vec<SkillTargetView>, SkillError> {
    let mut views = Vec::new();
    for target_id in target_ids {
        let target = inventory
            .targets
            .iter()
            .find(|target| &target.target_id == target_id)
            .ok_or_else(|| {
                invalid_source_error("a normalized Agent Skill target is unavailable")
            })?;
        views.push(target.clone());
    }
    views.sort_by(|left, right| left.target_id.cmp(&right.target_id));
    Ok(views)
}

fn target_path(
    paths: &SkillsPaths,
    target: &SkillTargetView,
    skill_name: &str,
) -> Result<PathBuf, SkillError> {
    paths
        .expand_user(&target.global_dir)
        .filter(|path| path.is_absolute())
        .map(|path| path.join(skill_name))
        .ok_or_else(|| invalid_source_error("a verified Agent Skill target path is invalid"))
}

fn expected_target_roots(
    paths: &SkillsPaths,
    targets: &[PlannedTarget],
) -> Result<Vec<ExpectedTargetRoot>, SkillError> {
    targets
        .iter()
        .map(|target| expected_target_root(paths, target))
        .collect()
}

fn expected_target_root(
    paths: &SkillsPaths,
    target: &PlannedTarget,
) -> Result<ExpectedTargetRoot, SkillError> {
    let root = paths
        .expand_user(&target.global_dir)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| invalid_source_error("a verified Agent Skill target path is invalid"))?;
    let root_path = normalized_physical_path(&root)?;
    let mut anchor = root.clone();
    let mut remaining_components = Vec::new();

    loop {
        match fs::symlink_metadata(&anchor) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
                    return Err(SkillError::UnsafePath {
                        message: "a verified Agent Skill target parent is not a directory".into(),
                        path: String::new(),
                    });
                }
                let canonical =
                    fs::canonicalize(&anchor).map_err(|error| io_error(&anchor, error))?;
                let canonical_metadata = fs::symlink_metadata(&canonical)
                    .map_err(|error| io_error(&canonical, error))?;
                if !canonical_metadata.file_type().is_dir() {
                    return Err(SkillError::UnsafePath {
                        message: "a verified Agent Skill target parent is not a directory".into(),
                        path: String::new(),
                    });
                }
                remaining_components.reverse();
                let (anchor_device, anchor_inode, anchor_mode) =
                    directory_identity(&canonical_metadata);
                return Ok(ExpectedTargetRoot {
                    target_id: target.target_id.clone(),
                    root_path,
                    anchor_path: normalized_physical_path(&canonical)?,
                    anchor_device,
                    anchor_inode,
                    anchor_mode,
                    remaining_components,
                });
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let component = anchor
                    .file_name()
                    .and_then(|value| value.to_str())
                    .filter(|value| !value.is_empty() && !matches!(*value, "." | ".."))
                    .ok_or_else(|| {
                        invalid_source_error("a verified Agent Skill target path is invalid")
                    })?;
                remaining_components.push(component.to_owned());
                anchor = anchor
                    .parent()
                    .ok_or_else(|| {
                        invalid_source_error("a verified Agent Skill target path is invalid")
                    })?
                    .to_path_buf();
            }
            Err(error) => return Err(io_error(&anchor, error)),
        }
    }
}

fn normalized_physical_path(path: &Path) -> Result<String, SkillError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return invalid_source("a verified Agent Skill target path is invalid");
    }
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| invalid_source_error("a verified Agent Skill target path is not UTF-8"))
}

#[cfg(unix)]
fn directory_identity(metadata: &fs::Metadata) -> (u64, u64, u32) {
    (metadata.dev(), metadata.ino(), metadata.mode())
}

#[cfg(not(unix))]
fn directory_identity(metadata: &fs::Metadata) -> (u64, u64, u32) {
    (
        0,
        metadata.len(),
        u32::from(metadata.permissions().readonly()),
    )
}

fn inspect_central(path: &Path) -> Result<Option<String>, SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path, error)),
        Ok(metadata) if metadata.file_type().is_dir() => hash_tree(path).map(Some),
        Ok(_) => conflict_result("central Skill content has an unsupported type"),
    }
}

fn inspect_link(path: &Path, central: &Path, paths: &SkillsPaths) -> Result<LinkState, SkillError> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(LinkState::Missing),
        Err(error) => return Err(io_error(path, error)),
        Ok(metadata) => metadata,
    };
    if metadata.file_type().is_dir() {
        return Ok(LinkState::Directory {
            tree_hash: hash_tree(path)?,
        });
    }
    if !metadata.file_type().is_symlink() {
        return conflict_result("an Agent Skill target has an unsupported type");
    }
    let raw_target = fs::read_link(path).map_err(|error| io_error(path, error))?;
    match fs::metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound || is_symlink_loop(&error) => {
            Ok(LinkState::BrokenSymlink { target: raw_target })
        }
        Err(error) => Err(io_error(path, error)),
        Ok(_) => {
            let resolved = fs::canonicalize(path).map_err(|error| io_error(path, error))?;
            let central_resolved = fs::canonicalize(central).ok();
            if central_resolved.as_ref() == Some(&resolved) {
                if raw_target != central {
                    return conflict_result(
                        "a relative managed Skill link cannot be changed transactionally",
                    );
                }
                Ok(LinkState::ManagedSymlink {
                    target: central.to_path_buf(),
                })
            } else {
                let central_root = fs::canonicalize(paths.skills_dir()).ok();
                if central_root
                    .as_ref()
                    .is_some_and(|root| resolved == *root || resolved.starts_with(root))
                {
                    return conflict_result(
                        "an Agent Skill link points to different managed content",
                    );
                }
                Ok(LinkState::UnknownSymlink { target: raw_target })
            }
        }
    }
}

#[cfg(unix)]
fn is_symlink_loop(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

#[cfg(not(unix))]
fn is_symlink_loop(_error: &std::io::Error) -> bool {
    false
}

fn is_link_conflict(state: &LinkState) -> bool {
    matches!(
        state,
        LinkState::BrokenSymlink { .. }
            | LinkState::Directory { .. }
            | LinkState::UnknownSymlink { .. }
    )
}

fn is_safe_absent_transition(state: &LinkState) -> bool {
    matches!(state, LinkState::Missing | LinkState::ManagedSymlink { .. })
}

fn existing_states(inventory: &SkillsInventory, name: &str) -> BTreeSet<InventoryState> {
    inventory
        .items
        .iter()
        .filter(|item| item.name == name)
        .flat_map(|item| item.states.iter().cloned())
        .collect()
}

fn planned_targets(
    target_views: &[SkillTargetView],
    expected_links: &[ExpectedLink],
) -> Vec<PlannedTarget> {
    target_views
        .iter()
        .map(|target| {
            let expected = expected_links
                .iter()
                .filter(|link| link.target_id == target.target_id)
                .map(|link| planned_link_state(&link.state))
                .max_by_key(planned_link_priority)
                .unwrap_or(PlannedLinkState::Missing);
            PlannedTarget {
                target_id: target.target_id.clone(),
                global_dir: target.global_dir.clone(),
                expected,
                primary_agent_ids: target.primary_agent_ids.clone(),
                affected_agent_ids: target.affected_agent_ids.clone(),
            }
        })
        .collect()
}

fn planned_link_state(state: &LinkState) -> PlannedLinkState {
    match state {
        LinkState::Missing => PlannedLinkState::Missing,
        LinkState::ManagedSymlink { .. } => PlannedLinkState::Managed,
        LinkState::BrokenSymlink { .. } => PlannedLinkState::Broken,
        LinkState::Directory { .. } => PlannedLinkState::Directory,
        LinkState::UnknownSymlink { .. } => PlannedLinkState::UnknownSymlink,
    }
}

fn planned_link_priority(state: &PlannedLinkState) -> u8 {
    match state {
        PlannedLinkState::Missing => 0,
        PlannedLinkState::Managed => 1,
        PlannedLinkState::Broken => 2,
        PlannedLinkState::UnknownSymlink => 3,
        PlannedLinkState::Directory => 4,
    }
}

fn plan_warnings(targets: &[PlannedTarget], selected_agent_ids: &[String]) -> Vec<String> {
    let selected = selected_agent_ids.iter().collect::<BTreeSet<_>>();
    targets
        .iter()
        .filter_map(|target| {
            let shared = target
                .affected_agent_ids
                .iter()
                .filter(|agent_id| !selected.contains(agent_id))
                .cloned()
                .collect::<Vec<_>>();
            (!shared.is_empty()).then(|| {
                format!(
                    "Target {} also affects installed Agents: {}",
                    target.target_id,
                    shared.join(", ")
                )
            })
        })
        .collect()
}

fn ensure_recovery_clear() -> Result<(), SkillError> {
    if has_pending_recovery()? {
        return Err(SkillError::RecoveryRequired {
            message: "a pending Skills operation must be recovered before planning".into(),
        });
    }
    Ok(())
}

fn create_operation_root(paths: &SkillsPaths, operation_id: &str) -> Result<PathBuf, SkillError> {
    validate_operation_id(operation_id)?;
    Ok(StagingRoot::open_or_create(paths)?
        .create_operation(operation_id)?
        .path()
        .to_path_buf())
}

fn persist_plan(paths: &SkillsPaths, persisted: &PersistedPlan) -> Result<(), SkillError> {
    let bytes = serde_json::to_vec(persisted).map_err(|_| SkillError::InvalidSource {
        message: "the Skills plan could not be encoded safely".into(),
    })?;
    if bytes.len() as u64 > MAX_PLAN_BYTES {
        return Err(SkillError::LimitExceeded {
            limit: "operation_plan".into(),
            actual: bytes.len() as u64,
            allowed: MAX_PLAN_BYTES,
        });
    }
    StagingRoot::open(paths)?
        .open_operation(&persisted.plan.operation_id)?
        .write_private_atomic(PLAN_FILE, &bytes, MAX_PLAN_BYTES)
}

fn load_plan(paths: &SkillsPaths, operation_id: &str) -> Result<PersistedPlan, SkillError> {
    let bytes = StagingRoot::open(paths)?
        .open_operation(operation_id)?
        .read_private(PLAN_FILE, MAX_PLAN_BYTES)?;
    let persisted: PersistedPlan = serde_json::from_slice(&bytes)
        .map_err(|_| invalid_source_error("the reviewed Skills plan is malformed"))?;
    let canonical = serde_json::to_vec(&persisted)
        .map_err(|_| invalid_source_error("the reviewed Skills plan is malformed"))?;
    if canonical != bytes {
        return invalid_source("the reviewed Skills plan is not canonical");
    }
    if persisted.schema_version != PLAN_SCHEMA_VERSION
        || persisted.plan.operation_id != operation_id
        || candidate_hash(&persisted)? != persisted.plan.candidate_hash
        || aggregate_findings_hash(&persisted.plan.skills)? != persisted.plan.findings_hash
    {
        return Err(stale_error(
            "the reviewed Skills plan failed integrity validation",
        ));
    }
    Ok(persisted)
}

fn remove_unjournaled_operation(paths: &SkillsPaths, operation_id: &str) -> Result<(), SkillError> {
    StagingRoot::open(paths)?
        .remove_operation_if_exists(operation_id)
        .map(|_| ())
}

fn collapse_home(path: &Path, home: &Path) -> String {
    if path == home {
        return "~".into();
    }
    if let Ok(relative) = path.strip_prefix(home) {
        return format!("~/{}", normalized_path(relative));
    }
    normalized_path(path)
}

fn normalized_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::RootDir => Some(String::new()),
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::CurDir | Component::ParentDir => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn sanitize_result<T>(result: Result<T, SkillError>) -> Result<T, SkillError> {
    result.map_err(sanitize_error)
}

fn sanitize_error(error: SkillError) -> SkillError {
    match error {
        SkillError::InvalidManifest { message, .. } => SkillError::InvalidManifest {
            message: super::capped_message(message),
            path: String::new(),
        },
        SkillError::UnsafePath { message, .. } => SkillError::UnsafePath {
            message: super::capped_message(message),
            path: String::new(),
        },
        SkillError::Conflict { message, .. } => SkillError::Conflict {
            message: super::capped_message(message),
            path: String::new(),
        },
        SkillError::Io { message, .. } => SkillError::Io {
            message: super::capped_message(message),
            path: None,
        },
        other => other,
    }
}

fn invalid_source<T>(message: &str) -> Result<T, SkillError> {
    Err(invalid_source_error(message))
}

fn invalid_source_error(message: &str) -> SkillError {
    SkillError::InvalidSource {
        message: super::capped_message(message),
    }
}

fn conflict(message: &str) -> Result<PersistedPlan, SkillError> {
    conflict_result(message)
}

fn conflict_result<T>(message: &str) -> Result<T, SkillError> {
    Err(SkillError::Conflict {
        message: super::capped_message(message),
        path: String::new(),
    })
}

fn stale<T>(message: &str) -> Result<T, SkillError> {
    Err(stale_error(message))
}

fn stale_error(message: &str) -> SkillError {
    SkillError::PlanStale {
        message: super::capped_message(message),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::skills::{
        execute_transaction_with_failpoint, resolve_source, Failpoint, GithubEndpoints,
        SkillSourceInput,
    };
    use crate::testenv::TestHome;

    #[test]
    fn install_transaction_failure_rolls_back_central_links_and_settings_together() {
        let home = TestHome::new("ops-rollback");
        fs::create_dir_all(home.home.join(".claude")).unwrap();
        fs::create_dir_all(home.home.join("Library/Application Support/Cursor")).unwrap();
        let source = home.home.join("source/rollback-all");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: rollback-all\ndescription: Rollback fixture\n---\n",
        )
        .unwrap();
        let resolution = resolve_source(
            SkillSourceInput::Local {
                path: source.to_string_lossy().into_owned(),
            },
            GithubEndpoints::production(),
        )
        .unwrap();
        let plan = plan_install(PlanInstallRequest {
            resolution_id: resolution.operation_id,
            skill_names: vec!["rollback-all".into()],
            agent_ids: vec!["claude-code".into(), "cursor".into()],
            replace_conflicts: false,
        })
        .unwrap();
        let paths = SkillsPaths::from_env().unwrap();
        let persisted = load_plan(&paths, &plan.operation_id).unwrap();
        let before_settings = current_settings_snapshot().unwrap();
        let spec = transaction_spec(&paths, &persisted).unwrap();

        let error =
            execute_transaction_with_failpoint(spec, Some(Failpoint::AfterFirstLink)).unwrap_err();

        assert!(matches!(error, SkillError::Io { .. }));
        assert!(!paths.central_skill("rollback-all").exists());
        assert!(!home.home.join(".claude/skills/rollback-all").exists());
        assert!(!home.home.join(".cursor/skills/rollback-all").exists());
        assert_eq!(current_settings_snapshot().unwrap(), before_settings);
        assert!(!paths.staging_skills_dir().join(&plan.operation_id).exists());
        assert!(!paths.journals_skills_dir().exists());
    }
}
