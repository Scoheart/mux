use crate::adapter::Adapter;
use crate::types::McpConfig;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use toml::Value as Toml;

pub struct TomlAdapter {
    pub key: String,
}

impl TomlAdapter {
    // Note (intentional divergence from TS): the TS TomlAdapter hard-codes the
    // section name `mcp_servers`, whereas this Rust adapter honors the `key`
    // passed by the caller. The Rust behavior is the more correct/general one.
    pub fn new(key: &str) -> Self {
        Self { key: key.to_string() }
    }

    fn read_root(&self, path: &Path) -> Toml {
        fs::read_to_string(path)
            .ok()
            .and_then(|c| c.parse::<Toml>().ok())
            .filter(Toml::is_table)
            .unwrap_or_else(|| Toml::Table(toml::map::Map::new()))
    }

    fn write_root(&self, path: &Path, root: &Toml) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let serialized = toml::to_string_pretty(root).map_err(|e| e.to_string())?;
        fs::write(path, serialized).map_err(|e| e.to_string())
    }
}

impl Adapter for TomlAdapter {
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig> {
        let root = self.read_root(path);
        let Some(section) = root.get(&self.key).and_then(Toml::as_table) else {
            return BTreeMap::new();
        };
        // Per-entry parse (toml -> json -> McpConfig); skip non-conforming entries
        // rather than discarding the whole section.
        section
            .iter()
            .filter_map(|(name, val)| {
                serde_json::to_value(val)
                    .ok()
                    .and_then(|j| serde_json::from_value::<McpConfig>(j).ok())
                    .map(|cfg| (name.clone(), cfg))
            })
            .collect()
    }

    fn upsert(&self, path: &Path, name: &str, cfg: &McpConfig) -> Result<(), String> {
        let mut root = self.read_root(path);
        let cfg_toml: Toml = serde_json::to_value(cfg)
            .map_err(|e| e.to_string())
            .and_then(|j| serde_json::from_value(j).map_err(|e| e.to_string()))?;
        let table = root.as_table_mut().expect("root is a table");
        if !table.get(&self.key).map(Toml::is_table).unwrap_or(false) {
            table.insert(self.key.clone(), Toml::Table(toml::map::Map::new()));
        }
        if let Some(Toml::Table(section)) = table.get_mut(&self.key) {
            section.insert(name.to_string(), cfg_toml);
        }
        self.write_root(path, &root)
    }

    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let mut root = self.read_root(path);
        if let Some(Toml::Table(section)) = root.as_table_mut().and_then(|t| t.get_mut(&self.key)) {
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
    use crate::types::{McpConfig, StdioConfig};

    fn tmp(name: &str) -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("mux-toml-{}-{}.toml", name, std::process::id()));
        d
    }

    fn stdio(cmd: &str) -> McpConfig {
        McpConfig::Stdio(StdioConfig { command: cmd.into(), args: None, env: None })
    }

    #[test]
    fn upsert_then_read_roundtrips() {
        let p = tmp("rt");
        let adapter = TomlAdapter::new("mcp_servers");
        adapter.upsert(&p, "github", &stdio("npx")).unwrap();
        assert!(adapter.read(&p).contains_key("github"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn upsert_and_remove_preserve_other_servers() {
        let p = tmp("preserve");
        let adapter = TomlAdapter::new("mcp_servers");
        adapter.upsert(&p, "a", &stdio("x")).unwrap();
        adapter.upsert(&p, "b", &stdio("y")).unwrap();
        adapter.remove(&p, &["a".to_string()]).unwrap();
        let back = adapter.read(&p);
        assert!(!back.contains_key("a"));
        assert!(back.contains_key("b"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn upsert_keeps_unmodelled_fields_on_other_servers() {
        let p = tmp("extra");
        std::fs::write(
            &p,
            "[mcp_servers.mine]\ncommand = \"my-tool\"\ncwd = \"/tmp\"\n",
        )
        .unwrap();
        TomlAdapter::new("mcp_servers").upsert(&p, "github", &stdio("npx")).unwrap();
        let root: Toml = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        let servers = root.get("mcp_servers").unwrap().as_table().unwrap();
        assert!(servers.contains_key("github"));
        assert_eq!(servers["mine"]["cwd"].as_str(), Some("/tmp")); // preserved
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn remove_missing_file_is_noop() {
        let p = tmp("rm-noop");
        let _ = std::fs::remove_file(&p);
        let adapter = TomlAdapter::new("mcp_servers");
        adapter.remove(&p, &["a".to_string()]).unwrap();
        assert!(!p.exists());
    }
}
