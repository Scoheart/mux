use crate::core::settings::{load_settings, mutate_settings};
use crate::core::types::RegistryEntry;

/// The bundled "official" collection, embedded at compile time from the repo-root
/// data/registry.json. It is **no longer a built-in catalog base** — the catalog
/// is entirely source-driven now. This is exposed only so the user can opt in to
/// it as a one-click *local* source ("官方精选合集"); see
/// `commands::add_builtin_collection`.
const BUILTIN_JSON: &str = include_str!("../../../../data/registry.json");

pub fn builtin_registry() -> Vec<RegistryEntry> {
    serde_json::from_str(BUILTIN_JSON).expect("registry.json must be valid")
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

/// All catalog entries: servers from every **enabled** source (remote + local),
/// with the user's `settings.registry` (manual / discovered / overrides) layered
/// on top — a user entry shadows a source entry with the same composite key
/// (`name::transport`). No built-in base is included.
pub fn read_registry() -> Vec<RegistryEntry> {
    use std::collections::HashMap;
    let settings = load_settings();
    let mut by_key: HashMap<String, RegistryEntry> = HashMap::new();
    // Sources in list order — a later source shadows an earlier one on key clash.
    for def in settings.sources.as_deref().unwrap_or_default() {
        if !def.enabled {
            continue;
        }
        for e in crate::core::sources::source_entries(def) {
            by_key.insert(e.key(), e);
        }
    }
    // User registry entries always win (edits / manual / discovered / overrides).
    for e in settings.registry.unwrap_or_default() {
        by_key.insert(e.key(), e);
    }
    by_key.into_values().collect()
}

/// Persist (create or overwrite) a user registry entry into `settings.registry`.
pub fn write_registry_entry(entry: &RegistryEntry) -> std::io::Result<()> {
    mutate_settings(|s| upsert(s.registry.get_or_insert_with(Vec::new), entry.clone()))
}

/// Composite keys (`name::transport`) of entries that have a user override
/// (i.e. live in `settings.registry` — manual, discovered, or an edited source
/// entry). Used by the UI to mark an entry as customized / revertable.
pub fn user_override_keys() -> Vec<String> {
    load_settings()
        .registry
        .unwrap_or_default()
        .iter()
        .map(|e| e.key())
        .collect()
}

/// Remove the user override for `name`+`transport`. If a source still provides
/// that key, the source's version shows through again; otherwise it disappears
/// from the catalog. A missing entry is a no-op success.
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
    fn builtin_collection_loads_40_plus() {
        // Still parseable (used by the opt-in curated local source), just no
        // longer part of the default catalog.
        assert!(builtin_registry().len() >= 40);
    }

    #[test]
    fn upsert_replaces_same_key_and_remove_deletes() {
        let mut list = vec![stdio("filesystem", "a")];
        upsert(&mut list, stdio("filesystem", "b"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].config.stdio.as_ref().unwrap().command, "b");

        remove(&mut list, "filesystem", "stdio");
        assert!(list.is_empty());
        // Deleting again is a no-op.
        remove(&mut list, "filesystem", "stdio");
        assert!(list.is_empty());
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
