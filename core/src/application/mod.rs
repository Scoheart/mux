//! Frontend-facing use cases.
//!
//! CLI, Tauri, and future frontends should depend on this layer instead of
//! composing storage, recovery, adapters, or domain engines themselves.

pub mod agents;
pub mod assets;
pub mod bootstrap;
mod error;
mod gate;
pub mod mcp;
pub mod models;
pub mod network;
pub mod operations;
pub mod skills;
pub mod ui;
pub mod update;
pub mod workspace;

/// Stable service facade for non-Rust consumers and future frontends.
pub struct MuxCore;

impl MuxCore {
    pub fn bootstrap(
        frontend: bootstrap::Frontend,
    ) -> Result<bootstrap::BootstrapReport, bootstrap::BootstrapError> {
        bootstrap::bootstrap(frontend)
    }

    pub fn snapshot() -> crate::domain::error::CoreResult<workspace::WorkspaceSnapshot> {
        workspace::snapshot()
    }

    pub fn plan(
        request: operations::PlanOperationRequest,
    ) -> crate::domain::error::CoreResult<operations::OperationPlan> {
        operations::plan(request)
    }

    pub fn commit(
        request: operations::CommitOperationRequest,
    ) -> crate::domain::error::CoreResult<operations::OperationCommitResult> {
        operations::commit(request)
    }

    pub fn cancel(
        request: operations::CancelOperationRequest,
    ) -> crate::domain::error::CoreResult<()> {
        operations::cancel(request)
    }
}
