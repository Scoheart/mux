use crate::adapter::Adapter;
use crate::types::McpConfig;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use toml::Value as Toml;
use toml_edit::{Document, Item, Table};

pub struct TomlAdapter {
    pub key: String,
}

impl TomlAdapter {
    // Note (intentional divergence from TS): the TS TomlAdapter hard-codes the
    // section name `mcp_servers`, whereas this Rust adapter honors the `key`
    // passed by the caller. The Rust behavior is the more correct/general one.
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
        }
    }

    fn read_root(&self, path: &Path) -> Toml {
        fs::read_to_string(path)
            .ok()
            .and_then(|c| c.parse::<Toml>().ok())
            .filter(Toml::is_table)
            .unwrap_or_else(|| Toml::Table(toml::map::Map::new()))
    }

    fn read_document(&self, path: &Path) -> Result<Document, String> {
        match fs::read_to_string(path) {
            Ok(text) => text.parse::<Document>().map_err(|e| {
                format!(
                    "refusing to modify invalid TOML at {}: {}",
                    path.display(),
                    e
                )
            }),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Document::new()),
            Err(e) => Err(format!("failed to read {}: {}", path.display(), e)),
        }
    }

    fn write_document(&self, path: &Path, document: &Document) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, document.to_string()).map_err(|e| e.to_string())
    }

    fn config_item(cfg: &McpConfig) -> Result<Item, String> {
        toml_edit::ser::to_document(cfg)
            .map(|document| Item::Table(document.as_table().clone()))
            .map_err(|e| e.to_string())
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
        let mut document = self.read_document(path)?;
        if !document.as_table().contains_key(&self.key) {
            document
                .as_table_mut()
                .insert(&self.key, Item::Table(Table::new()));
        }
        let section = document
            .as_table_mut()
            .get_mut(&self.key)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| {
                format!(
                    "refusing to modify {}: '{}' is not a TOML table",
                    path.display(),
                    self.key
                )
            })?;
        section.insert(name, Self::config_item(cfg)?);
        self.write_document(path, &document)
    }

    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let mut document = self.read_document(path)?;
        let Some(section_item) = document.as_table_mut().get_mut(&self.key) else {
            return Ok(());
        };
        let section = section_item.as_table_mut().ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}' is not a TOML table",
                path.display(),
                self.key
            )
        })?;
        let mut changed = false;
        for name in names {
            changed |= section.remove(name).is_some();
        }
        if changed {
            self.write_document(path, &document)?;
        }
        Ok(())
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
        McpConfig::Stdio(StdioConfig {
            command: cmd.into(),
            args: None,
            env: None,
        })
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
        TomlAdapter::new("mcp_servers")
            .upsert(&p, "github", &stdio("npx"))
            .unwrap();
        let root: Toml = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        let servers = root.get("mcp_servers").unwrap().as_table().unwrap();
        assert!(servers.contains_key("github"));
        assert_eq!(servers["mine"]["cwd"].as_str(), Some("/tmp")); // preserved
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn upsert_only_edits_mcp_table_and_preserves_private_toml() {
        let p = tmp("private");
        let original = r#"# Private agent settings must remain byte-for-byte intact.
model = "gpt-private" # keep inline comment

[history]
persistence = "save-all"

# Existing user-managed MCP.
[mcp_servers.mine]
command = "my-tool"
cwd = "/tmp" # unmodelled field
"#;
        std::fs::write(&p, original).unwrap();

        TomlAdapter::new("mcp_servers")
            .upsert(&p, "github", &stdio("npx"))
            .unwrap();

        let written = std::fs::read_to_string(&p).unwrap();
        assert!(written.starts_with(original));
        assert!(written.contains("[mcp_servers.github]"));
        assert!(written.contains("command = \"npx\""));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn invalid_toml_is_never_overwritten() {
        let p = tmp("invalid");
        let original = "token = \"secret\"\n[mcp_servers.git\ncommand = \"broken\"\n";
        std::fs::write(&p, original).unwrap();

        let result = TomlAdapter::new("mcp_servers").upsert(&p, "github", &stdio("npx"));

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn non_table_mcp_section_is_never_replaced() {
        let p = tmp("wrong-section");
        let original = "token = \"secret\"\nmcp_servers = \"managed elsewhere\"\n";
        std::fs::write(&p, original).unwrap();

        let result = TomlAdapter::new("mcp_servers").upsert(&p, "github", &stdio("npx"));

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
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

    #[test]
    fn removing_absent_server_does_not_rewrite_file() {
        let p = tmp("rm-absent");
        let original =
            "# keep formatting\nmodel = 'private'\n\n[mcp_servers.mine]\ncommand = 'x'\n";
        std::fs::write(&p, original).unwrap();

        TomlAdapter::new("mcp_servers")
            .remove(&p, &["missing".to_string()])
            .unwrap();

        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }
}
