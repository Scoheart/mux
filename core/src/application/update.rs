//! Product update use cases.

pub use crate::update::UpgradeOutcome;

pub fn managed_by_desktop_app() -> Option<std::path::PathBuf> {
    crate::update::managed_by_desktop_app()
}

pub fn upgrade_cli(current_version: &str) -> Result<Option<UpgradeOutcome>, String> {
    crate::update::upgrade_cli(current_version)
}

pub fn passive_check_notice(current_version: &str) -> Option<String> {
    crate::update::passive_check_notice(current_version)
}
