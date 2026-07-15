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

use crate::disabled::DisabledEntry;
use crate::paths::{backups_dir, mux_dir, registry_dir, settings_file, user_agents_file};
use crate::safe_write::write_private_if_unchanged;
use crate::types::{AgentDefinition, ModelProfile, RegistryEntry, SourceDef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Error, ErrorKind};
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
    /// User/custom/override registry entries (manual + discovered + overrides),
    /// layered over the entries contributed by `sources` on read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<Vec<RegistryEntry>>,
    /// User-added catalog sources (subscribed remote URLs + local files). Their
    /// servers are parsed from cached files under `~/.mux/sources/` on read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<SourceDef>>,
    /// Disable snapshots, keyed by agent id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<BTreeMap<String, Vec<DisabledEntry>>>,
    /// Reusable model endpoints. API keys are intentionally excluded and live
    /// only in the macOS Keychain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_profiles: Option<BTreeMap<String, ModelProfile>>,
    /// Managed model Agent id -> profile id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_assignments: Option<BTreeMap<String, String>>,
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

/// Serializes read-modify-write within this process. Cross-process changes are
/// detected by an optimistic content check before atomic replacement.
static LOCK: Mutex<()> = Mutex::new(());

/// Read the whole settings document. Missing or corrupt file ⇒ defaults
/// (empty user data), matching the per-file tolerance this replaced.
pub fn load_settings() -> Settings {
    fs::read_to_string(settings_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn read_optional(path: &Path) -> std::io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn parse_for_update(path: &Path) -> std::io::Result<(Settings, Option<String>)> {
    let original = read_optional(path)?;
    let settings = match original.as_deref() {
        Some(content) => serde_json::from_str(content).map_err(|error| {
            Error::new(
                ErrorKind::InvalidData,
                format!(
                    "refusing to replace invalid MUX settings at {}: {}",
                    path.display(),
                    error
                ),
            )
        })?,
        None => Settings::default(),
    };
    Ok((settings, original))
}

fn save_with_expected(settings: &Settings, original: Option<&str>) -> std::io::Result<()> {
    let mut settings = settings.clone();
    settings.version.get_or_insert(1);
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
    write_private_if_unchanged(&settings_file(), original, &json).map_err(Error::other)
}

/// Persist the whole settings document (pretty JSON, atomic). Stamps `version`.
pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    let path = settings_file();
    let original = read_optional(&path)?;
    save_with_expected(settings, original.as_deref())
}

/// Load → apply `f` to one section → save, under the process lock. Returns
/// whatever `f` returns once the save succeeds.
pub fn mutate_settings<F, R>(f: F) -> std::io::Result<R>
where
    F: FnOnce(&mut Settings) -> R,
{
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut settings, original) = parse_for_update(&settings_file())?;
    let out = f(&mut settings);
    save_with_expected(&settings, original.as_deref())?;
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
    let stamp = super::paths::backup_timestamp();
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
    use crate::types::{RegistryConfig, StdioConfig};

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
                stdio: Some(StdioConfig {
                    command: "c".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        };
        s.registry.get_or_insert_with(Vec::new).push(entry);
        let back = serde_json::to_string(&s).unwrap();
        assert!(back.contains("\"weird\":true"));
        assert!(back.contains("\"active\""));
        assert!(back.contains("\"name\":\"x\""));
    }

    #[test]
    fn mutation_refuses_to_replace_corrupt_settings() {
        let th = crate::testenv::TestHome::new("settings-corrupt");
        let path = th.home.join(".mux/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"registry": ["#;
        std::fs::write(&path, original).unwrap();

        let result = mutate_settings(|settings| settings.imported = Some("now".into()));

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn settings_are_written_with_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let th = crate::testenv::TestHome::new("settings-mode");
        mutate_settings(|settings| settings.imported = Some("now".into())).unwrap();

        let mode = std::fs::metadata(th.home.join(".mux/settings.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
