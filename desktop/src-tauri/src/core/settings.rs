//! Single consolidated user-data file: `~/.mux/settings.json`.
//!
//! MUX's desktop app and the CLI share `~/.mux/`. Historically that meant a
//! sprawl of files: `registry/<name>__<transport>.json` (one per custom MCP),
//! `agents.json`, `disabled.json`, `state.json`, and a `.imported` marker. This
//! module collapses all of it into one `settings.json`.
//!
//! Cross-tool rule: the desktop fully types the sections it owns
//! (`agents`/`registry`/`disabled`) and carries the CLI-owned ones
//! (`state`/`imported`) plus any unknown future keys (`extra`) through opaquely,
//! so a desktop write never clobbers data the CLI wrote, and vice versa. Every
//! mutation is read-whole → modify one section → write-whole (atomically).

use crate::core::disabled::DisabledEntry;
use crate::core::paths::{backups_dir, mux_dir, registry_dir, settings_file, user_agents_file};
use crate::core::types::{AgentDefinition, RegistryEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// The whole `~/.mux/settings.json` document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// User agent map. Absent ⇒ fall back to the builtin agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<BTreeMap<String, AgentDefinition>>,
    /// User/custom/override registry entries (merged over builtins on read).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<Vec<RegistryEntry>>,
    /// Disable snapshots, keyed by agent id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<BTreeMap<String, Vec<DisabledEntry>>>,
    /// CLI-owned: last applied state. Opaque to the desktop — carried through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
    /// CLI-owned: first-scan import marker (ISO timestamp). Carried through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported: Option<String>,
    /// Forward-compat: any unknown top-level keys survive a round-trip so an
    /// older binary never drops a newer one's data.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Serializes read-modify-write within this process so two concurrent commands
/// can't lose each other's section update. Cross-process races (CLI + desktop
/// at once) are rare and bounded by the atomic rename in `save_settings`.
static LOCK: Mutex<()> = Mutex::new(());

/// Read the whole settings document. Missing or corrupt file ⇒ defaults
/// (empty user data), matching the per-file tolerance this replaced.
pub fn load_settings() -> Settings {
    fs::read_to_string(settings_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Atomically write `bytes` to `path` (temp sibling + rename) so a crash mid-write
/// can never leave a torn settings file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Persist the whole settings document (pretty JSON, atomic). Stamps `version`.
pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    let mut settings = settings.clone();
    settings.version.get_or_insert(1);
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_atomic(&settings_file(), json.as_bytes())
}

/// Load → apply `f` to one section → save, under the process lock. Returns
/// whatever `f` returns once the save succeeds.
pub fn mutate_settings<F, R>(f: F) -> std::io::Result<R>
where
    F: FnOnce(&mut Settings) -> R,
{
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = load_settings();
    let out = f(&mut settings);
    save_settings(&settings)?;
    Ok(out)
}

/// Move `from` into `legacy_dir` (best-effort) if it exists.
fn archive(from: &Path, legacy_dir: &Path, name: &str) {
    if from.exists() {
        let _ = fs::create_dir_all(legacy_dir);
        let _ = fs::rename(from, legacy_dir.join(name));
    }
}

/// One-time migration: if `settings.json` is absent but legacy files exist, fold
/// them into a fresh `settings.json`, then move the old files aside into
/// `~/.mux/backups/legacy-<ts>/` (reversible, not deleted). Idempotent: a no-op
/// once `settings.json` exists.
pub fn migrate_if_needed() {
    let settings_path = settings_file();
    if settings_path.exists() {
        return;
    }

    let reg_dir = registry_dir();
    let agents_path = user_agents_file();
    let disabled_path = mux_dir().join("disabled.json");
    let state_path = mux_dir().join("state.json");
    let imported_path = mux_dir().join(".imported");

    let any_legacy = reg_dir.is_dir()
        || agents_path.exists()
        || disabled_path.exists()
        || state_path.exists()
        || imported_path.exists();
    if !any_legacy {
        return;
    }

    let mut s = Settings {
        version: Some(1),
        ..Default::default()
    };

    // registry/*.json → registry[]  (dedup by composite key, last file wins)
    if reg_dir.is_dir() {
        let mut by_key: BTreeMap<String, RegistryEntry> = BTreeMap::new();
        if let Ok(rd) = fs::read_dir(&reg_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    if let Ok(c) = fs::read_to_string(&p) {
                        if let Ok(entry) = serde_json::from_str::<RegistryEntry>(&c) {
                            by_key.insert(entry.key(), entry);
                        }
                    }
                }
            }
        }
        if !by_key.is_empty() {
            s.registry = Some(by_key.into_values().collect());
        }
    }
    // agents.json → agents
    if let Ok(c) = fs::read_to_string(&agents_path) {
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, AgentDefinition>>(&c) {
            s.agents = Some(map);
        }
    }
    // disabled.json → disabled
    if let Ok(c) = fs::read_to_string(&disabled_path) {
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, Vec<DisabledEntry>>>(&c) {
            s.disabled = Some(map);
        }
    }
    // state.json → state (opaque)
    if let Ok(c) = fs::read_to_string(&state_path) {
        if let Ok(v) = serde_json::from_str::<Value>(&c) {
            s.state = Some(v);
        }
    }
    // .imported → imported
    if let Ok(c) = fs::read_to_string(&imported_path) {
        let t = c.trim();
        if !t.is_empty() {
            s.imported = Some(t.to_string());
        }
    }

    // Only archive the legacy files once the new file is safely written.
    if save_settings(&s).is_err() {
        return;
    }
    let stamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let legacy_dir = backups_dir().join(format!("legacy-{}", stamp));
    archive(&reg_dir, &legacy_dir, "registry");
    archive(&agents_path, &legacy_dir, "agents.json");
    archive(&disabled_path, &legacy_dir, "disabled.json");
    archive(&state_path, &legacy_dir, "state.json");
    archive(&imported_path, &legacy_dir, ".imported");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{RegistryConfig, StdioConfig};

    #[test]
    fn unknown_and_cli_sections_survive_roundtrip() {
        // A settings doc the CLI wrote (state + imported + a future key) must
        // come back intact after the desktop deserializes and re-serializes it.
        let json = r#"{
            "version": 1,
            "registry": [],
            "state": {"active": [{"name":"git","scope":"global","agents":["claude-code"]}]},
            "imported": "2026-01-02T03:04:05",
            "futureThing": {"a": 1}
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.state.is_some());
        assert_eq!(s.imported.as_deref(), Some("2026-01-02T03:04:05"));
        assert!(s.extra.contains_key("futureThing"));
        let back = serde_json::to_string(&s).unwrap();
        assert!(back.contains("futureThing"));
        assert!(back.contains("\"active\""));
        assert!(back.contains("2026-01-02T03:04:05"));
    }

    #[test]
    fn registry_section_mutation_preserves_passthrough() {
        // Simulate: load a CLI-authored doc, mutate the registry section in
        // memory, ensure state/extra are still there.
        let json = r#"{"state":{"active":[]},"weird":true}"#;
        let mut s: Settings = serde_json::from_str(json).unwrap();
        let entry = RegistryEntry {
            name: "x".into(),
            description: "".into(),
            tags: vec![],
            config: RegistryConfig {
                stdio: Some(StdioConfig { command: "c".into(), args: None, env: None }),
                http: None,
            },
            origin: None,
        };
        s.registry.get_or_insert_with(Vec::new).push(entry);
        let back = serde_json::to_string(&s).unwrap();
        assert!(back.contains("\"weird\":true"));
        assert!(back.contains("\"active\""));
        assert!(back.contains("\"name\":\"x\""));
    }
}
