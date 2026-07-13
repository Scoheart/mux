use mux_core::adapter::{get_agent_adapter, get_agent_adapter_for};
use mux_core::agents::builtin_agents;
use mux_core::codec::{from_name, normalize_with_codec};
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
    assert_eq!(writable, 37);

    for (agent_id, definition) in agents {
        if definition.global.is_none() {
            continue;
        }
        let path = temp_file(
            &format!("matrix-{agent_id}"),
            match definition.format.as_str() {
                "toml" => "toml",
                "yaml" => "yaml",
                _ => "json",
            },
        );
        let adapter = get_agent_adapter_for(&definition, &agent_id);
        let codec = from_name(definition.codec.as_deref(), &agent_id);
        let supports_http = definition
            .transports
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|transport| transport == "http");
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
        if !supports_http {
            assert!(remote_result.is_err(), "{agent_id} must reject remote MCPs");
        } else {
            remote_result
                .unwrap_or_else(|error| panic!("{agent_id} failed to write HTTP config: {error}"));
        }
        let scanned = adapter.read(&path);
        let scanned_local = scanned
            .get("local")
            .unwrap_or_else(|| panic!("{agent_id} did not scan its stdio entry"));
        assert_eq!(
            scanned_local,
            &normalize_with_codec(codec, &local),
            "{agent_id}"
        );
        if !supports_http {
            assert!(!scanned.contains_key("remote"));
        } else {
            let scanned_remote = scanned
                .get("remote")
                .unwrap_or_else(|| panic!("{agent_id} did not scan its HTTP entry"));
            assert_eq!(
                scanned_remote,
                &normalize_with_codec(codec, &remote),
                "{agent_id}"
            );
        }
        let snapshot = adapter
            .snapshot(&path, "local")
            .unwrap_or_else(|error| panic!("{agent_id} failed to snapshot: {error}"))
            .unwrap_or_else(|| panic!("{agent_id} returned no snapshot"));
        adapter
            .remove_snapshot(&path, "local", &snapshot)
            .unwrap_or_else(|error| panic!("{agent_id} failed to remove snapshot: {error}"));
        assert!(!adapter.read(&path).contains_key("local"), "{agent_id}");
        adapter
            .restore(&path, "local", &snapshot)
            .unwrap_or_else(|error| panic!("{agent_id} failed to restore snapshot: {error}"));
        assert_eq!(
            adapter.read(&path).get("local"),
            Some(&normalize_with_codec(codec, &local)),
            "{agent_id}"
        );
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
        ("amp", "~/.config/amp/settings.json"),
        ("amazon-q", "~/.aws/amazonq/default.json"),
        ("antigravity", "~/.gemini/config/mcp_config.json"),
        ("augment", "~/.augment/settings.json"),
        ("boltai", "~/.boltai/mcp.json"),
        ("claude-code", "~/.claude.json"),
        (
            "claude-desktop",
            "~/Library/Application Support/Claude/claude_desktop_config.json",
        ),
        ("cline", "~/.cline/data/settings/cline_mcp_settings.json"),
        ("codebuddy-code", "~/.codebuddy/.mcp.json"),
        ("codex", "~/.codex/config.toml"),
        ("continue", "~/.continue/config.yaml"),
        ("copilot-cli", "~/.copilot/mcp-config.json"),
        ("crush", "~/.config/crush/crush.json"),
        ("cursor", "~/.cursor/mcp.json"),
        ("factory-droid", "~/.factory/mcp.json"),
        ("firebender", "~/.firebender/firebender.json"),
        ("gemini", "~/.gemini/settings.json"),
        (
            "goose",
            "~/Library/Application Support/Block/goose/config/config.yaml",
        ),
        ("hermes", "~/.hermes/config.yaml"),
        ("junie", "~/.junie/mcp/mcp.json"),
        ("kilo-code", "~/.config/kilo/kilo.jsonc"),
        ("kimi-code", "~/.kimi-code/mcp.json"),
        ("kiro", "~/.kiro/settings/mcp.json"),
        ("lmstudio", "~/.lmstudio/mcp.json"),
        ("mistral-vibe", "~/.vibe/config.toml"),
        ("opencode", "~/.config/opencode/opencode.json"),
        ("openhands", "~/.openhands/mcp.json"),
        ("pi", "~/.pi/agent/mcp.json"),
        ("qoder", "~/.qoder/settings.json"),
        ("qwen-code", "~/.qwen/settings.json"),
        (
            "roo-code",
            "~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json",
        ),
        ("rovo-dev", "~/.rovodev/mcp.json"),
        ("tabnine", "~/.tabnine/mcp_servers.json"),
        (
            "vscode",
            "~/Library/Application Support/Code/User/mcp.json",
        ),
        ("warp", "~/.warp/.mcp.json"),
        ("windsurf", "~/.codeium/windsurf/mcp_config.json"),
        ("zed", "~/.config/zed/settings.json"),
    ];

    for (agent_id, path) in expected {
        assert_eq!(agents[agent_id].global.as_deref(), Some(path), "{agent_id}");
    }
    for agent_id in ["devin", "qoderwork"] {
        assert!(agents[agent_id].global.is_none(), "{agent_id}");
    }
}

#[test]
fn verified_and_catalog_definitions_have_auditable_boundaries() {
    let verified: std::collections::BTreeMap<String, mux_core::types::AgentDefinition> =
        serde_json::from_str(include_str!("../../data/agents.json")).unwrap();
    let catalog: std::collections::BTreeMap<String, mux_core::types::AgentDefinition> =
        serde_json::from_str(include_str!("../../data/agent-catalog.json")).unwrap();
    let verified_ids: std::collections::BTreeSet<_> = verified.keys().cloned().collect();
    let catalog_ids: std::collections::BTreeSet<_> = catalog.keys().cloned().collect();
    let all_ids: std::collections::BTreeSet<_> =
        verified_ids.union(&catalog_ids).cloned().collect();

    assert_eq!(verified.len(), 39);
    assert_eq!(catalog.len(), 175);
    assert_eq!(verified_ids.intersection(&catalog_ids).count(), 23);
    assert_eq!(all_ids.len(), 191);
    assert_eq!(
        verified
            .values()
            .filter(|item| item.global.is_some())
            .count(),
        37
    );
    assert!(catalog.len() >= 170);
    for (id, definition) in verified {
        assert_eq!(definition.builtin, Some(true), "{id}");
        assert!(
            matches!(
                definition.codec.as_deref(),
                Some(
                    "standard"
                        | "claude_desktop"
                        | "explicit_type"
                        | "url_inferred"
                        | "vscode"
                        | "codex"
                        | "opencode"
                        | "gemini"
                        | "windsurf"
                        | "qoder"
                        | "copilot"
                        | "cline"
                        | "roo"
                        | "warp"
                        | "kimi"
                        | "transport"
                        | "tabnine"
                        | "continue"
                        | "goose"
                        | "vibe"
                        | "server_url"
                        | "url_transport"
                        | "stdio_only"
                )
            ),
            "{id}: unknown codec {:?}",
            definition.codec
        );
        assert!(
            matches!(definition.layout.as_deref(), Some("map" | "list")),
            "{id}: invalid layout {:?}",
            definition.layout
        );
        if definition.layout.as_deref() == Some("list") {
            assert!(definition.identity_field.is_some(), "{id}");
            assert_ne!(definition.format, "json", "{id}");
        }
        let transports = definition.transports.as_deref().unwrap_or_default();
        assert!(
            transports
                .iter()
                .all(|transport| matches!(transport.as_str(), "stdio" | "http")),
            "{id}: invalid transport {transports:?}"
        );
        let unique_transports: std::collections::BTreeSet<_> = transports.iter().collect();
        assert_eq!(unique_transports.len(), transports.len(), "{id}");
        assert!(
            definition
                .name
                .as_deref()
                .is_some_and(|name| !name.is_empty()),
            "{id}"
        );
        assert!(
            definition
                .docs
                .as_deref()
                .is_some_and(|url| url.starts_with("https://")),
            "{id}"
        );
        assert_eq!(
            definition.verified_at.as_deref(),
            Some("2026-07-14"),
            "{id}"
        );
        if definition.global.is_some() {
            let evidence = definition.evidence.as_deref().unwrap_or_default();
            assert!(
                matches!(
                    evidence,
                    "official" | "official-source" | "community-extension"
                ),
                "{id}: {evidence}"
            );
            if evidence == "community-extension" {
                assert!(
                    definition
                        .note
                        .as_deref()
                        .is_some_and(|note| note.contains("pi-mcp-adapter")),
                    "{id}"
                );
            }
            assert!(
                matches!(definition.format.as_str(), "json" | "toml" | "yaml"),
                "{id}"
            );
            assert!(!definition.key.is_empty(), "{id}");
            assert!(
                !definition
                    .transports
                    .as_deref()
                    .unwrap_or_default()
                    .is_empty(),
                "{id}"
            );
        }
    }
    for (id, definition) in catalog {
        assert_eq!(definition.builtin, Some(true), "{id}");
        assert!(definition.global.is_none(), "{id}");
        assert_eq!(definition.format, "unknown", "{id}");
        assert!(
            matches!(
                definition.evidence.as_deref(),
                Some("catalog" | "official" | "official-source")
            ),
            "{id}: invalid evidence {:?}",
            definition.evidence
        );
        assert!(
            definition
                .transports
                .as_deref()
                .unwrap_or_default()
                .is_empty(),
            "{id}"
        );
    }
}

#[test]
fn continue_yaml_list_is_lossless_and_adds_required_root_metadata() {
    let definition = &builtin_agents()["continue"];
    let path = temp_file("continue-policy", "yaml");
    let original = r#"# keep this comment
telemetry:
  apiKey: private
mcpServers:
  - name: docs
    type: streamable-http
    url: https://old.example/mcp
    requestOptions:
      timeout: 9000
      headers:
        Old: value
    toolPolicy:
      allow: [search]
"#;
    std::fs::write(&path, original).unwrap();

    get_agent_adapter_for(definition, "continue")
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    let root: Value = serde_yaml::from_str(&written).unwrap();
    let target = &root["mcpServers"][0];
    assert!(written.contains("# keep this comment"));
    assert_eq!(root["telemetry"]["apiKey"], "private");
    assert!(root.get("name").is_none());
    assert_eq!(target["name"], "docs");
    assert_eq!(target["type"], "streamable-http");
    assert_eq!(target["url"], "https://new.example/mcp");
    assert_eq!(target["requestOptions"]["timeout"], 9000);
    assert_eq!(target["requestOptions"]["headers"]["X-New"], "value");
    assert_eq!(target["toolPolicy"]["allow"][0], "search");

    let new_path = temp_file("continue-new", "yaml");
    get_agent_adapter_for(definition, "continue")
        .upsert(&new_path, "docs", &http("https://new.example/mcp"))
        .unwrap();
    let root: Value = serde_yaml::from_str(&std::fs::read_to_string(&new_path).unwrap()).unwrap();
    assert_eq!(root["name"], "Local Config");
    assert_eq!(root["version"], "0.0.1");
    assert_eq!(root["schema"], "v1");

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(new_path);
}

#[test]
fn goose_yaml_map_uses_current_fields_and_preserves_extension_policy() {
    let definition = &builtin_agents()["goose"];
    let path = temp_file("goose-policy", "yaml");
    let original = r#"# private Goose settings
provider: private-model
extensions:
  docs:
    name: Custom display name
    type: streamable_http
    uri: https://old.example/mcp
    description: keep me
    enabled: false
    timeout: 42
"#;
    std::fs::write(&path, original).unwrap();

    get_agent_adapter_for(definition, "goose")
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    let root: Value = serde_yaml::from_str(&written).unwrap();
    let target = &root["extensions"]["docs"];
    assert!(written.contains("# private Goose settings"));
    assert_eq!(root["provider"], "private-model");
    assert_eq!(target["name"], "Custom display name");
    assert_eq!(target["type"], "streamable_http");
    assert_eq!(target["uri"], "https://new.example/mcp");
    assert_eq!(target["description"], "keep me");
    assert_eq!(target["enabled"], false);
    assert_eq!(target["timeout"], 42);
    assert!(target.get("url").is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn vibe_toml_list_is_lossless_and_uses_transport_field() {
    let definition = &builtin_agents()["mistral-vibe"];
    let path = temp_file("vibe-policy", "toml");
    let original = r#"model = "private-model" # keep root

[[mcp_servers]]
name = "docs"
transport = "http"
url = "https://old.example/mcp"
enabled = false
timeout = 42 # keep policy
"#;
    std::fs::write(&path, original).unwrap();

    get_agent_adapter_for(definition, "mistral-vibe")
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    let root: toml::Value = written.parse().unwrap();
    let target = &root["mcp_servers"][0];
    assert!(written.contains("# keep root"));
    assert!(written.contains("# keep policy"));
    assert_eq!(root["model"].as_str(), Some("private-model"));
    assert_eq!(target["name"].as_str(), Some("docs"));
    assert_eq!(target["transport"].as_str(), Some("streamable-http"));
    assert_eq!(target["url"].as_str(), Some("https://new.example/mcp"));
    assert_eq!(target["enabled"].as_bool(), Some(false));
    assert_eq!(target["timeout"].as_integer(), Some(42));
    let _ = std::fs::remove_file(path);
}

#[test]
fn new_json_codecs_use_documented_nested_and_transport_fields() {
    let agents = builtin_agents();
    let path = temp_file("new-json-codecs", "json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"docs":{"url":"https://old","requestInit":{"cache":"private","headers":{"Old":"value"}},"policy":{"allow":true}}}}"#,
    )
    .unwrap();
    get_agent_adapter_for(&agents["tabnine"], "tabnine")
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .unwrap();
    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let target = &root["mcpServers"]["docs"];
    assert_eq!(target["requestInit"]["cache"], "private");
    assert_eq!(target["requestInit"]["headers"]["X-New"], "value");
    assert_eq!(target["policy"]["allow"], true);

    for (agent_id, expected_transport) in [("rovo-dev", "http"), ("kimi-code", "")] {
        let path = temp_file(agent_id, "json");
        get_agent_adapter_for(&agents[agent_id], agent_id)
            .upsert(&path, "docs", &http("https://new.example/mcp"))
            .unwrap();
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let target = &root["mcpServers"]["docs"];
        if expected_transport.is_empty() {
            assert!(target.get("transport").is_none(), "{agent_id}");
        } else {
            assert_eq!(target["transport"], expected_transport, "{agent_id}");
        }
        let _ = std::fs::remove_file(path);
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn audited_remote_schemas_use_exact_documented_fields() {
    let agents = builtin_agents();

    let path = temp_file("antigravity-server-url", "json");
    get_agent_adapter_for(&agents["antigravity"], "antigravity")
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .unwrap();
    let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let target = &root["mcpServers"]["docs"];
    assert_eq!(target["serverUrl"], "https://new.example/mcp");
    assert!(target.get("url").is_none());
    let _ = std::fs::remove_file(path);

    for agent_id in ["augment", "openhands"] {
        let path = temp_file(agent_id, "json");
        get_agent_adapter_for(&agents[agent_id], agent_id)
            .upsert(&path, "docs", &http("https://new.example/mcp"))
            .unwrap();
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let target = &root["mcpServers"]["docs"];
        assert_eq!(target["type"], "http", "{agent_id}");
        assert_eq!(target["url"], "https://new.example/mcp", "{agent_id}");
        let _ = std::fs::remove_file(path);
    }

    let path = temp_file("boltai-stdio-only", "json");
    let error = get_agent_adapter_for(&agents["boltai"], "boltai")
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .unwrap_err();
    assert!(error.contains("stdio servers only"));
    assert!(!path.exists());
}

#[test]
fn hermes_uses_transport_only_for_legacy_sse() {
    let agents = builtin_agents();
    let path = temp_file("hermes-transports", "yaml");
    let adapter = get_agent_adapter_for(&agents["hermes"], "hermes");
    adapter
        .upsert(&path, "http", &http("https://new.example/mcp"))
        .unwrap();
    adapter
        .upsert(
            &path,
            "sse",
            &McpConfig::Http(HttpConfig {
                kind: "sse".into(),
                url: "https://new.example/sse".into(),
                headers: None,
            }),
        )
        .unwrap();
    let root: Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(root["mcp_servers"]["http"].get("transport").is_none());
    assert_eq!(root["mcp_servers"]["sse"]["transport"], "sse");
    let _ = std::fs::remove_file(path);
}

#[test]
fn duplicate_yaml_server_keys_fail_closed() {
    let agents = builtin_agents();
    let path = temp_file("duplicate-yaml", "yaml");
    let original =
        "private: keep\nmcp_servers:\n  docs: {url: https://one}\n  docs: {url: https://two}\n";
    std::fs::write(&path, original).unwrap();
    let result = get_agent_adapter_for(&agents["hermes"], "hermes").upsert(
        &path,
        "docs",
        &http("https://new.example/mcp"),
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let _ = std::fs::remove_file(path);
}

#[test]
fn duplicate_toml_list_identities_fail_closed() {
    let agents = builtin_agents();
    let path = temp_file("duplicate-toml-list", "toml");
    let original = r#"private = "keep"

[[mcp_servers]]
name = "docs"
transport = "http"
url = "https://one"

[[mcp_servers]]
name = "docs"
transport = "http"
url = "https://two"
"#;
    std::fs::write(&path, original).unwrap();
    let adapter = get_agent_adapter_for(&agents["mistral-vibe"], "mistral-vibe");
    assert!(adapter.read(&path).is_empty());
    assert!(adapter
        .upsert(&path, "docs", &http("https://new.example/mcp"))
        .is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let _ = std::fs::remove_file(path);
}

#[test]
fn catalog_only_and_unknown_formats_fail_closed() {
    let definition = &builtin_agents()["devin"];
    let path = temp_file("unsupported", "json");
    let result = get_agent_adapter_for(definition, "devin").upsert(
        &path,
        "docs",
        &http("https://new.example/mcp"),
    );
    assert!(result.is_err());
    assert!(!path.exists());
}
