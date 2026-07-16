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
use toml_edit::{Array, Document, Item, Table};

const KEYCHAIN_ACCOUNT: &str = "api-key";
const QODER_DOCS: &str = "https://docs.qoder.com/user-guide/chat/custom-models";
const GROK_BUILD_MODEL_DOCS: &str = "https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md";

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
    None
}

fn credential_exists(profile_id: &str) -> bool {
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
    Err("secure model credentials are currently supported on macOS only".into())
}

#[cfg(target_os = "macos")]
fn delete_credential(profile_id: &str) -> Result<(), String> {
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
        let metadata_changed = settings
            .model_profiles
            .as_ref()
            .and_then(|profiles| profiles.get(&profile_id))
            .is_some_and(|existing| existing != &profile);
        settings
            .model_profiles
            .get_or_insert_with(BTreeMap::new)
            .insert(profile_id.clone(), profile);
        if metadata_changed {
            if let Some(assignments) = settings.model_assignments.as_mut() {
                assignments.retain(|_, assigned| assigned != &profile_id);
            }
        }
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
    if let Err(error) = mutate_settings(|settings| {
        if let Some(profiles) = settings.model_profiles.as_mut() {
            profiles.remove(profile_id);
        }
        if let Some(assignments) = settings.model_assignments.as_mut() {
            assignments.retain(|_, assigned| assigned != profile_id);
        }
    }) {
        if let Some(credential) = previous_credential {
            let _ = set_credential(profile_id, &credential);
        }
        return Err(error.to_string());
    }
    Ok(())
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
    let profile = profile_for_apply(profile_id)?;
    ensure_supported(agent_id, &profile.protocol)?;
    let has_credential = credential_exists(profile_id);
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
    fn editing_profile_metadata_clears_stale_assignments() {
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

        assert!(load_settings()
            .model_assignments
            .unwrap_or_default()
            .get("codex")
            .is_none());
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
