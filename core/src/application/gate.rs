//! Process-wide consistency and backend-readiness gate.
//!
//! Domain engines retain their own narrow locks for crash safety. This gate
//! coordinates frontend-facing operations in one process and makes startup
//! recovery a hard write boundary: queries remain available for diagnosis,
//! while every staging or final mutation requires a fully ready backend.

use crate::domain::error::{CoreError, CoreResult};
use crate::resources::skill::SkillError;
use crate::safe_write::{acquire_settings_lock, SettingsLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockWriteGuard};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BackendStatus {
    Starting,
    Ready,
    ReadOnly { stage: String, message: String },
}

impl BackendStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

static WORKSPACE_GATE: RwLock<()> = RwLock::new(());
static BACKEND_STATUS: RwLock<BackendStatus> = RwLock::new(BackendStatus::Starting);
static TEST_LIFECYCLE_GATE: Mutex<()> = Mutex::new(());

/// Read-only operations are deliberately available in every backend state.
pub(crate) fn query<T>(operation: impl FnOnce() -> T) -> T {
    let _guard = WORKSPACE_GATE
        .read()
        .unwrap_or_else(|error| error.into_inner());
    operation()
}

/// Compatibility name for existing query services.
pub(crate) fn read<T>(operation: impl FnOnce() -> T) -> T {
    query(operation)
}

/// A preparation may write private staging, so it is blocked unless startup
/// completed successfully. It uses the exclusive process gate because a
/// recovery-required result must be latched before another mutation starts.
pub(crate) fn prepare<R: GatedResult>(stage: &'static str, operation: impl FnOnce() -> R) -> R {
    mutate(stage, operation)
}

/// Existing application writers enter through this compatibility name. The
/// generic result adapter preserves their public error types while enforcing
/// one readiness policy for all domains.
pub(crate) fn write<R: GatedResult>(operation: impl FnOnce() -> R) -> R {
    mutate("application_mutation", operation)
}

pub(crate) fn mutate<R: GatedResult>(stage: &'static str, operation: impl FnOnce() -> R) -> R {
    let _guard = WORKSPACE_GATE
        .write()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(error) = blocker_for_status(&current_status()) {
        return R::blocked(error);
    }
    let _cross_process_guard = match acquire_cross_process_mutation_lock() {
        Ok(guard) => guard,
        Err(_) => return R::blocked(mutation_coordination_error()),
    };
    if let Some(error) = mutation_blocker() {
        return R::blocked(error);
    }

    let result = operation();
    if let Some(message) = result.recovery_required() {
        latch_read_only(stage, message);
    }
    result
}

pub(crate) fn mutate_core<T>(
    stage: &'static str,
    operation: impl FnOnce() -> CoreResult<T>,
) -> CoreResult<T> {
    mutate(stage, operation)
}

pub fn status() -> BackendStatus {
    let _guard = WORKSPACE_GATE
        .read()
        .unwrap_or_else(|error| error.into_inner());
    current_status()
}

/// Startup is the only privileged writer allowed while the backend is
/// `Starting` or `ReadOnly`. The permit holds the exclusive gate across every
/// recovery and migration stage.
pub(crate) fn begin_bootstrap() -> BootstrapPermit {
    let guard = WORKSPACE_GATE
        .write()
        .unwrap_or_else(|error| error.into_inner());
    set_status(BackendStatus::Starting);
    BootstrapPermit {
        _guard: guard,
        completed: false,
    }
}

pub(crate) struct BootstrapPermit {
    _guard: RwLockWriteGuard<'static, ()>,
    completed: bool,
}

impl BootstrapPermit {
    pub(crate) fn finish(mut self, status: BackendStatus) {
        debug_assert!(!matches!(status, BackendStatus::Starting));
        set_status(status);
        self.completed = true;
    }
}

impl Drop for BootstrapPermit {
    fn drop(&mut self) {
        if !self.completed {
            set_status(BackendStatus::ReadOnly {
                stage: "bootstrap".into(),
                message: "backend bootstrap ended before publishing a final state".into(),
            });
        }
    }
}

fn current_status() -> BackendStatus {
    BACKEND_STATUS
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn set_status(status: BackendStatus) {
    *BACKEND_STATUS
        .write()
        .unwrap_or_else(|error| error.into_inner()) = status;
}

fn latch_read_only(stage: &str, message: String) {
    let mut status = BACKEND_STATUS
        .write()
        .unwrap_or_else(|error| error.into_inner());
    *status = latched_status(&status, stage, message);
}

fn mutation_blocker() -> Option<CoreError> {
    let status = current_status();
    if let Some(error) = blocker_for_status(&status) {
        return Some(error);
    }
    let pending = crate::assets::transaction::pending_recovery_error()?;
    latch_read_only("durable_recovery", pending);
    blocker_for_status(&current_status())
}

pub(crate) fn acquire_cross_process_mutation_lock() -> Result<SettingsLock, String> {
    acquire_settings_lock(
        &crate::paths::mux_dir()
            .join("locks")
            .join("application-mutation"),
    )
}

fn mutation_coordination_error() -> CoreError {
    CoreError {
        code: "mutation_busy".into(),
        message: "another MUX process is mutating shared state; retry this operation".into(),
        details: BTreeMap::new(),
        retry_at: None,
        confirmation: None,
    }
}

fn latched_status(current: &BackendStatus, stage: &str, message: String) -> BackendStatus {
    match current {
        BackendStatus::ReadOnly { .. } => current.clone(),
        BackendStatus::Starting | BackendStatus::Ready => BackendStatus::ReadOnly {
            stage: stage.into(),
            message,
        },
    }
}

/// TestHome already serializes the process-wide HOME/MUX_HOME environment for
/// its entire lifetime. Use that same scope to make application mutations
/// explicitly ready and restore the prior lifecycle state before releasing the
/// environment lock.
pub(crate) struct TestReadyGuard {
    previous: Option<BackendStatus>,
    _lifecycle_guard: MutexGuard<'static, ()>,
}

impl TestReadyGuard {
    pub(crate) fn enter() -> Self {
        Self::enter_status(BackendStatus::Ready)
    }

    fn enter_status(next: BackendStatus) -> Self {
        let lifecycle_guard = TEST_LIFECYCLE_GATE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _guard = WORKSPACE_GATE
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let previous = current_status();
        set_status(next);
        Self {
            previous: Some(previous),
            _lifecycle_guard: lifecycle_guard,
        }
    }

    pub(crate) fn restore(mut self) {
        self.restore_inner();
    }

    fn restore_inner(&mut self) {
        let Some(previous) = self.previous.take() else {
            return;
        };
        let _guard = WORKSPACE_GATE
            .write()
            .unwrap_or_else(|error| error.into_inner());
        set_status(previous);
    }
}

impl Drop for TestReadyGuard {
    fn drop(&mut self) {
        self.restore_inner();
    }
}

fn blocker_for_status(status: &BackendStatus) -> Option<CoreError> {
    match status {
        BackendStatus::Starting => {
            let mut details = BTreeMap::new();
            details.insert("backend_state".into(), Value::String("starting".into()));
            Some(CoreError {
                code: "backend_initializing".into(),
                message: "MUX backend startup has not completed".into(),
                details,
                retry_at: None,
                confirmation: None,
            })
        }
        BackendStatus::Ready => None,
        BackendStatus::ReadOnly { stage, message } => {
            let mut details = BTreeMap::new();
            details.insert("backend_state".into(), Value::String("read_only".into()));
            details.insert("stage".into(), Value::String(stage.clone()));
            Some(CoreError {
                code: "recovery_required".into(),
                message: format!("MUX is read-only because {stage} failed: {message}"),
                details,
                retry_at: None,
                confirmation: None,
            })
        }
    }
}

pub(crate) trait GatedResult {
    fn blocked(error: CoreError) -> Self;
    fn recovery_required(&self) -> Option<String>;
}

impl<T> GatedResult for CoreResult<T> {
    fn blocked(error: CoreError) -> Self {
        Err(error)
    }

    fn recovery_required(&self) -> Option<String> {
        self.as_ref()
            .err()
            .filter(|error| error.code == "recovery_required")
            .map(|error| error.message.clone())
    }
}

impl<T> GatedResult for Result<T, String> {
    fn blocked(error: CoreError) -> Self {
        Err(format!("{}: {}", error.code, error.message))
    }

    fn recovery_required(&self) -> Option<String> {
        let error = self.as_ref().err()?;
        if error == "recovery_required" {
            return Some(error.clone());
        }
        if let Some(message) = error
            .strip_prefix("recovery_required:")
            .map(|message| message.trim().to_string())
        {
            return Some(message);
        }
        crate::assets::transaction::pending_recovery_error()
            .map(|pending| format!("{error}; {pending}"))
    }
}

impl<T> GatedResult for Result<T, SkillError> {
    fn blocked(error: CoreError) -> Self {
        if error.code == "recovery_required" {
            Err(SkillError::RecoveryRequired {
                message: error.message,
            })
        } else {
            Err(SkillError::Conflict {
                message: error.message,
                path: String::new(),
            })
        }
    }

    fn recovery_required(&self) -> Option<String> {
        match self {
            Err(SkillError::RecoveryRequired { message }) => Some(message.clone()),
            Err(_) => crate::assets::transaction::pending_recovery_error(),
            Ok(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn status_serialization_is_stable_and_tagged() {
        assert_eq!(
            serde_json::to_value(BackendStatus::Starting).unwrap(),
            serde_json::json!({"state": "starting"})
        );
        assert_eq!(
            serde_json::to_value(BackendStatus::ReadOnly {
                stage: "asset_recovery".into(),
                message: "broken journal".into(),
            })
            .unwrap(),
            serde_json::json!({
                "state": "read_only",
                "stage": "asset_recovery",
                "message": "broken journal",
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn cross_process_mutation_lock_never_follows_a_symlinked_lock_directory() {
        use std::os::unix::fs::symlink;

        let home = crate::testenv::TestHome::new("gate-lock-symlink");
        let mux = home.home.join(".mux");
        let outside = home.home.join("outside-locks");
        std::fs::create_dir(&mux).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, mux.join("locks")).unwrap();

        let Err(error) = acquire_cross_process_mutation_lock() else {
            panic!("symlinked application lock directory was accepted");
        };

        assert!(error.contains("unsafe"), "{error}");
        assert!(!outside.join("application-mutation.lockfile").exists());
    }

    #[test]
    fn starting_and_read_only_have_structured_blockers_but_ready_does_not() {
        let starting = blocker_for_status(&BackendStatus::Starting).unwrap();
        assert_eq!(starting.code, "backend_initializing");
        assert_eq!(starting.details["backend_state"], "starting");

        let read_only = blocker_for_status(&BackendStatus::ReadOnly {
            stage: "skill_recovery".into(),
            message: "unsafe journal".into(),
        })
        .unwrap();
        assert_eq!(read_only.code, "recovery_required");
        assert_eq!(read_only.details["stage"], "skill_recovery");
        assert!(blocker_for_status(&BackendStatus::Ready).is_none());
    }

    #[test]
    fn every_supported_error_shape_identifies_recovery_required() {
        let core: CoreResult<()> = Err(CoreError::new("recovery_required", "core"));
        assert_eq!(core.recovery_required().as_deref(), Some("core"));

        let legacy: Result<(), String> = Err("recovery_required: legacy".into());
        assert_eq!(legacy.recovery_required().as_deref(), Some("legacy"));

        let skill: Result<(), SkillError> = Err(SkillError::RecoveryRequired {
            message: "skill".into(),
        });
        assert_eq!(skill.recovery_required().as_deref(), Some("skill"));
    }

    #[cfg(unix)]
    #[test]
    fn ordinary_skill_error_is_promoted_when_a_global_claim_appears() {
        let _home = crate::testenv::TestHome::new("gate-skill-claim");
        let _claim = crate::safe_write::install_test_global_mutation_claim().unwrap();
        let skill: Result<(), SkillError> = Err(SkillError::Conflict {
            message: "stale target".into(),
            path: String::new(),
        });

        assert!(skill.recovery_required().is_some());
    }

    #[test]
    fn recovery_latch_preserves_the_first_blocker() {
        let first = latched_status(
            &BackendStatus::Ready,
            "asset_commit",
            "rollback failed".into(),
        );
        assert_eq!(
            first,
            BackendStatus::ReadOnly {
                stage: "asset_commit".into(),
                message: "rollback failed".into(),
            }
        );
        assert_eq!(
            latched_status(&first, "later", "second failure".into()),
            first
        );
    }

    #[test]
    fn skill_mutation_contention_remains_retryable_not_recovery_required() {
        let blocked =
            <Result<(), SkillError> as GatedResult>::blocked(mutation_coordination_error());

        assert!(matches!(blocked, Err(SkillError::Conflict { .. })));
    }

    #[test]
    fn read_only_allows_queries_but_never_runs_preparations_or_mutations() {
        let _status = TestReadyGuard::enter_status(BackendStatus::ReadOnly {
            stage: "asset_recovery".into(),
            message: "broken journal".into(),
        });
        let query_called = Cell::new(false);
        query(|| query_called.set(true));
        assert!(query_called.get());

        let prepare_called = Cell::new(false);
        let prepare_result: Result<(), String> = prepare("asset_plan", || {
            prepare_called.set(true);
            Ok(())
        });
        assert!(!prepare_called.get());
        assert!(prepare_result
            .unwrap_err()
            .starts_with("recovery_required:"));

        let mutation_called = Cell::new(false);
        let mutation_result: CoreResult<()> = mutate_core("test_mutation", || {
            mutation_called.set(true);
            Ok(())
        });
        assert!(!mutation_called.get());
        assert_eq!(mutation_result.unwrap_err().code, "recovery_required");
    }

    #[test]
    fn runtime_recovery_error_latches_backend_read_only() {
        let _home = crate::testenv::TestHome::new("gate-runtime-recovery");
        let result: CoreResult<()> = mutate_core("asset_commit", || {
            Err(CoreError::new("recovery_required", "rollback failed"))
        });
        assert_eq!(result.unwrap_err().code, "recovery_required");
        assert_eq!(
            status(),
            BackendStatus::ReadOnly {
                stage: "asset_commit".into(),
                message: "rollback failed".into(),
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn pending_global_claim_blocks_the_operation_and_latches_read_only() {
        let _home = crate::testenv::TestHome::new("gate-global-claim");
        let _claim = crate::safe_write::install_test_global_mutation_claim().unwrap();
        let called = Cell::new(false);

        let result: CoreResult<()> = mutate_core("test_mutation", || {
            called.set(true);
            Ok(())
        });

        assert!(!called.get());
        assert_eq!(result.unwrap_err().code, "recovery_required");
        assert!(matches!(
            status(),
            BackendStatus::ReadOnly { ref stage, .. } if stage == "durable_recovery"
        ));
    }

    #[test]
    fn pending_asset_manifest_blocks_every_application_mutation() {
        let _home = crate::testenv::TestHome::new("gate-asset-recovery");
        let foreign_id = uuid::Uuid::new_v4().to_string();
        let rollback = crate::assets::planner::operation_root(&foreign_id).join("rollback");
        std::fs::create_dir_all(&rollback).unwrap();
        std::fs::write(rollback.join("manifest.json"), b"{}").unwrap();
        let called = Cell::new(false);

        let result: CoreResult<()> = mutate_core("unrelated_mutation", || {
            called.set(true);
            Ok(())
        });

        assert!(!called.get());
        assert_eq!(result.unwrap_err().code, "recovery_required");
        assert!(matches!(
            status(),
            BackendStatus::ReadOnly { ref stage, .. } if stage == "durable_recovery"
        ));
    }

    #[test]
    fn starting_blocks_mutations_without_running_them() {
        let _status = TestReadyGuard::enter_status(BackendStatus::Starting);
        let called = Cell::new(false);
        let result: CoreResult<()> = mutate_core("test_mutation", || {
            called.set(true);
            Ok(())
        });
        assert!(!called.get());
        assert_eq!(result.unwrap_err().code, "backend_initializing");
    }
}
