use crate::json_adapter::JsonAdapter;
use crate::toml_adapter::TomlAdapter;
use crate::types::McpConfig;
use std::collections::BTreeMap;
use std::path::Path;

pub trait Adapter {
    /// Read the `key` section as `{name: config}`, parsing each entry
    /// independently — an entry that doesn't fit the stdio/http shape is skipped
    /// (it stays on disk, it just isn't surfaced). A missing file, unparseable
    /// content, or absent key degrade gracefully to an empty map.
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig>;
    /// Insert or replace a SINGLE server by name under `key`, leaving every other
    /// entry's raw on-disk representation untouched (no lossy whole-section
    /// rewrite). Other top-level fields are preserved too.
    fn upsert(&self, path: &Path, name: &str, cfg: &McpConfig) -> Result<(), String>;
    /// Remove `names` from the `key` section, leaving all other entries untouched.
    /// No-op if the file does not exist.
    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String>;
}

/// Pick the adapter for a given config format. `"toml"` -> TOML, otherwise JSON.
pub fn get_adapter(format: &str, key: &str) -> Box<dyn Adapter> {
    if format == "toml" {
        Box::new(TomlAdapter::new(key))
    } else {
        Box::new(JsonAdapter::new(key))
    }
}
