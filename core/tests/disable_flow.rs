//! Disable/enable is a destructive-looking workflow, so exercise its durability
//! contract end to end against a real global Agent file.

use mux_core::disabled::{load_disabled, remember, DisabledEntry};
use mux_core::ops;
use mux_core::testenv::TestHome;
use mux_core::types::{McpConfig, StdioConfig};
use serde_json::Value;
use std::collections::HashMap;

fn claude_config() -> &'static str {
    r#"{
  "account": {"token": "private", "theme": "dark"},
  "mcpServers": {
    "srv": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "srv"],
      "env": {"TOKEN": "secret"},
      "enabled": false,
      "timeout": 120,
      "oauth": {"clientId": "client"},
      "allowedTools": ["read"]
    },
    "sibling": {"command": "keep", "approval": "always"}
  }
}
"#
}

#[test]
fn disable_enable_restores_complete_target_entry() {
    let th = TestHome::new("disable-restore");
    let path = th.home.join(".claude.json");
    std::fs::write(&path, claude_config()).unwrap();
    let before: Value = serde_json::from_str(claude_config()).unwrap();

    ops::disable("srv", "stdio", "global", &["claude-code".into()], None).unwrap();

    let disabled_file: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(disabled_file["mcpServers"].get("srv").is_none());
    assert_eq!(disabled_file["account"], before["account"]);
    assert_eq!(
        disabled_file["mcpServers"]["sibling"],
        before["mcpServers"]["sibling"]
    );
    let snapshot = load_disabled()["claude-code"][0]
        .snapshot
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(snapshot, before["mcpServers"]["srv"]);

    ops::enable("srv", "stdio", "global", &["claude-code".into()], None).unwrap();

    let restored: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(restored, before);
    assert!(!load_disabled().contains_key("claude-code"));
}

#[test]
fn disable_never_removes_entry_when_snapshot_cannot_be_saved() {
    let th = TestHome::new("disable-save-failure");
    let agent_path = th.home.join(".claude.json");
    std::fs::write(&agent_path, claude_config()).unwrap();
    let settings_path = th.home.join(".mux/settings.json");
    std::fs::create_dir_all(&settings_path).unwrap();

    let result = ops::disable("srv", "stdio", "global", &["claude-code".into()], None);

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(agent_path).unwrap(),
        claude_config()
    );
}

#[test]
fn unavailable_or_unknown_agents_fail_without_losing_snapshots() {
    let _home = TestHome::new("unavailable-agent");
    let config = McpConfig::Stdio(StdioConfig {
        command: "npx".into(),
        args: Some(vec!["-y".into(), "srv".into()]),
        env: None,
        cwd: None,
    });
    mux_core::registry::write_manual_entry(&mux_core::types::RegistryEntry {
        name: "srv".into(),
        description: String::new(),
        tags: Vec::new(),
        config: config.clone().into(),
        origin: None,
        repo: None,
    })
    .unwrap();

    let unavailable = ops::install(
        "srv",
        "stdio",
        "global",
        &["devin".into()],
        None,
        &HashMap::new(),
    )
    .unwrap_err();
    assert!(unavailable[0].contains("transport is not supported"));

    let unknown = ops::install(
        "srv",
        "stdio",
        "global",
        &["missing-agent".into()],
        None,
        &HashMap::new(),
    )
    .unwrap_err();
    assert!(unknown[0].contains("unknown Agent"));

    let saved = DisabledEntry {
        name: "srv".into(),
        transport: "stdio".into(),
        scope: "global".into(),
        config,
        snapshot: Some(serde_json::json!({"command": "npx"})),
    };
    remember("devin", saved.clone()).unwrap();
    let delete_error = ops::delete("srv", "stdio", "global", &["devin".into()], None).unwrap_err();
    assert!(delete_error[0].contains("snapshot retained"));
    assert_eq!(load_disabled()["devin"], vec![saved]);

    let clean = ops::clean(Some("missing-agent"));
    assert!(clean.cleaned.is_empty());
    assert!(clean.errors[0].contains("unknown Agent"));
}

#[test]
fn vt_code_enable_keeps_snapshot_when_root_mcp_switch_was_disabled() {
    let th = TestHome::new("vt-code-disabled-before-restore");
    let directory = th.home.join(".vtcode");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("vtcode.toml");
    std::fs::write(
        &path,
        r#"[mcp]
enabled = true

[[mcp.providers]]
name = "srv"
enabled = true
command = "npx"
args = ["-y", "srv"]
"#,
    )
    .unwrap();

    ops::disable("srv", "stdio", "global", &["vt-code".into()], None).unwrap();
    assert!(load_disabled().contains_key("vt-code"));

    let disabled_root = "[mcp]\nenabled = false\n";
    std::fs::write(&path, disabled_root).unwrap();
    let errors = ops::enable("srv", "stdio", "global", &["vt-code".into()], None).unwrap_err();

    assert!(errors.iter().any(|error| error.contains("mcp.enabled")));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disabled_root);
    assert!(load_disabled().contains_key("vt-code"));
}
