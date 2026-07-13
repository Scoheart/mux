//! Test-only environment isolation for `~/.mux` and agent-config paths.
//!
//! Historically every integration test hand-rolled `set_var("HOME", tmp)` +
//! `remove_var("HOME")`. That pattern had two real failure modes:
//!
//! 1. **`remove_var` is not a restore** — with `HOME` unset, `dirs::home_dir()`
//!    falls back to the passwd entry (the *real* home), so any core call after
//!    teardown silently touches real user data.
//! 2. **Parallel tests race** — env vars are process-global; two tests in one
//!    binary interleave `set_var`/`remove_var` and one of them ends up writing
//!    through the other's (or the real) HOME. This corrupted the real
//!    `~/.mux/sources/remote/*` cache on 2026-07-08.
//!
//! [`TestHome`] fixes both: a process-wide mutex serializes every user of the
//! env, `HOME` *and* `MUX_HOME` point into a fresh temp dir, and `Drop`
//! restores (not removes) the previous values. Multiple tests per file are
//! safe with this guard.
//!
//! Not intended for production use; hidden from docs.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static SEQ: AtomicU32 = AtomicU32::new(0);

/// RAII guard: fake `$HOME` + `$MUX_HOME` in a temp dir, serialized process-wide.
pub struct TestHome {
    /// The fake home directory (agent configs live under here, `.mux` inside it).
    pub home: PathBuf,
    saved_home: Option<OsString>,
    saved_mux_home: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl TestHome {
    /// Acquire the env lock, create a unique temp home, and point both
    /// `HOME` and `MUX_HOME` (→ `<home>/.mux`) at it.
    pub fn new(tag: &str) -> Self {
        let guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let home =
            std::env::temp_dir().join(format!("mux-test-{}-{}-{}", tag, std::process::id(), seq));
        std::fs::create_dir_all(&home).expect("create test home dir");
        let saved_home = std::env::var_os("HOME");
        let saved_mux_home = std::env::var_os("MUX_HOME");
        std::env::set_var("HOME", &home);
        std::env::set_var("MUX_HOME", home.join(".mux"));
        TestHome {
            home,
            saved_home,
            saved_mux_home,
            _guard: guard,
        }
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        // Restore, never remove-and-fall-back-to-real-home.
        match &self.saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match &self.saved_mux_home {
            Some(v) => std::env::set_var("MUX_HOME", v),
            None => std::env::remove_var("MUX_HOME"),
        }
        let _ = std::fs::remove_dir_all(&self.home);
        // _guard releases after this body — env is consistent before the next
        // holder proceeds.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirects_and_restores_env() {
        // Capture the original values from inside TestHome, after it owns the
        // process-wide lock. Reading before acquisition races another test's
        // temporary HOME even though both mutations themselves are serialized.
        let th = TestHome::new("selftest");
        let before_home = th.saved_home.clone();
        let before_mux = th.saved_mux_home.clone();
        assert_eq!(std::env::var_os("HOME"), Some(th.home.clone().into()));
        assert_eq!(
            crate::paths::mux_dir(),
            th.home.join(".mux"),
            "mux_dir must resolve into the fake home"
        );
        drop(th);
        assert_eq!(std::env::var_os("HOME"), before_home, "HOME restored");
        assert_eq!(
            std::env::var_os("MUX_HOME"),
            before_mux,
            "MUX_HOME restored"
        );
    }
}
