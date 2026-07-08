//! `ops::forget_entry` removes a manual entry from the catalog AND uninstalls it
//! from the agents that had it.
//!
//! One test per file — it mutates `$HOME` to isolate `~/.mux` (see the
//! integration-test `$HOME` race gotcha in CLAUDE.md).

use std::collections::HashMap;
use std::fs;

use mux_core::ops;
use mux_core::registry::{read_registry, write_manual_entry};
use mux_core::types::{RegistryConfig, RegistryEntry, StdioConfig};

#[test]
fn forget_removes_from_catalog_and_uninstalls() {
    let home = std::env::temp_dir().join(format!("mux-forget-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(home.join(".mux")).unwrap();
    std::env::set_var("HOME", &home);

    write_manual_entry(&RegistryEntry {
        name: "srv".into(),
        description: String::new(),
        tags: vec![],
        config: RegistryConfig {
            stdio: Some(StdioConfig { command: "npx".into(), args: None, env: None }),
            http: None,
        },
        origin: None,
        repo: None,
    })
    .unwrap();
    ops::install("srv", "stdio", "global", &["claude-code".into()], None, &HashMap::new()).unwrap();

    // Present in the catalog and installed in the agent.
    assert!(read_registry().iter().any(|e| e.name == "srv"));
    assert!(ops::scan_installed(None).iter().any(|i| i.agent == "claude-code" && i.name == "srv"));

    // Forget: gone from the catalog and uninstalled from the agent.
    ops::forget_entry("srv", "stdio").unwrap();
    assert!(!read_registry().iter().any(|e| e.name == "srv"), "removed from catalog");
    assert!(!ops::scan_installed(None).iter().any(|i| i.name == "srv"), "uninstalled from agents");

    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
