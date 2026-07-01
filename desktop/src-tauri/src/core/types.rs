use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StdioConfig {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

fn default_http_type() -> String {
    "http".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpConfig {
    // Many real configs write an http server as just `{ "url": "…" }` (no type),
    // or use `streamable-http` / `sse`. Default the type so those are still
    // recognized during a scan instead of being dropped.
    #[serde(rename = "type", default = "default_http_type")]
    pub kind: String, // "http" | "sse" | "streamable-http" | …
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum McpConfig {
    Stdio(StdioConfig),
    Http(HttpConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RegistryConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdio: Option<StdioConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryEntry {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub config: RegistryConfig,
    /// Where this entry came from. Absent for builtin entries (those are
    /// inferred at runtime); present for entries written to ~/.mux/registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<RegistryOrigin>,
}

/// Transport bucket of a config: "stdio" (local process) or "http" (remote,
/// covers http+sse). This is the second half of a registry entry's identity.
pub fn transport_of(config: &McpConfig) -> &'static str {
    match config {
        McpConfig::Stdio(_) => "stdio",
        McpConfig::Http(_) => "http",
    }
}

impl RegistryEntry {
    /// Transport bucket of this entry ("stdio" | "http"). An entry carries
    /// exactly one transport; if both were somehow present, stdio wins.
    pub fn transport(&self) -> &'static str {
        if self.config.stdio.is_some() {
            "stdio"
        } else {
            "http"
        }
    }

    /// Composite identity: `name::transport`. Two entries with the same name
    /// but different transports (e.g. figma stdio vs http) are distinct.
    pub fn key(&self) -> String {
        format!("{}::{}", self.name, self.transport())
    }
}

/// Provenance of a custom registry entry.
/// `kind` is "discovered" (scanned from a local app config) or "manual"
/// (created by the user). `agent`/`scope` describe the source app for
/// discovered entries and are omitted for manual ones.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryOrigin {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDefinition {
    pub global: Option<String>,
    pub project: Option<String>,
    pub format: String, // "json" | "toml"
    pub key: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_entry_roundtrips_stdio() {
        let json = r#"{"name":"git","description":"d","tags":["builtin"],
            "config":{"stdio":{"command":"npx","args":["-y","x"]}}}"#;
        let e: RegistryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.name, "git");
        assert_eq!(e.config.stdio.as_ref().unwrap().command, "npx");
    }
}
