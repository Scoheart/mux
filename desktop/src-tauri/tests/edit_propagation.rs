// Editing a registry entry must propagate the new config to agents that
// installed it "clean", while leaving hand-customized installs untouched.
use std::collections::HashMap;
use std::fs;

use serde_json::Value;

use desktop_lib::commands::{add_agent, apply_install, upsert_registry_entry, InstallRequest};
use mux_core::types::{AgentDefinition, RegistryConfig, RegistryEntry, StdioConfig};

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mux-edit-{}-{}", tag, std::process::id()));
    fs::create_dir_all(&d).unwrap();
    d
}

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
            }),
            http: None,
        },
        origin: None,
        repo: None,
    }
}

#[test]
fn editing_registry_propagates_to_clean_installs_but_not_customized() {
    let home = unique_dir("home");
    std::env::set_var("HOME", &home);

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

    // Seed + install git (a clean install).
    upsert_registry_entry(git_entry(&["-y", "git-mcp"])).unwrap();
    apply_install(InstallRequest {
        server_name: "git".into(),
        transport: "stdio".into(),
        scope: "global".into(),
        agents: vec!["myagent".into()],
        project_dir: None,
        overrides: HashMap::new(),
    })
    .unwrap();
    assert_eq!(read_args(), vec!["-y", "git-mcp"]);

    // EDIT the registry entry → the clean install should now follow it. (THE FIX)
    upsert_registry_entry(git_entry(&["-y", "git-mcp", "--verbose"])).unwrap();
    assert_eq!(
        read_args(),
        vec!["-y", "git-mcp", "--verbose"],
        "a clean install should update when the registry entry is edited"
    );

    // Hand-customize the agent's copy, then edit the registry again →
    // the customized install must be preserved (not clobbered).
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&agent_file).unwrap()).unwrap();
    v["mcpServers"]["git"]["args"] = serde_json::json!(["custom-arg"]);
    fs::write(&agent_file, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    upsert_registry_entry(git_entry(&["-y", "final"])).unwrap();
    assert_eq!(
        read_args(),
        vec!["custom-arg"],
        "a hand-customized install must NOT be overwritten by a registry edit"
    );

    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
