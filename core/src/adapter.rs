use crate::codec::{for_agent, Codec};
use crate::json_adapter::JsonAdapter;
use crate::toml_adapter::TomlAdapter;
use crate::types::McpConfig;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub trait Adapter {
    /// Read the `key` section as `{name: config}`, parsing each entry
    /// independently — an entry that doesn't fit the stdio/http shape is skipped
    /// (it stays on disk, it just isn't surfaced). A missing file, unparseable
    /// content, or absent key degrade gracefully to an empty map.
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig>;
    /// Insert or update a SINGLE server by name under `key`, leaving every other
    /// entry's raw on-disk representation untouched. Within an existing target,
    /// only codec-owned connection fields change; user policy fields survive.
    fn upsert(&self, path: &Path, name: &str, cfg: &McpConfig) -> Result<(), String>;
    /// Remove `names` from the `key` section, leaving all other entries untouched.
    /// No-op if the file does not exist.
    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String>;
    /// Capture one complete server entry before a destructive operation. Unlike
    /// `read`, this includes Agent-owned policy fields that MUX does not model.
    fn snapshot(&self, path: &Path, name: &str) -> Result<Option<Value>, String>;
    /// Remove one entry only if its complete semantic value still matches the
    /// snapshot persisted by the disable flow.
    fn remove_snapshot(&self, path: &Path, name: &str, snapshot: &Value) -> Result<(), String>;
    /// Restore a previously captured complete entry. Refuses to overwrite an
    /// entry recreated by the Agent or user while it was disabled.
    fn restore(&self, path: &Path, name: &str, snapshot: &Value) -> Result<(), String>;
}

/// Pick the adapter for a given config format. `"toml"` -> TOML, otherwise JSON.
pub fn get_adapter(format: &str, key: &str) -> Box<dyn Adapter> {
    get_adapter_with_codec(format, key, Codec::Standard)
}

pub fn get_agent_adapter(format: &str, key: &str, agent_id: &str) -> Box<dyn Adapter> {
    get_adapter_with_codec(format, key, for_agent(agent_id))
}

fn get_adapter_with_codec(format: &str, key: &str, codec: Codec) -> Box<dyn Adapter> {
    if format == "toml" {
        Box::new(TomlAdapter::with_codec(key, codec))
    } else {
        Box::new(JsonAdapter::with_codec(key, codec))
    }
}
