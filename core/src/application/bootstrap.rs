//! One fail-closed startup path for every MUX frontend.

use super::gate::{BackendStatus, CapabilityDomain};
use crate::domain::assets::{AssetCommitRequest, AssetOperationPlan, DomainPlan};
use crate::domain::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{LazyLock, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frontend {
    Cli,
    Desktop,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapStage {
    GlobalWriteRecovery,
    SkillRecovery,
    AssetRecovery,
    SettingsMigration,
    McpRegistryMigration,
    ModelProfileMigration,
    ModelProviderMigration,
    ModelReconciliation,
}

impl BootstrapStage {
    pub fn code(self) -> &'static str {
        match self {
            Self::GlobalWriteRecovery => "global_write_recovery",
            Self::SkillRecovery => "skill_recovery",
            Self::AssetRecovery => "asset_recovery",
            Self::SettingsMigration => "settings_migration",
            Self::McpRegistryMigration => "mcp_registry_migration",
            Self::ModelProfileMigration => "model_profile_migration",
            Self::ModelProviderMigration => "model_provider_migration",
            Self::ModelReconciliation => "model_reconciliation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapWarning {
    pub stage: BootstrapStage,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapReport {
    pub warnings: Vec<BootstrapWarning>,
    pub skill_updates_allowed: bool,
    pub status: BackendStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapError {
    pub stage: BootstrapStage,
    pub message: String,
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.stage.code(), self.message)
    }
}

impl std::error::Error for BootstrapError {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationResolutionStrategy {
    UseMux,
    KeepAgent,
    Recheck,
    Later,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelMigrationState {
    pub profile_id: String,
    pub enabled: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationBlocker {
    pub agent_id: String,
    pub agent_name: String,
    pub target_files: Vec<String>,
    pub profile_id: String,
    pub reason: String,
    pub message: String,
    pub before: ModelMigrationState,
    pub after: ModelMigrationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_agent_fallback_profile_id: Option<String>,
    pub keep_agent_released_profile_ids: Vec<String>,
    pub migrates_keychain_reference: bool,
    pub agent_restart_recommended: bool,
    pub mux_owned_field_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationActionPlan {
    pub strategy: MigrationResolutionStrategy,
    pub title: String,
    pub consequence: String,
    pub modifies_agent_targets: bool,
    pub preserves_agent_targets: bool,
    pub plan: AssetOperationPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationReview {
    pub stage: BootstrapStage,
    pub source_schema_version: u32,
    pub target_schema_version: u32,
    pub review_hash: String,
    pub can_commit: bool,
    pub requires_conflict_confirmation: bool,
    pub blockers: Vec<MigrationBlocker>,
    pub actions: Vec<MigrationActionPlan>,
    pub supported_actions: Vec<MigrationResolutionStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveMigrationRequest {
    pub review_hash: String,
    pub strategy: MigrationResolutionStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_hash: Option<String>,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationResolutionOutcome {
    pub changed: bool,
    pub status: BackendStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<MigrationReview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_plan: Option<AssetOperationPlan>,
}

static ACTIVE_MIGRATION_REVIEW: LazyLock<RwLock<Option<MigrationReview>>> =
    LazyLock::new(|| RwLock::new(None));

enum BootstrapProgress {
    Ready,
    Review(MigrationReview),
    CapabilityUnavailable(BootstrapError),
}

/// Recover incomplete writes, migrate storage, and reconcile projections while
/// holding the process-wide exclusive gate. A reviewable Model migration is a
/// dedicated state, not a recovery failure; every other startup error remains
/// fail-closed.
pub fn bootstrap(frontend: Frontend) -> Result<BootstrapReport, BootstrapError> {
    let permit = super::gate::begin_bootstrap();
    clear_active_review();
    let _cross_process_guard = match super::gate::acquire_cross_process_mutation_lock() {
        Ok(guard) => guard,
        Err(_) => {
            let error = BootstrapError {
                stage: BootstrapStage::GlobalWriteRecovery,
                message: "failed to acquire the cross-process mutation lock".into(),
            };
            let status = read_only_status(&error);
            permit.finish(status.clone());
            return failure_outcome(frontend, error, status);
        }
    };
    match bootstrap_unlocked() {
        Ok(BootstrapProgress::Ready) => {
            let status = BackendStatus::Ready;
            permit.finish(status.clone());
            Ok(BootstrapReport {
                warnings: Vec::new(),
                skill_updates_allowed: true,
                status,
            })
        }
        Ok(BootstrapProgress::Review(review)) => {
            let status = review_status(&review);
            set_active_review(Some(review));
            permit.finish(status.clone());
            Ok(BootstrapReport {
                warnings: Vec::new(),
                // Avoid an automatic update-state write invalidating the
                // hash-bound Model candidate. Manual Skill operations remain
                // available through the capability-scoped gate.
                skill_updates_allowed: false,
                status,
            })
        }
        Ok(BootstrapProgress::CapabilityUnavailable(error)) => {
            let status = capability_unavailable_status(&error);
            let warning = match &status {
                BackendStatus::CapabilityUnavailable { message, .. } => message.clone(),
                _ => unreachable!("capability status constructor returned another state"),
            };
            permit.finish(status.clone());
            Ok(BootstrapReport {
                warnings: vec![BootstrapWarning {
                    stage: error.stage,
                    message: warning,
                }],
                skill_updates_allowed: true,
                status,
            })
        }
        Err(error) => {
            let status = read_only_status(&error);
            permit.finish(status.clone());
            failure_outcome(frontend, error, status)
        }
    }
}

pub fn migration_review() -> Option<MigrationReview> {
    super::gate::query(active_review)
}

/// Resolve only the currently published Model migration review. The process
/// gate and the same cross-process application lock used by bootstrap are held
/// across candidate validation, the asset transaction, and all later stages.
pub fn resolve_migration(
    request: ResolveMigrationRequest,
) -> CoreResult<MigrationResolutionOutcome> {
    if request.strategy == MigrationResolutionStrategy::Later {
        let review = active_review().ok_or_else(review_unavailable_error)?;
        if review.review_hash != request.review_hash {
            return Err(review_stale_error());
        }
        return Ok(MigrationResolutionOutcome {
            changed: false,
            status: review_status(&review),
            review: Some(review),
            selected_plan: None,
        });
    }

    let current = active_review().ok_or_else(review_unavailable_error)?;
    if current.review_hash != request.review_hash {
        return Err(review_stale_error());
    }
    let selected = action_plan(&current, request.strategy).cloned();
    if matches!(
        request.strategy,
        MigrationResolutionStrategy::UseMux | MigrationResolutionStrategy::KeepAgent
    ) {
        let selected = selected.as_ref().ok_or_else(review_stale_error)?;
        if request.candidate_hash.as_deref() != Some(selected.plan.candidate_hash.as_str()) {
            return Err(review_stale_error());
        }
        if !request.dry_run && !request.confirmed {
            return Err(CoreError::new(
                "confirmation_required",
                "Explicitly confirm the reviewed Model migration candidate",
            )
            .with_detail(
                "candidate_hash",
                Value::String(selected.plan.candidate_hash.clone()),
            ));
        }
    }
    if request.dry_run {
        return Ok(MigrationResolutionOutcome {
            changed: false,
            status: review_status(&current),
            review: Some(current),
            selected_plan: selected.map(|action| action.plan),
        });
    }

    let permit = super::gate::begin_migration_resolution(&request.review_hash)?;
    let _cross_process_guard = match super::gate::acquire_cross_process_mutation_lock() {
        Ok(guard) => guard,
        Err(_) => {
            let status = review_status(&current);
            permit.finish(status);
            return Err(CoreError::new(
                "mutation_busy",
                "Another MUX process is changing shared state; retry this operation",
            ));
        }
    };

    if request.strategy == MigrationResolutionStrategy::Recheck {
        if let Err(message) = cancel_review_plans(&current) {
            clear_active_review();
            let error = BootstrapError {
                stage: BootstrapStage::ModelProfileMigration,
                message,
            };
            if model_failure_requires_global_recovery(&error.message) {
                let status = read_only_status(&error);
                permit.finish(status);
                return Err(bootstrap_core_error(error));
            }
            let status = capability_unavailable_status(&error);
            permit.finish(status.clone());
            return Ok(MigrationResolutionOutcome {
                changed: false,
                status,
                review: None,
                selected_plan: selected.map(|action| action.plan),
            });
        }
        match model_profile_stage() {
            Ok(BootstrapProgress::Review(review)) => {
                let status = review_status(&review);
                set_active_review(Some(review.clone()));
                permit.finish(status.clone());
                return Ok(MigrationResolutionOutcome {
                    changed: false,
                    status,
                    review: Some(review),
                    selected_plan: selected.map(|action| action.plan),
                });
            }
            Ok(BootstrapProgress::Ready) => match run_post_profile_steps(&BTreeSet::new()) {
                Ok(()) => {
                    clear_active_review();
                    let status = BackendStatus::Ready;
                    permit.finish(status.clone());
                    return Ok(MigrationResolutionOutcome {
                        changed: true,
                        status,
                        review: None,
                        selected_plan: selected.map(|action| action.plan),
                    });
                }
                Err(error) if model_failure_requires_global_recovery(&error.message) => {
                    clear_active_review();
                    let status = read_only_status(&error);
                    permit.finish(status);
                    return Err(bootstrap_core_error(error));
                }
                Err(error) => {
                    clear_active_review();
                    let status = capability_unavailable_status(&error);
                    permit.finish(status.clone());
                    return Ok(MigrationResolutionOutcome {
                        changed: true,
                        status,
                        review: None,
                        selected_plan: selected.map(|action| action.plan),
                    });
                }
            },
            Ok(BootstrapProgress::CapabilityUnavailable(error)) => {
                clear_active_review();
                let status = capability_unavailable_status(&error);
                permit.finish(status.clone());
                return Ok(MigrationResolutionOutcome {
                    changed: false,
                    status,
                    review: None,
                    selected_plan: selected.map(|action| action.plan),
                });
            }
            Err(error) => {
                clear_active_review();
                let status = read_only_status(&error);
                permit.finish(status);
                return Err(bootstrap_core_error(error));
            }
        }
    }

    let selected = selected.expect("commit strategies select a plan");
    let commit = crate::assets::commit_asset_operation(AssetCommitRequest {
        operation_id: selected.plan.operation_id.clone(),
        candidate_hash: selected.plan.candidate_hash.clone(),
        conflict_confirmation: selected
            .plan
            .requires_conflict_confirmation
            .then(|| selected.plan.candidate_hash.clone()),
    });
    if let Err(message) = commit {
        if is_stale_review_error(&message) {
            let status = review_status(&current);
            set_active_review(Some(current));
            permit.finish(status);
            return Err(review_stale_error());
        }
        let error = BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message,
        };
        clear_active_review();
        for action in &current.actions {
            let _ = crate::assets::cancel_asset_operation(&action.plan.operation_id);
        }
        if model_failure_requires_global_recovery(&error.message) {
            let status = read_only_status(&error);
            permit.finish(status);
            return Err(bootstrap_core_error(error));
        }
        let status = capability_unavailable_status(&error);
        permit.finish(status.clone());
        return Ok(MigrationResolutionOutcome {
            changed: false,
            status,
            review: None,
            selected_plan: Some(selected.plan),
        });
    }
    for action in &current.actions {
        if action.plan.operation_id != selected.plan.operation_id {
            let _ = crate::assets::cancel_asset_operation(&action.plan.operation_id);
        }
    }
    let preserved_agent_targets = if request.strategy == MigrationResolutionStrategy::KeepAgent {
        current
            .blockers
            .iter()
            .map(|blocker| blocker.agent_id.clone())
            .collect()
    } else {
        BTreeSet::new()
    };
    if let Err(error) = run_post_profile_steps(&preserved_agent_targets) {
        clear_active_review();
        if model_failure_requires_global_recovery(&error.message) {
            let status = read_only_status(&error);
            permit.finish(status);
            return Err(bootstrap_core_error(error));
        }
        let status = capability_unavailable_status(&error);
        permit.finish(status.clone());
        return Ok(MigrationResolutionOutcome {
            changed: true,
            status,
            review: None,
            selected_plan: Some(selected.plan),
        });
    }
    clear_active_review();
    let status = BackendStatus::Ready;
    permit.finish(status.clone());
    Ok(MigrationResolutionOutcome {
        changed: true,
        status,
        review: None,
        selected_plan: Some(selected.plan),
    })
}

fn bootstrap_unlocked() -> Result<BootstrapProgress, BootstrapError> {
    run_steps(vec![
        (
            BootstrapStage::GlobalWriteRecovery,
            Box::new(crate::safe_write::recover_global_mutation_intents),
        ),
        (
            BootstrapStage::SkillRecovery,
            Box::new(|| {
                crate::resources::skill::recover_pending()
                    .map_err(|error| error.into_command_parts().message)
            }),
        ),
        (
            BootstrapStage::AssetRecovery,
            Box::new(|| crate::assets::recover_pending_asset_operations().map(|_| ())),
        ),
        (
            BootstrapStage::SettingsMigration,
            Box::new(|| {
                crate::settings::migrate_if_needed()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }),
        ),
        (
            BootstrapStage::McpRegistryMigration,
            Box::new(|| {
                crate::resources::mcp::registry::migrate_registry_to_sources()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }),
        ),
    ])?;
    match model_profile_stage()? {
        BootstrapProgress::Review(review) => Ok(BootstrapProgress::Review(review)),
        BootstrapProgress::CapabilityUnavailable(error) => {
            Ok(BootstrapProgress::CapabilityUnavailable(error))
        }
        BootstrapProgress::Ready => match run_post_profile_steps(&BTreeSet::new()) {
            Ok(()) => Ok(BootstrapProgress::Ready),
            Err(error) if model_failure_requires_global_recovery(&error.message) => Err(error),
            Err(error) => Ok(BootstrapProgress::CapabilityUnavailable(error)),
        },
    }
}

fn model_profile_stage() -> Result<BootstrapProgress, BootstrapError> {
    let Some(plan) =
        crate::assets::plan_model_schema_v2_migration().map_err(|message| BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message,
        })?
    else {
        return Ok(BootstrapProgress::Ready);
    };
    if !plan.can_commit {
        let _ = crate::assets::cancel_asset_operation(&plan.operation_id);
        return Ok(BootstrapProgress::CapabilityUnavailable(BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message: "model_schema_migration_hard_blocked".into(),
        }));
    }
    if plan.requires_conflict_confirmation {
        return match build_review(plan) {
            Ok(review) => Ok(BootstrapProgress::Review(review)),
            Err(error) if is_model_capability_blocker(&error.message) => {
                Ok(BootstrapProgress::CapabilityUnavailable(error))
            }
            Err(error) => Err(error),
        };
    }
    let commit = crate::assets::commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash,
        conflict_confirmation: None,
    });
    if let Err(message) = commit {
        let error = BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message,
        };
        return if model_failure_requires_global_recovery(&error.message) {
            Err(error)
        } else {
            Ok(BootstrapProgress::CapabilityUnavailable(error))
        };
    }
    Ok(BootstrapProgress::Ready)
}

fn is_model_capability_blocker(message: &str) -> bool {
    message.starts_with("model_schema_migration_")
}

fn model_failure_requires_global_recovery(message: &str) -> bool {
    message == "recovery_required"
        || message.starts_with("recovery_required:")
        || message.contains("operation committed but")
        || crate::assets::transaction::pending_recovery_error().is_some()
}

fn run_post_profile_steps(
    preserved_agent_targets: &BTreeSet<String>,
) -> Result<(), BootstrapError> {
    run_steps(vec![
        (
            BootstrapStage::ModelProviderMigration,
            Box::new(|| {
                crate::resources::model::migrate_model_providers_v3_if_needed()?;
                if crate::resources::model::model_provider_reapply_pending()? {
                    crate::resources::model::reapply_managed_models_after_provider_migration(
                        preserved_agent_targets,
                    )?;
                    crate::resources::model::complete_model_provider_reapply()?;
                }
                Ok(())
            }),
        ),
        (
            BootstrapStage::ModelReconciliation,
            Box::new(crate::resources::model::reconcile_active_models),
        ),
    ])
}

fn build_review(use_mux_plan: AssetOperationPlan) -> Result<MigrationReview, BootstrapError> {
    let parsed = use_mux_plan
        .warnings
        .iter()
        .filter_map(|warning| parse_model_warning(warning))
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        let _ = crate::assets::cancel_asset_operation(&use_mux_plan.operation_id);
        return Err(BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message: "model_schema_migration_hard_blocked".into(),
        });
    }
    let settings = crate::settings::load_settings_strict().map_err(|error| BootstrapError {
        stage: BootstrapStage::ModelProfileMigration,
        message: error.to_string(),
    })?;
    // One Agent may store several Profiles in the same native file. Keeping
    // that file byte-for-byte unchanged means none of its legacy identities or
    // Keychain helpers can be rewritten safely, so release the complete Model
    // selection for each affected Agent as external observation.
    let affected_agents = parsed
        .iter()
        .map(|(agent_id, _, _)| agent_id.clone())
        .collect::<BTreeSet<_>>();
    let released = affected_agents
        .into_iter()
        .map(|agent_id| {
            let mut profile_ids = settings
                .model_selection(&agent_id)
                .profiles
                .into_keys()
                .collect::<BTreeSet<_>>();
            profile_ids.extend(parsed.iter().filter_map(|(parsed_agent, profile_id, _)| {
                (parsed_agent == &agent_id).then_some(profile_id.clone())
            }));
            (agent_id, profile_ids)
        })
        .collect::<BTreeMap<_, _>>();
    let keep_agent_plan =
        crate::assets::lifecycle::plan_model_schema_v2_migration_preserving(&released)
            .map_err(|message| BootstrapError {
                stage: BootstrapStage::ModelProfileMigration,
                message,
            })?
            .ok_or_else(|| BootstrapError {
                stage: BootstrapStage::ModelProfileMigration,
                message: "model_schema_migration_stale".into(),
            })?;
    if !keep_agent_plan.can_commit {
        let _ = crate::assets::cancel_asset_operation(&use_mux_plan.operation_id);
        let _ = crate::assets::cancel_asset_operation(&keep_agent_plan.operation_id);
        return Err(BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message: "model_schema_migration_keep_agent_hard_blocked".into(),
        });
    }

    let (id_map, _) =
        crate::resources::model::migrated_profiles_v2(&settings).map_err(|message| {
            BootstrapError {
                stage: BootstrapStage::ModelProfileMigration,
                message,
            }
        })?;
    let agent_views = crate::resources::model::list_agents()
        .into_iter()
        .map(|agent| (agent.id.clone(), agent))
        .collect::<BTreeMap<_, _>>();
    let (use_before, use_after) = model_sides(&use_mux_plan)?;
    let (_, keep_after) = model_sides(&keep_agent_plan)?;
    let mut blockers = Vec::new();
    for (agent_id, old_profile_id, reason) in parsed {
        let new_profile_id = id_map.get(&old_profile_id).ok_or_else(|| BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message: "model_schema_migration_missing_profile".into(),
        })?;
        let before_selection = use_before.get(&agent_id).cloned().unwrap_or_default();
        let after_selection = use_after.get(&agent_id).cloned().unwrap_or_default();
        let before_record = before_selection.profiles.get(&old_profile_id);
        let after_record = after_selection.profiles.get(new_profile_id);
        let agent = agent_views.get(&agent_id);
        blockers.push(MigrationBlocker {
            agent_id: agent_id.clone(),
            agent_name: agent
                .map(|agent| agent.name.clone())
                .unwrap_or_else(|| humanize_id(&agent_id)),
            target_files: agent
                .map(|agent| agent.config_paths.clone())
                .unwrap_or_default(),
            profile_id: old_profile_id.clone(),
            reason: reason.clone(),
            message: blocker_message(&reason).into(),
            before: ModelMigrationState {
                profile_id: old_profile_id.clone(),
                enabled: before_record.is_none_or(|record| record.enabled),
                active: before_selection.active_profile_id.as_deref()
                    == Some(old_profile_id.as_str()),
            },
            after: ModelMigrationState {
                profile_id: new_profile_id.clone(),
                enabled: after_record.is_none_or(|record| record.enabled),
                active: after_selection.active_profile_id.as_deref()
                    == Some(new_profile_id.as_str()),
            },
            keep_agent_fallback_profile_id: keep_after
                .get(&agent_id)
                .and_then(|selection| selection.active_profile_id.clone()),
            keep_agent_released_profile_ids: released
                .get(&agent_id)
                .into_iter()
                .flatten()
                .cloned()
                .collect(),
            migrates_keychain_reference: crate::resources::model::credential_present(
                &old_profile_id,
            ),
            agent_restart_recommended: true,
            mux_owned_field_categories: vec![
                "Model identity".into(),
                "Provider and endpoint".into(),
                "active Model pointer".into(),
                "credential reference".into(),
            ],
        });
    }
    blockers.sort_by(|left, right| {
        (&left.agent_id, &left.profile_id).cmp(&(&right.agent_id, &right.profile_id))
    });
    let actions = vec![
        MigrationActionPlan {
            strategy: MigrationResolutionStrategy::UseMux,
            title: "Use MUX configuration and continue".into(),
            consequence: "Replace only reviewed MUX-owned Model fields, migrate Model identities and Keychain references, then continue startup.".into(),
            modifies_agent_targets: true,
            preserves_agent_targets: false,
            plan: use_mux_plan,
        },
        MigrationActionPlan {
            strategy: MigrationResolutionStrategy::KeepAgent,
            title: "Keep current Agent configuration and continue".into(),
            consequence: "Keep affected Agent files and their existing Keychain references byte-for-byte unchanged, and release every Model relationship for those Agents as external observations.".into(),
            modifies_agent_targets: false,
            preserves_agent_targets: true,
            plan: keep_agent_plan,
        },
    ];
    let review_hash = hash_review(&actions);
    Ok(MigrationReview {
        stage: BootstrapStage::ModelProfileMigration,
        source_schema_version: 1,
        target_schema_version: 2,
        review_hash,
        can_commit: actions.iter().all(|action| action.plan.can_commit),
        requires_conflict_confirmation: true,
        blockers,
        actions,
        supported_actions: vec![
            MigrationResolutionStrategy::UseMux,
            MigrationResolutionStrategy::KeepAgent,
            MigrationResolutionStrategy::Recheck,
            MigrationResolutionStrategy::Later,
        ],
    })
}

fn model_sides(
    plan: &AssetOperationPlan,
) -> Result<(&ModelSelectionMap, &ModelSelectionMap), BootstrapError> {
    match &plan.domain_plan {
        DomainPlan::Model { before, after } => Ok((before, after)),
        _ => Err(BootstrapError {
            stage: BootstrapStage::ModelProfileMigration,
            message: "model_schema_migration_domain_mismatch".into(),
        }),
    }
}

type ModelSelectionMap = BTreeMap<String, crate::domain::assets::ModelAgentSelection>;

fn parse_model_warning(warning: &str) -> Option<(String, String, String)> {
    let (agent_id, remainder) = warning.split_once(" / model:")?;
    let (profile_id, reason) = remainder.split_once(": ")?;
    matches!(reason, "model_owned_fields_drift" | "model_target_missing").then(|| {
        (
            agent_id.to_string(),
            profile_id.to_string(),
            reason.to_string(),
        )
    })
}

fn blocker_message(reason: &str) -> &'static str {
    match reason {
        "model_owned_fields_drift" => "Agent 中由 MUX 管理的模型字段已与中央配置不同",
        "model_target_missing" => "Agent 的模型配置目标不存在",
        _ => "模型配置需要人工审核",
    }
}

fn humanize_id(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn hash_review(actions: &[MigrationActionPlan]) -> String {
    let bindings = actions
        .iter()
        .map(|action| (&action.strategy, &action.plan.candidate_hash))
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&bindings).expect("migration review bindings serialize");
    hex::encode(Sha256::digest(bytes))
}

fn action_plan(
    review: &MigrationReview,
    strategy: MigrationResolutionStrategy,
) -> Option<&MigrationActionPlan> {
    review
        .actions
        .iter()
        .find(|action| action.strategy == strategy)
}

fn cancel_review_plans(review: &MigrationReview) -> Result<(), String> {
    for action in &review.actions {
        crate::assets::cancel_asset_operation(&action.plan.operation_id)?;
    }
    Ok(())
}

fn active_review() -> Option<MigrationReview> {
    ACTIVE_MIGRATION_REVIEW
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn set_active_review(review: Option<MigrationReview>) {
    *ACTIVE_MIGRATION_REVIEW
        .write()
        .unwrap_or_else(|error| error.into_inner()) = review;
}

fn clear_active_review() {
    set_active_review(None);
}

fn review_status(review: &MigrationReview) -> BackendStatus {
    BackendStatus::MigrationReviewRequired {
        stage: review.stage.code().into(),
        review_hash: review.review_hash.clone(),
        message: "Model configuration upgrade needs confirmation".into(),
        blocked_capabilities: vec![CapabilityDomain::Model],
    }
}

fn capability_unavailable_status(error: &BootstrapError) -> BackendStatus {
    let hard_blocked = is_model_capability_blocker(&error.message);
    BackendStatus::CapabilityUnavailable {
        capability: CapabilityDomain::Model,
        stage: error.stage.code().into(),
        code: if hard_blocked {
            "migration_hard_blocked"
        } else {
            "model_capability_unavailable"
        }
        .into(),
        message: if hard_blocked {
            "Model configuration cannot be migrated automatically; repair the affected Agent Model target, then restart or recheck MUX"
        } else {
            "Model startup maintenance did not complete safely; inspect the affected Agent or Provider configuration, then restart MUX"
        }
        .into(),
    }
}

fn read_only_status(error: &BootstrapError) -> BackendStatus {
    BackendStatus::ReadOnly {
        stage: error.stage.code().into(),
        message: error.message.clone(),
    }
}

fn review_unavailable_error() -> CoreError {
    CoreError::new(
        "migration_review_unavailable",
        "No Model migration review is currently pending",
    )
}

fn review_stale_error() -> CoreError {
    CoreError::new(
        "migration_review_stale",
        "The Model migration candidate changed; recheck it before continuing",
    )
}

fn is_stale_review_error(message: &str) -> bool {
    message.contains("stale")
        || message.contains("changed after review")
        || message.contains("did not match the reviewed")
}

fn bootstrap_core_error(error: BootstrapError) -> CoreError {
    CoreError::new(
        "bootstrap_failed",
        "MUX could not continue the Model migration",
    )
    .with_detail("stage", Value::String(error.stage.code().into()))
}

type BootstrapStep<'a> = (BootstrapStage, Box<dyn FnOnce() -> Result<(), String> + 'a>);

fn run_steps(steps: Vec<BootstrapStep<'_>>) -> Result<(), BootstrapError> {
    for (stage, operation) in steps {
        operation().map_err(|message| BootstrapError { stage, message })?;
    }
    Ok(())
}

fn failure_outcome(
    frontend: Frontend,
    error: BootstrapError,
    status: BackendStatus,
) -> Result<BootstrapReport, BootstrapError> {
    match frontend {
        Frontend::Cli => Err(error),
        Frontend::Desktop => Ok(BootstrapReport {
            warnings: vec![BootstrapWarning {
                stage: error.stage,
                message: error.message,
            }],
            skill_updates_allowed: false,
            status,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::assets::{ModelAgentSelection, ModelConsumptionRecord};
    use crate::domain::types::{ModelProfile, ModelProtocol};
    use crate::settings::{load_settings_strict, mutate_settings};
    use crate::testenv::TestHome;
    use std::cell::RefCell;
    use std::fs;

    fn failure(stage: BootstrapStage) -> BootstrapError {
        BootstrapError {
            stage,
            message: "broken journal".into(),
        }
    }

    fn legacy_claude_profile() -> ModelProfile {
        ModelProfile {
            id: "legacy-claude".into(),
            provider_id: None,
            name: "Legacy Claude".into(),
            provider: "anthropic".into(),
            model_vendor: Some("anthropic".into()),
            native_ids: Default::default(),
            protocol: ModelProtocol::AnthropicMessages,
            base_url: "https://api.anthropic.com".into(),
            endpoint_path: "/v1/messages".into(),
            model: "claude-sonnet-4-5".into(),
            env_key: Some("ANTHROPIC_API_KEY".into()),
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(true),
        }
    }

    fn legacy_responses_profile(id: &str, model: &str) -> ModelProfile {
        ModelProfile {
            id: id.into(),
            provider_id: None,
            name: id.into(),
            provider: "custom".into(),
            model_vendor: Some("openai".into()),
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: format!("https://{id}.example.invalid/v1"),
            endpoint_path: String::new(),
            model: model.into(),
            env_key: None,
            context_window: None,
            max_output_tokens: None,
            reasoning: Some(false),
        }
    }

    fn drifted_claude_fixture(tag: &str) -> (TestHome, std::path::PathBuf, Vec<u8>) {
        let home = TestHome::new(tag);
        let profile = legacy_claude_profile();
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(profile.id.clone(), profile.clone());
        })
        .unwrap();
        crate::resources::model::apply_credential_update(
            &profile.id,
            Some("bootstrap-review-secret"),
        )
        .unwrap();
        crate::resources::model::apply_profile("claude-code", &profile.id).unwrap();
        let target = home.home.join(".claude/settings.json");
        let synced = fs::read_to_string(&target).unwrap();
        let drifted = synced.replace(&profile.model, "claude-local-alias");
        assert_ne!(synced, drifted);
        fs::write(&target, drifted.as_bytes()).unwrap();
        (home, target, drifted.into_bytes())
    }

    #[test]
    fn cli_recovery_fails_closed() {
        let error = failure(BootstrapStage::AssetRecovery);
        let result = failure_outcome(
            Frontend::Cli,
            error,
            BackendStatus::ReadOnly {
                stage: "asset_recovery".into(),
                message: "broken journal".into(),
            },
        )
        .unwrap_err();
        assert_eq!(result.stage, BootstrapStage::AssetRecovery);
    }

    #[test]
    fn desktop_failure_is_diagnostic_but_read_only() {
        let error = failure(BootstrapStage::SkillRecovery);
        let report = failure_outcome(
            Frontend::Desktop,
            error,
            BackendStatus::ReadOnly {
                stage: "skill_recovery".into(),
                message: "broken journal".into(),
            },
        )
        .unwrap();
        assert!(!report.skill_updates_allowed);
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn stages_have_stable_serialized_codes() {
        assert_eq!(
            serde_json::to_string(&BootstrapStage::GlobalWriteRecovery).unwrap(),
            "\"global_write_recovery\""
        );
        assert_eq!(
            BootstrapStage::ModelProviderMigration.code(),
            "model_provider_migration"
        );
    }

    #[test]
    fn ordered_stages_stop_at_the_first_failure() {
        let executed = RefCell::new(Vec::new());
        let result = run_steps(vec![
            (
                BootstrapStage::GlobalWriteRecovery,
                Box::new(|| {
                    executed.borrow_mut().push("global");
                    Ok(())
                }),
            ),
            (
                BootstrapStage::SkillRecovery,
                Box::new(|| {
                    executed.borrow_mut().push("skill");
                    Err("broken journal".into())
                }),
            ),
            (
                BootstrapStage::AssetRecovery,
                Box::new(|| {
                    executed.borrow_mut().push("asset");
                    Ok(())
                }),
            ),
        ]);

        assert_eq!(result.unwrap_err().stage, BootstrapStage::SkillRecovery);
        assert_eq!(*executed.borrow(), ["global", "skill"]);
    }

    #[test]
    fn drifted_model_bootstrap_publishes_review_and_keep_agent_continues() {
        let (_home, target, original) = drifted_claude_fixture("bootstrap-review-keep");

        let report = bootstrap(Frontend::Desktop).unwrap();
        assert!(matches!(
            report.status,
            BackendStatus::MigrationReviewRequired { .. }
        ));
        assert!(!report.skill_updates_allowed);
        let review = migration_review().unwrap();
        assert_eq!(review.blockers.len(), 1);
        assert_eq!(review.blockers[0].agent_id, "claude-code");
        assert_eq!(
            review.blockers[0].message,
            "Agent 中由 MUX 管理的模型字段已与中央配置不同"
        );
        assert!(!serde_json::to_string(&review)
            .unwrap()
            .contains("claude-local-alias"));
        let action = action_plan(&review, MigrationResolutionStrategy::KeepAgent)
            .unwrap()
            .clone();
        let outcome = resolve_migration(ResolveMigrationRequest {
            review_hash: review.review_hash,
            strategy: MigrationResolutionStrategy::KeepAgent,
            candidate_hash: Some(action.plan.candidate_hash),
            confirmed: true,
            dry_run: false,
        })
        .unwrap();

        assert!(outcome.changed);
        assert_eq!(outcome.status, BackendStatus::Ready);
        assert_eq!(fs::read(target).unwrap(), original);
        let settings = load_settings_strict().unwrap();
        assert_eq!(settings.version, Some(crate::settings::SETTINGS_VERSION));
        assert!(settings.model_selection("claude-code").profiles.is_empty());
        let migrated_id = settings.model_profiles.unwrap().into_keys().next().unwrap();
        assert!(crate::resources::model::credential_present("legacy-claude"));
        assert!(crate::resources::model::credential_present(&migrated_id));
    }

    #[test]
    fn keep_agent_releases_its_shared_target_and_migrates_other_agents() {
        let home = TestHome::new("bootstrap-review-keep-shared-target");
        let pi_drifted = legacy_responses_profile("pi-drifted", "first-model");
        let pi_backup = legacy_responses_profile("pi-backup", "backup-model");
        let codex = legacy_responses_profile("codex-managed", "codex-model");
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings.model_profiles.get_or_insert_default().extend([
                (pi_drifted.id.clone(), pi_drifted.clone()),
                (pi_backup.id.clone(), pi_backup.clone()),
                (codex.id.clone(), codex.clone()),
            ]);
        })
        .unwrap();
        for profile in [&pi_drifted, &pi_backup, &codex] {
            crate::resources::model::apply_credential_update(
                &profile.id,
                Some("bootstrap-shared-target-secret"),
            )
            .unwrap();
        }
        crate::resources::model::apply_profile_consumption("pi", &pi_drifted.id, true).unwrap();
        crate::resources::model::apply_profile_consumption("pi", &pi_backup.id, false).unwrap();
        crate::resources::model::apply_profile("codex", &codex.id).unwrap();
        mutate_settings(|settings| {
            settings.set_model_selection(
                "pi",
                ModelAgentSelection {
                    profiles: BTreeMap::from([
                        (
                            pi_drifted.id.clone(),
                            ModelConsumptionRecord {
                                profile_id: pi_drifted.id.clone(),
                                enabled: true,
                                last_selected_at: None,
                            },
                        ),
                        (
                            pi_backup.id.clone(),
                            ModelConsumptionRecord {
                                profile_id: pi_backup.id.clone(),
                                enabled: true,
                                last_selected_at: None,
                            },
                        ),
                    ]),
                    active_profile_id: Some(pi_drifted.id.clone()),
                },
            );
        })
        .unwrap();

        let pi_models = home.home.join(".pi/agent/models.json");
        let pi_settings = home.home.join(".pi/agent/settings.json");
        let synced_models = fs::read_to_string(&pi_models).unwrap();
        let drifted_models = synced_models.replace("first-model", "local-first-alias");
        assert_ne!(synced_models, drifted_models);
        fs::write(&pi_models, drifted_models).unwrap();
        let pi_models_before = fs::read(&pi_models).unwrap();
        let pi_settings_before = fs::read(&pi_settings).unwrap();

        bootstrap(Frontend::Desktop).unwrap();
        let review = migration_review().unwrap();
        assert_eq!(review.blockers.len(), 1);
        assert_eq!(review.blockers[0].agent_id, "pi");
        assert_eq!(
            review.blockers[0].keep_agent_released_profile_ids,
            vec![pi_backup.id.clone(), pi_drifted.id.clone()]
        );
        let action = action_plan(&review, MigrationResolutionStrategy::KeepAgent)
            .unwrap()
            .clone();
        let outcome = resolve_migration(ResolveMigrationRequest {
            review_hash: review.review_hash,
            strategy: MigrationResolutionStrategy::KeepAgent,
            candidate_hash: Some(action.plan.candidate_hash),
            confirmed: true,
            dry_run: false,
        })
        .unwrap();

        assert_eq!(outcome.status, BackendStatus::Ready);
        assert_eq!(fs::read(pi_models).unwrap(), pi_models_before);
        assert_eq!(fs::read(pi_settings).unwrap(), pi_settings_before);
        let settings = load_settings_strict().unwrap();
        assert!(settings.model_selection("pi").profiles.is_empty());
        let codex_selection = settings.model_selection("codex");
        let codex_id = codex_selection.active_profile_id.unwrap();
        let profiles = settings.model_profiles.unwrap();
        let migrated_codex = profiles.get(&codex_id).unwrap();
        assert_eq!(migrated_codex.model, codex.model);
        assert_eq!(
            crate::resources::model::observe_profile("codex", migrated_codex).unwrap(),
            crate::resources::model::ModelObservedState::Synced
        );
        assert!(crate::resources::model::credential_present(&pi_drifted.id));
        assert!(crate::resources::model::credential_present(&pi_backup.id));
        assert!(!crate::resources::model::credential_present(&codex.id));
        assert!(crate::resources::model::credential_present(&codex_id));
    }

    #[test]
    fn missing_model_target_is_reviewable() {
        let _home = TestHome::new("bootstrap-review-missing");
        let profile = legacy_claude_profile();
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(profile.id.clone(), profile.clone());
            settings
                .model_assignments
                .get_or_insert_default()
                .insert("claude-code".into(), profile.id.clone());
        })
        .unwrap();

        let report = bootstrap(Frontend::Desktop).unwrap();
        assert!(matches!(
            report.status,
            BackendStatus::MigrationReviewRequired { .. }
        ));
        let review = migration_review().unwrap();
        assert_eq!(review.blockers[0].reason, "model_target_missing");
        assert_eq!(review.blockers[0].target_files, ["~/.claude/settings.json"]);
    }

    #[test]
    fn unparseable_model_target_remains_a_hard_blocker() {
        let home = TestHome::new("bootstrap-hard-block");
        let profile = legacy_claude_profile();
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(profile.id.clone(), profile.clone());
            settings
                .model_assignments
                .get_or_insert_default()
                .insert("claude-code".into(), profile.id.clone());
        })
        .unwrap();
        let target = home.home.join(".claude/settings.json");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"{not-valid-json").unwrap();

        let report = bootstrap(Frontend::Desktop).unwrap();
        assert!(matches!(
            report.status,
            BackendStatus::CapabilityUnavailable {
                capability: CapabilityDomain::Model,
                ref stage,
                ..
            } if stage == "model_profile_migration"
        ));
        assert!(report.skill_updates_allowed);
        assert!(migration_review().is_none());
        assert_eq!(fs::read(&target).unwrap(), b"{not-valid-json");
    }

    #[test]
    fn model_failure_scope_depends_on_recovery_evidence() {
        let _home = TestHome::new("bootstrap-failure-scope");
        assert!(!model_failure_requires_global_recovery(
            "asset operation failed and was rolled back: target rejected"
        ));
        assert!(model_failure_requires_global_recovery(
            "recovery_required: rollback could not be proven"
        ));
        assert!(model_failure_requires_global_recovery(
            "asset operation committed but staging cleanup failed"
        ));

        let local = capability_unavailable_status(&BootstrapError {
            stage: BootstrapStage::ModelProviderMigration,
            message: "provider target rejected".into(),
        });
        assert!(matches!(
            local,
            BackendStatus::CapabilityUnavailable { ref code, .. }
                if code == "model_capability_unavailable"
        ));
    }

    #[test]
    fn drifted_model_use_mux_candidate_finishes_bootstrap_and_syncs_agent() {
        let (_home, target, original) = drifted_claude_fixture("bootstrap-review-mux");
        bootstrap(Frontend::Desktop).unwrap();
        let review = migration_review().unwrap();
        let action = action_plan(&review, MigrationResolutionStrategy::UseMux)
            .unwrap()
            .clone();

        let result = resolve_migration(ResolveMigrationRequest {
            review_hash: review.review_hash,
            strategy: MigrationResolutionStrategy::UseMux,
            candidate_hash: Some(action.plan.candidate_hash),
            confirmed: true,
            dry_run: false,
        });
        assert!(
            result.is_ok(),
            "{result:?}; status={:?}",
            super::super::gate::status()
        );
        let outcome = result.unwrap();

        assert_eq!(outcome.status, BackendStatus::Ready);
        assert_ne!(fs::read(&target).unwrap(), original);
        let settings = load_settings_strict().unwrap();
        assert_eq!(settings.version, Some(crate::settings::SETTINGS_VERSION));
        assert_eq!(settings.model_provider_reapply_pending, None);
        let selection = settings.model_selection("claude-code");
        let migrated_id = selection.active_profile_id.unwrap();
        let profile = settings
            .model_profiles
            .unwrap()
            .remove(&migrated_id)
            .unwrap();
        assert_eq!(
            crate::resources::model::observe_profile("claude-code", &profile).unwrap(),
            crate::resources::model::ModelObservedState::Synced
        );
        assert!(!crate::resources::model::credential_present(
            "legacy-claude"
        ));
        assert!(crate::resources::model::credential_present(&migrated_id));
    }

    #[test]
    fn migration_dry_run_preserves_files_and_the_published_review() {
        let (home, target, _) = drifted_claude_fixture("bootstrap-review-dry-run");
        bootstrap(Frontend::Desktop).unwrap();
        let review = migration_review().unwrap();
        let action = action_plan(&review, MigrationResolutionStrategy::UseMux)
            .unwrap()
            .clone();
        let settings_path = home.home.join(".mux/settings.json");
        let settings_before = fs::read(&settings_path).unwrap();
        let target_before = fs::read(&target).unwrap();

        let outcome = resolve_migration(ResolveMigrationRequest {
            review_hash: review.review_hash.clone(),
            strategy: MigrationResolutionStrategy::UseMux,
            candidate_hash: Some(action.plan.candidate_hash.clone()),
            confirmed: false,
            dry_run: true,
        })
        .unwrap();

        assert!(!outcome.changed);
        assert_eq!(outcome.status, review_status(&review));
        assert_eq!(outcome.review, Some(review.clone()));
        assert_eq!(
            outcome.selected_plan.unwrap().candidate_hash,
            action.plan.candidate_hash
        );
        assert_eq!(fs::read(settings_path).unwrap(), settings_before);
        assert_eq!(fs::read(target).unwrap(), target_before);
        assert_eq!(migration_review(), Some(review));
    }

    #[test]
    fn migration_commit_rejects_a_target_changed_after_review() {
        let (_home, target, _) = drifted_claude_fixture("bootstrap-review-stale");
        bootstrap(Frontend::Desktop).unwrap();
        let review = migration_review().unwrap();
        let action = action_plan(&review, MigrationResolutionStrategy::UseMux)
            .unwrap()
            .clone();
        fs::write(&target, b"{\"model\":\"changed-again\"}\n").unwrap();

        let error = resolve_migration(ResolveMigrationRequest {
            review_hash: review.review_hash,
            strategy: MigrationResolutionStrategy::UseMux,
            candidate_hash: Some(action.plan.candidate_hash),
            confirmed: true,
            dry_run: false,
        })
        .unwrap_err();

        assert_eq!(error.code, "migration_review_stale");
        assert!(matches!(
            super::super::gate::status(),
            BackendStatus::MigrationReviewRequired { .. }
        ));
    }

    #[test]
    fn restarting_regenerates_an_equivalent_review_with_stable_candidate_hashes() {
        let (_home, _target, _) = drifted_claude_fixture("bootstrap-review-restart");
        bootstrap(Frontend::Desktop).unwrap();
        let first = migration_review().unwrap();
        let first_hashes = first
            .actions
            .iter()
            .map(|action| (action.strategy, action.plan.candidate_hash.clone()))
            .collect::<Vec<_>>();
        let first_persisted = first
            .actions
            .iter()
            .map(|action| {
                crate::assets::planner::load_operation(&action.plan.operation_id).unwrap()
            })
            .collect::<Vec<_>>();

        bootstrap(Frontend::Desktop).unwrap();
        let second = migration_review().unwrap();
        let second_hashes = second
            .actions
            .iter()
            .map(|action| (action.strategy, action.plan.candidate_hash.clone()))
            .collect::<Vec<_>>();
        let second_persisted = second
            .actions
            .iter()
            .map(|action| {
                crate::assets::planner::load_operation(&action.plan.operation_id).unwrap()
            })
            .collect::<Vec<_>>();

        for (left, right) in first_persisted.iter().zip(&second_persisted) {
            assert_eq!(left.settings_hash, right.settings_hash, "settings hash");
            assert_eq!(
                left.settings_target_hash, right.settings_target_hash,
                "settings target hash"
            );
            assert_eq!(left.target_hashes, right.target_hashes, "target hashes");
            assert_eq!(left.lifecycle, right.lifecycle, "lifecycle");
            assert_eq!(left.plan.domain_plan, right.plan.domain_plan, "domain plan");
            assert_eq!(left.plan.warnings, right.plan.warnings, "warnings");
        }
        assert_eq!(
            first_hashes, second_hashes,
            "review hashes: {} != {}",
            first.review_hash, second.review_hash
        );
        assert_eq!(first.review_hash, second.review_hash);
        assert_ne!(
            first.actions[0].plan.operation_id,
            second.actions[0].plan.operation_id
        );
    }
}
