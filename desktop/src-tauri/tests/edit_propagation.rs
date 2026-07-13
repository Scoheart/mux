// Editing a registry entry must auto-sync the new config to every agent that
// has it installed — clean AND hand-customized/drifted copies alike (each write
// is backed up first). This replaced the old conservative "clean installs only"
// propagation, which left drifted installs permanently stale and forced a
// manual 重新同步 after every edit.
use std::collections::HashMap;
use std::fs;

use serde_json::Value;

use desktop_lib::commands::{add_agent, apply_install, upsert_registry_entry, InstallRequest};
use mux_core::types::{AgentDefinition, RegistryConfig, RegistryEntry, StdioConfig};

fn git_entry(args: &[&str]) -> RegistryEntry {
    RegistryEntry {
        name: "git".into(),
        description: String::new(),
        tags: vec![],
        config: RegistryConfig {
            stdio: Some(StdioConfig {
                command: "npx".into(),
                args: Some(args.iter().map(|s| s.to_string()).collect()),
                env: None,
                cwd: None,
            }),
            http: None,
        },
        origin: None,
        repo: None,
    }
}

#[test]
fn editing_registry_auto_syncs_all_installs_including_customized() {
    let th = mux_core::testenv::TestHome::new("edit-prop");
    let home = th.home.clone();

    // An agent with a known global config file under HOME.
    add_agent(
        "myagent".into(),
        AgentDefinition {
            global: Some("~/myagent.json".into()),
            project: None,
            format: "json".into(),
            key: "mcpServers".into(),
            enabled: true,
            builtin: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    let agent_file = home.join("myagent.json");

    let read_args = || -> Vec<String> {
        let v: Value = serde_json::from_str(&fs::read_to_string(&agent_file).unwrap()).unwrap();
        v["mcpServers"]["git"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    };

    // Seed + install git. A brand-new entry has no installs → nothing synced.
    let synced = upsert_registry_entry(git_entry(&["-y", "git-mcp"])).unwrap();
    assert!(synced.is_empty(), "brand-new entry has nothing to sync");
    apply_install(InstallRequest {
        server_name: "git".into(),
        transport: "stdio".into(),
        agents: vec!["myagent".into()],
        overrides: HashMap::new(),
    })
    .unwrap();
    assert_eq!(read_args(), vec!["-y", "git-mcp"]);

    // EDIT the registry entry → the clean install follows it, and the save
    // reports which agents were synced.
    let synced = upsert_registry_entry(git_entry(&["-y", "git-mcp", "--verbose"])).unwrap();
    assert_eq!(synced, vec!["myagent".to_string()]);
    assert_eq!(
        read_args(),
        vec!["-y", "git-mcp", "--verbose"],
        "a clean install should update when the registry entry is edited"
    );

    // Hand-customize the agent's copy, then edit the registry again → saving
    // auto-syncs the customized copy too (the old conservative behavior left it
    // permanently stale; the file write is backed up first).
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&agent_file).unwrap()).unwrap();
    v["mcpServers"]["git"]["args"] = serde_json::json!(["custom-arg"]);
    fs::write(&agent_file, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let synced = upsert_registry_entry(git_entry(&["-y", "final"])).unwrap();
    assert_eq!(synced, vec!["myagent".to_string()]);
    assert_eq!(
        read_args(),
        vec!["-y", "final"],
        "a hand-customized install is auto-synced on save too"
    );

    // Description/tags-only edit (same config) → no sync, no rewrite.
    let synced = upsert_registry_entry(git_entry(&["-y", "final"])).unwrap();
    assert!(synced.is_empty(), "config-unchanged save syncs nothing");
}
