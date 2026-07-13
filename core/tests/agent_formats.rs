use mux_core::adapter::get_agent_adapter;
use mux_core::agents::builtin_agents;
use mux_core::codec::normalize_for_agent;
use mux_core::types::{HttpConfig, McpConfig, StdioConfig};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

fn temp_file(name: &str, extension: &str) -> PathBuf {
    let token = format!(
        "{}-{}",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    );
    std::env::temp_dir().join(format!("mux-agent-format-{name}-{token}.{extension}"))
}

fn fixture(name: &str) -> &'static str {
    match name {
        "opencode" => include_str!("fixtures/opencode.json"),
        "codex" => include_str!("fixtures/codex.toml"),
        "gemini" => include_str!("fixtures/gemini.json"),
        "windsurf" => include_str!("fixtures/windsurf.json"),
        "cline" => include_str!("fixtures/cline.json"),
        _ => panic!("unknown fixture"),
    }
}

fn write_fixture(name: &str, extension: &str) -> PathBuf {
    let path = temp_file(name, extension);
    std::fs::write(&path, fixture(name)).unwrap();
    path
}

fn http(url: &str) -> McpConfig {
    McpConfig::Http(HttpConfig {
        kind: "streamable-http".into(),
        url: url.into(),
        headers: Some(HashMap::from([("X-New".into(), "value".into())])),
    })
}

#[test]
fn scans_official_opencode_gemini_and_windsurf_shapes() {
    let open_path = write_fixture("opencode", "json");
    let open = get_agent_adapter("json", "mcp", "opencode").read(&open_path);
    assert!(matches!(open["local-tools"], McpConfig::Stdio(_)));
    assert!(matches!(open["remote-tools"], McpConfig::Http(_)));

    let gemini_path = write_fixture("gemini", "json");
    let gemini = get_agent_adapter("json", "mcpServers", "gemini").read(&gemini_path);
    assert_eq!(
        match &gemini["docs"] {
            McpConfig::Http(config) => config.kind.as_str(),
            _ => panic!("expected HTTP"),
        },
        "streamable-http"
    );
    assert_eq!(
        match &gemini["legacy"] {
            McpConfig::Http(config) => config.kind.as_str(),
            _ => panic!("expected SSE"),
        },
        "sse"
    );

    let windsurf_path = write_fixture("windsurf", "json");
    let windsurf = get_agent_adapter("json", "mcpServers", "windsurf").read(&windsurf_path);
    assert!(matches!(windsurf["docs"], McpConfig::Http(_)));

    for path in [open_path, gemini_path, windsurf_path] {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn opencode_update_preserves_private_settings_and_entry_policy() {
    let path = write_fixture("opencode", "json");
    let adapter = get_agent_adapter("json", "mcp", "opencode");
    adapter
        .upsert(
            &path,
            "local-tools",
            &McpConfig::Stdio(StdioConfig {
                command: "bun".into(),
                args: Some(vec!["x".into(), "new-server".into()]),
                env: Some(HashMap::from([("NEW_TOKEN".into(), "new-value".into())])),
                cwd: Some("/new/worktree".into()),
            }),
        )
        .unwrap();

    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(root["model"], "provider/private-model");
    let target = &root["mcp"]["local-tools"];
    assert_eq!(target["type"], "local");
    assert_eq!(
        target["command"],
        serde_json::json!(["bun", "x", "new-server"])
    );
    assert_eq!(target["environment"]["NEW_TOKEN"], "new-value");
    assert_eq!(target["enabled"], false);
    assert_eq!(target["timeout"], 9000);
    assert_eq!(root["mcp"]["remote-tools"]["oauth"], false);
    let _ = std::fs::remove_file(path);
}

#[test]
fn codex_update_preserves_tool_policy_and_uses_http_headers() {
    let path = write_fixture("codex", "toml");
    let adapter = get_agent_adapter("toml", "mcp_servers", "codex");
    adapter
        .upsert(&path, "figma", &http("https://new.example.com/mcp"))
        .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    let root: toml::Value = written.parse().unwrap();
    let target = &root["mcp_servers"]["figma"];
    assert_eq!(root["model"].as_str(), Some("gpt-private"));
    assert_eq!(root["history"]["persistence"].as_str(), Some("save-all"));
    assert_eq!(target["url"].as_str(), Some("https://new.example.com/mcp"));
    assert_eq!(target["http_headers"]["X-New"].as_str(), Some("value"));
    assert_eq!(target["bearer_token_env_var"].as_str(), Some("FIGMA_TOKEN"));
    assert_eq!(
        target["tools"]["open"]["approval_mode"].as_str(),
        Some("approve")
    );
    assert!(
        written.contains("enabled_tools = [\"open\", \"screenshot\"] # keep policy and comment")
    );
    assert!(!target.as_table().unwrap().contains_key("type"));
    assert!(!target.as_table().unwrap().contains_key("headers"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn copilot_defaults_tools_only_for_new_entries() {
    let path = temp_file("copilot", "json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"existing":{"type":"http","url":"https://old","tools":["read"]}}}"#,
    )
    .unwrap();
    let adapter = get_agent_adapter("json", "mcpServers", "copilot-cli");
    adapter
        .upsert(&path, "existing", &http("https://new"))
        .unwrap();
    adapter.upsert(&path, "new", &http("https://new")).unwrap();

    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        root["mcpServers"]["existing"]["tools"],
        serde_json::json!(["read"])
    );
    assert_eq!(root["mcpServers"]["new"]["tools"], serde_json::json!(["*"]));
    let _ = std::fs::remove_file(path);
}

#[test]
fn cline_update_uses_nested_transport_and_preserves_registration_state() {
    let path = write_fixture("cline", "json");
    let adapter = get_agent_adapter("json", "mcpServers", "cline");
    let scanned = adapter.read(&path);
    assert!(matches!(scanned["shared-local"], McpConfig::Stdio(_)));
    assert!(matches!(scanned["shared-http"], McpConfig::Http(_)));

    adapter
        .upsert(
            &path,
            "shared-local",
            &McpConfig::Stdio(StdioConfig {
                command: "bunx".into(),
                args: Some(vec!["new-server".into()]),
                env: None,
                cwd: Some("/new/worktree".into()),
            }),
        )
        .unwrap();

    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let target = &root["mcpServers"]["shared-local"];
    assert_eq!(root["account"]["profile"], "private");
    assert_eq!(target["transport"]["type"], "stdio");
    assert_eq!(target["transport"]["command"], "bunx");
    assert_eq!(target["transport"]["cwd"], "/new/worktree");
    assert_eq!(target["disabled"], true);
    assert_eq!(target["metadata"]["owner"], "user");
    assert_eq!(target["oauth"]["tokens"]["access_token"], "fixture-token");
    assert!(target.get("command").is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn target_entry_with_non_object_value_is_never_replaced() {
    let path = temp_file("bad-target", "json");
    let original = r#"{"private":{"token":"secret"},"mcp":{"docs":"managed elsewhere"}}"#;
    std::fs::write(&path, original).unwrap();
    let result =
        get_agent_adapter("json", "mcp", "opencode").upsert(&path, "docs", &http("https://new"));
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let _ = std::fs::remove_file(path);
}

#[test]
fn every_writable_builtin_roundtrips_through_its_wire_format() {
    let agents = builtin_agents();
    let writable = agents
        .values()
        .filter(|agent| agent.global.is_some())
        .count();
    assert_eq!(writable, 18);

    for (agent_id, definition) in agents {
        if definition.global.is_none() {
            continue;
        }
        let path = temp_file(
            &format!("matrix-{agent_id}"),
            if definition.format == "toml" {
                "toml"
            } else {
                "json"
            },
        );
        let adapter = get_agent_adapter(&definition.format, &definition.key, &agent_id);
        let local = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "matrix-server".into()]),
            env: Some(HashMap::from([("TOKEN".into(), "fixture".into())])),
            cwd: Some("/tmp/mux-matrix".into()),
        });
        let remote = http("https://example.com/mcp");

        adapter
            .upsert(&path, "local", &local)
            .unwrap_or_else(|error| panic!("{agent_id} failed to write stdio config: {error}"));
        let remote_result = adapter.upsert(&path, "remote", &remote);
        if agent_id == "claude-desktop" {
            assert!(
                remote_result.is_err(),
                "Claude Desktop must reject remote MCPs"
            );
        } else {
            remote_result
                .unwrap_or_else(|error| panic!("{agent_id} failed to write HTTP config: {error}"));
        }
        let scanned = adapter.read(&path);
        assert_eq!(
            scanned["local"],
            normalize_for_agent(&agent_id, &local),
            "{agent_id}"
        );
        if agent_id == "claude-desktop" {
            assert!(!scanned.contains_key("remote"));
        } else {
            assert_eq!(
                scanned["remote"],
                normalize_for_agent(&agent_id, &remote),
                "{agent_id}"
            );
        }
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn standard_agent_update_preserves_target_policy_fields() {
    let path = temp_file("kiro-policy", "json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"docs":{"url":"https://old","disabled":true,"autoApprove":["read"],"oauth":{"clientId":"private"}}}}"#,
    )
    .unwrap();
    get_agent_adapter("json", "mcpServers", "kiro")
        .upsert(&path, "docs", &http("https://new"))
        .unwrap();

    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let target = &root["mcpServers"]["docs"];
    assert_eq!(target["url"], "https://new");
    assert_eq!(target["disabled"], true);
    assert_eq!(target["autoApprove"], serde_json::json!(["read"]));
    assert_eq!(target["oauth"]["clientId"], "private");
    let _ = std::fs::remove_file(path);
}

#[test]
fn warp_remote_uses_url_inference_and_preserves_environment() {
    let path = temp_file("warp-remote", "json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"docs":{"type":"streamable-http","url":"https://old","env":{"API_KEY":"${API_KEY}"}}}}"#,
    )
    .unwrap();

    get_agent_adapter("json", "mcpServers", "warp")
        .upsert(&path, "docs", &http("https://new"))
        .unwrap();

    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let target = &root["mcpServers"]["docs"];
    assert_eq!(target["url"], "https://new");
    assert_eq!(target["env"]["API_KEY"], "${API_KEY}");
    assert!(target.get("type").is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn builtin_global_paths_match_current_product_docs() {
    let agents = builtin_agents();
    let expected = [
        ("claude-code", "~/.claude.json"),
        (
            "claude-desktop",
            "~/Library/Application Support/Claude/claude_desktop_config.json",
        ),
        ("cursor", "~/.cursor/mcp.json"),
        (
            "vscode",
            "~/Library/Application Support/Code/User/mcp.json",
        ),
        ("codex", "~/.codex/config.toml"),
        ("zed", "~/.config/zed/settings.json"),
        ("windsurf", "~/.codeium/windsurf/mcp_config.json"),
        (
            "roo-code",
            "~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json",
        ),
        ("gemini", "~/.gemini/settings.json"),
        ("qoder", "~/.qoder/settings.json"),
        ("kiro", "~/.kiro/settings/mcp.json"),
        ("junie", "~/.junie/mcp/mcp.json"),
        ("amazon-q", "~/.aws/amazonq/default.json"),
        ("opencode", "~/.config/opencode/opencode.json"),
        ("copilot-cli", "~/.copilot/mcp-config.json"),
        ("cline", "~/.cline/data/settings/cline_mcp_settings.json"),
        ("warp", "~/.warp/.mcp.json"),
        ("pi", "~/.pi/agent/mcp.json"),
    ];

    for (agent_id, path) in expected {
        assert_eq!(agents[agent_id].global.as_deref(), Some(path), "{agent_id}");
    }
    for agent_id in ["continue", "devin", "qoderwork"] {
        assert!(agents[agent_id].global.is_none(), "{agent_id}");
    }
}

#[test]
fn every_builtin_writes_the_documented_wire_shape() {
    let agents = builtin_agents();
    let local = McpConfig::Stdio(StdioConfig {
        command: "npx".into(),
        args: Some(vec!["-y".into(), "matrix-server".into()]),
        env: Some(HashMap::from([("TOKEN".into(), "fixture".into())])),
        cwd: Some("/tmp/mux-matrix".into()),
    });
    let remote = http("https://example.com/mcp");

    for (agent_id, definition) in agents {
        if definition.global.is_none() {
            continue;
        }
        let path = temp_file(
            &format!("wire-{agent_id}"),
            if definition.format == "toml" {
                "toml"
            } else {
                "json"
            },
        );
        let adapter = get_agent_adapter(&definition.format, &definition.key, &agent_id);
        adapter.upsert(&path, "local", &local).unwrap();
        if agent_id != "claude-desktop" {
            adapter.upsert(&path, "remote", &remote).unwrap();
        }

        if agent_id == "codex" {
            let root: toml::Value = std::fs::read_to_string(&path).unwrap().parse().unwrap();
            let local = &root["mcp_servers"]["local"];
            let remote = &root["mcp_servers"]["remote"];
            assert_eq!(local["command"].as_str(), Some("npx"));
            assert_eq!(local["cwd"].as_str(), Some("/tmp/mux-matrix"));
            assert!(local.get("type").is_none());
            assert_eq!(remote["url"].as_str(), Some("https://example.com/mcp"));
            assert!(remote.get("http_headers").is_some());
            assert!(remote.get("headers").is_none());
            let _ = std::fs::remove_file(path);
            continue;
        }

        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let local = &root[&definition.key]["local"];
        let remote = &root[&definition.key]["remote"];

        match agent_id.as_str() {
            "opencode" => {
                assert_eq!(local["type"], "local");
                assert_eq!(local["command"][0], "npx");
                assert!(local.get("args").is_none());
                assert_eq!(local["environment"]["TOKEN"], "fixture");
                assert_eq!(remote["type"], "remote");
                assert!(remote.get("url").is_some());
            }
            "cline" => {
                assert_eq!(local["transport"]["type"], "stdio");
                assert_eq!(local["transport"]["command"], "npx");
                assert_eq!(remote["transport"]["type"], "streamableHttp");
                assert!(remote["transport"].get("url").is_some());
                assert!(local.get("command").is_none());
                assert!(remote.get("url").is_none());
            }
            "gemini" => {
                assert_eq!(local["command"], "npx");
                assert!(remote.get("httpUrl").is_some());
                assert!(remote.get("url").is_none());
                assert!(remote.get("type").is_none());
            }
            "windsurf" => {
                assert_eq!(local["command"], "npx");
                assert!(remote.get("serverUrl").is_some());
                assert!(remote.get("url").is_none());
                assert!(remote.get("type").is_none());
            }
            "warp" => {
                assert_eq!(local["working_directory"], "/tmp/mux-matrix");
                assert!(local.get("cwd").is_none());
                assert!(remote.get("url").is_some());
                assert!(remote.get("type").is_none());
            }
            "copilot-cli" => {
                assert_eq!(local["type"], "local");
                assert_eq!(remote["type"], "http");
                assert_eq!(local["tools"], serde_json::json!(["*"]));
                assert_eq!(remote["tools"], serde_json::json!(["*"]));
            }
            "claude-code" | "amazon-q" | "vscode" | "qoder" => {
                assert_eq!(local["type"], "stdio", "{agent_id}");
                assert_eq!(remote["type"], "http", "{agent_id}");
            }
            "roo-code" => {
                assert!(local.get("type").is_none());
                assert_eq!(remote["type"], "streamable-http");
            }
            "claude-desktop" => {
                assert_eq!(local["command"], "npx");
                assert!(local.get("type").is_none());
                assert!(remote.is_null());
            }
            "cursor" | "zed" | "kiro" | "junie" | "pi" => {
                assert_eq!(local["command"], "npx", "{agent_id}");
                assert!(local.get("type").is_none(), "{agent_id}");
                assert!(remote.get("url").is_some(), "{agent_id}");
                assert!(remote.get("type").is_none(), "{agent_id}");
            }
            _ => panic!("missing wire-format assertion for {agent_id}"),
        }
        let _ = std::fs::remove_file(path);
    }
}
