use crate::core::adapter::Adapter;
use crate::core::types::McpConfig;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub struct JsonAdapter {
    pub key: String,
}

impl JsonAdapter {
    pub fn new(key: &str) -> Self {
        Self { key: key.to_string() }
    }

    /// Parse the whole file as a JSON object (or `{}` if missing/garbage).
    fn read_root(&self, path: &Path) -> Value {
        fs::read_to_string(path)
            .ok()
            .and_then(|c| serde_json::from_str::<Value>(&c).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}))
    }

    fn write_root(&self, path: &Path, root: &Value) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let serialized = serde_json::to_string_pretty(root).map_err(|e| e.to_string())?;
        fs::write(path, serialized + "\n").map_err(|e| e.to_string())
    }
}

impl Adapter for JsonAdapter {
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig> {
        let root = self.read_root(path);
        let Some(section) = root.get(self.key.as_str()).and_then(Value::as_object) else {
            return BTreeMap::new();
        };
        // Per-entry: a server that doesn't fit stdio/http is skipped, NOT allowed
        // to nuke the whole map (which is what caused real configs to be wiped).
        section
            .iter()
            .filter_map(|(name, val)| {
                serde_json::from_value::<McpConfig>(val.clone())
                    .ok()
                    .map(|cfg| (name.clone(), cfg))
            })
            .collect()
    }

    fn upsert(&self, path: &Path, name: &str, cfg: &McpConfig) -> Result<(), String> {
        let mut root = self.read_root(path);
        // read_root guarantees an object.
        let obj = root.as_object_mut().expect("root is an object");
        let section = obj.entry(self.key.clone()).or_insert_with(|| json!({}));
        if !section.is_object() {
            *section = json!({});
        }
        section
            .as_object_mut()
            .unwrap()
            .insert(name.to_string(), serde_json::to_value(cfg).map_err(|e| e.to_string())?);
        self.write_root(path, &root)
    }

    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String> {
        // No-op when the file does not exist (avoid creating an empty section).
        if !path.exists() {
            return Ok(());
        }
        let mut root = self.read_root(path);
        if let Some(section) = root.get_mut(self.key.as_str()).and_then(Value::as_object_mut) {
            for n in names {
                section.remove(n);
            }
        }
        self.write_root(path, &root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{McpConfig, StdioConfig};

    fn tmp(name: &str) -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("mux-json-{}-{}.json", name, std::process::id()));
        d
    }

    fn git() -> McpConfig {
        McpConfig::Stdio(StdioConfig { command: "npx".into(), args: Some(vec!["-y".into()]), env: None })
    }

    #[test]
    fn upsert_then_read_roundtrips() {
        let p = tmp("rt");
        let adapter = JsonAdapter::new("mcpServers");
        adapter.upsert(&p, "git", &git()).unwrap();
        assert!(adapter.read(&p).contains_key("git"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn upsert_preserves_other_keys_and_servers() {
        // The data-loss regression test: a user file with a sibling server AND a
        // non-conforming server (no command/url) must keep BOTH after we add one.
        let p = tmp("preserve");
        std::fs::write(
            &p,
            r#"{"otherKey":42,"mcpServers":{
                "mine":{"command":"my-tool","args":["x"],"cwd":"/tmp","disabled":false},
                "weird":{"url":"https://x","transport":"streamable-http"}
            }}"#,
        )
        .unwrap();
        let adapter = JsonAdapter::new("mcpServers");
        adapter.upsert(&p, "git", &git()).unwrap();

        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        // Sibling top-level key preserved.
        assert_eq!(v["otherKey"], 42);
        let servers = v["mcpServers"].as_object().unwrap();
        // All three present; the user's servers kept their raw, un-modelled fields.
        assert!(servers.contains_key("git"));
        assert_eq!(servers["mine"]["cwd"], "/tmp");
        assert_eq!(servers["mine"]["disabled"], false);
        assert_eq!(servers["weird"]["transport"], "streamable-http");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn remove_keeps_other_servers_including_non_conforming() {
        let p = tmp("rm");
        std::fs::write(
            &p,
            r#"{"mcpServers":{"git":{"command":"npx"},"weird":{"url":"https://x"}}}"#,
        )
        .unwrap();
        let adapter = JsonAdapter::new("mcpServers");
        adapter.remove(&p, &["git".to_string()]).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let servers = v["mcpServers"].as_object().unwrap();
        assert!(!servers.contains_key("git"));
        assert!(servers.contains_key("weird")); // non-conforming sibling survives
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn read_missing_file_is_empty() {
        let adapter = JsonAdapter::new("mcpServers");
        assert!(adapter.read(Path::new("/nonexistent/xyz.json")).is_empty());
    }

    #[test]
    fn read_skips_unparseable_entries_but_keeps_good_ones() {
        let p = tmp("mixed");
        std::fs::write(
            &p,
            r#"{"mcpServers":{"good":{"command":"npx"},"weird":{"foo":"bar"}}}"#,
        )
        .unwrap();
        let back = JsonAdapter::new("mcpServers").read(&p);
        assert!(back.contains_key("good"));
        assert!(!back.contains_key("weird"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn remove_missing_file_is_noop() {
        let p = tmp("rm-noop");
        let _ = std::fs::remove_file(&p);
        let adapter = JsonAdapter::new("mcpServers");
        adapter.remove(&p, &["git".to_string()]).unwrap();
        assert!(!p.exists());
    }
}
