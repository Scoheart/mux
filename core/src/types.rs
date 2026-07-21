use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Wire protocol used by a reusable model endpoint profile. These values match
/// the provider identifiers supported by the first managed Agent set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelProtocol {
    AnthropicMessages,
    OpenaiResponses,
    OpenaiCompletions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    /// API/计费渠道，例如 `openrouter`、`anthropic` 或 `custom`。
    /// v1 Profile 没有该字段；v2 migration 会在接管前补齐。
    #[serde(default)]
    pub provider: String,
    /// 模型开发商。它与访问渠道正交，例如通过 OpenRouter 使用 Claude
    /// 时 provider 是 `openrouter`，model_vendor 是 `anthropic`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_vendor: Option<String>,
    /// Agent-native provider/model keys retained during explicit adoption.
    /// New MUX profiles leave this empty and use a derived `mux_*` identity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub native_ids: BTreeMap<String, String>,
    pub protocol: ModelProtocol,
    pub base_url: String,
    pub model: String,
    /// Optional environment variable name used by Agents such as Grok Build
    /// that natively resolve per-model credentials from the process environment.
    /// This is metadata only; the credential value is never stored here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StdioConfig {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
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
            McpConfig::Stdio(c) => RegistryConfig {
                stdio: Some(c),
                http: None,
            },
            McpConfig::Http(c) => RegistryConfig {
                stdio: None,
                http: Some(c),
            },
        }
    }
}

impl SourceDef {
    /// A subscribed remote URL source, enabled, stamped now, `mcpServers` key.
    pub fn new_remote(id: String, name: String, url: String, format: String, now: String) -> Self {
        Self {
            url: Some(url),
            ..Self::base(id, "remote", name, format, now)
        }
    }

    /// A local-file source (imported or managed), enabled, stamped now.
    pub fn new_local(
        id: String,
        name: String,
        path: Option<String>,
        format: String,
        now: String,
    ) -> Self {
        Self {
            path,
            ..Self::base(id, "local", name, format, now)
        }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSkillsDirectory {
    pub target_id: String,
    pub global_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AgentInstallProbe {
    Path { path: String },
    Command { name: String },
    MacBundle { bundle_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSkillsCapability {
    pub target_id: String,
    pub global_dir: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<AgentSkillsDirectory>,
    pub docs: String,
    pub evidence: String,
    pub verified_at: String,
    #[serde(default)]
    pub probes: Vec<AgentInstallProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentDefinition {
    pub global: Option<String>,
    pub project: Option<String>,
    pub format: String, // "json" | "toml" | "yaml"
    pub key: String,
    /// Interpret a dotted JSON `key` as an object path instead of a literal
    /// top-level property name. This stays opt-in so historical and custom
    /// Agent definitions retain their original literal-key behavior.
    #[serde(default, skip_serializing_if = "is_false")]
    pub key_path: bool,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<bool>,
    /// Display metadata and evidence are intentionally part of the definition:
    /// the UI must distinguish a verified writable target from a discovered
    /// catalog entry whose on-disk schema is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>, // "official" | "catalog" | "custom"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    /// Wire-format metadata. Missing values retain the legacy standard map
    /// behavior so existing custom Agent definitions remain compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>, // "map" | "list"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<String>>,
    /// Required root fields to add when an adapter materializes its managed
    /// capability. Existing values are never overwritten; adapters that
    /// support nested defaults merge only missing descendants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_defaults: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<AgentSkillsCapability>,
}

fn is_false(value: &bool) -> bool {
    !*value
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
