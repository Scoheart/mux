// Manual + discovered entries are stored as managed *local sources* (files under
// ~/.mux/sources/local/), NOT in settings.json's registry. This verifies that.
use std::fs;

use desktop_lib::commands::{list_registry, list_sources, upsert_registry_entry};
use mux_core::types::{RegistryConfig, RegistryEntry, StdioConfig};

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mux-manual-{}-{}", tag, std::process::id()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn entry(name: &str, cmd: &str) -> RegistryEntry {
    RegistryEntry {
        name: name.into(),
        description: "d".into(),
        tags: vec!["t".into()],
        config: RegistryConfig {
            stdio: Some(StdioConfig { command: cmd.into(), args: None, env: None }),
            http: None,
        },
        origin: None,
    }
}

#[test]
fn manual_entry_is_stored_as_a_managed_local_source_file() {
    let home = unique_dir("home");
    std::env::set_var("HOME", &home);

    upsert_registry_entry(entry("my-tool", "my-cmd")).expect("create manual entry");

    // It shows in the catalog, tagged manual.
    let cat = list_registry();
    let e = cat.iter().find(|e| e.name == "my-tool").expect("my-tool in catalog");
    assert_eq!(e.origin.as_ref().unwrap().kind, "manual");

    // It is persisted as a file under ~/.mux/sources/local/manual.json …
    let manual_file = home.join(".mux").join("sources").join("local").join("manual.json");
    assert!(manual_file.exists(), "manual source file should exist at {:?}", manual_file);

    // … and NOT in settings.json's registry section.
    let settings = fs::read_to_string(home.join(".mux").join("settings.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&settings).unwrap();
    assert!(v.get("registry").is_none() || v["registry"].as_array().map(|a| a.is_empty()).unwrap_or(true),
        "manual entries must not live in settings.registry");

    // The managed "manual" source appears in list_sources, flagged managed.
    let manual_src = list_sources().into_iter().find(|s| s.id == "manual").expect("manual source listed");
    assert_eq!(manual_src.kind, "local");
    assert!(manual_src.managed);
    assert_eq!(manual_src.server_count, 1);

    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
