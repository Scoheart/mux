use crate::adapter::Adapter;
use crate::codec::{Codec, EntryPatch, ObjectPatch};
use crate::safe_write::write_if_unchanged;
use crate::types::McpConfig;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use toml::Value as Toml;
use toml_edit::{ArrayOfTables, Document, Item, Table};

/// Lossless TOML adapter for `[[section]]` arrays whose entries are identified
/// by a field such as `name` (Mistral Vibe).
pub struct TomlListAdapter {
    key: String,
    identity_field: String,
    codec: Codec,
    root_defaults: BTreeMap<String, Value>,
}

impl TomlListAdapter {
    pub fn with_spec(
        key: &str,
        identity_field: &str,
        codec: Codec,
        root_defaults: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            key: key.into(),
            identity_field: identity_field.into(),
            codec,
            root_defaults,
        }
    }

    fn read_document(&self, path: &Path) -> Result<(Document, Option<String>), String> {
        match fs::read_to_string(path) {
            Ok(text) => text
                .parse::<Document>()
                .map(|document| (document, Some(text)))
                .map_err(|error| {
                    format!(
                        "refusing to modify invalid TOML at {}: {}",
                        path.display(),
                        error
                    )
                }),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok((Document::new(), None)),
            Err(error) => Err(format!("failed to read {}: {}", path.display(), error)),
        }
    }

    fn write_document(
        &self,
        path: &Path,
        document: &Document,
        original: Option<&str>,
    ) -> Result<(), String> {
        write_if_unchanged(path, original, &document.to_string())
    }

    fn fields_table(fields: Vec<(String, Value)>) -> Result<Table, String> {
        let value = Value::Object(fields.into_iter().collect());
        toml_edit::ser::to_document(&value)
            .map(|document| document.as_table().clone())
            .map_err(|error| error.to_string())
    }

    fn identity(table: &Table, field: &str) -> Option<String> {
        table
            .get(field)
            .and_then(Item::as_value)
            .and_then(toml_edit::Value::as_str)
            .map(str::to_string)
    }

    fn ensure_unique_identities(&self, section: &ArrayOfTables, path: &Path) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for table in section.iter() {
            let Some(name) = Self::identity(table, &self.identity_field) else {
                continue;
            };
            if !seen.insert(name.clone()) {
                return Err(format!(
                    "refusing to modify {}: duplicate TOML identity '{}.{}' is ambiguous",
                    path.display(),
                    self.key,
                    name
                ));
            }
        }
        Ok(())
    }

    fn section_mut<'a>(
        &self,
        document: &'a mut Document,
        path: &Path,
        create: bool,
    ) -> Result<Option<&'a mut ArrayOfTables>, String> {
        if !document.as_table().contains_key(&self.key) && create {
            document
                .as_table_mut()
                .insert(&self.key, Item::ArrayOfTables(ArrayOfTables::new()));
        }
        match document.as_table_mut().get_mut(&self.key) {
            Some(Item::ArrayOfTables(section)) => Ok(Some(section)),
            None => Ok(None),
            Some(_) => Err(format!(
                "refusing to modify {}: '{}' is not a TOML array of tables",
                path.display(),
                self.key
            )),
        }
    }

    fn patch_nested_object(target: &mut Table, patch: ObjectPatch) -> Result<(), String> {
        if !target.contains_key(patch.parent) && patch.fields.is_empty() {
            return Ok(());
        }
        if !target.contains_key(patch.parent) {
            target.insert(patch.parent, Item::Table(Table::new()));
        }
        let nested = target
            .get_mut(patch.parent)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| format!("'{}' is not a TOML table", patch.parent))?;
        let fields = Self::fields_table(patch.fields)?;
        for field in patch.controlled {
            if let Some(value) = fields.get(field).cloned() {
                nested.insert(field, value);
            } else {
                nested.remove(field);
            }
        }
        Ok(())
    }

    fn patch_existing(
        &self,
        target: &mut Table,
        name: &str,
        patch: EntryPatch,
    ) -> Result<(), String> {
        let fields = Self::fields_table(patch.fields)?;
        for field in patch.controlled {
            if let Some(value) = fields.get(field).cloned() {
                target.insert(field, value);
            } else {
                target.remove(field);
            }
        }
        let defaults = Self::fields_table(patch.defaults)?;
        for (field, value) in defaults.iter() {
            if !target.contains_key(field) {
                target.insert(field, value.clone());
            }
        }
        for nested in patch.object_patches {
            Self::patch_nested_object(target, nested)?;
        }
        if !target.contains_key(&self.identity_field) {
            target.insert(&self.identity_field, toml_edit::value(name));
        }
        Ok(())
    }

    fn new_table(&self, name: &str, mut patch: EntryPatch) -> Result<Table, String> {
        patch.fields.extend(patch.defaults);
        for nested in patch.object_patches {
            if !nested.fields.is_empty() {
                patch.fields.push((
                    nested.parent.into(),
                    Value::Object(nested.fields.into_iter().collect()),
                ));
            }
        }
        patch
            .fields
            .push((self.identity_field.clone(), Value::String(name.into())));
        Self::fields_table(patch.fields)
    }

    fn apply_root_defaults(&self, document: &mut Document) -> Result<(), String> {
        let defaults = Self::fields_table(
            self.root_defaults
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )?;
        for (field, value) in defaults.iter() {
            if !document.as_table().contains_key(field) {
                document.as_table_mut().insert(field, value.clone());
            }
        }
        Ok(())
    }

    fn semantic_entries(&self, document: &Document) -> Result<Vec<Value>, String> {
        let semantic =
            toml::from_str::<Toml>(&document.to_string()).map_err(|error| error.to_string())?;
        let Some(entries) = semantic.get(&self.key).and_then(Toml::as_array) else {
            return Ok(Vec::new());
        };
        entries
            .iter()
            .map(|entry| serde_json::to_value(entry).map_err(|error| error.to_string()))
            .collect()
    }
}

impl Adapter for TomlListAdapter {
    fn read(&self, path: &Path) -> BTreeMap<String, McpConfig> {
        let Ok((document, _)) = self.read_document(path) else {
            return BTreeMap::new();
        };
        let Some(Item::ArrayOfTables(section)) = document.as_table().get(&self.key) else {
            return BTreeMap::new();
        };
        if self.ensure_unique_identities(section, path).is_err() {
            return BTreeMap::new();
        }
        self.semantic_entries(&document)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let name = entry
                    .as_object()?
                    .get(&self.identity_field)?
                    .as_str()?
                    .to_string();
                self.codec.decode(&entry).map(|config| (name, config))
            })
            .collect()
    }

    fn upsert(&self, path: &Path, name: &str, config: &McpConfig) -> Result<(), String> {
        let (mut document, original) = self.read_document(path)?;
        if original.is_none() {
            self.apply_root_defaults(&mut document)?;
        }
        let patch = self.codec.patch(config)?;
        let section = self
            .section_mut(&mut document, path, true)?
            .expect("created TOML array of tables");
        self.ensure_unique_identities(section, path)?;
        let indexes: Vec<_> = section
            .iter()
            .enumerate()
            .filter_map(|(index, table)| {
                (Self::identity(table, &self.identity_field).as_deref() == Some(name))
                    .then_some(index)
            })
            .collect();
        if indexes.len() > 1 {
            return Err(format!("duplicate TOML identity '{}.{}'", self.key, name));
        }
        if let Some(index) = indexes.first().copied() {
            self.patch_existing(section.get_mut(index).expect("existing table"), name, patch)?;
        } else {
            section.push(self.new_table(name, patch)?);
        }
        self.write_document(path, &document, original.as_deref())
    }

    fn remove(&self, path: &Path, names: &[String]) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let (mut document, original) = self.read_document(path)?;
        let Some(section) = self.section_mut(&mut document, path, false)? else {
            return Ok(());
        };
        self.ensure_unique_identities(section, path)?;
        let mut indexes: Vec<_> = section
            .iter()
            .enumerate()
            .filter_map(|(index, table)| {
                let identity = Self::identity(table, &self.identity_field)?;
                names.contains(&identity).then_some(index)
            })
            .collect();
        indexes.sort_unstable_by(|left, right| right.cmp(left));
        if indexes.is_empty() {
            return Ok(());
        }
        for index in indexes {
            section.remove(index);
        }
        self.write_document(path, &document, original.as_deref())
    }

    fn snapshot(&self, path: &Path, name: &str) -> Result<Option<Value>, String> {
        let (document, _) = self.read_document(path)?;
        let mut matches = self
            .semantic_entries(&document)?
            .into_iter()
            .filter(|entry| {
                entry
                    .as_object()
                    .and_then(|object| object.get(&self.identity_field))
                    .and_then(Value::as_str)
                    == Some(name)
            });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(format!("duplicate TOML identity '{}.{}'", self.key, name));
        }
        Ok(first)
    }

    fn remove_snapshot(&self, path: &Path, name: &str, snapshot: &Value) -> Result<(), String> {
        let current = self.snapshot(path, name)?.ok_or_else(|| {
            format!(
                "refusing to remove {}: '{}.{}' no longer exists",
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
        self.remove(path, &[name.to_string()])
    }

    fn restore(&self, path: &Path, name: &str, snapshot: &Value) -> Result<(), String> {
        let Some(mut fields) = snapshot.as_object().cloned() else {
            return Err("refusing to restore a non-table TOML snapshot".into());
        };
        if self.snapshot(path, name)?.is_some() {
            return Err(format!(
                "refusing to restore {}: '{}.{}' already exists",
                path.display(),
                self.key,
                name
            ));
        }
        fields
            .entry(self.identity_field.clone())
            .or_insert_with(|| Value::String(name.into()));
        let (mut document, original) = self.read_document(path)?;
        if original.is_none() {
            self.apply_root_defaults(&mut document)?;
        }
        let section = self
            .section_mut(&mut document, path, true)?
            .expect("created TOML array of tables");
        self.ensure_unique_identities(section, path)?;
        section.push(Self::fields_table(fields.into_iter().collect())?);
        self.write_document(path, &document, original.as_deref())
    }
}
