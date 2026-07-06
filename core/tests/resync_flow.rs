//! `ops::resync_entry` end-to-end: it re-stamps a clean install, skips a
//! hand-customized one unless forced, and force overwrites it.
//!
//! One test per file — it mutates `$HOME` to isolate `~/.mux` (see the
//! integration-test `$HOME` race gotcha in CLAUDE.md).

use std::collections::HashMap;
use std::fs;

use mux_core::ops;
use mux_core::registry::write_manual_entry;
use mux_core::types::{RegistryConfig, RegistryEntry, StdioConfig};

fn stdio_entry(args: &[&str]) -> RegistryEntry {
    RegistryEntry {
        name: "srv".into(),
        description: String::new(),
        tags: vec![],
        config: RegistryConfig {
            stdio: Some(StdioConfig {
                command: "npx".into(),
                args: Some(args.iter().map(|s| s.to_string()).collect()),
                env: None,
            }),
            http: None,
        },
        origin: None,
    }
}

fn is_customized(agent: &str) -> bool {
    ops::scan_installed(None)
        .iter()
        .find(|i| i.agent == agent && i.name == "srv")
        .map(|i| i.customized)
        .expect("srv should be installed for the agent")
}

#[test]
fn resync_pushes_clean_skips_then_forces_customized() {
    let home = std::env::temp_dir().join(format!("mux-resync-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(home.join(".mux")).unwrap();
    std::env::set_var("HOME", &home);

    // Seed a manual entry (v1) and install it to a builtin global agent.
    write_manual_entry(&stdio_entry(&["-y", "srv-a"])).unwrap();
    ops::install("srv", "stdio", "global", &["claude-code".into()], None, &HashMap::new()).unwrap();
    assert!(!is_customized("claude-code"), "fresh install is clean");

    // Edit the entry directly (v2) — this bypasses upsert_entry's auto-propagation,
    // so the on-disk config stays v1 while the registry says v2 → "customized".
    write_manual_entry(&stdio_entry(&["-y", "srv-b"])).unwrap();
    assert!(is_customized("claude-code"), "on-disk now drifts from registry");

    // Safe resync skips the customized install and reports it.
    let out = ops::resync_entry("srv", "stdio", false).unwrap();
    assert!(out.synced.is_empty(), "nothing synced when customized + !force");
    assert_eq!(out.skipped_customized, vec!["claude-code".to_string()]);
    assert!(is_customized("claude-code"), "still stale after a safe resync");

    // Forced resync overwrites it → on-disk matches registry again (clean).
    let out = ops::resync_entry("srv", "stdio", true).unwrap();
    assert_eq!(out.synced, vec!["claude-code".to_string()]);
    assert!(out.skipped_customized.is_empty());
    assert!(!is_customized("claude-code"), "forced resync re-stamped v2");

    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
