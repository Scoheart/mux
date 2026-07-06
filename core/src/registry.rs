use crate::settings::{load_settings, mutate_settings, Settings};
use crate::sources::{cached_path, source_entries};
use crate::types::{RegistryEntry, RegistryOrigin, SourceDef};
use std::fs;
use std::path::Path;

/// The bundled "official" collection, embedded at compile time from the repo-root
/// data/registry.json. It is **not a built-in catalog base** — the catalog is
/// entirely source-driven. Exposed only so the user can opt in to it as a
/// one-click *local* source ("官方精选合集"); see `commands::add_builtin_collection`.
const BUILTIN_JSON: &str = include_str!("../../data/registry.json");

pub fn builtin_registry() -> Vec<RegistryEntry> {
    serde_json::from_str(BUILTIN_JSON).expect("registry.json must be valid")
}

/// Ids of the two **managed** local sources. Manually-created and auto-discovered
/// entries are stored as ordinary local-source files under
/// `~/.mux/sources/local/<id>.json`, exactly like an added local file — not in
/// `settings.registry`. They appear in the Sources list like any other source.
pub const MANUAL_ID: &str = "manual";
pub const DISCOVERED_ID: &str = "discovered";

/// An in-memory `SourceDef` for a managed local source (enough to resolve its
/// cached file path and parse it). Not persisted by itself.
fn managed_def(id: &str, name: &str) -> SourceDef {
    SourceDef {
        id: id.into(),
        kind: "local".into(),
        name: name.into(),
        url: None,
        path: None,
        format: "json".into(),
        key: "mcpServers".into(),
        enabled: true,
        added_at: Some(super::sources::now_iso()),
        synced_at: None,
        server_count: None,
        error: None,
    }
}

/// Register a managed source in `settings.sources` if it isn't there yet.
fn ensure_managed(settings: &mut Settings, id: &str, name: &str) {
    let list = settings.sources.get_or_insert_with(Vec::new);
    if !list.iter().any(|d| d.id == id) {
        list.push(managed_def(id, name));
    }
}

fn read_array(path: &Path) -> Vec<RegistryEntry> {
    fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str::<Vec<RegistryEntry>>(&c).ok())
        .unwrap_or_default()
}

fn write_array(path: &Path, list: &[RegistryEntry]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(list)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

/// The entries currently stored in a managed source's file (origins preserved).
fn managed_entries(id: &str) -> Vec<RegistryEntry> {
    source_entries(&managed_def(id, id))
}

/// Upsert `entry` (by composite key) into a managed source's rich-array file,
/// creating the source registration on first use.
fn write_managed(id: &str, name: &str, entry: RegistryEntry) -> std::io::Result<()> {
    mutate_settings(|s| ensure_managed(s, id, name))?;
    let path = cached_path(&managed_def(id, name)).expect("managed source has a cached path");
    let mut list = read_array(&path);
    let key = entry.key();
    list.retain(|e| e.key() != key);
    list.push(entry);
    write_array(&path, &list)
}

/// Remove the entry matching `target_key` from a managed source's file.
fn remove_managed(id: &str, name: &str, target_key: &str) -> std::io::Result<()> {
    let path = cached_path(&managed_def(id, name)).expect("managed source has a cached path");
    let mut list = read_array(&path);
    let before = list.len();
    list.retain(|e| e.key() != target_key);
    if list.len() != before {
        write_array(&path, &list)?;
    }
    Ok(())
}

/// Store a user-created / edited catalog entry into the **manual** local source
/// (the user's own layer, highest precedence). Origin is normalized to "manual".
pub fn write_manual_entry(entry: &RegistryEntry) -> std::io::Result<()> {
    let mut e = entry.clone();
    e.origin = Some(RegistryOrigin {
        kind: "manual".into(),
        agent: None,
        scope: None,
        source: Some(MANUAL_ID.into()),
    });
    write_managed(MANUAL_ID, "手动添加", e)
}

/// Store an auto-discovered entry into the **discovered** local source. The
/// entry keeps its `origin` ("discovered" + source app).
pub fn write_discovered_entry(entry: &RegistryEntry) -> std::io::Result<()> {
    write_managed(DISCOVERED_ID, "自动探索", entry.clone())
}

/// All catalog entries, assembled from every enabled source and deduped by
/// composite key (`name::transport`) with precedence low→high:
///   external sources (subscribed remote + added local, in list order)
///     < discovered (auto-detected)
///     < manual (the user's own edits win over everything).
pub fn read_registry() -> Vec<RegistryEntry> {
    use std::collections::HashMap;
    let defs = load_settings().sources.unwrap_or_default();
    let mut by_key: HashMap<String, RegistryEntry> = HashMap::new();
    // 1. external sources (everything that isn't a managed source), in order.
    for def in defs
        .iter()
        .filter(|d| d.enabled && d.id != MANUAL_ID && d.id != DISCOVERED_ID)
    {
        for e in source_entries(def) {
            by_key.insert(e.key(), e);
        }
    }
    // 2. discovered layer.
    if let Some(def) = defs.iter().find(|d| d.id == DISCOVERED_ID && d.enabled) {
        for e in source_entries(def) {
            by_key.insert(e.key(), e);
        }
    }
    // 3. manual layer (highest precedence).
    if let Some(def) = defs.iter().find(|d| d.id == MANUAL_ID && d.enabled) {
        for e in source_entries(def) {
            by_key.insert(e.key(), e);
        }
    }
    by_key.into_values().collect()
}

/// Composite keys of entries in the **manual** source — i.e. user overrides that
/// the UI can revert ("恢复默认").
pub fn user_override_keys() -> Vec<String> {
    managed_entries(MANUAL_ID).iter().map(|e| e.key()).collect()
}

/// Remove a user override (`name`+`transport`) from the manual source. If another
/// source still provides that key, it shows through again. A missing entry is a
/// no-op success.
pub fn delete_registry_entry(name: &str, transport: &str) -> std::io::Result<()> {
    remove_managed(MANUAL_ID, "手动添加", &format!("{}::{}", name, transport))
}

/// One-time migration: fold any legacy `settings.registry` entries into the
/// managed source files (discovered→discovered, everything else→manual), then
/// clear `settings.registry`. Idempotent (no-op once the section is empty).
pub fn migrate_registry_to_sources() {
    let Some(entries) = load_settings().registry.filter(|r| !r.is_empty()) else {
        return;
    };
    for e in entries {
        let discovered = e
            .origin
            .as_ref()
            .map(|o| o.kind == "discovered")
            .unwrap_or(false);
        if discovered {
            let _ = write_discovered_entry(&e);
        } else {
            let _ = write_manual_entry(&e);
        }
    }
    let _ = mutate_settings(|s| s.registry = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_collection_parses() {
        // Still parseable (used by the opt-in curated local source), just no
        // longer part of the default catalog. Currently trimmed to two entries.
        let names: Vec<String> = builtin_registry().into_iter().map(|e| e.name).collect();
        assert_eq!(
            names,
            vec![
                "context7".to_string(),
                "supabase".to_string(),
                "luma".to_string(),
                "firecrawl".to_string(),
            ]
        );
    }

    #[test]
    fn managed_def_paths_are_stable() {
        assert!(cached_path(&managed_def(MANUAL_ID, "手动添加"))
            .unwrap()
            .ends_with("local/manual.json"));
        assert!(cached_path(&managed_def(DISCOVERED_ID, "自动探索"))
            .unwrap()
            .ends_with("local/discovered.json"));
    }
}
