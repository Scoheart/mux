use crate::adapter::Adapter;
use crate::codec::{Codec, ObjectPatch};
use crate::safe_write::{write_if_unchanged, write_if_unchanged_with_settings_lock};
use crate::types::McpConfig;
use jsonc_parser::cst::{CstInputValue, CstNode, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub struct JsonAdapter {
    pub key: String,
    codec: Codec,
    root_defaults: BTreeMap<String, Value>,
}

impl JsonAdapter {
    pub fn new(key: &str) -> Self {
        Self::with_codec(key, Codec::Standard)
    }

    pub fn with_codec(key: &str, codec: Codec) -> Self {
        Self::with_spec(key, codec, BTreeMap::new())
    }

    pub fn with_spec(key: &str, codec: Codec, root_defaults: BTreeMap<String, Value>) -> Self {
        Self {
            key: key.to_string(),
            codec,
            root_defaults,
        }
    }

    fn read_document(&self, path: &Path) -> Result<(CstRootNode, Option<String>), String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(format!("failed to read {}: {}", path.display(), e)),
        };
        let root = CstRootNode::parse(
            text.as_deref().unwrap_or_default(),
            &ParseOptions::default(),
        )
        .map_err(|e| {
            format!(
                "refusing to modify invalid JSON/JSONC at {}: {}",
                path.display(),
                e
            )
        })?;
        Ok((root, text))
    }

    fn write_document(
        &self,
        path: &Path,
        root: &CstRootNode,
        original: Option<&str>,
    ) -> Result<(), String> {
        let content = root.to_string();
        if self.codec == Codec::Cline {
            write_if_unchanged_with_settings_lock(path, original, &content)
        } else {
            write_if_unchanged(path, original, &content)
        }
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

    fn property_count(object: &CstObject, name: &str) -> usize {
        object
            .properties()
            .into_iter()
            .filter(|property| {
                property
                    .name()
                    .and_then(|name| name.decoded_value().ok())
                    .is_some_and(|decoded| decoded == name)
            })
            .count()
    }

    fn ensure_unique_property(
        object: &CstObject,
        name: &str,
        path: &Path,
        context: &str,
    ) -> Result<(), String> {
        if Self::property_count(object, name) > 1 {
            return Err(format!(
                "refusing to modify {}: duplicate JSON key '{}.{}' is ambiguous",
                path.display(),
                context,
                name
            ));
        }
        Ok(())
    }

    fn ensure_unique_keys(object: &CstObject, path: &Path, context: &str) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for property in object.properties() {
            let Some(name) = property.name().and_then(|name| name.decoded_value().ok()) else {
                continue;
            };
            if !seen.insert(name.clone()) {
                return Err(format!(
                    "refusing to modify {}: duplicate JSON key '{}.{}' is ambiguous",
                    path.display(),
                    context,
                    name
                ));
            }
        }
        Ok(())
    }

    fn ensure_unique_nested_keys(node: &CstNode, path: &Path, context: &str) -> Result<(), String> {
        if let Some(object) = node.as_object() {
            Self::ensure_unique_keys(&object, path, context)?;
            for property in object.properties() {
                let name = property
                    .name()
                    .and_then(|name| name.decoded_value().ok())
                    .unwrap_or_else(|| "<unknown>".into());
                if let Some(value) = property.value() {
                    Self::ensure_unique_nested_keys(
                        &value,
                        path,
                        &format!("{}.{}", context, name),
                    )?;
                }
            }
        } else if let Some(array) = node.as_array() {
            for (index, element) in array.elements().into_iter().enumerate() {
                Self::ensure_unique_nested_keys(
                    &element,
                    path,
                    &format!("{}[{}]", context, index),
                )?;
            }
        }
        Ok(())
    }

    fn patch_nested_object(
        target: &CstObject,
        patch: ObjectPatch,
        path: &Path,
        context: &str,
    ) -> Result<(), String> {
        Self::ensure_unique_property(target, patch.parent, path, context)?;
        if target.get(patch.parent).is_none() && patch.fields.is_empty() {
            return Ok(());
        }
        let nested = target.object_value_or_create(patch.parent).ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}.{}' is not an object",
                path.display(),
                context,
                patch.parent
            )
        })?;
        Self::ensure_unique_keys(&nested, path, &format!("{}.{}", context, patch.parent))?;
        for field in patch.controlled {
            if let Some((_, value)) = patch.fields.iter().find(|(name, _)| name == field) {
                if let Some(property) = nested.get(field) {
                    property.set_value(Self::input_value(value.clone()));
                } else {
                    nested.append(field, Self::input_value(value.clone()));
                }
            } else if let Some(property) = nested.get(field) {
                property.remove();
            }
        }
        Ok(())
    }

    fn new_entry_value(mut patch: crate::codec::EntryPatch) -> Value {
        patch.fields.extend(patch.defaults);
        let mut fields: serde_json::Map<String, Value> = patch.fields.into_iter().collect();
        for nested in patch.object_patches {
            if !nested.fields.is_empty() {
                fields.insert(
                    nested.parent.into(),
                    Value::Object(nested.fields.into_iter().collect()),
                );
            }
        }
        Value::Object(fields)
    }
}

impl Adapter for JsonAdapter {
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig> {
        let Ok((root, _)) = self.read_document(path) else {
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
                    .and_then(|value| self.codec.decode(&value))
                    .map(|cfg| (name, cfg))
            })
            .collect()
    }

    fn upsert(&self, path: &Path, name: &str, cfg: &McpConfig) -> Result<(), String> {
        let (root, original) = self.read_document(path)?;
        let object = root.object_value_or_create().ok_or_else(|| {
            format!(
                "refusing to modify {}: JSON root is not an object",
                path.display()
            )
        })?;
        if original.is_none() {
            for (field, value) in &self.root_defaults {
                if object.get(field).is_none() {
                    object.append(field, Self::input_value(value.clone()));
                }
            }
        }
        Self::ensure_unique_property(&object, &self.key, path, "$root")?;
        let section = object.object_value_or_create(&self.key).ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}' is not an object",
                path.display(),
                self.key
            )
        })?;
        Self::ensure_unique_keys(&section, path, &self.key)?;
        let patch = self.codec.patch(cfg)?;
        if let Some(property) = section.get(name) {
            if let Some(value) = property.value() {
                Self::ensure_unique_nested_keys(&value, path, &format!("{}.{}", self.key, name))?;
            }
            let target = property.object_value().ok_or_else(|| {
                format!(
                    "refusing to modify {}: '{}.{}' is not an object",
                    path.display(),
                    self.key,
                    name
                )
            })?;
            for field in patch.controlled {
                Self::ensure_unique_property(
                    &target,
                    field,
                    path,
                    &format!("{}.{}", self.key, name),
                )?;
            }
            for field in patch.controlled {
                if let Some((_, value)) = patch.fields.iter().find(|(name, _)| name == field) {
                    if let Some(property) = target.get(field) {
                        property.set_value(Self::input_value(value.clone()));
                    } else {
                        target.append(field, Self::input_value(value.clone()));
                    }
                } else if let Some(property) = target.get(field) {
                    property.remove();
                }
            }
            for nested in patch.object_patches {
                Self::patch_nested_object(
                    &target,
                    nested,
                    path,
                    &format!("{}.{}", self.key, name),
                )?;
            }
            for (field, value) in patch.defaults {
                if target.get(&field).is_none() {
                    target.append(&field, Self::input_value(value));
                }
            }
        } else {
            section.append(name, Self::input_value(Self::new_entry_value(patch)));
        }
        self.write_document(path, &root, original.as_deref())
    }

    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let (root, original) = self.read_document(path)?;
        let object = root.object_value().ok_or_else(|| {
            format!(
                "refusing to modify {}: JSON root is not an object",
                path.display()
            )
        })?;
        Self::ensure_unique_property(&object, &self.key, path, "$root")?;
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
        Self::ensure_unique_keys(&section, path, &self.key)?;
        let mut changed = false;
        for name in names {
            if let Some(property) = section.get(name) {
                property.remove();
                changed = true;
            }
        }
        if changed {
            self.write_document(path, &root, original.as_deref())?;
        }
        Ok(())
    }

    fn snapshot(&self, path: &Path, name: &str) -> Result<Option<Value>, String> {
        let (root, _) = self.read_document(path)?;
        let object = root.object_value().ok_or_else(|| {
            format!(
                "refusing to read {}: JSON root is not an object",
                path.display()
            )
        })?;
        Self::ensure_unique_property(&object, &self.key, path, "$root")?;
        let Some(section_property) = object.get(&self.key) else {
            return Ok(None);
        };
        let section = section_property.object_value().ok_or_else(|| {
            format!(
                "refusing to read {}: '{}' is not an object",
                path.display(),
                self.key
            )
        })?;
        Self::ensure_unique_keys(&section, path, &self.key)?;
        let Some(property) = section.get(name) else {
            return Ok(None);
        };
        let node = property.value().ok_or_else(|| {
            format!(
                "refusing to read {}: '{}.{}' has no JSON value",
                path.display(),
                self.key,
                name
            )
        })?;
        Self::ensure_unique_nested_keys(&node, path, &format!("{}.{}", self.key, name))?;
        let value = property.to_serde_value().ok_or_else(|| {
            format!(
                "refusing to read {}: '{}.{}' has no valid JSON value",
                path.display(),
                self.key,
                name
            )
        })?;
        if !value.is_object() {
            return Err(format!(
                "refusing to read {}: '{}.{}' is not an object",
                path.display(),
                self.key,
                name
            ));
        }
        Ok(Some(value))
    }

    fn remove_snapshot(&self, path: &Path, name: &str, snapshot: &Value) -> Result<(), String> {
        let (root, original) = self.read_document(path)?;
        let object = root.object_value().ok_or_else(|| {
            format!(
                "refusing to modify {}: JSON root is not an object",
                path.display()
            )
        })?;
        Self::ensure_unique_property(&object, &self.key, path, "$root")?;
        let section = object.object_value(&self.key).ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}' is missing or not an object",
                path.display(),
                self.key
            )
        })?;
        Self::ensure_unique_keys(&section, path, &self.key)?;
        let property = section.get(name).ok_or_else(|| {
            format!(
                "refusing to remove {}: '{}.{}' no longer exists",
                path.display(),
                self.key,
                name
            )
        })?;
        let node = property.value().ok_or_else(|| {
            format!(
                "refusing to remove {}: '{}.{}' has no JSON value",
                path.display(),
                self.key,
                name
            )
        })?;
        Self::ensure_unique_nested_keys(&node, path, &format!("{}.{}", self.key, name))?;
        let current = property.to_serde_value().ok_or_else(|| {
            format!(
                "refusing to remove {}: '{}.{}' has no valid JSON value",
                path.display(),
                self.key,
                name
            )
        })?;
        if &current != snapshot {
            return Err(format!(
                "refusing to remove {}: '{}.{}' changed after its snapshot was saved",
                path.display(),
                self.key,
                name
            ));
        }
        property.remove();
        self.write_document(path, &root, original.as_deref())
    }

    fn restore(&self, path: &Path, name: &str, snapshot: &Value) -> Result<(), String> {
        if !snapshot.is_object() {
            return Err("refusing to restore a non-object MCP snapshot".into());
        }
        let (root, original) = self.read_document(path)?;
        let object = root.object_value_or_create().ok_or_else(|| {
            format!(
                "refusing to modify {}: JSON root is not an object",
                path.display()
            )
        })?;
        Self::ensure_unique_property(&object, &self.key, path, "$root")?;
        let section = object.object_value_or_create(&self.key).ok_or_else(|| {
            format!(
                "refusing to modify {}: '{}' is not an object",
                path.display(),
                self.key
            )
        })?;
        Self::ensure_unique_keys(&section, path, &self.key)?;
        if section.get(name).is_some() {
            return Err(format!(
                "refusing to restore {}: '{}.{}' already exists",
                path.display(),
                self.key,
                name
            ));
        }
        section.append(name, Self::input_value(snapshot.clone()));
        self.write_document(path, &root, original.as_deref())
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
            cwd: None,
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
            "{{\n  \"mcpServers\": {{\n    \"git\": {{\"command\":\"old\"}},\n    {sibling}\n  }},\n  \"account\": {{\"id\":7}}\n}}\n"
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
    fn duplicate_mcp_section_is_never_modified() {
        let p = tmp("duplicate-section");
        let original =
            r#"{"mcpServers":{"one":{"command":"one"}},"mcpServers":{"two":{"command":"two"}}}"#;
        std::fs::write(&p, original).unwrap();

        let result = JsonAdapter::new("mcpServers").upsert(&p, "git", &git());

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn duplicate_server_name_is_never_modified_or_removed() {
        let p = tmp("duplicate-server");
        let original = r#"{"mcpServers":{"git":{"command":"one"},"git":{"command":"two"}}}"#;
        std::fs::write(&p, original).unwrap();
        let adapter = JsonAdapter::new("mcpServers");

        assert!(adapter.upsert(&p, "git", &git()).is_err());
        assert!(adapter.remove(&p, &["git".to_string()]).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn duplicate_controlled_field_is_never_modified() {
        let p = tmp("duplicate-field");
        let original =
            r#"{"mcpServers":{"git":{"command":"one","command":"two","disabled":false}}}"#;
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
    fn snapshot_restore_preserves_agent_owned_fields() {
        let p = tmp("snapshot-restore");
        let original = r#"{
  "account": {"token": "private"},
  "mcpServers": {
    "git": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "git"],
      "enabled": false,
      "timeout": 120,
      "oauth": {"clientId": "client"},
      "allowedTools": ["read"]
    },
    "sibling": {"command": "keep"}
  }
}
"#;
        std::fs::write(&p, original).unwrap();
        let adapter = JsonAdapter::new("mcpServers");

        let snapshot = adapter.snapshot(&p, "git").unwrap().unwrap();
        adapter.remove(&p, &["git".to_string()]).unwrap();
        adapter.restore(&p, "git", &snapshot).unwrap();

        let written: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(written["account"]["token"], "private");
        assert_eq!(written["mcpServers"]["sibling"]["command"], "keep");
        assert_eq!(written["mcpServers"]["git"], snapshot);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn restore_refuses_to_overwrite_recreated_entry() {
        let p = tmp("snapshot-conflict");
        let original = r#"{"mcpServers":{"git":{"command":"newer"}}}"#;
        std::fs::write(&p, original).unwrap();
        let snapshot = serde_json::json!({"command": "older", "enabled": false});

        let result = JsonAdapter::new("mcpServers").restore(&p, "git", &snapshot);

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_refuses_nested_duplicate_policy_fields() {
        let p = tmp("snapshot-duplicate-policy");
        let original = r#"{"mcpServers":{"git":{"command":"npx","oauth":{"clientId":"one","clientId":"two"}}}}"#;
        std::fs::write(&p, original).unwrap();

        let result = JsonAdapter::new("mcpServers").snapshot(&p, "git");

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_removal_refuses_a_changed_target() {
        let p = tmp("snapshot-changed");
        let adapter = JsonAdapter::new("mcpServers");
        std::fs::write(
            &p,
            r#"{"mcpServers":{"git":{"command":"npx","timeout":10}}}"#,
        )
        .unwrap();
        let snapshot = adapter.snapshot(&p, "git").unwrap().unwrap();
        let newer = r#"{"mcpServers":{"git":{"command":"npx","timeout":20}}}"#;
        std::fs::write(&p, newer).unwrap();

        let result = adapter.remove_snapshot(&p, "git", &snapshot);

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), newer);
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
