//! One fail-closed startup path for every MUX frontend.

use super::gate::BackendStatus;
use serde::{Deserialize, Serialize};
use std::fmt;

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

/// Recover incomplete writes, migrate storage, and reconcile projections while
/// holding the process-wide exclusive gate. Desktop remains available for
/// diagnosis after a failure, but the backend is published as read-only before
/// any IPC handler exists. CLI callers fail immediately.
pub fn bootstrap(frontend: Frontend) -> Result<BootstrapReport, BootstrapError> {
    let permit = super::gate::begin_bootstrap();
    let _cross_process_guard = match super::gate::acquire_cross_process_mutation_lock() {
        Ok(guard) => guard,
        Err(_) => {
            let error = BootstrapError {
                stage: BootstrapStage::GlobalWriteRecovery,
                message: "failed to acquire the cross-process mutation lock".into(),
            };
            let status = BackendStatus::ReadOnly {
                stage: error.stage.code().into(),
                message: error.message.clone(),
            };
            permit.finish(status.clone());
            return failure_outcome(frontend, error, status);
        }
    };
    match bootstrap_unlocked() {
        Ok(()) => {
            let status = BackendStatus::Ready;
            permit.finish(status.clone());
            Ok(BootstrapReport {
                warnings: Vec::new(),
                skill_updates_allowed: true,
                status,
            })
        }
        Err(error) => {
            let status = BackendStatus::ReadOnly {
                stage: error.stage.code().into(),
                message: error.message.clone(),
            };
            permit.finish(status.clone());
            failure_outcome(frontend, error, status)
        }
    }
}

/// Strict dependency order. The first error returns immediately, so no later
/// recovery, migration, reconciliation, or background mutation can run.
fn bootstrap_unlocked() -> Result<(), BootstrapError> {
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
        (
            BootstrapStage::ModelProfileMigration,
            Box::new(|| crate::assets::migrate_model_profiles_v2_if_needed().map(|_| ())),
        ),
        (
            BootstrapStage::ModelProviderMigration,
            Box::new(|| {
                crate::resources::model::migrate_model_providers_v3_if_needed().map(|_| ())
            }),
        ),
        (
            BootstrapStage::ModelReconciliation,
            Box::new(crate::resources::model::reconcile_active_models),
        ),
    ])
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
    use std::cell::RefCell;

    fn failure(stage: BootstrapStage) -> BootstrapError {
        BootstrapError {
            stage,
            message: "broken journal".into(),
        }
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
        assert_eq!(
            report.status,
            BackendStatus::ReadOnly {
                stage: "skill_recovery".into(),
                message: "broken journal".into(),
            }
        );
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
}
