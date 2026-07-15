use crate::types::{HttpConfig, McpConfig, StdioConfig};
use serde_json::{Map, Value};
use std::collections::HashMap;

const STANDARD_FIELDS: &[&str] = &["type", "command", "args", "env", "cwd", "url", "headers"];
const CODEX_FIELDS: &[&str] = &[
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "headers",
    "http_headers",
];
const OPENCODE_FIELDS: &[&str] = &[
    "type",
    "command",
    "args",
    "env",
    "environment",
    "cwd",
    "url",
    "headers",
];
const GEMINI_FIELDS: &[&str] = &[
    "type", "command", "args", "env", "cwd", "url", "httpUrl", "headers",
];
const WINDSURF_FIELDS: &[&str] = &[
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "serverUrl",
    "headers",
];
const CLINE_FIELDS: &[&str] = &[
    "transport",
    "transportType",
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "headers",
];
const WARP_FIELDS: &[&str] = &[
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "working_directory",
    "url",
    "headers",
];
const WARP_HTTP_FIELDS: &[&str] = &[
    "type",
    "command",
    "args",
    "cwd",
    "working_directory",
    "url",
    "headers",
];
const KIMI_FIELDS: &[&str] = &[
    "transport",
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "headers",
];
const TRANSPORT_FIELDS: &[&str] = &[
    "transport",
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "headers",
];
const TABNINE_FIELDS: &[&str] = &[
    "transport",
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "headers",
];
const CONTINUE_FIELDS: &[&str] = &["type", "command", "args", "env", "cwd", "url"];
const GOOSE_FIELDS: &[&str] = &["type", "cmd", "args", "envs", "cwd", "uri", "headers"];
const VIBE_FIELDS: &[&str] = &[
    "transport",
    "type",
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "headers",
];
const HEADERS_FIELD: &[&str] = &["headers"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Standard,
    ClaudeDesktop,
    ExplicitType,
    UrlInferred,
    VsCode,
    Codex,
    OpenCode,
    Gemini,
    Windsurf,
    Qoder,
    QoderWork,
    Copilot,
    Cline,
    Roo,
    Warp,
    Kimi,
    Transport,
    Tabnine,
    Continue,
    Goose,
    Vibe,
    StdioOnly,
}

pub struct ObjectPatch {
    pub parent: &'static str,
    pub controlled: &'static [&'static str],
    pub fields: Vec<(String, Value)>,
}

pub struct EntryPatch {
    pub controlled: &'static [&'static str],
    pub fields: Vec<(String, Value)>,
    /// Fields required for a new entry but owned by the user once present.
    pub defaults: Vec<(String, Value)>,
    /// Connection fields nested inside an Agent-owned object. Adapters patch
    /// only the listed children so sibling policy fields survive.
    pub object_patches: Vec<ObjectPatch>,
}

pub fn for_agent(agent_id: &str) -> Codec {
    match agent_id {
        "claude-desktop" => Codec::ClaudeDesktop,
        "claude-code" | "amazon-q" => Codec::ExplicitType,
        "cursor" | "zed" | "kiro" | "junie" | "pi" => Codec::UrlInferred,
        "vscode" => Codec::VsCode,
        "codex" => Codec::Codex,
        "opencode" => Codec::OpenCode,
        "gemini" => Codec::Gemini,
        "windsurf" => Codec::Windsurf,
        "qoder" => Codec::Qoder,
        "qoderwork" => Codec::QoderWork,
        "copilot-cli" => Codec::Copilot,
        "cline" => Codec::Cline,
        "roo-code" => Codec::Roo,
        "warp" => Codec::Warp,
        _ => Codec::Standard,
    }
}

pub fn from_name(name: Option<&str>, agent_id: &str) -> Codec {
    match name {
        Some("standard") => Codec::Standard,
        Some("claude_desktop") => Codec::ClaudeDesktop,
        Some("explicit_type") => Codec::ExplicitType,
        Some("url_inferred") => Codec::UrlInferred,
        Some("vscode") => Codec::VsCode,
        Some("codex") => Codec::Codex,
        Some("opencode") => Codec::OpenCode,
        Some("gemini") => Codec::Gemini,
        Some("windsurf") => Codec::Windsurf,
        Some("qoder") => Codec::Qoder,
        Some("qoderwork") => Codec::QoderWork,
        Some("copilot") => Codec::Copilot,
        Some("cline") => Codec::Cline,
        Some("roo") => Codec::Roo,
        Some("warp") => Codec::Warp,
        Some("kimi") => Codec::Kimi,
        Some("transport") => Codec::Transport,
        Some("tabnine") => Codec::Tabnine,
        Some("continue") => Codec::Continue,
        Some("goose") => Codec::Goose,
        Some("vibe") => Codec::Vibe,
        Some("server_url") => Codec::Windsurf,
        Some("url_transport") => Codec::Kimi,
        Some("stdio_only") => Codec::StdioOnly,
        _ => for_agent(agent_id),
    }
}

/// Convert a catalog config to the canonical value that will be observed after
/// writing it to, then reading it back from, a specific Agent. Some formats
/// collapse several HTTP labels into one wire representation.
pub fn normalize_for_agent(agent_id: &str, config: &McpConfig) -> McpConfig {
    normalize_with_codec(for_agent(agent_id), config)
}

pub fn normalize_with_codec(codec: Codec, config: &McpConfig) -> McpConfig {
    let Ok(patch) = codec.patch(config) else {
        return config.clone();
    };
    let mut fields: Map<String, Value> = patch.fields.into_iter().collect();
    for object_patch in patch.object_patches {
        if !object_patch.fields.is_empty() {
            fields.insert(
                object_patch.parent.into(),
                Value::Object(object_patch.fields.into_iter().collect()),
            );
        }
    }
    let value = Value::Object(fields);
    codec.decode(&value).unwrap_or_else(|| config.clone())
}

impl Codec {
    pub fn decode(self, value: &Value) -> Option<McpConfig> {
        if self == Codec::Cline {
            if let Some(transport) = value.as_object().and_then(|object| object.get("transport")) {
                return self.decode_flat(transport);
            }
        }
        self.decode_flat(value)
    }

    fn decode_flat(self, value: &Value) -> Option<McpConfig> {
        let object = value.as_object()?;
        if self == Codec::OpenCode {
            if let Some(command) = object.get("command").and_then(Value::as_array) {
                let mut command = command.iter().map(Value::as_str);
                let executable = command.next()??.to_string();
                let args = command
                    .collect::<Option<Vec<_>>>()?
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                return Some(McpConfig::Stdio(StdioConfig {
                    command: executable,
                    args: (!args.is_empty()).then_some(args),
                    env: string_map(object.get("environment")),
                    cwd: string_field(object, "cwd"),
                }));
            }
        }

        let command_key = if self == Codec::Goose {
            "cmd"
        } else {
            "command"
        };
        if let Some(command) = object.get(command_key).and_then(Value::as_str) {
            return Some(McpConfig::Stdio(StdioConfig {
                command: command.to_string(),
                args: string_array(object.get("args")),
                env: string_map(object.get(if self == Codec::Goose { "envs" } else { "env" })),
                cwd: string_field(
                    object,
                    if self == Codec::Warp {
                        "working_directory"
                    } else {
                        "cwd"
                    },
                ),
            }));
        }

        let (url_key, default_kind) = match self {
            Codec::Gemini if object.get("httpUrl").and_then(Value::as_str).is_some() => {
                ("httpUrl", "streamable-http")
            }
            Codec::Gemini => ("url", "sse"),
            Codec::Windsurf if object.get("serverUrl").and_then(Value::as_str).is_some() => {
                ("serverUrl", "streamable-http")
            }
            Codec::Goose => ("uri", "streamable-http"),
            _ => ("url", "http"),
        };
        let url = object.get(url_key).and_then(Value::as_str)?.to_string();
        let raw_kind = object
            .get(
                if matches!(
                    self,
                    Codec::Kimi | Codec::Transport | Codec::Tabnine | Codec::Vibe
                ) {
                    "transport"
                } else {
                    "type"
                },
            )
            .and_then(Value::as_str);
        let kind = match (self, raw_kind) {
            (Codec::OpenCode, Some("remote")) => "http",
            (Codec::Cline, Some("streamableHttp")) => "streamable-http",
            (Codec::Goose, Some("streamable_http")) => "streamable-http",
            (_, Some("remote" | "http")) => "http",
            (_, Some("streamableHttp" | "streamable-http")) => "streamable-http",
            (_, Some("sse")) => "sse",
            (_, Some("ws")) => "ws",
            (_, Some(kind)) => kind,
            _ => default_kind,
        }
        .to_string();
        let headers = match self {
            Codec::Codex => string_map(object.get("http_headers")),
            Codec::Continue => nested_string_map(object, "requestOptions", "headers"),
            Codec::Tabnine => nested_string_map(object, "requestInit", "headers"),
            _ => string_map(object.get("headers")),
        };
        Some(McpConfig::Http(HttpConfig { kind, url, headers }))
    }

    pub fn patch(self, config: &McpConfig) -> Result<EntryPatch, String> {
        if matches!(config, McpConfig::Http(_)) {
            if self == Codec::ClaudeDesktop {
                return Err(
                    "Claude Desktop's local config accepts stdio servers only; add remote MCPs as Claude Connectors"
                        .into(),
                );
            }
            if self == Codec::StdioOnly {
                return Err(
                    "this Agent's file-based MCP configuration accepts stdio servers only".into(),
                );
            }
        }
        let mut fields = Vec::new();
        let mut defaults = Vec::new();
        let mut object_patches = Vec::new();
        match config {
            McpConfig::Stdio(stdio) => match self {
                Codec::ExplicitType | Codec::Qoder | Codec::QoderWork => {
                    fields.push(("type".into(), Value::String("stdio".into())));
                    push_stdio_fields(&mut fields, stdio, "cwd");
                }
                Codec::OpenCode => {
                    fields.push(("type".into(), Value::String("local".into())));
                    let mut command = vec![Value::String(stdio.command.clone())];
                    command.extend(
                        stdio
                            .args
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(Value::String),
                    );
                    fields.push(("command".into(), Value::Array(command)));
                    push_optional(&mut fields, "environment", &stdio.env);
                    push_optional(&mut fields, "cwd", &stdio.cwd);
                }
                Codec::VsCode => {
                    fields.push(("type".into(), Value::String("stdio".into())));
                    push_stdio_fields(&mut fields, stdio, "cwd");
                }
                Codec::Copilot => {
                    fields.push(("type".into(), Value::String("local".into())));
                    push_stdio_fields(&mut fields, stdio, "cwd");
                    defaults.push((
                        "tools".into(),
                        Value::Array(vec![Value::String("*".into())]),
                    ));
                }
                Codec::Cline => {
                    let mut transport = vec![
                        ("type".into(), Value::String("stdio".into())),
                        ("command".into(), Value::String(stdio.command.clone())),
                    ];
                    push_optional(&mut transport, "args", &stdio.args);
                    push_optional(&mut transport, "env", &stdio.env);
                    push_optional(&mut transport, "cwd", &stdio.cwd);
                    fields.push((
                        "transport".into(),
                        Value::Object(transport.into_iter().collect()),
                    ));
                }
                Codec::Warp => push_stdio_fields(&mut fields, stdio, "working_directory"),
                Codec::Kimi => push_stdio_fields(&mut fields, stdio, "cwd"),
                Codec::Transport => {
                    fields.push(("transport".into(), Value::String("stdio".into())));
                    push_stdio_fields(&mut fields, stdio, "cwd");
                }
                Codec::Tabnine => {
                    push_stdio_fields(&mut fields, stdio, "cwd");
                    object_patches.push(ObjectPatch {
                        parent: "requestInit",
                        controlled: HEADERS_FIELD,
                        fields: Vec::new(),
                    });
                }
                Codec::Continue => {
                    fields.push(("type".into(), Value::String("stdio".into())));
                    push_stdio_fields(&mut fields, stdio, "cwd");
                    object_patches.push(ObjectPatch {
                        parent: "requestOptions",
                        controlled: HEADERS_FIELD,
                        fields: Vec::new(),
                    });
                }
                Codec::Goose => {
                    fields.push(("type".into(), Value::String("stdio".into())));
                    fields.push(("cmd".into(), Value::String(stdio.command.clone())));
                    fields.push((
                        "args".into(),
                        Value::Array(
                            stdio
                                .args
                                .clone()
                                .unwrap_or_default()
                                .into_iter()
                                .map(Value::String)
                                .collect(),
                        ),
                    ));
                    push_optional(&mut fields, "envs", &stdio.env);
                    push_optional(&mut fields, "cwd", &stdio.cwd);
                    defaults.push(("description".into(), Value::String(String::new())));
                }
                Codec::Vibe => {
                    fields.push(("transport".into(), Value::String("stdio".into())));
                    push_stdio_fields(&mut fields, stdio, "cwd");
                }
                _ => push_stdio_fields(&mut fields, stdio, "cwd"),
            },
            McpConfig::Http(http) => match self {
                Codec::ExplicitType => {
                    fields.push((
                        "type".into(),
                        Value::String(if http.kind == "sse" { "sse" } else { "http" }.into()),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::UrlInferred => push_http_fields(&mut fields, http, "url", "headers"),
                Codec::OpenCode => {
                    fields.push(("type".into(), Value::String("remote".into())));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::Codex => push_http_fields(&mut fields, http, "url", "http_headers"),
                Codec::Gemini => push_http_fields(
                    &mut fields,
                    http,
                    if http.kind == "sse" { "url" } else { "httpUrl" },
                    "headers",
                ),
                Codec::Windsurf => push_http_fields(&mut fields, http, "serverUrl", "headers"),
                Codec::QoderWork => {
                    fields.push((
                        "type".into(),
                        Value::String(
                            if http.kind == "sse" {
                                "sse"
                            } else {
                                "streamable-http"
                            }
                            .into(),
                        ),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::Qoder => {
                    fields.push((
                        "type".into(),
                        Value::String(
                            match http.kind.as_str() {
                                "sse" => "sse",
                                "ws" => "ws",
                                _ => "http",
                            }
                            .into(),
                        ),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::VsCode => {
                    fields.push((
                        "type".into(),
                        Value::String(if http.kind == "sse" { "sse" } else { "http" }.into()),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::Copilot => {
                    fields.push((
                        "type".into(),
                        Value::String(if http.kind == "sse" { "sse" } else { "http" }.into()),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                    defaults.push((
                        "tools".into(),
                        Value::Array(vec![Value::String("*".into())]),
                    ));
                }
                Codec::Cline => {
                    let mut transport = vec![
                        (
                            "type".into(),
                            Value::String(
                                if http.kind == "sse" {
                                    "sse"
                                } else {
                                    "streamableHttp"
                                }
                                .into(),
                            ),
                        ),
                        ("url".into(), Value::String(http.url.clone())),
                    ];
                    push_optional(&mut transport, "headers", &http.headers);
                    fields.push((
                        "transport".into(),
                        Value::Object(transport.into_iter().collect()),
                    ));
                }
                Codec::Roo => {
                    fields.push((
                        "type".into(),
                        Value::String(
                            if http.kind == "sse" {
                                "sse"
                            } else {
                                "streamable-http"
                            }
                            .into(),
                        ),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::Warp => push_http_fields(&mut fields, http, "url", "headers"),
                Codec::Kimi => {
                    if http.kind == "sse" {
                        fields.push(("transport".into(), Value::String("sse".into())));
                    }
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::Transport => {
                    fields.push((
                        "transport".into(),
                        Value::String(if http.kind == "sse" { "sse" } else { "http" }.into()),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                Codec::Tabnine => {
                    if http.kind == "sse" {
                        fields.push(("transport".into(), Value::String("sse".into())));
                    }
                    fields.push(("url".into(), Value::String(http.url.clone())));
                    let mut nested = Vec::new();
                    push_optional(&mut nested, "headers", &http.headers);
                    object_patches.push(ObjectPatch {
                        parent: "requestInit",
                        controlled: HEADERS_FIELD,
                        fields: nested,
                    });
                }
                Codec::Continue => {
                    fields.push((
                        "type".into(),
                        Value::String(
                            if http.kind == "sse" {
                                "sse"
                            } else {
                                "streamable-http"
                            }
                            .into(),
                        ),
                    ));
                    fields.push(("url".into(), Value::String(http.url.clone())));
                    let mut nested = Vec::new();
                    push_optional(&mut nested, "headers", &http.headers);
                    object_patches.push(ObjectPatch {
                        parent: "requestOptions",
                        controlled: HEADERS_FIELD,
                        fields: nested,
                    });
                }
                Codec::Goose => {
                    if http.kind == "sse" {
                        return Err(
                            "Goose no longer accepts SSE extensions; use Streamable HTTP".into(),
                        );
                    }
                    fields.push(("type".into(), Value::String("streamable_http".into())));
                    push_http_fields(&mut fields, http, "uri", "headers");
                    defaults.push(("description".into(), Value::String(String::new())));
                }
                Codec::Vibe => {
                    if http.kind == "sse" {
                        return Err("Mistral Vibe does not support the legacy SSE transport".into());
                    }
                    fields.push((
                        "transport".into(),
                        Value::String(
                            if http.kind == "streamable-http" {
                                "streamable-http"
                            } else {
                                "http"
                            }
                            .into(),
                        ),
                    ));
                    push_http_fields(&mut fields, http, "url", "headers");
                }
                _ => {
                    if http.kind != "http" {
                        fields.push(("type".into(), Value::String(http.kind.clone())));
                    }
                    push_http_fields(&mut fields, http, "url", "headers");
                }
            },
        }
        Ok(EntryPatch {
            controlled: match (self, config) {
                // Warp permits environment variables on URL-based entries for
                // runtime substitution. MUX cannot model those as HTTP headers,
                // so they remain Agent-owned when the URL is updated.
                (Codec::Warp, McpConfig::Http(_)) => WARP_HTTP_FIELDS,
                _ => self.controlled_fields(),
            },
            fields,
            defaults,
            object_patches,
        })
    }

    fn controlled_fields(self) -> &'static [&'static str] {
        match self {
            Codec::Codex => CODEX_FIELDS,
            Codec::OpenCode => OPENCODE_FIELDS,
            Codec::Gemini => GEMINI_FIELDS,
            Codec::Windsurf => WINDSURF_FIELDS,
            Codec::Cline => CLINE_FIELDS,
            Codec::Warp => WARP_FIELDS,
            Codec::Kimi => KIMI_FIELDS,
            Codec::Transport => TRANSPORT_FIELDS,
            Codec::Tabnine => TABNINE_FIELDS,
            Codec::Continue => CONTINUE_FIELDS,
            Codec::Goose => GOOSE_FIELDS,
            Codec::Vibe => VIBE_FIELDS,
            _ => STANDARD_FIELDS,
        }
    }
}

pub fn decode_any(value: &Value) -> Option<McpConfig> {
    [
        Codec::Standard,
        Codec::OpenCode,
        Codec::Gemini,
        Codec::Windsurf,
        Codec::Codex,
        Codec::Cline,
    ]
    .into_iter()
    .find_map(|codec| codec.decode(value))
}

fn string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_string)
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    let values = value?.as_array()?;
    values
        .iter()
        .map(|value| value.as_str().map(str::to_string))
        .collect()
}

fn string_map(value: Option<&Value>) -> Option<HashMap<String, String>> {
    let object = value?.as_object()?;
    object
        .iter()
        .map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
        .collect()
}

fn nested_string_map(
    object: &Map<String, Value>,
    parent: &str,
    child: &str,
) -> Option<HashMap<String, String>> {
    object
        .get(parent)
        .and_then(Value::as_object)
        .and_then(|nested| string_map(nested.get(child)))
}

fn push_stdio_fields(fields: &mut Vec<(String, Value)>, stdio: &StdioConfig, cwd_key: &str) {
    fields.push(("command".into(), Value::String(stdio.command.clone())));
    push_optional(fields, "args", &stdio.args);
    push_optional(fields, "env", &stdio.env);
    push_optional(fields, cwd_key, &stdio.cwd);
}

fn push_http_fields(
    fields: &mut Vec<(String, Value)>,
    http: &HttpConfig,
    url_key: &str,
    headers_key: &str,
) {
    fields.push((url_key.into(), Value::String(http.url.clone())));
    push_optional(fields, headers_key, &http.headers);
}

fn push_optional<T: serde::Serialize>(
    fields: &mut Vec<(String, Value)>,
    key: &str,
    value: &Option<T>,
) {
    if let Some(value) = value {
        if let Ok(value) = serde_json::to_value(value) {
            fields.push((key.into(), value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_local_roundtrip_uses_command_array() {
        let raw = serde_json::json!({
            "type": "local",
            "command": ["npx", "-y", "server"],
            "environment": {"TOKEN": "value"},
            "enabled": false
        });
        let config = Codec::OpenCode.decode(&raw).unwrap();
        let patch = Codec::OpenCode.patch(&config).unwrap();
        let fields: Map<String, Value> = patch.fields.into_iter().collect();
        assert_eq!(
            fields["command"],
            serde_json::json!(["npx", "-y", "server"])
        );
        assert_eq!(fields["type"], "local");
        assert_eq!(fields["environment"]["TOKEN"], "value");
    }

    #[test]
    fn agent_specific_remote_keys_are_canonical() {
        let config = McpConfig::Http(HttpConfig {
            kind: "streamable-http".into(),
            url: "https://example.com/mcp".into(),
            headers: None,
        });
        let open: Map<String, Value> = Codec::OpenCode
            .patch(&config)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        let codex: Map<String, Value> = Codec::Codex
            .patch(&config)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        let gemini: Map<String, Value> = Codec::Gemini
            .patch(&config)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        let windsurf: Map<String, Value> = Codec::Windsurf
            .patch(&config)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        assert_eq!(open["type"], "remote");
        assert!(codex.contains_key("url") && !codex.contains_key("type"));
        assert!(gemini.contains_key("httpUrl") && !gemini.contains_key("url"));
        assert!(windsurf.contains_key("serverUrl") && !windsurf.contains_key("url"));
    }

    #[test]
    fn explicit_and_inferred_transports_follow_agent_schemas() {
        let local = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: None,
            env: None,
            cwd: None,
        });
        let remote = McpConfig::Http(HttpConfig {
            kind: "streamable-http".into(),
            url: "https://example.com/mcp".into(),
            headers: None,
        });

        let explicit_local: Map<String, Value> = Codec::ExplicitType
            .patch(&local)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        let explicit_remote: Map<String, Value> = Codec::ExplicitType
            .patch(&remote)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        let inferred_remote: Map<String, Value> = Codec::UrlInferred
            .patch(&remote)
            .unwrap()
            .fields
            .into_iter()
            .collect();

        assert_eq!(explicit_local["type"], "stdio");
        assert_eq!(explicit_remote["type"], "http");
        assert!(!inferred_remote.contains_key("type"));
        assert_eq!(
            Codec::UrlInferred.decode(&Value::Object(inferred_remote)),
            Some(McpConfig::Http(HttpConfig {
                kind: "http".into(),
                url: "https://example.com/mcp".into(),
                headers: None,
            }))
        );
    }

    #[test]
    fn qoder_preserves_websocket_transport() {
        let remote = McpConfig::Http(HttpConfig {
            kind: "ws".into(),
            url: "wss://example.com/mcp".into(),
            headers: None,
        });
        let fields: Map<String, Value> = Codec::Qoder
            .patch(&remote)
            .unwrap()
            .fields
            .into_iter()
            .collect();

        assert_eq!(fields["type"], "ws");
        assert_eq!(Codec::Qoder.decode(&Value::Object(fields)), Some(remote));
    }

    #[test]
    fn qoderwork_uses_documented_transport_types() {
        let local = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: None,
            cwd: None,
        });
        let remote = McpConfig::Http(HttpConfig {
            kind: "http".into(),
            url: "https://example.com/mcp".into(),
            headers: None,
        });
        let local_fields: Map<String, Value> = Codec::QoderWork
            .patch(&local)
            .unwrap()
            .fields
            .into_iter()
            .collect();
        let remote_fields: Map<String, Value> = Codec::QoderWork
            .patch(&remote)
            .unwrap()
            .fields
            .into_iter()
            .collect();

        assert_eq!(local_fields["type"], "stdio");
        assert_eq!(remote_fields["type"], "streamable-http");
        assert_eq!(
            Codec::QoderWork.decode(&Value::Object(remote_fields)),
            Some(McpConfig::Http(HttpConfig {
                kind: "streamable-http".into(),
                url: "https://example.com/mcp".into(),
                headers: None,
            }))
        );
    }

    #[test]
    fn cline_uses_nested_transport_registration() {
        let raw = serde_json::json!({
            "transport": {
                "type": "streamableHttp",
                "url": "https://example.com/mcp",
                "headers": {"Authorization": "Bearer token"}
            },
            "disabled": true
        });
        let config = Codec::Cline.decode(&raw).unwrap();
        let patch = Codec::Cline.patch(&config).unwrap();
        let fields: Map<String, Value> = patch.fields.into_iter().collect();
        assert_eq!(fields["transport"]["type"], "streamableHttp");
        assert_eq!(fields["transport"]["url"], "https://example.com/mcp");
        assert!(!fields.contains_key("url"));
    }
}
