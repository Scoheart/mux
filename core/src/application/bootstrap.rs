//! One fail-closed startup path for every MUX frontend.
//!
//! Startup recovers MUX-owned transactions and upgrades central metadata. It
//! never treats Agent files as migration input and never rewrites them. Agent
//! state is observed after startup through the regular inventory pipeline.

use super::gate::{BackendStatus, CapabilityDomain};
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

enum BootstrapProgress {
    Ready,
    ModelCentralStateUnavailable(BootstrapError),
}

/// Recover incomplete MUX writes and migrate only MUX-owned central metadata.
/// External Agent edits are not errors and are deliberately absent from this
/// path; they become relationship observations after bootstrap.
pub fn bootstrap(frontend: Frontend) -> Result<BootstrapReport, BootstrapError> {
    let permit = super::gate::begin_bootstrap();
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
        Ok(BootstrapProgress::ModelCentralStateUnavailable(error)) => {
            let status = model_central_state_status(&error);
            permit.finish(status.clone());
            Ok(BootstrapReport {
                warnings: vec![BootstrapWarning {
                    stage: error.stage,
                    message: error.message,
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

    for (stage, operation) in [
        (
            BootstrapStage::ModelProfileMigration,
            crate::assets::migrate_model_profiles_v2_if_needed as fn() -> Result<bool, String>,
        ),
        (
            BootstrapStage::ModelProviderMigration,
            crate::resources::model::migrate_model_providers_v3_if_needed,
        ),
    ] {
        if let Err(message) = operation() {
            let error = BootstrapError { stage, message };
            return if model_failure_requires_global_recovery(&error.message) {
                Err(error)
            } else {
                Ok(BootstrapProgress::ModelCentralStateUnavailable(error))
            };
        }
    }
    Ok(BootstrapProgress::Ready)
}

fn model_failure_requires_global_recovery(message: &str) -> bool {
    message == "recovery_required"
        || message.starts_with("recovery_required:")
        || message.contains("operation committed but")
        || crate::assets::transaction::pending_recovery_error().is_some()
}

fn model_central_state_status(error: &BootstrapError) -> BackendStatus {
    BackendStatus::CapabilityUnavailable {
        capability: CapabilityDomain::Model,
        stage: error.stage.code().into(),
        code: "model_central_state_unavailable".into(),
        message: "MUX central Model metadata could not be upgraded safely; Agent files were left untouched"
            .into(),
    }
}

fn read_only_status(error: &BootstrapError) -> BackendStatus {
    BackendStatus::ReadOnly {
        stage: error.stage.code().into(),
        message: error.message.clone(),
    }
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

    fn legacy_profile() -> ModelProfile {
        ModelProfile {
            id: "legacy-profile".into(),
            provider_id: None,
            name: "Legacy".into(),
            provider: "custom".into(),
            model_vendor: Some("openai".into()),
            native_ids: Default::default(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://example.invalid/v1".into(),
            endpoint_path: String::new(),
            model: "gpt-external".into(),
            env_key: Some("EXAMPLE_API_KEY".into()),
            context_window: None,
            max_output_tokens: None,
            reasoning: None,
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
    fn model_schema_upgrade_never_rewrites_external_agent_state() {
        let home = TestHome::new("bootstrap-observed-model");
        let profile = legacy_profile();
        mutate_settings(|settings| {
            settings.version = Some(1);
            settings
                .model_profiles
                .get_or_insert_default()
                .insert(profile.id.clone(), profile.clone());
            settings.set_model_selection(
                "grok-build",
                ModelAgentSelection {
                    profiles: std::collections::BTreeMap::from([(
                        profile.id.clone(),
                        ModelConsumptionRecord {
                            profile_id: profile.id.clone(),
                            enabled: true,
                            last_selected_at: None,
                        },
                    )]),
                    active_profile_id: Some(profile.id.clone()),
                },
            );
        })
        .unwrap();
        let target = home.home.join(".grok/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let external = b"[models]\ndefault = 'external'\n";
        fs::write(&target, external).unwrap();

        let report = bootstrap(Frontend::Desktop).unwrap();

        assert!(matches!(report.status, BackendStatus::Ready));
        assert_eq!(fs::read(&target).unwrap(), external);
        assert_eq!(
            load_settings_strict().unwrap().version,
            Some(crate::settings::SETTINGS_VERSION)
        );
    }
}
