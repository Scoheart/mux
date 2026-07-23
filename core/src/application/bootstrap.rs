//! One startup path for every MUX frontend.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frontend {
    Cli,
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapStage {
    SkillRecovery,
    AssetRecovery,
    ModelMigration,
    ModelReconciliation,
}

impl BootstrapStage {
    pub fn code(self) -> &'static str {
        match self {
            Self::SkillRecovery => "skill_recovery",
            Self::AssetRecovery => "asset_recovery",
            Self::ModelMigration => "model_migration",
            Self::ModelReconciliation => "model_reconciliation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapWarning {
    pub stage: BootstrapStage,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapReport {
    pub warnings: Vec<BootstrapWarning>,
    pub skill_updates_allowed: bool,
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

/// Migrate storage, recover incomplete operations, and prepare the shared
/// runtime. CLI fails closed on an unresolved transaction; Desktop stays
/// available for diagnosis but suppresses background Skill updates.
pub fn bootstrap(frontend: Frontend) -> Result<BootstrapReport, BootstrapError> {
    super::gate::write(|| bootstrap_unlocked(frontend))
}

fn bootstrap_unlocked(frontend: Frontend) -> Result<BootstrapReport, BootstrapError> {
    crate::settings::migrate_if_needed();
    crate::resources::mcp::registry::migrate_registry_to_sources();

    let mut warnings = Vec::new();
    let skill_recovery_ok = record_recovery(
        frontend,
        BootstrapStage::SkillRecovery,
        crate::resources::skill::recover_pending()
            .map(|_| ())
            .map_err(|error| error.into_command_parts().message),
        &mut warnings,
    )?;
    let asset_recovery_ok = record_recovery(
        frontend,
        BootstrapStage::AssetRecovery,
        crate::assets::recover_pending_asset_operations().map(|_| ()),
        &mut warnings,
    )?;
    let model_migration_ok = if asset_recovery_ok {
        match crate::assets::migrate_model_profiles_v2_if_needed() {
            Ok(_) => true,
            Err(message) => {
                warnings.push(BootstrapWarning {
                    stage: BootstrapStage::ModelMigration,
                    message,
                });
                false
            }
        }
    } else {
        false
    };
    let model_reconciliation_ok = if model_migration_ok {
        record_recovery(
            frontend,
            BootstrapStage::ModelReconciliation,
            crate::resources::model::reconcile_active_models(),
            &mut warnings,
        )?
    } else {
        false
    };

    Ok(BootstrapReport {
        skill_updates_allowed: skill_recovery_ok
            && asset_recovery_ok
            && model_migration_ok
            && model_reconciliation_ok,
        warnings,
    })
}

fn record_recovery(
    frontend: Frontend,
    stage: BootstrapStage,
    result: Result<(), String>,
    warnings: &mut Vec<BootstrapWarning>,
) -> Result<bool, BootstrapError> {
    match result {
        Ok(()) => Ok(true),
        Err(message) if frontend == Frontend::Cli => Err(BootstrapError { stage, message }),
        Err(message) => {
            warnings.push(BootstrapWarning { stage, message });
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_recovery_fails_closed() {
        let error = record_recovery(
            Frontend::Cli,
            BootstrapStage::AssetRecovery,
            Err("broken journal".into()),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(error.stage, BootstrapStage::AssetRecovery);
    }

    #[test]
    fn desktop_recovery_stays_diagnostic() {
        let mut warnings = Vec::new();
        let recovered = record_recovery(
            Frontend::Desktop,
            BootstrapStage::SkillRecovery,
            Err("broken journal".into()),
            &mut warnings,
        )
        .unwrap();
        assert!(!recovered);
        assert_eq!(warnings.len(), 1);
    }
}
