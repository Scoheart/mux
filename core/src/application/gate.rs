//! Process-wide consistency gate for frontend-facing workspace operations.
//!
//! Domain engines retain their own narrow locks for crash safety. This gate
//! coordinates threads in one process; the workspace snapshot additionally
//! uses a bounded, non-blocking two-lock protocol for the shared Skills and
//! settings filesystem locks to coordinate with cooperating MUX processes
//! without assuming every writer has the same lock order. External Agent files
//! remain outside those locks.

use std::sync::RwLock;

static WORKSPACE_GATE: RwLock<()> = RwLock::new(());

pub(crate) fn read<T>(operation: impl FnOnce() -> T) -> T {
    let _guard = WORKSPACE_GATE
        .read()
        .unwrap_or_else(|error| error.into_inner());
    operation()
}

pub(crate) fn write<T>(operation: impl FnOnce() -> T) -> T {
    let _guard = WORKSPACE_GATE
        .write()
        .unwrap_or_else(|error| error.into_inner());
    operation()
}
