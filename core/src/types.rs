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
    /// Optional homepage / source repository URL (e.g. a GitHub repo). Shown as a
    /// link in the UI; free-form, not part of the entry's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
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

/// Provenance of a catalog entry.
/// `kind` is one of:
///   - "discovered" — scanned from a local app config (`agent`/`scope` set),
///   - "manual"     — created by the user by hand,
///   - "remote"     — came from a subscribed remote source (`source` = its id),
///   - "local"      — came from a local file source (`source` = its id).
/// `agent`/`scope` describe the source app for discovered entries; `source`
/// references the owning `SourceDef` for remote/local entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryOrigin {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Id of the `SourceDef` this entry came from (remote/local sources only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

fn default_mcp_key() -> String {
    "mcpServers".to_string()
}

/// A user-added catalog source: either a subscribed remote URL or a local file.
/// The actual servers are parsed from a cached copy on disk under
/// `~/.mux/sources/<kind>/<id>.<ext>`. There is intentionally no "builtin" kind —
/// the catalog is entirely user-driven (subscribe / add local).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceDef {
    pub id: String,
    pub kind: String, // "remote" | "local"
    pub name: String,
    /// Remote sources: the subscribed URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Local sources: the original picked path (stored portably as `~/…`), used
    /// for re-copy on refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub format: String, // "json" | "toml"
    #[serde(default = "default_mcp_key")]
    pub key: String, // config section key, default "mcpServers"
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_count: Option<u32>,
    /// Last fetch/parse error, if any (keeps the source visible but flagged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<McpConfig> for RegistryConfig {
    fn from(cfg: McpConfig) -> Self {
        match cfg {
            McpConfig::Stdio(c) => RegistryConfig { stdio: Some(c), http: None },
            McpConfig::Http(c) => RegistryConfig { stdio: None, http: Some(c) },
        }
    }
}

impl SourceDef {
    /// A subscribed remote URL source, enabled, stamped now, `mcpServers` key.
    pub fn new_remote(id: String, name: String, url: String, format: String, now: String) -> Self {
        Self { url: Some(url), ..Self::base(id, "remote", name, format, now) }
    }

    /// A local-file source (imported or managed), enabled, stamped now.
    pub fn new_local(id: String, name: String, path: Option<String>, format: String, now: String) -> Self {
        Self { path, ..Self::base(id, "local", name, format, now) }
    }

    fn base(id: String, kind: &str, name: String, format: String, now: String) -> Self {
        Self {
            id,
            kind: kind.into(),
            name,
            url: None,
            path: None,
            format,
            key: "mcpServers".into(),
            enabled: true,
            added_at: Some(now.clone()),
            synced_at: Some(now),
            server_count: None,
            error: None,
        }
    }
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
