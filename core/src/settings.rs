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

use crate::domain::assets::{McpConsumptionRecord, ModelConsumptionRecord};
use crate::domain::mcp::DisabledEntry;
use crate::domain::skill::ManagedSkillRecord;
use crate::domain::types::{AgentDefinition, ModelProfile, RegistryEntry, SourceDef};
use crate::paths::{backups_dir, mux_dir, registry_dir, settings_file, user_agents_file};
use crate::safe_write::{acquire_settings_lock, write_private_if_unchanged};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::sync::Mutex;

pub const SETTINGS_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UiSettings {
    #[serde(default)]
    pub pinned_agents: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// MUX-owned network preferences. Proxy credentials are intentionally not
/// supported here so settings.json never becomes a credential store.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NetworkSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// User-selected write locations that are independent from an Agent's audited
/// wire schema. MCP section keys plus Model and Skills paths live here so a
/// location override never mutates the built-in codec contract. Skills keeps
/// one primary write directory plus zero or more compatibility read targets.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AgentConfigPathOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_paths: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_global_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_alias_dirs: Option<Vec<String>>,
}

/// The whole `~/.mux/settings.json` document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// User agent map. Absent ⇒ fall back to the builtin agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<BTreeMap<String, AgentDefinition>>,
    /// Per-Agent MCP key and Model/Skills path overrides. These are kept
    /// separate from audited Agent definitions so changing a location never
    /// changes a codec, target identity, evidence, or install probe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_config_paths: Option<BTreeMap<String, AgentConfigPathOverride>>,
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
    /// Current/active managed Model Profile per Agent. This legacy-compatible
    /// pointer is retained so older MUX builds do not lose the current model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_assignments: Option<BTreeMap<String, String>>,
    /// Installed Model Profiles per Agent, including disabled relationships.
    /// Older settings are projected into this collection from
    /// `model_assignments` and persisted on the next Model mutation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_consumptions: Option<BTreeMap<String, BTreeMap<String, ModelConsumptionRecord>>>,
    /// Desired MCP consumption, keyed by canonical Agent id and then by the
    /// stable central Registry asset key (`name::transport`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_consumptions: Option<BTreeMap<String, BTreeMap<String, McpConsumptionRecord>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_skills: Option<BTreeMap<String, ManagedSkillRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_assignments: Option<BTreeMap<String, BTreeSet<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_update_checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiSettings>,
    /// Optional proxy used by MUX-owned outbound requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkSettings>,
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

impl Settings {
    /// Return the canonical Model state while transparently projecting legacy
    /// `model_assignments` into an enabled installed relationship.
    pub fn model_selection(&self, agent_id: &str) -> crate::domain::assets::ModelAgentSelection {
        use crate::domain::assets::{ModelAgentSelection, ModelConsumptionRecord};

        let mut selection = ModelAgentSelection {
            profiles: self
                .model_consumptions
                .as_ref()
                .and_then(|all| all.get(agent_id))
                .cloned()
                .unwrap_or_default(),
            active_profile_id: self
                .model_assignments
                .as_ref()
                .and_then(|all| all.get(agent_id))
                .cloned(),
        };
        if let Some(profile_id) = selection.active_profile_id.clone() {
            selection
                .profiles
                .entry(profile_id.clone())
                .or_insert(ModelConsumptionRecord {
                    profile_id,
                    enabled: true,
                    last_selected_at: None,
                });
        }
        selection
    }

    /// Persist one canonical Model state in both the multi-profile collection
    /// and the legacy-compatible active pointer.
    pub fn set_model_selection(
        &mut self,
        agent_id: &str,
        selection: crate::domain::assets::ModelAgentSelection,
    ) {
        let all = self.model_consumptions.get_or_insert_default();
        if selection.profiles.is_empty() {
            all.remove(agent_id);
        } else {
            all.insert(agent_id.to_string(), selection.profiles);
        }
        let assignments = self.model_assignments.get_or_insert_default();
        match selection.active_profile_id {
            Some(profile_id) => {
                assignments.insert(agent_id.to_string(), profile_id);
            }
            None => {
                assignments.remove(agent_id);
            }
        }
    }
}

/// Serializes read-modify-write within this process. Each transaction also
/// holds the filesystem settings lock for cooperative cross-process writers.
static LOCK: Mutex<()> = Mutex::new(());

/// Read the whole settings document. Missing or corrupt file ⇒ defaults
/// (empty user data), matching the per-file tolerance this replaced.
pub fn load_settings() -> Settings {
    load_settings_strict().unwrap_or_default()
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

pub(crate) fn load_settings_strict() -> std::io::Result<Settings> {
    parse_for_update(&settings_file()).map(|(settings, _)| settings)
}

fn save_with_expected(
    path: &Path,
    settings: &Settings,
    original: Option<&str>,
) -> std::io::Result<()> {
    let mut settings = settings.clone();
    settings.version.get_or_insert(SETTINGS_VERSION);
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
    write_private_if_unchanged(path, original, &json).map_err(Error::other)
}

/// Persist the whole settings document (pretty JSON, atomic). Stamps `version`.
pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    let path = settings_file();
    // Global lock order is filesystem → process. Asset transactions already
    // hold the filesystem lock while calling nested settings mutations; taking
    // these in the opposite order lets a concurrent raw writer stall both
    // threads until the filesystem-lock timeout.
    let _filesystem_guard = acquire_settings_lock(&path).map_err(Error::other)?;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let original = read_optional(&path)?;
    save_with_expected(&path, settings, original.as_deref())
}

/// Load → apply `f` to one section → save under the process and filesystem
/// transaction locks. Returns whatever `f` returns once the save succeeds.
pub fn mutate_settings<F, R>(f: F) -> std::io::Result<R>
where
    F: FnOnce(&mut Settings) -> R,
{
    let path = settings_file();
    let _filesystem_guard = acquire_settings_lock(&path).map_err(Error::other)?;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut settings, original) = parse_for_update(&path)?;
    let out = f(&mut settings);
    save_with_expected(&path, &settings, original.as_deref())?;
    Ok(out)
}

/// Load → validate and apply `f` to one section → save under the process and
/// filesystem transaction locks. A closure error leaves the snapshot unwritten.
pub fn mutate_settings_checked<F, R>(f: F) -> std::io::Result<R>
where
    F: FnOnce(&mut Settings) -> std::io::Result<R>,
{
    let path = settings_file();
    let _filesystem_guard = acquire_settings_lock(&path).map_err(Error::other)?;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut settings, original) = parse_for_update(&path)?;
    let out = f(&mut settings)?;
    save_with_expected(&path, &settings, original.as_deref())?;
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

    let has_legacy = || {
        reg_dir.is_dir()
            || agents_path.exists()
            || disabled_path.exists()
            || state_path.exists()
            || imported_path.exists()
    };
    // Avoid creating ~/.mux solely to discover that there is nothing to
    // migrate. Once legacy state exists, serialize the complete check, write,
    // and archival decision with every other cooperating settings writer.
    if !has_legacy() {
        return;
    }

    let _filesystem_guard = match acquire_settings_lock(&settings_path) {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let _process_guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
    // Another process may have completed migration or created fresh settings
    // while this process was waiting for the filesystem lock.
    if settings_path.exists() || !has_legacy() {
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
    // The caller already owns both locks, so use the internal CAS writer
    // directly instead of re-entering the non-reentrant process mutex.
    if save_with_expected(&settings_path, &s, None).is_err() {
        return;
    }
    let stamp = super::paths::backup_timestamp();
    let legacy_dir = backups_dir().join(format!("legacy-{stamp}"));
    archive(&reg_dir, &legacy_dir, "registry");
    archive(&agents_path, &legacy_dir, "agents.json");
    archive(&disabled_path, &legacy_dir, "disabled.json");
    archive(&state_path, &legacy_dir, "state.json");
    archive(&imported_path, &legacy_dir, ".imported");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{RegistryConfig, StdioConfig};
    use crate::safe_write::acquire_settings_lock;
    use std::sync::mpsc;
    use std::time::Duration;

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
    fn skill_sections_and_unknown_fields_survive_settings_roundtrip() {
        let json = r#"{
          "managed_skills": {},
          "skill_assignments": {"review-changes":["claude-user"]},
          "skill_update_checked_at": "2026-07-16T08:00:00Z",
          "future_section": {"keep": true}
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.managed_skills.as_ref().unwrap().is_empty());
        assert!(
            settings.skill_assignments.as_ref().unwrap()["review-changes"].contains("claude-user")
        );
        assert_eq!(
            settings.skill_update_checked_at.as_deref(),
            Some("2026-07-16T08:00:00Z")
        );
        let encoded = serde_json::to_value(settings).unwrap();
        assert_eq!(
            encoded["skill_assignments"]["review-changes"][0],
            "claude-user"
        );
        assert_eq!(encoded["future_section"]["keep"], true);
    }

    #[test]
    fn consumption_and_existing_assignments_survive_settings_roundtrip() {
        let json = r#"{
          "mcp_consumptions": {
            "claude-code": {
              "github::stdio": {
                "asset_key": "github::stdio",
                "enabled": true,
                "overrides": {"args":["--read-only"]}
              }
            }
          },
          "model_assignments": {"claude-code":"work"},
          "skill_assignments": {"review-changes":["claude-user"]},
          "state": {"active":[]},
          "imported": "2026-07-18T00:00:00Z",
          "future_consumption_field": {"keep":true}
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        let record = &settings.mcp_consumptions.as_ref().unwrap()["claude-code"]["github::stdio"];
        assert_eq!(record.asset_key, "github::stdio");
        assert_eq!(
            record.overrides.args.as_deref(),
            Some(["--read-only".to_string()].as_slice())
        );

        let encoded = serde_json::to_value(settings).unwrap();
        assert_eq!(encoded["model_assignments"]["claude-code"], "work");
        assert_eq!(
            encoded["skill_assignments"]["review-changes"][0],
            "claude-user"
        );
        assert_eq!(encoded["state"]["active"], serde_json::json!([]));
        assert_eq!(encoded["imported"], "2026-07-18T00:00:00Z");
        assert_eq!(encoded["future_consumption_field"]["keep"], true);
    }

    #[test]
    fn legacy_model_assignment_projects_and_persists_as_multi_profile_selection() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"model_assignments":{"grok-build":"work"}}"#).unwrap();

        let mut selection = settings.model_selection("grok-build");
        assert_eq!(selection.active_profile_id.as_deref(), Some("work"));
        assert!(selection.profiles["work"].enabled);
        assert!(settings.model_consumptions.is_none());

        selection.profiles.insert(
            "personal".into(),
            ModelConsumptionRecord {
                profile_id: "personal".into(),
                enabled: true,
                last_selected_at: None,
            },
        );
        settings.set_model_selection("grok-build", selection);
        assert!(settings.model_consumptions.as_ref().unwrap()["grok-build"].contains_key("work"));
        assert!(
            settings.model_consumptions.as_ref().unwrap()["grok-build"].contains_key("personal")
        );
        assert_eq!(
            settings.model_assignments.as_ref().unwrap()["grok-build"],
            "work"
        );
    }

    #[test]
    fn strict_loader_defaults_missing_and_rejects_corrupt_settings() {
        let th = crate::testenv::TestHome::new("settings-strict");
        assert!(load_settings_strict().unwrap().extra.is_empty());

        let path = th.home.join(".mux/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"managed_skills": ["#).unwrap();

        let error = load_settings_strict().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(load_settings().managed_skills.is_none());
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

    #[test]
    fn ui_section_and_unknown_fields_survive_settings_roundtrip() {
        let json = r#"{
      "ui": {"pinned_agents": ["claude-code", "codex"]},
      "future_section": {"keep": true}
    }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(
            settings.ui.as_ref().unwrap().pinned_agents,
            vec!["claude-code", "codex"]
        );
        let encoded = serde_json::to_value(settings).unwrap();
        assert_eq!(encoded["ui"]["pinned_agents"][0], "claude-code");
        assert_eq!(encoded["future_section"]["keep"], true);
    }

    #[test]
    fn network_section_and_unknown_fields_survive_settings_roundtrip() {
        let json = r#"{
      "network": {
        "proxy_url": "http://127.0.0.1:7890",
        "future_network_key": {"keep": true}
      },
      "future_section": {"keep": true}
    }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(
            settings.network.as_ref().unwrap().proxy_url.as_deref(),
            Some("http://127.0.0.1:7890")
        );
        let encoded = serde_json::to_value(settings).unwrap();
        assert_eq!(encoded["network"]["future_network_key"]["keep"], true);
        assert_eq!(encoded["future_section"]["keep"], true);
    }

    #[test]
    fn mutation_refuses_a_concurrent_settings_change() {
        let home = crate::testenv::TestHome::new("settings-concurrent");
        let path = home.home.join(".mux/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"imported":"original"}"#).unwrap();

        let result = mutate_settings(|settings| {
            settings.imported = Some("candidate".into());
            std::fs::write(&path, r#"{"imported":"concurrent"}"#).unwrap();
        });

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            r#"{"imported":"concurrent"}"#,
        );
    }

    #[test]
    fn mutation_waits_for_settings_lock_before_reading_document() {
        let home = crate::testenv::TestHome::new("settings-filesystem-lock");
        let path = home.home.join(".mux/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"imported":"original","future":{"value":1}}"#).unwrap();
        let filesystem_lock = acquire_settings_lock(&path).unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (continue_tx, continue_rx) = mpsc::channel();

        let mutation = std::thread::spawn(move || {
            mutate_settings(|settings| {
                entered_tx.send(()).unwrap();
                continue_rx.recv().unwrap();
                settings.imported = Some("mutation".into());
            })
        });

        let entered_while_locked = entered_rx.recv_timeout(Duration::from_millis(150)).is_ok();
        std::fs::write(&path, r#"{"imported":"serialized","future":{"value":2}}"#).unwrap();
        drop(filesystem_lock);
        continue_tx.send(()).unwrap();
        let result = mutation.join().unwrap();

        assert!(
            !entered_while_locked,
            "settings mutation entered before the filesystem lock was released"
        );
        result.unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(saved["imported"], "mutation");
        assert_eq!(saved["future"]["value"], 2);
    }

    #[test]
    fn settings_writers_take_the_filesystem_lock_before_the_process_mutex() {
        let home = crate::testenv::TestHome::new("settings-lock-order");
        let path = home.home.join(".mux/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"imported":"original"}"#).unwrap();

        let filesystem_lock = acquire_settings_lock(&path).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            mutate_settings(|settings| settings.imported = Some("writer".into()))
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let process_guard = LOCK
            .try_lock()
            .expect("a writer waiting on the filesystem lock must not hold the process mutex");
        drop(process_guard);

        // This models an asset transaction: the current thread already owns
        // the filesystem lock, then enters a nested settings mutation.
        mutate_settings(|settings| settings.imported = Some("transaction".into())).unwrap();
        drop(filesystem_lock);
        writer.join().unwrap().unwrap();

        assert_eq!(
            load_settings_strict().unwrap().imported.as_deref(),
            Some("writer")
        );
    }

    #[test]
    fn legacy_migration_rechecks_settings_after_waiting_for_filesystem_lock() {
        let home = crate::testenv::TestHome::new("migration-lock");
        let mux = home.home.join(".mux");
        let path = mux.join("settings.json");
        std::fs::create_dir_all(&mux).unwrap();
        std::fs::write(mux.join("agents.json"), r#"{"legacy":{"name":"Legacy"}}"#).unwrap();

        let filesystem_lock = acquire_settings_lock(&path).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let migration = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            migrate_if_needed();
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let fresh = r#"{"imported":"created-while-migration-waited"}"#;
        std::fs::write(&path, fresh).unwrap();
        drop(filesystem_lock);
        migration.join().unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), fresh);
        assert!(
            mux.join("agents.json").exists(),
            "legacy inputs must not be archived when fresh settings already won"
        );
    }

    #[test]
    fn checked_mutation_does_not_write_when_closure_fails() {
        let home = crate::testenv::TestHome::new("settings-checked-failure");
        let path = home.home.join(".mux/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"imported":"original"}"#;
        std::fs::write(&path, original).unwrap();

        let result = mutate_settings_checked(|settings| -> std::io::Result<()> {
            settings.imported = Some("candidate".into());
            Err(Error::other("validation failed"))
        });

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
