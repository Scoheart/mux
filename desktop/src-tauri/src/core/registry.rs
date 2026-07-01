use crate::core::settings::{load_settings, mutate_settings};
use crate::core::types::RegistryEntry;

/// Built-in registry: embedded at compile time from the repo-root data/registry.json
/// (the single shared source of truth, also consumed by the TS CLI). Pointing
/// directly at the root file avoids a hand-synced desktop/data copy drifting.
const BUILTIN_JSON: &str = include_str!("../../../../data/registry.json");

pub fn builtin_registry() -> Vec<RegistryEntry> {
    serde_json::from_str(BUILTIN_JSON).expect("registry.json must be valid")
}

/// Merge user entries over builtins: a user entry shadows the builtin with the
/// same composite key (`name::transport`); user entries are deduped, last wins.
fn merge_with_builtin(user: Vec<RegistryEntry>) -> Vec<RegistryEntry> {
    use std::collections::HashMap;
    let mut user_by_key: HashMap<String, RegistryEntry> = HashMap::new();
    for e in user {
        user_by_key.insert(e.key(), e);
    }
    let user_keys: std::collections::HashSet<_> = user_by_key.keys().cloned().collect();
    let mut result: Vec<RegistryEntry> = builtin_registry()
        .into_iter()
        .filter(|b| !user_keys.contains(&b.key()))
        .collect();
    result.extend(user_by_key.into_values());
    result
}

/// Insert or replace `entry` in the user list, keyed by composite key.
fn upsert(list: &mut Vec<RegistryEntry>, entry: RegistryEntry) {
    let key = entry.key();
    list.retain(|e| e.key() != key);
    list.push(entry);
}

/// Remove the user entry matching `name`+`transport`, if present.
fn remove(list: &mut Vec<RegistryEntry>, name: &str, transport: &str) {
    let target = format!("{}::{}", name, transport);
    list.retain(|e| e.key() != target);
}

/// All registry entries: builtins merged with the user's `settings.registry`.
pub fn read_registry() -> Vec<RegistryEntry> {
    merge_with_builtin(load_settings().registry.unwrap_or_default())
}

/// Persist (create or overwrite) a user registry entry into `settings.registry`.
pub fn write_registry_entry(entry: &RegistryEntry) -> std::io::Result<()> {
    mutate_settings(|s| upsert(s.registry.get_or_insert_with(Vec::new), entry.clone()))
}

/// Composite keys (`name::transport`) of entries that have a user override.
pub fn user_override_keys() -> Vec<String> {
    load_settings()
        .registry
        .unwrap_or_default()
        .iter()
        .map(|e| e.key())
        .collect()
}

/// Remove the user override for `name`+`transport` (reverting to builtin if one
/// exists). A missing entry is a no-op success.
pub fn delete_registry_entry(name: &str, transport: &str) -> std::io::Result<()> {
    mutate_settings(|s| {
        if let Some(list) = s.registry.as_mut() {
            remove(list, name, transport);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{HttpConfig, RegistryConfig, StdioConfig};

    fn stdio(name: &str, cmd: &str) -> RegistryEntry {
        RegistryEntry {
            name: name.into(),
            description: cmd.into(),
            tags: vec![],
            config: RegistryConfig {
                stdio: Some(StdioConfig { command: cmd.into(), args: None, env: None }),
                http: None,
            },
            origin: None,
        }
    }
    fn http(name: &str, url: &str) -> RegistryEntry {
        RegistryEntry {
            name: name.into(),
            description: url.into(),
            tags: vec![],
            config: RegistryConfig {
                stdio: None,
                http: Some(HttpConfig { kind: "http".into(), url: url.into(), headers: None }),
            },
            origin: None,
        }
    }

    #[test]
    fn builtin_loads_40_plus() {
        assert!(builtin_registry().len() >= 40);
    }

    #[test]
    fn user_entry_overrides_builtin() {
        let merged = merge_with_builtin(vec![stdio("filesystem", "custom-cmd")]);
        let fs_entry = merged.iter().find(|e| e.name == "filesystem").unwrap();
        assert_eq!(fs_entry.config.stdio.as_ref().unwrap().command, "custom-cmd");
        // Exactly one filesystem entry (the override replaced the builtin).
        assert_eq!(merged.iter().filter(|e| e.name == "filesystem").count(), 1);
    }

    #[test]
    fn upsert_replaces_same_key_and_delete_reverts() {
        let mut list = vec![stdio("filesystem", "a")];
        upsert(&mut list, stdio("filesystem", "b"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].config.stdio.as_ref().unwrap().command, "b");

        remove(&mut list, "filesystem", "stdio");
        assert!(list.is_empty());
        // Deleting again is a no-op.
        remove(&mut list, "filesystem", "stdio");
        assert!(list.is_empty());
        // And with the override gone, the builtin shows through.
        let merged = merge_with_builtin(list);
        assert!(merged.iter().any(|e| e.name == "filesystem"));
    }

    #[test]
    fn same_name_different_transport_coexist() {
        let mut list = Vec::new();
        upsert(&mut list, stdio("zz-tool", "npx"));
        upsert(&mut list, http("zz-tool", "https://x"));
        assert_eq!(list.len(), 2);

        // Deleting one transport leaves the other intact.
        remove(&mut list, "zz-tool", "stdio");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].transport(), "http");
    }
}
