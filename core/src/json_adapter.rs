use crate::adapter::Adapter;
use crate::types::McpConfig;
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub struct JsonAdapter {
    pub key: String,
}

impl JsonAdapter {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
        }
    }

    fn read_document(&self, path: &Path) -> Result<CstRootNode, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("failed to read {}: {}", path.display(), e)),
        };
        CstRootNode::parse(&text, &ParseOptions::default()).map_err(|e| {
            format!(
                "refusing to modify invalid JSON/JSONC at {}: {}",
                path.display(),
                e
            )
        })
    }

    fn write_document(&self, path: &Path, root: &CstRootNode) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, root.to_string()).map_err(|e| e.to_string())
    }

    fn input_value(value: Value) -> CstInputValue {
        match value {
            Value::Null => CstInputValue::Null,
            Value::Bool(value) => CstInputValue::Bool(value),
            Value::Number(value) => CstInputValue::Number(value.to_string()),
            Value::String(value) => CstInputValue::String(value),
            Value::Array(values) => {
                CstInputValue::Array(values.into_iter().map(Self::input_value).collect())
            }
            Value::Object(values) => CstInputValue::Object(
                values
                    .into_iter()
                    .map(|(name, value)| (name, Self::input_value(value)))
                    .collect(),
            ),
        }
    }
}

impl Adapter for JsonAdapter {
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig> {
        let Ok(root) = self.read_document(path) else {
            return BTreeMap::new();
        };
        let Some(section) = root
            .object_value()
            .and_then(|object| object.object_value(&self.key))
        else {
            return BTreeMap::new();
        };
        section
            .properties()
            .into_iter()
            .filter_map(|property| {
                let name = property.name()?.decoded_value().ok()?;
                property
                    .to_serde_value()
                    .and_then(|value| serde_json::from_value::<McpConfig>(value).ok())
                    .map(|cfg| (name, cfg))
            })
            .collect()
    }

    fn upsert(&self, path: &Path, name: &str, cfg: &McpConfig) -> Result<(), String> {
        let root = self.read_document(path)?;
        let object = root.object_value_or_create().ok_or_else(|| {
            format!(
                "refusing to modify {}: JSON root is not an object",
                path.display()
            )
        })?;
        let section = object.object_value_or_create(&self.key).ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}' is not an object",
                path.display(),
                self.key
            )
        })?;
        let value = serde_json::to_value(cfg)
            .map(Self::input_value)
            .map_err(|e| e.to_string())?;
        if let Some(property) = section.get(name) {
            property.set_value(value);
        } else {
            section.append(name, value);
        }
        self.write_document(path, &root)
    }

    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let root = self.read_document(path)?;
        let object = root.object_value().ok_or_else(|| {
            format!(
                "refusing to modify {}: JSON root is not an object",
                path.display()
            )
        })?;
        let Some(section_property) = object.get(&self.key) else {
            return Ok(());
        };
        let section = section_property.object_value().ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}' is not an object",
                path.display(),
                self.key
            )
        })?;
        let mut changed = false;
        for name in names {
            if let Some(property) = section.get(name) {
                property.remove();
                changed = true;
            }
        }
        if changed {
            self.write_document(path, &root)?;
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
        d.push(format!("mux-json-{}-{}.json", name, std::process::id()));
        d
    }

    fn git() -> McpConfig {
        McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into()]),
            env: None,
        })
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
    fn upsert_only_edits_mcp_section_and_preserves_jsonc() {
        let p = tmp("jsonc-private");
        let original = r#"{
  // Private agent settings must never be rewritten or exposed.
  "account": { "token" : "secret", "flags": [1,  2] },
  "mcpServers": {
    // Existing user-managed server.
    "mine": { "command" : "my-tool", "cwd": "/tmp" },
  },
  "theme": "dark"
}
"#;
        std::fs::write(&p, original).unwrap();

        JsonAdapter::new("mcpServers")
            .upsert(&p, "git", &git())
            .unwrap();

        let written = std::fs::read_to_string(&p).unwrap();
        assert!(written.contains(
            "// Private agent settings must never be rewritten or exposed.\n  \"account\": { \"token\" : \"secret\", \"flags\": [1,  2] },"
        ));
        assert!(written.contains(
            "// Existing user-managed server.\n    \"mine\": { \"command\" : \"my-tool\", \"cwd\": \"/tmp\" },"
        ));
        assert!(written.ends_with("  \"theme\": \"dark\"\n}\n"));
        assert!(JsonAdapter::new("mcpServers").read(&p).contains_key("git"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn updating_one_server_keeps_sibling_bytes() {
        let p = tmp("target-only");
        let sibling = r#""private": { "command" : "keep", "env": {"TOKEN":"secret"} }"#;
        let original = format!(
            "{{\n  \"mcpServers\": {{\n    \"git\": {{\"command\":\"old\"}},\n    {}\n  }},\n  \"account\": {{\"id\":7}}\n}}\n",
            sibling
        );
        std::fs::write(&p, &original).unwrap();

        JsonAdapter::new("mcpServers")
            .upsert(&p, "git", &git())
            .unwrap();

        let written = std::fs::read_to_string(&p).unwrap();
        assert!(written.contains(sibling));
        assert!(written.contains("\"account\": {\"id\":7}"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn invalid_json_is_never_overwritten() {
        let p = tmp("invalid");
        let original = r#"{"private":{"token":"secret"},"mcpServers": "#;
        std::fs::write(&p, original).unwrap();

        let result = JsonAdapter::new("mcpServers").upsert(&p, "git", &git());

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn non_object_mcp_section_is_never_replaced() {
        let p = tmp("wrong-section");
        let original = r#"{"private":{"token":"secret"},"mcpServers":"managed elsewhere"}"#;
        std::fs::write(&p, original).unwrap();

        let result = JsonAdapter::new("mcpServers").upsert(&p, "git", &git());

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
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

    #[test]
    fn removing_absent_server_does_not_rewrite_file() {
        let p = tmp("rm-absent");
        let original = "{\n  // keep formatting\n  \"mcpServers\": {},\n  \"private\": { \"token\" : \"secret\" }\n}\n";
        std::fs::write(&p, original).unwrap();

        JsonAdapter::new("mcpServers")
            .remove(&p, &["missing".to_string()])
            .unwrap();

        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }
}
