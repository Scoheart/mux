// Renaming a manual entry (as the edit page does: write the new name, delete the
// old) leaves exactly the renamed entry in the manual source — no duplicate.
use desktop_lib::commands::{delete_registry_entry, list_registry, upsert_registry_entry};
use mux_core::types::{RegistryConfig, RegistryEntry, StdioConfig};

fn entry(name: &str) -> RegistryEntry {
    RegistryEntry {
        name: name.into(),
        description: "d".into(),
        tags: vec![],
        config: RegistryConfig {
            stdio: Some(StdioConfig { command: "uvx".into(), args: Some(vec!["mcp-jenkins".into()]), env: None }),
            http: None,
        },
        origin: None,
        repo: None,
    }
}

#[test]
fn renaming_a_manual_entry_replaces_it_without_duplicate() {
    let _th = mux_core::testenv::TestHome::new("rename");

    // Create a manual entry.
    upsert_registry_entry(entry("my-mcp-server")).unwrap();
    assert!(list_registry().iter().any(|e| e.name == "my-mcp-server"));

    // Rename: write the new name, then remove the old (same transport).
    upsert_registry_entry(entry("jenkins")).unwrap();
    delete_registry_entry("my-mcp-server".into(), "stdio".into()).unwrap();

    let cat = list_registry();
    assert!(cat.iter().any(|e| e.name == "jenkins"), "renamed entry present");
    assert!(!cat.iter().any(|e| e.name == "my-mcp-server"), "old name gone");
    assert_eq!(cat.iter().filter(|e| e.name == "jenkins").count(), 1, "no duplicate");
}
