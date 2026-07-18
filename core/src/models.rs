//! Reusable model endpoint profiles and safe per-Agent configuration writers.
//!
//! The managed set is deliberately small: Claude Code, Codex, and Pi are
//! written through documented user-level configuration surfaces. Other Agents
//! remain guidance-only until they expose a per-profile writer that can consume
//! credentials from Keychain without persisting plaintext secrets.

use crate::applier::backup;
use crate::paths::{backup_timestamp, backups_dir};
use crate::safe_write::{remove_if_unchanged, write_if_unchanged};
use crate::settings::{load_settings, mutate_settings};
use crate::types::{ModelProfile, ModelProtocol};
use jsonc_parser::cst::{CstInputValue, CstNode, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
#[cfg(target_os = "macos")]
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use toml_edit::{Array, Document, Item, Table};

const KEYCHAIN_ACCOUNT: &str = "api-key";
const CREDENTIAL_ROLLBACK_PREFIX: &str = "__asset-operation-rollback__";
const QODER_DOCS: &str = "https://docs.qoder.com/user-guide/chat/custom-models";
const GROK_BUILD_MODEL_DOCS: &str = "https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md";
const MINIMAX_CODE_DOCS: &str = "https://agent.minimax.io/download";

static TEST_CREDENTIALS: LazyLock<Mutex<BTreeMap<String, Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

fn test_credential_key(profile_id: &str) -> String {
    format!(
        "{}::{profile_id}",
        std::env::var("MUX_TEST_PROBE_ROOT").unwrap_or_default()
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelProfileView {
    #[serde(flatten)]
    pub profile: ModelProfile,
    pub credential_saved: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelAgentView {
    pub id: String,
    pub name: String,
    /// `managed` or `guided`.
    pub mode: String,
    pub installed: bool,
    pub config_path: String,
    pub docs: String,
    pub assigned_profile: Option<String>,
    pub supported_protocols: Vec<ModelProtocol>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelApplyResult {
    pub agent: String,
    pub profile: String,
    pub files: Vec<String>,
    pub restart_required: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelObservedState {
    Synced,
    Missing,
    Drifted,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalModelObservedState {
    Absent,
    Present,
    Conflicted,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn config_path(relative: &str) -> PathBuf {
    home().join(relative)
}

fn keychain_service(profile_id: &str) -> String {
    format!("com.scoheart.mux.model-profile.{profile_id}")
}

fn security_command(profile_id: &str) -> Vec<String> {
    vec![
        "/usr/bin/security".into(),
        "find-generic-password".into(),
        "-w".into(),
        "-s".into(),
        keychain_service(profile_id),
        "-a".into(),
        KEYCHAIN_ACCOUNT.into(),
    ]
}

fn security_shell_command(profile_id: &str) -> String {
    security_command(profile_id).join(" ")
}

#[cfg(target_os = "macos")]
fn read_credential(profile_id: &str) -> Option<Vec<u8>> {
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        return TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&test_credential_key(profile_id))
            .cloned();
    }
    let output = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-w",
            "-s",
            &keychain_service(profile_id),
            "-a",
            KEYCHAIN_ACCOUNT,
        ])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut value = output.stdout;
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b'\n' | b'\r'))
    {
        value.pop();
    }
    Some(value)
}

#[cfg(not(target_os = "macos"))]
fn read_credential(_profile_id: &str) -> Option<Vec<u8>> {
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        return TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&test_credential_key(_profile_id))
            .cloned();
    }
    None
}

fn credential_exists(profile_id: &str) -> bool {
    // TestHome sets this marker specifically to isolate probes from the real
    // machine. Never consult the user's Keychain from a test process.
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        return TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains_key(&test_credential_key(profile_id))
            || std::env::var("MUX_TEST_MODEL_CREDENTIAL_PROFILES")
                .ok()
                .is_some_and(|profiles| profiles.split(',').any(|item| item == profile_id));
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                &keychain_service(profile_id),
                "-a",
                KEYCHAIN_ACCOUNT,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(not(target_os = "macos"))]
    false
}

#[cfg(target_os = "macos")]
fn set_credential(profile_id: &str, credential: &[u8]) -> Result<(), String> {
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(test_credential_key(profile_id), credential.to_vec());
        return Ok(());
    }
    if credential.contains(&b'\n') || credential.contains(&b'\r') {
        return Err("API key cannot contain a newline".into());
    }
    let mut child = Command::new("/usr/bin/security")
        .args([
            "add-generic-password",
            "-s",
            &keychain_service(profile_id),
            "-a",
            KEYCHAIN_ACCOUNT,
            "-T",
            "/usr/bin/security",
            "-U",
            "-w",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start macOS Keychain helper: {error}"))?;
    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| "failed to open macOS Keychain helper input".to_string())?;
    // `security ... -w` prompts twice for a new item. Sending the value through
    // stdin keeps it out of argv, process listings, logs, and shell history.
    stdin
        .write_all(credential)
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.write_all(credential))
        .and_then(|_| stdin.write_all(b"\n"))
        .map_err(|error| format!("failed to send API key to macOS Keychain: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("macOS Keychain helper failed: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to save API key in macOS Keychain: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(not(target_os = "macos"))]
fn set_credential(_profile_id: &str, _credential: &[u8]) -> Result<(), String> {
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(test_credential_key(_profile_id), _credential.to_vec());
        return Ok(());
    }
    Err("secure model credentials are currently supported on macOS only".into())
}

#[cfg(target_os = "macos")]
fn delete_credential(profile_id: &str) -> Result<(), String> {
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&test_credential_key(profile_id));
        return Ok(());
    }
    if !credential_exists(profile_id) {
        return Ok(());
    }
    let output = Command::new("/usr/bin/security")
        .args([
            "delete-generic-password",
            "-s",
            &keychain_service(profile_id),
            "-a",
            KEYCHAIN_ACCOUNT,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("failed to start macOS Keychain helper: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to remove API key from macOS Keychain: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(not(target_os = "macos"))]
fn delete_credential(_profile_id: &str) -> Result<(), String> {
    if std::env::var_os("MUX_TEST_PROBE_ROOT").is_some() {
        TEST_CREDENTIALS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&test_credential_key(_profile_id));
    }
    Ok(())
}

fn validate_profile(profile: &ModelProfile) -> Result<(), String> {
    let valid_id = !profile.id.is_empty()
        && profile.id.len() <= 64
        && profile.id.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'-' | b'_' | b'.'))
        });
    if !valid_id {
        return Err("profile id must be 1-64 lowercase letters, digits, '.', '_' or '-'".into());
    }
    if profile.name.trim().is_empty() {
        return Err("profile name is required".into());
    }
    if profile.model.trim().is_empty() {
        return Err("model id is required".into());
    }
    let base_url = profile.base_url.trim();
    if !(base_url.starts_with("https://") || base_url.starts_with("http://"))
        || base_url.chars().any(char::is_whitespace)
    {
        return Err("base URL must be an http(s) URL without whitespace".into());
    }
    if profile.context_window == Some(0) || profile.max_output_tokens == Some(0) {
        return Err("token limits must be greater than zero".into());
    }
    Ok(())
}

pub(crate) fn validate_profile_draft(profile: &ModelProfile) -> Result<(), String> {
    validate_profile(profile)
}

pub(crate) fn credential_snapshot(profile_id: &str) -> Option<Vec<u8>> {
    read_credential(profile_id)
}

fn credential_rollback_profile_id(operation_id: &str, profile_id: &str) -> String {
    format!("{CREDENTIAL_ROLLBACK_PREFIX}.{operation_id}.{profile_id}")
}

/// Persist the pre-transaction credential in Keychain, never in the operation
/// journal. The first byte distinguishes a missing credential from an empty
/// lookup result; user credentials themselves cannot contain newlines because
/// the normal Keychain writer rejects them.
pub(crate) fn persist_credential_rollback(
    operation_id: &str,
    profile_id: &str,
    credential: Option<&[u8]>,
) -> Result<(), String> {
    let mut payload = Vec::with_capacity(1 + credential.map_or(0, |value| value.len()));
    match credential {
        Some(value) => {
            payload.push(1);
            payload.extend_from_slice(value);
        }
        None => payload.push(0),
    }
    set_credential(
        &credential_rollback_profile_id(operation_id, profile_id),
        &payload,
    )
}

/// Returns `None` when no durable rollback item exists, `Some(None)` when the
/// original Profile had no credential, and `Some(Some(bytes))` otherwise.
pub(crate) fn credential_rollback_snapshot(
    operation_id: &str,
    profile_id: &str,
) -> Result<Option<Option<Vec<u8>>>, String> {
    let Some(payload) = read_credential(&credential_rollback_profile_id(operation_id, profile_id))
    else {
        return Ok(None);
    };
    let Some((&tag, value)) = payload.split_first() else {
        return Err("credential rollback item is invalid".into());
    };
    match tag {
        0 if value.is_empty() => Ok(Some(None)),
        1 => Ok(Some(Some(value.to_vec()))),
        _ => Err("credential rollback item is invalid".into()),
    }
}

pub(crate) fn clear_credential_rollback(
    operation_id: &str,
    profile_id: &str,
) -> Result<(), String> {
    delete_credential(&credential_rollback_profile_id(operation_id, profile_id))
}

pub(crate) fn restore_credential_snapshot(
    profile_id: &str,
    credential: Option<&[u8]>,
) -> Result<(), String> {
    match credential {
        Some(value) => set_credential(profile_id, value),
        None => delete_credential(profile_id),
    }
}

pub fn list_profiles() -> Vec<ModelProfileView> {
    load_settings()
        .model_profiles
        .unwrap_or_default()
        .into_values()
        .map(|profile| ModelProfileView {
            credential_saved: credential_exists(&profile.id),
            profile,
        })
        .collect()
}

/// Save metadata and optionally update its Keychain credential. `None` keeps
/// the existing credential; an empty string explicitly removes it.
pub fn save_profile(profile: ModelProfile, credential: Option<String>) -> Result<(), String> {
    validate_profile(&profile)?;
    let previous_credential = credential.as_ref().map(|_| read_credential(&profile.id));
    if let Some(value) = credential.as_deref() {
        if value.is_empty() {
            delete_credential(&profile.id)?;
        } else {
            set_credential(&profile.id, value.as_bytes())?;
        }
    }

    let profile_id = profile.id.clone();
    if let Err(error) = mutate_settings(|settings| {
        settings
            .model_profiles
            .get_or_insert_with(BTreeMap::new)
            .insert(profile_id.clone(), profile);
    }) {
        if let Some(previous) = previous_credential {
            match previous {
                Some(value) => {
                    let _ = set_credential(&profile_id, &value);
                }
                None => {
                    let _ = delete_credential(&profile_id);
                }
            }
        }
        return Err(error.to_string());
    }
    Ok(())
}

pub fn delete_profile(profile_id: &str) -> Result<(), String> {
    validate_profile_id(profile_id)?;
    let previous_credential = read_credential(profile_id);
    if credential_exists(profile_id) {
        delete_credential(profile_id)?;
    }
    if let Err(error) = delete_profile_metadata(profile_id) {
        if let Some(credential) = previous_credential {
            let _ = set_credential(profile_id, &credential);
        }
        return Err(error);
    }
    Ok(())
}

pub(crate) fn delete_profile_metadata(profile_id: &str) -> Result<(), String> {
    validate_profile_id(profile_id)?;
    mutate_settings(|settings| {
        if let Some(profiles) = settings.model_profiles.as_mut() {
            profiles.remove(profile_id);
        }
        if let Some(assignments) = settings.model_assignments.as_mut() {
            assignments.retain(|_, assigned| assigned != profile_id);
        }
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn credential_present(profile_id: &str) -> bool {
    credential_exists(profile_id)
}

pub(crate) fn apply_credential_update(
    profile_id: &str,
    credential: Option<&str>,
) -> Result<(), String> {
    match credential {
        None => Ok(()),
        Some("") => delete_credential(profile_id),
        Some(value) => set_credential(profile_id, value.as_bytes()),
    }
}

fn validate_profile_id(profile_id: &str) -> Result<(), String> {
    let dummy = ModelProfile {
        id: profile_id.into(),
        name: "x".into(),
        protocol: ModelProtocol::OpenaiResponses,
        base_url: "http://localhost".into(),
        model: "x".into(),
        context_window: None,
        max_output_tokens: None,
        reasoning: false,
    };
    validate_profile(&dummy)
}

fn command_exists(names: &[&str]) -> bool {
    let mut directories = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    directories.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home().join(".local/bin"),
        home().join(".bun/bin"),
        home().join(".cargo/bin"),
    ]);
    directories
        .iter()
        .any(|directory| names.iter().any(|name| directory.join(name).is_file()))
}

fn agent_installed(names: &[&str], config_locations: &[&str], app_locations: &[&str]) -> bool {
    command_exists(names)
        || config_locations
            .iter()
            .any(|location| config_path(location).exists())
        || app_locations
            .iter()
            .any(|location| Path::new(location).exists())
}

pub fn list_agents() -> Vec<ModelAgentView> {
    let assignments = load_settings().model_assignments.unwrap_or_default();
    vec![
        ModelAgentView {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            mode: "managed".into(),
            installed: agent_installed(&["claude"], &[".claude"], &[]),
            config_path: "~/.claude/settings.json".into(),
            docs: "https://code.claude.com/docs/en/settings".into(),
            assigned_profile: assignments.get("claude-code").cloned(),
            supported_protocols: vec![ModelProtocol::AnthropicMessages],
            note: "Anthropic-compatible endpoint; restart the session after applying.".into(),
        },
        ModelAgentView {
            id: "codex".into(),
            name: "Codex".into(),
            mode: "managed".into(),
            installed: agent_installed(&["codex"], &[".codex"], &["/Applications/Codex.app"]),
            config_path: "~/.codex/config.toml".into(),
            docs: "https://developers.openai.com/codex/config-advanced".into(),
            assigned_profile: assignments.get("codex").cloned(),
            supported_protocols: vec![ModelProtocol::OpenaiResponses],
            note: "Custom providers currently use the Responses API.".into(),
        },
        ModelAgentView {
            id: "grok-build".into(),
            name: "Grok Build".into(),
            mode: "guided".into(),
            installed: agent_installed(&["grok"], &[".grok"], &[]),
            config_path: "~/.grok/config.toml".into(),
            docs: GROK_BUILD_MODEL_DOCS.into(),
            assigned_profile: None,
            supported_protocols: vec![
                ModelProtocol::AnthropicMessages,
                ModelProtocol::OpenaiResponses,
                ModelProtocol::OpenaiCompletions,
            ],
            note: "Grok Build supports custom model endpoints, but its per-model credential is a literal api_key or environment variable; use the official config flow to avoid persisting a MUX Keychain secret in plaintext.".into(),
        },
        ModelAgentView {
            id: "pi".into(),
            name: "Pi".into(),
            mode: "managed".into(),
            installed: agent_installed(&["pi"], &[".pi/agent"], &[]),
            config_path: "~/.pi/agent/models.json + settings.json".into(),
            docs: "https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md".into(),
            assigned_profile: assignments.get("pi").cloned(),
            supported_protocols: vec![
                ModelProtocol::AnthropicMessages,
                ModelProtocol::OpenaiResponses,
                ModelProtocol::OpenaiCompletions,
            ],
            note: "MUX updates the custom provider and default model as one transaction.".into(),
        },
        ModelAgentView {
            id: "minimax-code".into(),
            name: "MiniMax Code".into(),
            mode: "guided".into(),
            installed: agent_installed(
                &["mavis"],
                &[".mavis"],
                &["/Applications/MiniMax Code.app"],
            ),
            config_path: "~/.mavis/config.yaml".into(),
            docs: MINIMAX_CODE_DOCS.into(),
            assigned_profile: None,
            supported_protocols: vec![
                ModelProtocol::AnthropicMessages,
                ModelProtocol::OpenaiResponses,
                ModelProtocol::OpenaiCompletions,
            ],
            note: "MiniMax Code supports custom providers, but its current config flow stores options.apiKey as plaintext YAML; use its own model configuration flow rather than exporting a MUX Keychain secret.".into(),
        },
        ModelAgentView {
            id: "qoder".into(),
            name: "Qoder".into(),
            mode: "guided".into(),
            installed: agent_installed(
                &["qoder", "qodercli"],
                &[".qoder"],
                &["/Applications/Qoder.app"],
            ),
            config_path: "~/.qoder/settings.json".into(),
            docs: QODER_DOCS.into(),
            assigned_profile: None,
            supported_protocols: Vec::new(),
            note: "Qoder has no public secure non-interactive BYOK writer; configure it through /model.".into(),
        },
    ]
}

/// Canonical read-only capability lookup used by the shared consumption
/// service. Keeping this projection here prevents another copy of the managed
/// and guided Agent matrix from drifting out of sync with the Model writer.
pub fn model_agent_capability(agent_id: &str) -> Option<ModelAgentView> {
    list_agents().into_iter().find(|agent| agent.id == agent_id)
}

/// Inspect only the fields owned by the Model adapter. Candidate generation
/// preserves all unrelated bytes, so comparing it with the source identifies
/// owned-field drift without treating comments or unknown settings as drift.
pub fn observe_profile(
    agent_id: &str,
    profile: &ModelProfile,
) -> Result<ModelObservedState, String> {
    let has_credential = credential_exists(&profile.id);
    let observed = match agent_id {
        "claude-code" => observe_prepared(prepare_claude(
            &config_path(".claude/settings.json"),
            profile,
            has_credential,
        )),
        "codex" => observe_prepared(prepare_codex(
            &config_path(".codex/config.toml"),
            profile,
            has_credential,
        )),
        "pi" => {
            let models = observe_prepared(prepare_pi_models(
                &config_path(".pi/agent/models.json"),
                profile,
                has_credential,
            ));
            let settings = observe_prepared(prepare_pi_settings(
                &config_path(".pi/agent/settings.json"),
                profile,
            ));
            combine_observed(models, settings)
        }
        _ => Ok(ModelObservedState::Conflicted),
    }?;
    // Credentials are optional because local and gateway endpoints may not
    // require authentication. Presence is surfaced on the central Profile; it
    // is not drift unless a future Profile schema explicitly marks it required.
    Ok(observed)
}

/// Detect model-owned fields when no desired Profile exists. This is a
/// deliberately identity-free external projection: it prevents a central
/// selection from silently taking over an Agent configuration without treating
/// the Agent file as a source of central Profile metadata.
pub fn observe_external_model(agent_id: &str) -> Result<ExternalModelObservedState, String> {
    match agent_id {
        "claude-code" => observe_external_claude(&config_path(".claude/settings.json")),
        "codex" => observe_external_codex(&config_path(".codex/config.toml")),
        "pi" => observe_external_pi(
            &config_path(".pi/agent/models.json"),
            &config_path(".pi/agent/settings.json"),
        ),
        _ => Ok(ExternalModelObservedState::Absent),
    }
}

fn observe_external_claude(path: &Path) -> Result<ExternalModelObservedState, String> {
    let (root, original) = match read_jsonc(path) {
        Ok(value) => value,
        Err(_) if path.exists() => return Ok(ExternalModelObservedState::Conflicted),
        Err(error) => return Err(error),
    };
    if original.is_none() {
        return Ok(ExternalModelObservedState::Absent);
    }
    let object = match json_root_object(&root, path) {
        Ok(object) => object,
        Err(_) => return Ok(ExternalModelObservedState::Conflicted),
    };
    if ensure_unique_keys(&object, path, "$root").is_err() {
        return Ok(ExternalModelObservedState::Conflicted);
    }
    let mut present = object.get("model").is_some() || object.get("apiKeyHelper").is_some();
    if object.get("env").is_some() {
        let Some(env) = object.object_value("env") else {
            return Ok(ExternalModelObservedState::Conflicted);
        };
        if ensure_unique_keys(&env, path, "env").is_err() {
            return Ok(ExternalModelObservedState::Conflicted);
        }
        present |= [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
        ]
        .iter()
        .any(|field| env.get(field).is_some());
    }
    Ok(if present {
        ExternalModelObservedState::Present
    } else {
        ExternalModelObservedState::Absent
    })
}

fn observe_external_codex(path: &Path) -> Result<ExternalModelObservedState, String> {
    let (document, original) = match read_toml(path) {
        Ok(value) => value,
        Err(_) if path.exists() => return Ok(ExternalModelObservedState::Conflicted),
        Err(error) => return Err(error),
    };
    if original.is_none() {
        return Ok(ExternalModelObservedState::Absent);
    }
    Ok(
        if document.as_table().contains_key("model")
            || document.as_table().contains_key("model_provider")
        {
            ExternalModelObservedState::Present
        } else {
            ExternalModelObservedState::Absent
        },
    )
}

fn observe_external_pi(
    models_path: &Path,
    settings_path: &Path,
) -> Result<ExternalModelObservedState, String> {
    let models_present = match read_jsonc(models_path) {
        Ok((_, None)) => false,
        Ok((root, Some(_))) => {
            let object = match json_root_object(&root, models_path) {
                Ok(object) => object,
                Err(_) => return Ok(ExternalModelObservedState::Conflicted),
            };
            if ensure_unique_keys(&object, models_path, "$root").is_err() {
                return Ok(ExternalModelObservedState::Conflicted);
            }
            if object.get("providers").is_some() {
                let Some(providers) = object.object_value("providers") else {
                    return Ok(ExternalModelObservedState::Conflicted);
                };
                if ensure_unique_keys(&providers, models_path, "providers").is_err() {
                    return Ok(ExternalModelObservedState::Conflicted);
                }
                !providers.properties().is_empty()
            } else {
                false
            }
        }
        Err(_) if models_path.exists() => return Ok(ExternalModelObservedState::Conflicted),
        Err(error) => return Err(error),
    };
    let settings_present = match read_jsonc(settings_path) {
        Ok((_, None)) => false,
        Ok((root, Some(_))) => {
            let object = match json_root_object(&root, settings_path) {
                Ok(object) => object,
                Err(_) => return Ok(ExternalModelObservedState::Conflicted),
            };
            if ensure_unique_keys(&object, settings_path, "$root").is_err() {
                return Ok(ExternalModelObservedState::Conflicted);
            }
            object.get("defaultProvider").is_some() || object.get("defaultModel").is_some()
        }
        Err(_) if settings_path.exists() => return Ok(ExternalModelObservedState::Conflicted),
        Err(error) => return Err(error),
    };
    Ok(if models_present || settings_present {
        ExternalModelObservedState::Present
    } else {
        ExternalModelObservedState::Absent
    })
}

fn observe_prepared(
    prepared: Result<(Option<String>, String), String>,
) -> Result<ModelObservedState, String> {
    match prepared {
        Ok((None, _)) => Ok(ModelObservedState::Missing),
        Ok((Some(original), candidate)) if original == candidate => Ok(ModelObservedState::Synced),
        Ok((Some(_), _)) => Ok(ModelObservedState::Drifted),
        Err(_) => Ok(ModelObservedState::Conflicted),
    }
}

fn combine_observed(
    left: Result<ModelObservedState, String>,
    right: Result<ModelObservedState, String>,
) -> Result<ModelObservedState, String> {
    use ModelObservedState::*;
    let (left, right) = (left?, right?);
    Ok(if left == Conflicted || right == Conflicted {
        Conflicted
    } else if left == Missing || right == Missing {
        Missing
    } else if left == Drifted || right == Drifted {
        Drifted
    } else {
        Synced
    })
}

fn profile_for_apply(profile_id: &str) -> Result<ModelProfile, String> {
    load_settings()
        .model_profiles
        .and_then(|profiles| profiles.get(profile_id).cloned())
        .ok_or_else(|| format!("unknown model profile: {profile_id}"))
}

fn ensure_supported(agent_id: &str, protocol: &ModelProtocol) -> Result<(), String> {
    let supported = match agent_id {
        "claude-code" => matches!(protocol, ModelProtocol::AnthropicMessages),
        "codex" => matches!(protocol, ModelProtocol::OpenaiResponses),
        "pi" => true,
        "qoder" => {
            return Err(format!(
                "Qoder custom models must currently be configured through /model; see {QODER_DOCS}"
            ))
        }
        "grok-build" => {
            return Err(format!(
                "Grok Build custom models require api_key or env_key and do not expose a secure per-model credential command; see {GROK_BUILD_MODEL_DOCS}"
            ))
        }
        "minimax-code" => {
            return Err(format!(
                "MiniMax Code custom providers currently persist options.apiKey in plaintext YAML and do not expose a Keychain-compatible credential command; see {MINIMAX_CODE_DOCS}"
            ))
        }
        _ => return Err(format!("unsupported model Agent: {agent_id}")),
    };
    if supported {
        Ok(())
    } else {
        Err(format!(
            "{} does not support the '{}' profile protocol in this MUX release",
            agent_id,
            protocol_name(protocol)
        ))
    }
}

fn protocol_name(protocol: &ModelProtocol) -> &'static str {
    match protocol {
        ModelProtocol::AnthropicMessages => "anthropic-messages",
        ModelProtocol::OpenaiResponses => "openai-responses",
        ModelProtocol::OpenaiCompletions => "openai-completions",
    }
}

pub fn apply_profile(agent_id: &str, profile_id: &str) -> Result<ModelApplyResult, String> {
    apply_profile_with_credential_presence(agent_id, profile_id, credential_exists(profile_id))
}

pub(crate) fn apply_profile_with_credential_presence(
    agent_id: &str,
    profile_id: &str,
    has_credential: bool,
) -> Result<ModelApplyResult, String> {
    let profile = profile_for_apply(profile_id)?;
    ensure_supported(agent_id, &profile.protocol)?;
    let result = match agent_id {
        "claude-code" => apply_claude(&profile, has_credential),
        "codex" => apply_codex(&profile, has_credential),
        "pi" => apply_pi(&profile, has_credential),
        _ => unreachable!("ensure_supported filtered unknown agents"),
    }?;

    mutate_settings(|settings| {
        settings
            .model_assignments
            .get_or_insert_with(BTreeMap::new)
            .insert(agent_id.into(), profile_id.into());
    })
    .map_err(|error| {
        format!("model config was applied, but MUX could not record the assignment: {error}")
    })?;
    Ok(result)
}

/// Remove only the fields owned by a previously assigned MUX Profile, then
/// clear the desired assignment. The caller must identify the exact Profile so
/// unrelated providers and Agent policy remain untouched.
pub fn clear_profile(agent_id: &str, profile_id: &str) -> Result<(), String> {
    let profile = profile_for_apply(profile_id)?;
    ensure_supported(agent_id, &profile.protocol)?;
    match observe_profile(agent_id, &profile)? {
        ModelObservedState::Synced => {}
        ModelObservedState::Missing => {}
        ModelObservedState::Drifted => {
            return Err("model_owned_fields_drift: review the Agent config before clearing".into())
        }
        ModelObservedState::Conflicted => {
            return Err("model_target_conflicted: the Agent config is ambiguous".into())
        }
    }
    match agent_id {
        "claude-code" => clear_one_model_file(
            &config_path(".claude/settings.json"),
            "claude-code",
            prepare_clear_claude,
        )?,
        "codex" => clear_one_model_file_with_profile(
            &config_path(".codex/config.toml"),
            "codex",
            &profile,
            prepare_clear_codex,
        )?,
        "pi" => clear_pi(&profile)?,
        _ => unreachable!("ensure_supported filtered unsupported model Agent"),
    }
    mutate_settings(|settings| {
        if let Some(assignments) = settings.model_assignments.as_mut() {
            if assignments.get(agent_id).is_some_and(|id| id == profile_id) {
                assignments.remove(agent_id);
            }
        }
    })
    .map_err(|error| error.to_string())
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {}", path.display(), error)),
    }
}

fn input_value(value: Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(value) => CstInputValue::Bool(value),
        Value::Number(value) => CstInputValue::Number(value.to_string()),
        Value::String(value) => CstInputValue::String(value),
        Value::Array(values) => CstInputValue::Array(values.into_iter().map(input_value).collect()),
        Value::Object(values) => CstInputValue::Object(
            values
                .into_iter()
                .map(|(name, value)| (name, input_value(value)))
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

fn ensure_unique(object: &CstObject, name: &str, path: &Path, context: &str) -> Result<(), String> {
    if property_count(object, name) > 1 {
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

fn set_json_property(
    object: &CstObject,
    name: &str,
    value: Option<Value>,
    path: &Path,
    context: &str,
) -> Result<(), String> {
    ensure_unique(object, name, path, context)?;
    match (object.get(name), value) {
        (Some(property), Some(value)) => property.set_value(input_value(value)),
        (None, Some(value)) => {
            object.append(name, input_value(value));
        }
        (Some(property), None) => property.remove(),
        (None, None) => {}
    }
    Ok(())
}

fn read_jsonc(path: &Path) -> Result<(CstRootNode, Option<String>), String> {
    let original = read_optional(path)?;
    let root = CstRootNode::parse(
        original.as_deref().unwrap_or_default(),
        &ParseOptions::default(),
    )
    .map_err(|error| {
        format!(
            "refusing to modify invalid JSON/JSONC at {}: {}",
            path.display(),
            error
        )
    })?;
    Ok((root, original))
}

fn json_root_object(root: &CstRootNode, path: &Path) -> Result<CstObject, String> {
    root.object_value_or_create().ok_or_else(|| {
        format!(
            "refusing to modify {}: JSON root is not an object",
            path.display()
        )
    })
}

fn truthy_json_value(node: Option<CstNode>) -> bool {
    node.and_then(|node| node.to_serde_value())
        .is_some_and(|value| match value {
            Value::Bool(value) => value,
            Value::String(value) => {
                let normalized = value.trim().to_ascii_lowercase();
                !normalized.is_empty() && normalized != "0" && normalized != "false"
            }
            Value::Null => false,
            _ => true,
        })
}

fn prepare_claude(
    path: &Path,
    profile: &ModelProfile,
    has_credential: bool,
) -> Result<(Option<String>, String), String> {
    let (root, original) = read_jsonc(path)?;
    let object = json_root_object(&root, path)?;
    ensure_unique_keys(&object, path, "$root")?;
    ensure_unique(&object, "env", path, "$root")?;

    let env = object.object_value_or_create("env").ok_or_else(|| {
        format!(
            "refusing to modify {}: 'env' is not an object",
            path.display()
        )
    })?;
    ensure_unique_keys(&env, path, "env")?;
    for field in [
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        if truthy_json_value(env.get(field).and_then(|property| property.value())) {
            return Err(format!(
                "refusing to replace active {} routing in {}",
                field,
                path.display()
            ));
        }
    }

    set_json_property(
        &object,
        "model",
        Some(Value::String(profile.model.clone())),
        path,
        "$root",
    )?;
    set_json_property(
        &object,
        "apiKeyHelper",
        has_credential.then(|| Value::String(security_shell_command(&profile.id))),
        path,
        "$root",
    )?;
    set_json_property(
        &env,
        "ANTHROPIC_BASE_URL",
        Some(Value::String(profile.base_url.clone())),
        path,
        "env",
    )?;
    // These have higher precedence than apiKeyHelper. Applying a MUX profile is
    // an explicit connection takeover, so remove only these conflicting fields.
    set_json_property(&env, "ANTHROPIC_AUTH_TOKEN", None, path, "env")?;
    set_json_property(&env, "ANTHROPIC_API_KEY", None, path, "env")?;
    Ok((original, root.to_string()))
}

fn prepare_clear_claude(path: &Path) -> Result<(Option<String>, String), String> {
    let (root, original) = read_jsonc(path)?;
    if original.is_none() {
        return Ok((None, String::new()));
    }
    let object = json_root_object(&root, path)?;
    ensure_unique_keys(&object, path, "$root")?;
    set_json_property(&object, "model", None, path, "$root")?;
    set_json_property(&object, "apiKeyHelper", None, path, "$root")?;
    if let Some(env) = object.object_value("env") {
        ensure_unique_keys(&env, path, "env")?;
        set_json_property(&env, "ANTHROPIC_BASE_URL", None, path, "env")?;
    }
    Ok((original, root.to_string()))
}

fn read_toml(path: &Path) -> Result<(Document, Option<String>), String> {
    let original = read_optional(path)?;
    let document = original
        .as_deref()
        .unwrap_or_default()
        .parse::<Document>()
        .map_err(|error| {
            format!(
                "refusing to modify invalid TOML at {}: {}",
                path.display(),
                error
            )
        })?;
    Ok((document, original))
}

fn prepare_codex(
    path: &Path,
    profile: &ModelProfile,
    has_credential: bool,
) -> Result<(Option<String>, String), String> {
    let (mut document, original) = read_toml(path)?;
    document["model"] = toml_edit::value(&profile.model);
    let provider_id = codex_provider_id(&profile.id);
    document["model_provider"] = toml_edit::value(&provider_id);

    if !document.as_table().contains_key("model_providers") {
        document
            .as_table_mut()
            .insert("model_providers", Item::Table(Table::new()));
    }
    let providers = document
        .get_mut("model_providers")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| {
            format!(
                "refusing to modify {}: 'model_providers' is not a TOML table",
                path.display()
            )
        })?;
    if !providers.contains_key(&provider_id) {
        providers.insert(&provider_id, Item::Table(Table::new()));
    }
    let provider = providers
        .get_mut(&provider_id)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| format!("MUX provider '{provider_id}' is not a TOML table"))?;
    provider.insert("name", toml_edit::value(&profile.name));
    provider.insert("base_url", toml_edit::value(&profile.base_url));
    provider.insert("wire_api", toml_edit::value("responses"));
    provider.remove("env_key");
    provider.remove("experimental_bearer_token");
    provider.remove("requires_openai_auth");

    if has_credential {
        let mut auth = Table::new();
        let command = security_command(&profile.id);
        auth.insert("command", toml_edit::value(&command[0]));
        let mut args = Array::new();
        for argument in &command[1..] {
            args.push(argument.as_str());
        }
        auth.insert("args", toml_edit::value(args));
        provider.insert("auth", Item::Table(auth));
    } else {
        provider.remove("auth");
    }
    Ok((original, document.to_string()))
}

fn prepare_clear_codex(
    path: &Path,
    profile: &ModelProfile,
) -> Result<(Option<String>, String), String> {
    let (mut document, original) = read_toml(path)?;
    if original.is_none() {
        return Ok((None, String::new()));
    }
    document.remove("model");
    document.remove("model_provider");
    if let Some(providers) = document
        .get_mut("model_providers")
        .and_then(Item::as_table_mut)
    {
        providers.remove(&codex_provider_id(&profile.id));
    }
    Ok((original, document.to_string()))
}

fn codex_provider_id(profile_id: &str) -> String {
    let mut encoded = String::from("mux_");
    for byte in profile_id.bytes() {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn pi_provider_value(profile: &ModelProfile, has_credential: bool) -> Value {
    let mut provider = serde_json::Map::from_iter([
        ("baseUrl".into(), Value::String(profile.base_url.clone())),
        (
            "api".into(),
            Value::String(protocol_name(&profile.protocol).into()),
        ),
    ]);
    if has_credential {
        provider.insert(
            "apiKey".into(),
            Value::String(format!("!{}", security_shell_command(&profile.id))),
        );
    }
    let mut model = serde_json::Map::from_iter([
        ("id".into(), Value::String(profile.model.clone())),
        ("name".into(), Value::String(profile.name.clone())),
        ("reasoning".into(), Value::Bool(profile.reasoning)),
        (
            "input".into(),
            Value::Array(vec![Value::String("text".into())]),
        ),
    ]);
    if let Some(value) = profile.context_window {
        model.insert("contextWindow".into(), Value::Number(value.into()));
    }
    if let Some(value) = profile.max_output_tokens {
        model.insert("maxTokens".into(), Value::Number(value.into()));
    }
    provider.insert("models".into(), Value::Array(vec![Value::Object(model)]));
    Value::Object(provider)
}

fn prepare_pi_models(
    path: &Path,
    profile: &ModelProfile,
    has_credential: bool,
) -> Result<(Option<String>, String), String> {
    let (root, original) = read_jsonc(path)?;
    let object = json_root_object(&root, path)?;
    ensure_unique_keys(&object, path, "$root")?;
    ensure_unique(&object, "providers", path, "$root")?;
    let providers = object.object_value_or_create("providers").ok_or_else(|| {
        format!(
            "refusing to modify {}: 'providers' is not an object",
            path.display()
        )
    })?;
    ensure_unique_keys(&providers, path, "providers")?;
    let provider_id = format!("mux-{}", profile.id);
    set_json_property(
        &providers,
        &provider_id,
        Some(pi_provider_value(profile, has_credential)),
        path,
        "providers",
    )?;
    Ok((original, root.to_string()))
}

fn prepare_pi_settings(
    path: &Path,
    profile: &ModelProfile,
) -> Result<(Option<String>, String), String> {
    let (root, original) = read_jsonc(path)?;
    let object = json_root_object(&root, path)?;
    ensure_unique_keys(&object, path, "$root")?;
    set_json_property(
        &object,
        "defaultProvider",
        Some(Value::String(format!("mux-{}", profile.id))),
        path,
        "$root",
    )?;
    set_json_property(
        &object,
        "defaultModel",
        Some(Value::String(profile.model.clone())),
        path,
        "$root",
    )?;
    Ok((original, root.to_string()))
}

fn prepare_clear_pi_models(
    path: &Path,
    profile: &ModelProfile,
) -> Result<(Option<String>, String), String> {
    let (root, original) = read_jsonc(path)?;
    if original.is_none() {
        return Ok((None, String::new()));
    }
    let object = json_root_object(&root, path)?;
    ensure_unique_keys(&object, path, "$root")?;
    if let Some(providers) = object.object_value("providers") {
        ensure_unique_keys(&providers, path, "providers")?;
        set_json_property(
            &providers,
            &format!("mux-{}", profile.id),
            None,
            path,
            "providers",
        )?;
    }
    Ok((original, root.to_string()))
}

fn prepare_clear_pi_settings(path: &Path) -> Result<(Option<String>, String), String> {
    let (root, original) = read_jsonc(path)?;
    if original.is_none() {
        return Ok((None, String::new()));
    }
    let object = json_root_object(&root, path)?;
    ensure_unique_keys(&object, path, "$root")?;
    set_json_property(&object, "defaultProvider", None, path, "$root")?;
    set_json_property(&object, "defaultModel", None, path, "$root")?;
    Ok((original, root.to_string()))
}

fn backup_config(path: &Path, agent: &str, stamp: &str) -> Result<(), String> {
    backup(path, &backups_dir(), stamp, agent, "model")
}

fn apply_claude(profile: &ModelProfile, has_credential: bool) -> Result<ModelApplyResult, String> {
    let path = config_path(".claude/settings.json");
    let (original, content) = prepare_claude(&path, profile, has_credential)?;
    backup_config(&path, "claude-code", &backup_timestamp())?;
    write_if_unchanged(&path, original.as_deref(), &content)?;
    Ok(ModelApplyResult {
        agent: "claude-code".into(),
        profile: profile.id.clone(),
        files: vec![path.display().to_string()],
        restart_required: true,
        message: "Claude Code model routing updated; start a new session to use it.".into(),
    })
}

fn apply_codex(profile: &ModelProfile, has_credential: bool) -> Result<ModelApplyResult, String> {
    let path = config_path(".codex/config.toml");
    let (original, content) = prepare_codex(&path, profile, has_credential)?;
    backup_config(&path, "codex", &backup_timestamp())?;
    write_if_unchanged(&path, original.as_deref(), &content)?;
    Ok(ModelApplyResult {
        agent: "codex".into(),
        profile: profile.id.clone(),
        files: vec![path.display().to_string()],
        restart_required: true,
        message: "Codex model provider updated; start a new session to use it.".into(),
    })
}

type ClearModelPrepare = fn(&Path) -> Result<(Option<String>, String), String>;
type ClearModelWithProfilePrepare =
    fn(&Path, &ModelProfile) -> Result<(Option<String>, String), String>;

fn clear_one_model_file(
    path: &Path,
    backup_name: &str,
    prepare: ClearModelPrepare,
) -> Result<(), String> {
    let (original, content) = prepare(path)?;
    let Some(original) = original else {
        return Ok(());
    };
    backup_config(path, backup_name, &backup_timestamp())?;
    write_if_unchanged(path, Some(&original), &content)
}

fn clear_one_model_file_with_profile(
    path: &Path,
    backup_name: &str,
    profile: &ModelProfile,
    prepare: ClearModelWithProfilePrepare,
) -> Result<(), String> {
    let (original, content) = prepare(path, profile)?;
    let Some(original) = original else {
        return Ok(());
    };
    backup_config(path, backup_name, &backup_timestamp())?;
    write_if_unchanged(path, Some(&original), &content)
}

fn rollback(path: &Path, original: Option<&str>, written: &str) -> Result<(), String> {
    match original {
        Some(original) => write_if_unchanged(path, Some(written), original),
        None => remove_if_unchanged(path, written),
    }
}

fn write_pi_transaction(
    models_path: &Path,
    models_original: Option<&str>,
    models_content: &str,
    settings_path: &Path,
    settings_original: Option<&str>,
    settings_content: &str,
) -> Result<(), String> {
    write_if_unchanged(models_path, models_original, models_content)?;
    if let Err(error) = write_if_unchanged(settings_path, settings_original, settings_content) {
        return match rollback(models_path, models_original, models_content) {
            Ok(()) => Err(format!(
                "Pi settings update failed and models.json was rolled back: {error}"
            )),
            Err(rollback_error) => Err(format!(
                "Pi settings update failed ({error}); models.json rollback also failed ({rollback_error})"
            )),
        };
    }
    Ok(())
}

fn apply_pi(profile: &ModelProfile, has_credential: bool) -> Result<ModelApplyResult, String> {
    let models_path = config_path(".pi/agent/models.json");
    let settings_path = config_path(".pi/agent/settings.json");
    let (models_original, models_content) =
        prepare_pi_models(&models_path, profile, has_credential)?;
    let (settings_original, settings_content) = prepare_pi_settings(&settings_path, profile)?;
    let stamp = backup_timestamp();
    backup_config(&models_path, "pi-models", &stamp)?;
    backup_config(&settings_path, "pi-settings", &stamp)?;

    write_pi_transaction(
        &models_path,
        models_original.as_deref(),
        &models_content,
        &settings_path,
        settings_original.as_deref(),
        &settings_content,
    )?;

    Ok(ModelApplyResult {
        agent: "pi".into(),
        profile: profile.id.clone(),
        files: vec![
            models_path.display().to_string(),
            settings_path.display().to_string(),
        ],
        restart_required: true,
        message: "Pi custom provider and default model updated.".into(),
    })
}

fn clear_pi(profile: &ModelProfile) -> Result<(), String> {
    let models_path = config_path(".pi/agent/models.json");
    let settings_path = config_path(".pi/agent/settings.json");
    let (models_original, models_content) = prepare_clear_pi_models(&models_path, profile)?;
    let (settings_original, settings_content) = prepare_clear_pi_settings(&settings_path)?;
    if models_original.is_none() && settings_original.is_none() {
        return Ok(());
    }
    let stamp = backup_timestamp();
    backup_config(&models_path, "pi-models", &stamp)?;
    backup_config(&settings_path, "pi-settings", &stamp)?;
    match (models_original.as_deref(), settings_original.as_deref()) {
        (Some(models_before), Some(settings_before)) => write_pi_transaction(
            &models_path,
            Some(models_before),
            &models_content,
            &settings_path,
            Some(settings_before),
            &settings_content,
        ),
        (Some(models_before), None) => {
            write_if_unchanged(&models_path, Some(models_before), &models_content)
        }
        (None, Some(settings_before)) => {
            write_if_unchanged(&settings_path, Some(settings_before), &settings_content)
        }
        (None, None) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testenv::TestHome;

    fn anthropic_profile() -> ModelProfile {
        ModelProfile {
            id: "team-anthropic".into(),
            name: "Team Anthropic".into(),
            protocol: ModelProtocol::AnthropicMessages,
            base_url: "https://gateway.example.test/anthropic".into(),
            model: "claude-sonnet-custom".into(),
            context_window: Some(200_000),
            max_output_tokens: Some(16_384),
            reasoning: true,
        }
    }

    fn responses_profile() -> ModelProfile {
        ModelProfile {
            id: "team-openai".into(),
            name: "Team OpenAI".into(),
            protocol: ModelProtocol::OpenaiResponses,
            base_url: "https://gateway.example.test/v1".into(),
            model: "gpt-custom".into(),
            context_window: Some(128_000),
            max_output_tokens: Some(16_000),
            reasoning: true,
        }
    }

    #[test]
    fn profile_metadata_never_serializes_a_credential() {
        let profile = responses_profile();
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("credential"));
        assert!(!json.contains("api_key"));
    }

    #[test]
    fn detects_agent_from_existing_config_without_shell_path() {
        let th = TestHome::new("model-agent-detect");
        fs::create_dir_all(th.home.join(".pi/agent")).unwrap();
        assert!(agent_installed(&[], &[".pi/agent"], &[]));
    }

    #[test]
    fn editing_profile_metadata_preserves_desired_assignments() {
        let _th = TestHome::new("model-profile-edit");
        let profile = responses_profile();
        save_profile(profile.clone(), None).unwrap();
        mutate_settings(|settings| {
            settings
                .model_assignments
                .get_or_insert_with(BTreeMap::new)
                .insert("codex".into(), profile.id.clone());
        })
        .unwrap();

        let mut changed = profile;
        changed.model = "gpt-custom-new".into();
        save_profile(changed, None).unwrap();

        assert_eq!(
            load_settings()
                .model_assignments
                .unwrap_or_default()
                .get("codex")
                .map(String::as_str),
            Some("team-openai")
        );
    }

    #[test]
    fn clearing_pi_does_not_create_a_missing_counterpart_file() {
        let th = TestHome::new("model-pi-clear-one-file");
        let models_path = th.home.join(".pi/agent/models.json");
        let settings_path = th.home.join(".pi/agent/settings.json");
        fs::create_dir_all(models_path.parent().unwrap()).unwrap();
        fs::write(
            &models_path,
            r#"{"providers":{"mux-team-openai":{"baseUrl":"https://gateway.example.test/v1","api":"openai-responses","models":[{"id":"gpt-custom","name":"Team OpenAI"}]}}}"#,
        )
        .unwrap();

        clear_pi(&responses_profile()).unwrap();

        assert!(models_path.exists());
        assert!(!settings_path.exists());
    }

    #[test]
    fn claude_patch_preserves_jsonc_and_unrelated_settings() {
        let th = TestHome::new("model-claude");
        let path = th.home.join(".claude/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{
  // keep this comment
  "permissions": { "allow": ["Bash(git:*)"] },
  "model": "old",
  "env": {
    "KEEP_ME": "yes",
    "ANTHROPIC_AUTH_TOKEN": "old-secret"
  }
}"#,
        )
        .unwrap();

        let (_, content) = prepare_claude(&path, &anthropic_profile(), true).unwrap();
        assert!(content.contains("keep this comment"));
        assert!(content.contains("KEEP_ME"));
        assert!(content.contains("permissions"));
        assert!(!content.contains("old-secret"));
        assert!(content.contains("apiKeyHelper"));
        assert!(content.contains("claude-sonnet-custom"));
    }

    #[test]
    fn claude_refuses_to_override_cloud_provider_routing() {
        let th = TestHome::new("model-claude-cloud");
        let path = th.home.join(".claude/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"env":{"CLAUDE_CODE_USE_BEDROCK":"1"}}"#;
        fs::write(&path, original).unwrap();
        assert!(prepare_claude(&path, &anthropic_profile(), true).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn codex_patch_preserves_mcp_and_other_provider_tables() {
        let th = TestHome::new("model-codex");
        let path = th.home.join(".codex/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"# keep this comment
model = "old"

[mcp_servers.github]
url = "https://example.test/mcp"

[model_providers.existing]
name = "Existing"
base_url = "https://existing.test/v1"
wire_api = "responses"
"#,
        )
        .unwrap();

        let (_, content) = prepare_codex(&path, &responses_profile(), true).unwrap();
        assert!(content.contains("keep this comment"));
        assert!(content.contains("mcp_servers.github"));
        assert!(content.contains("model_providers.existing"));
        assert!(content.contains("model_providers.mux_7465616d2d6f70656e6169"));
        assert!(content.contains("find-generic-password"));
    }

    #[test]
    fn pi_patch_accepts_jsonc_and_preserves_other_providers() {
        let th = TestHome::new("model-pi");
        let path = th.home.join(".pi/agent/models.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{
  // user provider
  "providers": {
    "local": { "baseUrl": "http://localhost:8080", "models": [], },
  },
}"#,
        )
        .unwrap();

        let (_, content) = prepare_pi_models(&path, &responses_profile(), true).unwrap();
        assert!(content.contains("user provider"));
        assert!(content.contains("\"local\""));
        assert!(content.contains("\"mux-team-openai\""));
        assert!(content.contains("!/usr/bin/security"));
    }

    #[test]
    fn protocol_matrix_rejects_incompatible_assignments() {
        assert!(ensure_supported("claude-code", &ModelProtocol::OpenaiResponses).is_err());
        assert!(ensure_supported("codex", &ModelProtocol::AnthropicMessages).is_err());
        assert!(ensure_supported("pi", &ModelProtocol::AnthropicMessages).is_ok());
        assert!(ensure_supported("qoder", &ModelProtocol::OpenaiResponses).is_err());
        assert!(ensure_supported("grok-build", &ModelProtocol::OpenaiResponses).is_err());
    }

    #[test]
    fn grok_build_is_a_guided_three_protocol_target() {
        let _th = TestHome::new("model-grok-build");
        let agents = list_agents();
        let grok = agents
            .iter()
            .find(|agent| agent.id == "grok-build")
            .expect("Grok Build model target");
        assert_eq!(grok.mode, "guided");
        assert_eq!(grok.config_path, "~/.grok/config.toml");
        assert_eq!(grok.supported_protocols.len(), 3);
        assert!(grok.docs.contains("11-custom-models.md"));
    }

    #[test]
    fn minimax_code_is_a_guided_three_protocol_target() {
        let _th = TestHome::new("model-minimax-code");
        let agents = list_agents();
        let minimax = agents
            .iter()
            .find(|agent| agent.id == "minimax-code")
            .expect("MiniMax Code model target");
        assert_eq!(minimax.mode, "guided");
        assert_eq!(minimax.config_path, "~/.mavis/config.yaml");
        assert_eq!(minimax.supported_protocols.len(), 3);
        assert!(minimax.docs.contains("agent.minimax.io/download"));
        assert!(ensure_supported("minimax-code", &ModelProtocol::OpenaiResponses).is_err());
    }

    #[test]
    fn isolated_apply_preserves_codex_mcp_and_records_assignment() {
        let th = TestHome::new("model-codex-apply");
        let path = th.home.join(".codex/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"model = "old"

[mcp_servers.keep]
url = "https://example.test/mcp"
"#,
        )
        .unwrap();
        save_profile(responses_profile(), None).unwrap();

        let result = apply_profile("codex", "team-openai").unwrap();

        assert_eq!(result.files, vec![path.display().to_string()]);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("mcp_servers.keep"));
        assert!(content.contains("model_providers.mux_7465616d2d6f70656e6169"));
        assert!(!content.contains("find-generic-password"));
        assert_eq!(
            load_settings()
                .model_assignments
                .unwrap()
                .get("codex")
                .map(String::as_str),
            Some("team-openai")
        );
        assert!(fs::read_dir(th.home.join(".mux/backups"))
            .unwrap()
            .next()
            .is_some());
    }

    #[test]
    fn pi_transaction_rolls_back_first_file_when_second_changed() {
        let th = TestHome::new("model-pi-rollback");
        let models = th.home.join("models.json");
        let settings = th.home.join("settings.json");
        fs::write(&models, "models-old").unwrap();
        fs::write(&settings, "settings-newer").unwrap();

        let error = write_pi_transaction(
            &models,
            Some("models-old"),
            "models-mux",
            &settings,
            Some("settings-old"),
            "settings-mux",
        )
        .unwrap_err();

        assert!(error.contains("rolled back"));
        assert_eq!(fs::read_to_string(models).unwrap(), "models-old");
        assert_eq!(fs::read_to_string(settings).unwrap(), "settings-newer");
    }

    #[test]
    fn codex_provider_ids_do_not_collapse_profile_punctuation() {
        assert_ne!(codex_provider_id("a-b"), codex_provider_id("a_b"));
        assert_ne!(codex_provider_id("a.b"), codex_provider_id("a_b"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "touches the current user's macOS Keychain"]
    fn macos_keychain_helper_round_trip() {
        let profile_id = format!("mux-smoke-{}", std::process::id());
        let _ = delete_credential(&profile_id);
        set_credential(&profile_id, b"mux-smoke-value").unwrap();
        let actual = read_credential(&profile_id);
        let helper = std::process::Command::new("/usr/bin/security")
            .args(&security_command(&profile_id)[1..])
            .output()
            .unwrap();
        let cleanup = delete_credential(&profile_id);
        assert_eq!(actual.as_deref(), Some(b"mux-smoke-value".as_slice()));
        assert!(helper.status.success());
        assert_eq!(
            String::from_utf8(helper.stdout).unwrap().trim(),
            "mux-smoke-value"
        );
        cleanup.unwrap();
        assert!(!credential_exists(&profile_id));
    }
}
