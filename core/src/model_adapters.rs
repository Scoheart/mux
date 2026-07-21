//! Lossless native Model configuration adapters for multi-model Agents.
//!
//! Each adapter owns only MUX-prefixed provider entries plus the documented
//! primary/current pointer. Unknown fields remain untouched and credentials are
//! represented only by the Agent's official environment-variable syntax.

use crate::scanner::expand_tilde;
use crate::types::{ModelProfile, ModelProtocol};
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{ArrayOfTables, Document, Item, Table};
use yaml_edit::{Document as YamlDocument, Mapping as YamlMapping, YamlFile, YamlNode};

#[derive(Debug, Clone)]
pub struct PreparedModelFile {
    pub path: PathBuf,
    pub original: Option<String>,
    /// `None` means the MUX-owned file should be removed.
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservedActiveModel {
    Managed(String),
    External,
    None,
    Conflicted,
}

pub fn prepare_apply(
    agent_id: &str,
    paths: &[PathBuf],
    profile: &ModelProfile,
    active: bool,
) -> Result<Vec<PreparedModelFile>, String> {
    let prepared = match agent_id {
        "opencode" | "kilo-code" => prepare_open_code(agent_id, &paths[0], profile, active)?,
        "qwen-code" => prepare_qwen(&paths[0], profile, active)?,
        "crush" => prepare_crush(&paths[0], profile, active)?,
        "mistral-vibe" => prepare_vibe(&paths[0], profile, active)?,
        "factory-droid" => prepare_factory(&paths[0], profile, active)?,
        "hermes" => prepare_hermes(&paths[0], profile, active)?,
        "goose" => return prepare_goose(&paths[0], profile, active),
        _ => return Err(format!("unsupported multi-model Agent: {agent_id}")),
    };
    Ok(vec![prepared])
}

pub fn prepare_clear(
    agent_id: &str,
    paths: &[PathBuf],
    profile: &ModelProfile,
) -> Result<Vec<PreparedModelFile>, String> {
    let prepared = match agent_id {
        "opencode" | "kilo-code" => prepare_clear_open_code(agent_id, &paths[0], profile)?,
        "qwen-code" => prepare_clear_qwen(&paths[0], profile)?,
        "crush" => prepare_clear_crush(&paths[0], profile)?,
        "mistral-vibe" => prepare_clear_vibe(&paths[0], profile)?,
        "factory-droid" => prepare_clear_factory(&paths[0], profile)?,
        "hermes" => prepare_clear_hermes(&paths[0], profile)?,
        "goose" => return prepare_clear_goose(&paths[0], profile),
        _ => return Err(format!("unsupported multi-model Agent: {agent_id}")),
    };
    Ok(vec![prepared])
}

pub fn target_files(
    agent_id: &str,
    config_paths: &[String],
    profile_ids: &[String],
    profiles: &std::collections::BTreeMap<String, ModelProfile>,
) -> Vec<String> {
    let mut paths = config_paths.to_vec();
    if agent_id == "goose" {
        let config = config_paths
            .first()
            .map(|path| expand_tilde(path))
            .unwrap_or_default();
        let directory = config
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("custom_providers");
        paths.extend(profile_ids.iter().filter_map(|profile_id| {
            let provider = profiles
                .get(profile_id)
                .map(|profile| provider_id_for("goose", profile))
                .unwrap_or_else(|| provider_id(profile_id));
            safe_provider_id(&provider).then(|| {
                crate::scanner::collapse_home(
                    &directory.join(format!("{provider}.json")).to_string_lossy(),
                )
            })
        }));
    }
    paths.sort();
    paths.dedup();
    paths
}

pub fn observe_prepared_files(
    prepared: Result<Vec<PreparedModelFile>, String>,
) -> crate::models::ModelObservedState {
    use crate::models::ModelObservedState::{Conflicted, Drifted, Missing, Synced};
    let Ok(files) = prepared else {
        return Conflicted;
    };
    let mut missing = false;
    let mut drifted = false;
    for file in files {
        match (&file.original, &file.content) {
            (None, Some(_)) => missing = true,
            (Some(original), Some(content)) if original != content => drifted = true,
            (Some(_), None) => drifted = true,
            _ => {}
        }
    }
    if missing {
        Missing
    } else if drifted {
        Drifted
    } else {
        Synced
    }
}

/// True when clearing this Profile would be a no-op. This distinguishes a
/// missing Profile entry from drift in another Profile that shares the same
/// Agent config file.
pub fn cleared_profile_absent(
    prepared: Result<Vec<PreparedModelFile>, String>,
) -> Result<bool, String> {
    Ok(prepared?.iter().all(|file| match &file.original {
        None => true,
        Some(original) => file.content.as_deref() == Some(original.as_str()),
    }))
}

pub fn observe_external(agent_id: &str, path: &Path) -> crate::models::ExternalModelObservedState {
    use crate::models::ExternalModelObservedState::{Absent, Conflicted, Present};
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Absent,
        Err(_) => return Conflicted,
    };
    let present = match agent_id {
        "mistral-vibe" => content.parse::<Document>().ok().is_some_and(|document| {
            document.contains_key("active_model")
                || document.get("providers").is_some()
                || document.get("models").is_some()
        }),
        "hermes" | "goose" => serde_yaml::from_str::<Value>(&content)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .is_some_and(|root| {
                root.contains_key("model")
                    || root.contains_key("active_provider")
                    || root.get("providers").is_some_and(|value| {
                        value
                            .as_object()
                            .is_some_and(|providers| !providers.is_empty())
                    })
            }),
        _ => read_jsonc(path)
            .ok()
            .and_then(|(root, original)| original.map(|_| root))
            .and_then(|root| root.to_serde_value())
            .and_then(|value| value.as_object().cloned())
            .is_some_and(|root| {
                [
                    "model",
                    "provider",
                    "providers",
                    "modelProviders",
                    "customModels",
                ]
                .iter()
                .any(|key| root.contains_key(*key))
            }),
    };
    if present {
        Present
    } else {
        Absent
    }
}

pub fn observe_active(
    agent_id: &str,
    paths: &[PathBuf],
    profiles: &std::collections::BTreeMap<String, ModelProfile>,
) -> ObservedActiveModel {
    let selected = match agent_id {
        "opencode" | "kilo-code" => json_string(&paths[0], &["model"]),
        "qwen-code" => json_string(&paths[0], &["model", "name"])
            .zip(json_string(
                &paths[0],
                &["security", "auth", "selectedType"],
            ))
            .map(|(model, auth)| format!("{auth}::{model}")),
        "crush" => json_string(&paths[0], &["models", "large", "provider"]),
        "factory-droid" => json_string(&paths[0], &["model"]),
        "mistral-vibe" => read_optional(&paths[0]).ok().flatten().and_then(|content| {
            content
                .parse::<Document>()
                .ok()?
                .get("active_model")?
                .as_str()
                .map(str::to_string)
        }),
        "hermes" => yaml_string(&paths[0], &["model", "provider"]),
        "goose" => yaml_string(&paths[0], &["active_provider"]),
        _ => None,
    };
    let Some(selected) = selected else {
        return ObservedActiveModel::None;
    };
    let matches: Vec<_> = profiles
        .values()
        .filter(|profile| match agent_id {
            "opencode" | "kilo-code" => {
                selected == format!("{}/{}", provider_id_for(agent_id, profile), profile.model)
            }
            "qwen-code" => selected == format!("{}::{}", qwen_auth_type(profile), profile.model),
            "factory-droid" => selected == profile.model,
            "hermes" => selected == format!("custom:{}", provider_id_for(agent_id, profile)),
            _ => selected == provider_id_for(agent_id, profile),
        })
        .map(|profile| profile.id.clone())
        .collect();
    match matches.as_slice() {
        [profile_id] => ObservedActiveModel::Managed(profile_id.clone()),
        [] => ObservedActiveModel::External,
        _ => ObservedActiveModel::Conflicted,
    }
}

fn json_string(path: &Path, keys: &[&str]) -> Option<String> {
    let (root, original) = read_jsonc(path).ok()?;
    original?;
    let mut value = root.to_serde_value()?;
    for key in keys {
        value = value.get(*key)?.clone();
    }
    value.as_str().map(str::to_string)
}

fn yaml_string(path: &Path, keys: &[&str]) -> Option<String> {
    let content = read_optional(path).ok()??;
    let mut value: Value = serde_yaml::from_str(&content).ok()?;
    for key in keys {
        value = value.get(*key)?.clone();
    }
    value.as_str().map(str::to_string)
}

fn provider_id(profile_id: &str) -> String {
    let mut encoded = String::from("mux_");
    for byte in profile_id.bytes() {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn provider_id_for(agent_id: &str, profile: &ModelProfile) -> String {
    profile
        .native_ids
        .get(agent_id)
        .cloned()
        .unwrap_or_else(|| provider_id(&profile.id))
}

fn safe_provider_id(provider: &str) -> bool {
    !provider.is_empty()
        && provider.len() <= 128
        && provider != "."
        && provider != ".."
        && provider
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn env_ref(profile: &ModelProfile, open: &str, close: &str) -> Option<String> {
    profile
        .env_key
        .as_ref()
        .map(|key| format!("{open}{key}{close}"))
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
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
                .map(|(key, value)| (key, input_value(value)))
                .collect(),
        ),
    }
}

fn read_jsonc(path: &Path) -> Result<(CstRootNode, Option<String>), String> {
    let original = read_optional(path)?;
    let root = CstRootNode::parse(
        original.as_deref().unwrap_or_default(),
        &ParseOptions::default(),
    )
    .map_err(|error| {
        format!(
            "refusing to modify invalid JSON/JSONC at {}: {error}",
            path.display()
        )
    })?;
    Ok((root, original))
}

fn root_object(root: &CstRootNode, path: &Path) -> Result<CstObject, String> {
    root.object_value_or_create().ok_or_else(|| {
        format!(
            "refusing to modify {}: JSON root is not an object",
            path.display()
        )
    })
}

fn ensure_unique(object: &CstObject, path: &Path, context: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for property in object.properties() {
        let Some(name) = property.name().and_then(|name| name.decoded_value().ok()) else {
            continue;
        };
        if !seen.insert(name.clone()) {
            return Err(format!(
                "refusing to modify {}: duplicate JSON key '{context}.{name}'",
                path.display()
            ));
        }
    }
    Ok(())
}

fn set_json(object: &CstObject, key: &str, value: Option<Value>) {
    match (object.get(key), value) {
        (Some(property), Some(value)) => property.set_value(input_value(value)),
        (None, Some(value)) => {
            object.append(key, input_value(value));
        }
        (Some(property), None) => property.remove(),
        (None, None) => {}
    }
}

fn array_value(object: &CstObject, key: &str) -> Vec<Value> {
    object
        .get(key)
        .and_then(|property| property.value())
        .and_then(|node| node.to_serde_value())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

fn prepared_json(path: &Path, original: Option<String>, root: CstRootNode) -> PreparedModelFile {
    PreparedModelFile {
        path: path.into(),
        original,
        content: Some(root.to_string()),
    }
}

fn open_code_provider(profile: &ModelProfile) -> Value {
    let package = match profile.protocol {
        ModelProtocol::AnthropicMessages => "@ai-sdk/anthropic",
        ModelProtocol::OpenaiResponses => "@ai-sdk/openai",
        ModelProtocol::OpenaiCompletions => "@ai-sdk/openai-compatible",
    };
    let mut options = Map::from_iter([("baseURL".into(), Value::String(profile.base_url.clone()))]);
    if let Some(value) = env_ref(profile, "{env:", "}") {
        options.insert("apiKey".into(), Value::String(value));
    }
    let mut model = Map::from_iter([
        ("id".into(), Value::String(profile.model.clone())),
        ("name".into(), Value::String(profile.name.clone())),
        ("reasoning".into(), Value::Bool(profile.reasoning)),
    ]);
    if profile.context_window.is_some() || profile.max_output_tokens.is_some() {
        let mut limit = Map::new();
        if let Some(value) = profile.context_window {
            limit.insert("context".into(), value.into());
        }
        if let Some(value) = profile.max_output_tokens {
            limit.insert("output".into(), value.into());
        }
        model.insert("limit".into(), Value::Object(limit));
    }
    json!({
        "name": profile.name,
        "npm": package,
        "options": options,
        "models": { profile.model.clone(): Value::Object(model) }
    })
}

fn prepare_open_code(
    agent_id: &str,
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    ensure_unique(&object, path, "$root")?;
    let providers = object.object_value_or_create("provider").ok_or_else(|| {
        format!(
            "refusing to modify {}: provider is not an object",
            path.display()
        )
    })?;
    ensure_unique(&providers, path, "provider")?;
    set_json(
        &providers,
        &provider_id_for(agent_id, profile),
        Some(open_code_provider(profile)),
    );
    if active {
        set_json(
            &object,
            "model",
            Some(Value::String(format!(
                "{}/{}",
                provider_id_for(agent_id, profile),
                profile.model
            ))),
        );
    }
    Ok(prepared_json(path, original, root))
}

fn prepare_clear_open_code(
    agent_id: &str,
    path: &Path,
    profile: &ModelProfile,
) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    ensure_unique(&object, path, "$root")?;
    if let Some(providers) = object.object_value("provider") {
        ensure_unique(&providers, path, "provider")?;
        set_json(&providers, &provider_id_for(agent_id, profile), None);
    }
    let selected = format!("{}/{}", provider_id_for(agent_id, profile), profile.model);
    let is_selected = object
        .get("model")
        .and_then(|p| p.value())
        .and_then(|n| n.to_serde_value())
        .and_then(|v| v.as_str().map(str::to_string))
        .as_deref()
        == Some(selected.as_str());
    if is_selected {
        set_json(&object, "model", None);
    }
    Ok(prepared_json(path, original, root))
}

fn qwen_auth_type(profile: &ModelProfile) -> &'static str {
    match profile.protocol {
        ModelProtocol::AnthropicMessages => "anthropic",
        _ => "openai",
    }
}

fn qwen_model(profile: &ModelProfile) -> Value {
    let mut value = Map::from_iter([
        ("id".into(), Value::String(profile.model.clone())),
        ("name".into(), Value::String(profile.name.clone())),
        ("baseUrl".into(), Value::String(profile.base_url.clone())),
    ]);
    if let Some(env_key) = &profile.env_key {
        value.insert("envKey".into(), Value::String(env_key.clone()));
    }
    if let Some(context) = profile.context_window {
        value.insert(
            "generationConfig".into(),
            json!({"contextWindowSize": context}),
        );
    }
    Value::Object(value)
}

fn qwen_matches(value: &Value, profile: &ModelProfile) -> bool {
    value.get("id").and_then(Value::as_str) == Some(profile.model.as_str())
        && value.get("baseUrl").and_then(Value::as_str) == Some(profile.base_url.as_str())
}

fn qwen_providers(object: &CstObject, path: &Path) -> Result<Option<Map<String, Value>>, String> {
    let Some(property) = object.get("modelProviders") else {
        return Ok(None);
    };
    property
        .value()
        .and_then(|node| node.to_serde_value())
        .and_then(|value| value.as_object().cloned())
        .map(Some)
        .ok_or_else(|| {
            format!(
                "refusing to modify {}: Qwen modelProviders is not an object",
                path.display()
            )
        })
}

fn qwen_provider_models(
    value: Option<Value>,
    auth: &str,
    path: &Path,
) -> Result<Vec<Value>, String> {
    // Qwen Code 0.20.0 still consumes `modelProviders.<auth>` as
    // `ModelConfig[]` and skips non-arrays. Older MUX builds followed a newer
    // docs draft and wrote `{ protocol, models }`, so migrate only that exact
    // wrapper and fail closed when it contains any Agent-owned extension.
    match value {
        None => Ok(Vec::new()),
        Some(Value::Array(models)) => Ok(models),
        Some(Value::Object(mut legacy)) => {
            let protocol = legacy.remove("protocol");
            let models = legacy.remove("models");
            if !legacy.is_empty()
                || protocol.as_ref().and_then(Value::as_str) != Some(auth)
                || !matches!(models, Some(Value::Array(_)))
            {
                return Err(format!(
                    "refusing to modify {}: Qwen modelProviders.{auth} is neither the stable array shape nor the exact legacy MUX wrapper",
                    path.display()
                ));
            }
            let Some(Value::Array(models)) = models else {
                unreachable!("legacy Qwen models shape was checked above")
            };
            Ok(models)
        }
        Some(_) => Err(format!(
            "refusing to modify {}: Qwen modelProviders.{auth} must be an array",
            path.display()
        )),
    }
}

fn prepare_qwen(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    ensure_unique(&object, path, "$root")?;
    let mut providers = qwen_providers(&object, path)?.unwrap_or_default();
    let auth = qwen_auth_type(profile);
    let mut models = qwen_provider_models(providers.remove(auth), auth, path)?;
    models.retain(|value| !qwen_matches(value, profile));
    models.push(qwen_model(profile));
    providers.insert(auth.into(), Value::Array(models));
    set_json(&object, "modelProviders", Some(Value::Object(providers)));
    if active {
        let model = object
            .object_value_or_create("model")
            .ok_or_else(|| "Qwen model is not an object".to_string())?;
        set_json(&model, "name", Some(Value::String(profile.model.clone())));
        let security = object
            .object_value_or_create("security")
            .ok_or_else(|| "Qwen security is not an object".to_string())?;
        let auth_object = security
            .object_value_or_create("auth")
            .ok_or_else(|| "Qwen security.auth is not an object".to_string())?;
        set_json(
            &auth_object,
            "selectedType",
            Some(Value::String(auth.into())),
        );
    }
    Ok(prepared_json(path, original, root))
}

fn prepare_clear_qwen(path: &Path, profile: &ModelProfile) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    ensure_unique(&object, path, "$root")?;
    let auth = qwen_auth_type(profile);
    if let Some(mut providers) = qwen_providers(&object, path)? {
        if let Some(provider) = providers.remove(auth) {
            let mut models = qwen_provider_models(Some(provider), auth, path)?;
            models.retain(|value| !qwen_matches(value, profile));
            if !models.is_empty() {
                providers.insert(auth.into(), Value::Array(models));
            }
        }
        set_json(&object, "modelProviders", Some(Value::Object(providers)));
    }
    if let Some(model) = object.object_value("model") {
        let selected = model
            .get("name")
            .and_then(|p| p.value())
            .and_then(|n| n.to_serde_value());
        if selected.as_ref().and_then(Value::as_str) == Some(profile.model.as_str()) {
            set_json(&model, "name", None);
            if let Some(security) = object.object_value("security") {
                if let Some(auth_object) = security.object_value("auth") {
                    set_json(&auth_object, "selectedType", None);
                }
            }
        }
    }
    Ok(prepared_json(path, original, root))
}

fn crush_provider(profile: &ModelProfile) -> Value {
    let provider_type = match profile.protocol {
        ModelProtocol::AnthropicMessages => "anthropic",
        _ => "openai-compat",
    };
    let mut value = Map::from_iter([
        (
            "id".into(),
            Value::String(provider_id_for("crush", profile)),
        ),
        ("name".into(), Value::String(profile.name.clone())),
        ("base_url".into(), Value::String(profile.base_url.clone())),
        ("type".into(), Value::String(provider_type.into())),
        ("disable".into(), Value::Bool(false)),
        (
            "models".into(),
            json!([{"id": profile.model, "name": profile.name}]),
        ),
    ]);
    if let Some(env_key) = &profile.env_key {
        value.insert("api_key".into(), Value::String(format!("${env_key}")));
    }
    Value::Object(value)
}

fn prepare_crush(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    ensure_unique(&object, path, "$root")?;
    let providers = object
        .object_value_or_create("providers")
        .ok_or_else(|| "Crush providers is not an object".to_string())?;
    set_json(
        &providers,
        &provider_id_for("crush", profile),
        Some(crush_provider(profile)),
    );
    if active {
        let models = object
            .object_value_or_create("models")
            .ok_or_else(|| "Crush models is not an object".to_string())?;
        set_json(
            &models,
            "large",
            Some(json!({"model": profile.model, "provider": provider_id_for("crush", profile)})),
        );
    }
    Ok(prepared_json(path, original, root))
}

fn selected_model_matches(value: &Value, profile: &ModelProfile) -> bool {
    value.get("provider").and_then(Value::as_str)
        == Some(provider_id_for("crush", profile).as_str())
        && value.get("model").and_then(Value::as_str) == Some(profile.model.as_str())
}

fn prepare_clear_crush(path: &Path, profile: &ModelProfile) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    if let Some(providers) = object.object_value("providers") {
        set_json(&providers, &provider_id_for("crush", profile), None);
    }
    if let Some(models) = object.object_value("models") {
        let slots: Vec<String> = models
            .properties()
            .into_iter()
            .filter_map(|p| p.name()?.decoded_value().ok())
            .collect();
        for slot in slots {
            let matches = models
                .get(&slot)
                .and_then(|p| p.value())
                .and_then(|n| n.to_serde_value())
                .is_some_and(|value| selected_model_matches(&value, profile));
            if matches {
                set_json(&models, &slot, None);
            }
        }
    }
    Ok(prepared_json(path, original, root))
}

fn read_toml(path: &Path) -> Result<(Document, Option<String>), String> {
    let original = read_optional(path)?;
    let document = original
        .as_deref()
        .unwrap_or_default()
        .parse::<Document>()
        .map_err(|error| {
            format!(
                "refusing to modify invalid TOML at {}: {error}",
                path.display()
            )
        })?;
    Ok((document, original))
}

fn table_string(table: &Table, key: &str) -> Option<String> {
    table.get(key).and_then(Item::as_str).map(str::to_string)
}

fn remove_aot_matching<F>(aot: &mut ArrayOfTables, predicate: F)
where
    F: Fn(&Table) -> bool,
{
    let mut index = aot.len();
    while index > 0 {
        index -= 1;
        if aot.get(index).is_some_and(&predicate) {
            aot.remove(index);
        }
    }
}

fn ensure_aot<'a>(document: &'a mut Document, key: &str) -> Result<&'a mut ArrayOfTables, String> {
    if !document.contains_key(key) {
        document[key] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    document
        .get_mut(key)
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| format!("{key} is not an array of tables"))
}

fn prepare_vibe(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<PreparedModelFile, String> {
    let (mut document, original) = read_toml(path)?;
    let id = provider_id_for("mistral-vibe", profile);
    let providers = ensure_aot(&mut document, "providers")?;
    remove_aot_matching(providers, |table| {
        table_string(table, "name").as_deref() == Some(id.as_str())
    });
    let mut provider = Table::new();
    provider.insert("name", toml_edit::value(&id));
    provider.insert("api_base", toml_edit::value(&profile.base_url));
    if let Some(env_key) = &profile.env_key {
        provider.insert("api_key_env_var", toml_edit::value(env_key));
    }
    provider.insert("api_style", toml_edit::value("openai"));
    provider.insert("backend", toml_edit::value("generic"));
    providers.push(provider);
    let models = ensure_aot(&mut document, "models")?;
    remove_aot_matching(models, |table| {
        table_string(table, "alias").as_deref() == Some(id.as_str())
    });
    let mut model = Table::new();
    model.insert("name", toml_edit::value(&profile.model));
    model.insert("provider", toml_edit::value(&id));
    model.insert("alias", toml_edit::value(&id));
    models.push(model);
    if active {
        document["active_model"] = toml_edit::value(&id);
    }
    Ok(PreparedModelFile {
        path: path.into(),
        original,
        content: Some(document.to_string()),
    })
}

fn prepare_clear_vibe(path: &Path, profile: &ModelProfile) -> Result<PreparedModelFile, String> {
    let (mut document, original) = read_toml(path)?;
    let id = provider_id_for("mistral-vibe", profile);
    if let Some(providers) = document
        .get_mut("providers")
        .and_then(Item::as_array_of_tables_mut)
    {
        remove_aot_matching(providers, |table| {
            table_string(table, "name").as_deref() == Some(id.as_str())
        });
    }
    if let Some(models) = document
        .get_mut("models")
        .and_then(Item::as_array_of_tables_mut)
    {
        remove_aot_matching(models, |table| {
            table_string(table, "alias").as_deref() == Some(id.as_str())
        });
    }
    if document.get("active_model").and_then(Item::as_str) == Some(id.as_str()) {
        document.remove("active_model");
    }
    Ok(PreparedModelFile {
        path: path.into(),
        original,
        content: Some(document.to_string()),
    })
}

fn factory_model(profile: &ModelProfile) -> Value {
    let provider = match profile.protocol {
        ModelProtocol::AnthropicMessages => "anthropic",
        ModelProtocol::OpenaiResponses => "openai",
        ModelProtocol::OpenaiCompletions => "generic-chat-completion-api",
    };
    let mut value = Map::from_iter([
        ("model".into(), Value::String(profile.model.clone())),
        ("displayName".into(), Value::String(profile.name.clone())),
        ("baseUrl".into(), Value::String(profile.base_url.clone())),
        ("provider".into(), Value::String(provider.into())),
    ]);
    if let Some(reference) = env_ref(profile, "${", "}") {
        value.insert("apiKey".into(), Value::String(reference));
    }
    if let Some(tokens) = profile.max_output_tokens {
        value.insert("maxOutputTokens".into(), tokens.into());
    }
    Value::Object(value)
}

fn factory_matches(value: &Value, profile: &ModelProfile) -> bool {
    value.get("model").and_then(Value::as_str) == Some(profile.model.as_str())
        && value.get("baseUrl").and_then(Value::as_str) == Some(profile.base_url.as_str())
}

fn prepare_factory(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    let mut models = array_value(&object, "customModels");
    models.retain(|value| !factory_matches(value, profile));
    models.push(factory_model(profile));
    set_json(&object, "customModels", Some(Value::Array(models)));
    if active {
        set_json(&object, "model", Some(Value::String(profile.model.clone())));
    }
    Ok(prepared_json(path, original, root))
}

fn prepare_clear_factory(path: &Path, profile: &ModelProfile) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path)?;
    let object = root_object(&root, path)?;
    if object.get("customModels").is_some() {
        let mut models = array_value(&object, "customModels");
        models.retain(|value| !factory_matches(value, profile));
        set_json(&object, "customModels", Some(Value::Array(models)));
    }
    let selected = object
        .get("model")
        .and_then(|p| p.value())
        .and_then(|n| n.to_serde_value());
    if selected.as_ref().and_then(Value::as_str) == Some(profile.model.as_str()) {
        set_json(&object, "model", None);
    }
    Ok(prepared_json(path, original, root))
}

// YAML adapters use yaml-edit for safe in-place additions and validated exact
// text blocks for deletion so comments and unknown fields remain intact.
fn prepare_hermes(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<PreparedModelFile, String> {
    prepare_yaml_model(path, profile, active, YamlAgent::Hermes)
}
fn prepare_clear_hermes(path: &Path, profile: &ModelProfile) -> Result<PreparedModelFile, String> {
    prepare_clear_yaml_model(path, profile, YamlAgent::Hermes)
}
fn prepare_goose(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
) -> Result<Vec<PreparedModelFile>, String> {
    let config = prepare_yaml_model(path, profile, active, YamlAgent::Goose)?;
    let provider_path = goose_provider_path(path, profile)?;
    let original = read_optional(&provider_path)?;
    let engine = if profile.protocol == ModelProtocol::AnthropicMessages {
        "anthropic"
    } else {
        "openai"
    };
    let context = profile.context_window.unwrap_or(128_000);
    let provider = json!({
        "name": provider_id_for("goose", profile), "engine": engine, "display_name": profile.name,
        "description": format!("MUX managed {} provider", profile.name),
        "api_key_env": profile.env_key.clone().unwrap_or_default(), "base_url": profile.base_url,
        "models": [{"name": profile.model, "context_limit": context, "input_token_cost": null,
            "output_token_cost": null, "currency": null, "supports_cache_control": null}],
        "headers": null, "timeout_seconds": null, "supports_streaming": true,
        "requires_auth": profile.env_key.is_some(), "dynamic_models": false,
        "skip_canonical_filtering": true, "setup_steps": [],
        "preserves_thinking": profile.reasoning
    });
    let content =
        serde_json::to_string_pretty(&provider).map_err(|error| error.to_string())? + "\n";
    Ok(vec![
        config,
        PreparedModelFile {
            path: provider_path,
            original,
            content: Some(content),
        },
    ])
}
fn prepare_clear_goose(
    path: &Path,
    profile: &ModelProfile,
) -> Result<Vec<PreparedModelFile>, String> {
    let config = prepare_clear_yaml_model(path, profile, YamlAgent::Goose)?;
    let provider_path = goose_provider_path(path, profile)?;
    let original = read_optional(&provider_path)?;
    Ok(vec![
        config,
        PreparedModelFile {
            path: provider_path,
            original,
            content: None,
        },
    ])
}

fn goose_provider_path(config: &Path, profile: &ModelProfile) -> Result<PathBuf, String> {
    let provider = provider_id_for("goose", profile);
    if !safe_provider_id(&provider) {
        return Err("invalid Goose provider identity: refusing unsafe provider filename".into());
    }
    Ok(config
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("custom_providers")
        .join(format!("{provider}.json")))
}

#[derive(Clone, Copy)]
enum YamlAgent {
    Hermes,
    Goose,
}

impl YamlAgent {
    fn id(self) -> &'static str {
        match self {
            Self::Hermes => "hermes",
            Self::Goose => "goose",
        }
    }
}

fn prepare_yaml_model(
    path: &Path,
    profile: &ModelProfile,
    active: bool,
    agent: YamlAgent,
) -> Result<PreparedModelFile, String> {
    let (file, document, original) = read_yaml(path)?;
    let root = yaml_root(&document, path)?;
    let id = provider_id_for(agent.id(), profile);
    let mut pending = Vec::new();
    match agent {
        YamlAgent::Hermes => {
            let transport = match profile.protocol {
                ModelProtocol::AnthropicMessages => "anthropic_messages",
                _ => "chat_completions",
            };
            yaml_set_section_entry(
                &root,
                &mut pending,
                "providers",
                &id,
                &json!({
                    "name": profile.name,
                    "api": profile.base_url,
                    "key_env": profile.env_key,
                    "default_model": profile.model,
                    "transport": transport,
                }),
                path,
            )?;
            yaml_set_section_entry(
                &root,
                &mut pending,
                "model_aliases",
                &id,
                &json!({
                    "model": profile.model,
                    "provider": format!("custom:{id}"),
                }),
                path,
            )?;
            if active {
                yaml_set_section_fields(
                    &root,
                    &mut pending,
                    "model",
                    &[
                        ("default", Value::String(profile.model.clone())),
                        ("provider", Value::String(format!("custom:{id}"))),
                    ],
                    path,
                )?;
                if let Some(model) = root.get_mapping("model") {
                    if model
                        .get("base_url")
                        .and_then(|node| yaml_node_to_json(&node).ok())
                        .and_then(|value| value.as_str().map(str::to_string))
                        .as_deref()
                        == Some(profile.base_url.as_str())
                    {
                        // Older MUX builds wrote this field. A named provider
                        // must own endpoint/auth resolution or Hermes bypasses
                        // the provider's `key_env` credential path.
                        model.remove("base_url");
                    }
                }
            }
        }
        YamlAgent::Goose => {
            yaml_set_section_entry(
                &root,
                &mut pending,
                "providers",
                &id,
                &json!({
                    "enabled": true,
                    "model": profile.model,
                    "configured": true,
                }),
                path,
            )?;
            if active {
                yaml_mapping_set(&root, "active_provider", &Value::String(id))?;
            }
        }
    }
    let content = render_yaml_with_pending_sections(&file, &pending, path)?;
    Ok(PreparedModelFile {
        path: path.into(),
        original,
        content: Some(content),
    })
}

fn prepare_clear_yaml_model(
    path: &Path,
    profile: &ModelProfile,
    agent: YamlAgent,
) -> Result<PreparedModelFile, String> {
    let (_file, document, original) = read_yaml(path)?;
    let root = yaml_root(&document, path)?;
    let id = provider_id_for(agent.id(), profile);
    let mut content = original.clone().unwrap_or_default();
    let state: Value = serde_yaml::from_str(if content.is_empty() { "{}" } else { &content })
        .map_err(|error| error.to_string())?;
    match agent {
        YamlAgent::Hermes => {
            validate_yaml_mapping_section(&root, "providers", path)?;
            validate_yaml_mapping_section(&root, "model_aliases", path)?;
            yaml_text_remove_mapping_entry(&mut content, "providers", &id)?;
            yaml_text_remove_mapping_entry(&mut content, "model_aliases", &id)?;
            let active = state["model"]["provider"]
                .as_str()
                .is_some_and(|value| value == format!("custom:{id}"));
            if active {
                let mut owned = vec!["default", "provider"];
                if state["model"]["base_url"].as_str() == Some(profile.base_url.as_str()) {
                    owned.push("base_url");
                }
                yaml_text_remove_fields(&mut content, "model", &owned)?;
            }
            yaml_text_reset_hermes_auxiliary(&mut content, &state, &root, profile, &id, path)?;
        }
        YamlAgent::Goose => {
            validate_yaml_mapping_section(&root, "providers", path)?;
            yaml_text_remove_mapping_entry(&mut content, "providers", &id)?;
            let active = state["active_provider"].as_str() == Some(id.as_str());
            if active {
                yaml_text_remove_root_key(&mut content, "active_provider")?;
            }
        }
    }
    validate_generated_yaml(&content, path)?;
    Ok(PreparedModelFile {
        path: path.into(),
        original,
        content: Some(content),
    })
}

fn read_yaml(path: &Path) -> Result<(YamlFile, YamlDocument, Option<String>), String> {
    match read_optional(path)? {
        Some(content) => {
            let file = YamlFile::from_str(&content).map_err(|error| {
                format!(
                    "refusing to modify invalid YAML at {}: {error}",
                    path.display()
                )
            })?;
            let documents: Vec<_> = file.documents().collect();
            if documents.len() != 1 {
                return Err(format!(
                    "refusing to modify {}: expected one YAML document",
                    path.display()
                ));
            }
            Ok((file, documents[0].clone(), Some(content)))
        }
        None => {
            let file = YamlFile::new();
            file.push_document(YamlDocument::new_mapping());
            let document = file.document().expect("new YAML document");
            Ok((file, document, None))
        }
    }
}

fn yaml_node_to_json(node: &YamlNode) -> Result<Value, String> {
    serde_yaml::from_str(&node.to_string()).map_err(|error| error.to_string())
}

fn yaml_ensure_unique(mapping: &YamlMapping, path: &Path, context: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for entry in mapping.entries() {
        let Some(key) = entry.key_node() else {
            continue;
        };
        let key =
            serde_json::to_string(&yaml_node_to_json(&key)?).map_err(|error| error.to_string())?;
        if !seen.insert(key.clone()) {
            return Err(format!(
                "refusing to modify {}: duplicate YAML key {key} in {context}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn yaml_root(document: &YamlDocument, path: &Path) -> Result<YamlMapping, String> {
    let root = document.as_mapping().ok_or_else(|| {
        format!(
            "refusing to modify {}: YAML root is not a mapping",
            path.display()
        )
    })?;
    yaml_ensure_unique(&root, path, "$root")?;
    Ok(root)
}

#[derive(Debug, Clone)]
struct YamlTextBlock {
    start: usize,
    end: usize,
    line_end: usize,
}

fn yaml_line_indent(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
}

fn yaml_key_line<'a>(line: &'a str, indent: usize, key: &str) -> Option<&'a str> {
    let line = line.trim_end_matches(['\r', '\n']);
    if yaml_line_indent(line) != indent || line.as_bytes().get(indent) == Some(&b'\t') {
        return None;
    }
    line.get(indent..)?.strip_prefix(key)?.strip_prefix(':')
}

fn yaml_text_lines(text: &str) -> Vec<(usize, usize, &str)> {
    let mut offset = 0;
    let mut lines = Vec::new();
    for line in text.split_inclusive('\n') {
        let end = offset + line.len();
        lines.push((offset, end, line));
        offset = end;
    }
    if offset < text.len() {
        lines.push((offset, text.len(), &text[offset..]));
    }
    lines
}

fn yaml_find_text_block(
    text: &str,
    scope_start: usize,
    scope_end: usize,
    indent: usize,
    key: &str,
) -> Result<Option<YamlTextBlock>, String> {
    let lines = yaml_text_lines(text);
    let matches: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, (start, _, line))| {
            *start >= scope_start
                && *start < scope_end
                && yaml_key_line(line, indent, key).is_some()
        })
        .map(|(index, _)| index)
        .collect();
    let Some(&index) = matches.first() else {
        return Ok(None);
    };
    if matches.len() != 1 {
        return Err(format!("ambiguous duplicate YAML key {key}"));
    }
    let (start, line_end, _) = lines[index];
    let mut end = scope_end;
    for (candidate_start, _, line) in lines.iter().skip(index + 1) {
        if *candidate_start >= scope_end {
            break;
        }
        let raw = line.trim_end_matches(['\r', '\n']);
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let candidate_indent = yaml_line_indent(raw);
        if candidate_indent <= indent {
            end = *candidate_start;
            break;
        }
    }
    Ok(Some(YamlTextBlock {
        start,
        end,
        line_end,
    }))
}

fn yaml_find_root_block(text: &str, key: &str) -> Result<Option<YamlTextBlock>, String> {
    yaml_find_text_block(text, 0, text.len(), 0, key)
}

fn yaml_find_child_block(
    text: &str,
    parent: &YamlTextBlock,
    indent: usize,
    key: &str,
) -> Result<Option<YamlTextBlock>, String> {
    yaml_find_text_block(text, parent.line_end, parent.end, indent, key)
}

fn yaml_text_remove_root_key(text: &mut String, key: &str) -> Result<(), String> {
    if let Some(block) = yaml_find_root_block(text, key)? {
        text.replace_range(block.start..block.end, "");
    }
    Ok(())
}

fn yaml_text_remove_mapping_entry(
    text: &mut String,
    section_name: &str,
    entry_name: &str,
) -> Result<(), String> {
    let Some(section) = yaml_find_root_block(text, section_name)? else {
        return Ok(());
    };
    let Some(entry) = yaml_find_child_block(text, &section, 2, entry_name)? else {
        return Ok(());
    };
    text.replace_range(entry.start..entry.end, "");
    let Some(section) = yaml_find_root_block(text, section_name)? else {
        return Ok(());
    };
    let has_entry = yaml_text_lines(text).iter().any(|(start, _, line)| {
        *start >= section.line_end
            && *start < section.end
            && !line.trim().is_empty()
            && !line.trim_start().starts_with('#')
            && yaml_line_indent(line) == 2
    });
    if !has_entry {
        text.replace_range(section.start..section.end, "");
    }
    Ok(())
}

fn yaml_text_remove_fields(
    text: &mut String,
    section_name: &str,
    fields: &[&str],
) -> Result<(), String> {
    for field in fields.iter().rev() {
        let Some(section) = yaml_find_root_block(text, section_name)? else {
            return Ok(());
        };
        if let Some(block) = yaml_find_child_block(text, &section, 2, field)? {
            text.replace_range(block.start..block.end, "");
        }
    }
    let Some(section) = yaml_find_root_block(text, section_name)? else {
        return Ok(());
    };
    let has_field = yaml_text_lines(text).iter().any(|(start, _, line)| {
        *start >= section.line_end
            && *start < section.end
            && !line.trim().is_empty()
            && !line.trim_start().starts_with('#')
            && yaml_line_indent(line) == 2
    });
    if !has_field {
        text.replace_range(section.start..section.end, "");
    }
    Ok(())
}

fn yaml_scalar_field_line(key: &str, value: &Value, indent: usize) -> Result<String, String> {
    let mut object = Map::new();
    object.insert(key.into(), value.clone());
    let yaml = serde_yaml::to_string(&Value::Object(object)).map_err(|error| error.to_string())?;
    Ok(yaml
        .lines()
        .map(|line| format!("{}{}\n", " ".repeat(indent), line))
        .collect())
}

fn yaml_text_set_nested_scalar(
    text: &mut String,
    section_name: &str,
    entry_name: &str,
    field_name: &str,
    value: &Value,
) -> Result<(), String> {
    let section = yaml_find_root_block(text, section_name)?
        .ok_or_else(|| format!("missing YAML section {section_name}"))?;
    let entry = yaml_find_child_block(text, &section, 2, entry_name)?
        .ok_or_else(|| format!("missing YAML entry {section_name}.{entry_name}"))?;
    let replacement = yaml_scalar_field_line(field_name, value, 4)?;
    if let Some(field) = yaml_find_child_block(text, &entry, 4, field_name)? {
        text.replace_range(field.start..field.end, &replacement);
    } else {
        text.insert_str(entry.end, &replacement);
    }
    Ok(())
}

fn validate_yaml_mapping_section(
    root: &YamlMapping,
    section_name: &str,
    path: &Path,
) -> Result<(), String> {
    let Some(node) = root.get(section_name) else {
        return Ok(());
    };
    let mapping = node.as_mapping().ok_or_else(|| {
        format!(
            "refusing to modify {}: {section_name} is not a mapping",
            path.display()
        )
    })?;
    yaml_ensure_unique(mapping, path, section_name)
}

fn yaml_text_reset_hermes_auxiliary(
    text: &mut String,
    state: &Value,
    root: &YamlMapping,
    profile: &ModelProfile,
    provider_id: &str,
    path: &Path,
) -> Result<(), String> {
    let Some(tasks) = state.get("auxiliary").and_then(Value::as_object) else {
        return Ok(());
    };
    validate_yaml_mapping_section(root, "auxiliary", path)?;
    let auxiliary = root
        .get_mapping("auxiliary")
        .ok_or_else(|| format!("{} auxiliary is not a mapping", path.display()))?;
    for (task_name, task) in tasks {
        let Some(task) = task.as_object() else {
            continue;
        };
        let provider_matches = task
            .get("provider")
            .and_then(Value::as_str)
            .is_some_and(|value| value == provider_id || value == format!("custom:{provider_id}"));
        let endpoint_matches = task.get("base_url").and_then(Value::as_str)
            == Some(profile.base_url.as_str())
            && task.get("model").and_then(Value::as_str) == Some(profile.model.as_str());
        if !provider_matches && !endpoint_matches {
            continue;
        }
        let task_mapping = auxiliary.get_mapping(task_name).ok_or_else(|| {
            format!(
                "refusing to modify {}: auxiliary.{task_name} is not a mapping",
                path.display()
            )
        })?;
        yaml_ensure_unique(&task_mapping, path, &format!("auxiliary.{task_name}"))?;
        yaml_text_set_nested_scalar(
            text,
            "auxiliary",
            task_name,
            "provider",
            &Value::String("auto".into()),
        )?;
        for key in ["model", "base_url", "api_key", "api_mode"] {
            if task.contains_key(key) {
                yaml_text_set_nested_scalar(
                    text,
                    "auxiliary",
                    task_name,
                    key,
                    &Value::String(String::new()),
                )?;
            }
        }
    }
    Ok(())
}

fn validate_generated_yaml(content: &str, path: &Path) -> Result<(), String> {
    serde_yaml::from_str::<Value>(if content.is_empty() { "{}" } else { content }).map_err(
        |error| {
            format!(
                "refusing to write invalid YAML generated for {}: {error}",
                path.display()
            )
        },
    )?;
    Ok(())
}

fn yaml_json_to_node(value: &Value) -> Result<YamlNode, String> {
    let yaml = serde_yaml::to_string(value).map_err(|error| error.to_string())?;
    let document = YamlDocument::from_str(&yaml).map_err(|error| error.to_string())?;
    if let Some(node) = document.as_mapping() {
        return Ok(YamlNode::Mapping(node));
    }
    if let Some(node) = document.as_sequence() {
        return Ok(YamlNode::Sequence(node));
    }
    if let Some(node) = document.as_scalar() {
        return Ok(YamlNode::Scalar(node));
    }
    Err("failed to construct YAML value".into())
}

fn yaml_mapping_set(target: &YamlMapping, key: &str, value: &Value) -> Result<(), String> {
    let node = yaml_json_to_node(value)?;
    target.remove(key);
    match node {
        YamlNode::Scalar(node) => target.set(key, node),
        YamlNode::Mapping(node) => target.set(key, node),
        YamlNode::Sequence(node) => target.set(key, node),
        YamlNode::Alias(node) => target.set(key, node),
        YamlNode::TaggedNode(node) => target.set(key, node),
    }
    Ok(())
}

fn yaml_set_section_entry(
    root: &YamlMapping,
    pending: &mut Vec<(String, Value)>,
    section_name: &str,
    entry_name: &str,
    value: &Value,
    path: &Path,
) -> Result<(), String> {
    let section = match root.get(section_name) {
        Some(node) => node.as_mapping().cloned().ok_or_else(|| {
            format!(
                "refusing to modify {}: {section_name} is not a mapping",
                path.display()
            )
        })?,
        None => {
            let mut entries = Map::new();
            entries.insert(entry_name.into(), value.clone());
            pending.push((section_name.into(), Value::Object(entries)));
            return Ok(());
        }
    };
    yaml_ensure_unique(&section, path, section_name)?;
    if section.is_empty() || (section.len() == 1 && section.contains_key(entry_name)) {
        root.remove(section_name);
        let mut entries = Map::new();
        entries.insert(entry_name.into(), value.clone());
        pending.push((section_name.into(), Value::Object(entries)));
        return Ok(());
    }
    section.remove(entry_name);
    yaml_mapping_set(&section, entry_name, value)
}

fn yaml_set_section_fields(
    root: &YamlMapping,
    pending: &mut Vec<(String, Value)>,
    section_name: &str,
    fields: &[(&str, Value)],
    path: &Path,
) -> Result<(), String> {
    let section = match root.get(section_name) {
        Some(node) => node.as_mapping().cloned().ok_or_else(|| {
            format!(
                "refusing to modify {}: {section_name} is not a mapping",
                path.display()
            )
        })?,
        None => {
            pending.push((
                section_name.into(),
                Value::Object(
                    fields
                        .iter()
                        .map(|(key, value)| ((*key).into(), value.clone()))
                        .collect(),
                ),
            ));
            return Ok(());
        }
    };
    yaml_ensure_unique(&section, path, section_name)?;
    if section.is_empty() {
        root.remove(section_name);
        pending.push((
            section_name.into(),
            Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| ((*key).into(), value.clone()))
                    .collect(),
            ),
        ));
        return Ok(());
    }
    for (key, value) in fields {
        yaml_mapping_set(&section, key, value)?;
    }
    Ok(())
}

fn render_yaml_with_pending_sections(
    file: &YamlFile,
    pending: &[(String, Value)],
    path: &Path,
) -> Result<String, String> {
    let mut content = file.to_string();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for (section_name, value) in pending {
        let mut section = Map::new();
        section.insert(section_name.clone(), value.clone());
        content.push_str(
            &serde_yaml::to_string(&Value::Object(section)).map_err(|error| error.to_string())?,
        );
    }
    if !content.ends_with('\n') {
        content.push('\n');
    }
    serde_yaml::from_str::<Value>(&content).map_err(|error| {
        format!(
            "refusing to write invalid YAML generated for {}: {error}",
            path.display()
        )
    })?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(protocol: ModelProtocol) -> ModelProfile {
        ModelProfile {
            id: "work".into(),
            name: "Work Model".into(),
            provider: "custom".into(),
            model_vendor: Some("vendor".into()),
            native_ids: Default::default(),
            protocol,
            base_url: "https://gateway.example/v1".into(),
            model: "vendor/model".into(),
            env_key: Some("WORK_API_KEY".into()),
            context_window: Some(128_000),
            max_output_tokens: Some(8_192),
            reasoning: true,
        }
    }

    fn temp_path(name: &str, extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mux-model-adapter-{name}-{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ))
    }

    #[test]
    fn opencode_and_kilo_keep_jsonc_comments_and_use_env_refs() {
        for agent in ["opencode", "kilo-code"] {
            let path = temp_path(agent, "jsonc");
            fs::write(&path, "{\n  // keep me\n  \"theme\": \"dark\"\n}\n").unwrap();
            let files = prepare_apply(
                agent,
                &[path.clone()],
                &profile(ModelProtocol::OpenaiCompletions),
                true,
            )
            .unwrap();
            let content = files[0].content.as_deref().unwrap();
            assert!(content.contains("// keep me"));
            assert!(content.contains("{env:WORK_API_KEY}"));
            assert!(content.contains("mux_776f726b/vendor/model"));
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn qwen_preserves_external_models_and_sets_official_selection_fields() {
        let path = temp_path("qwen", "json");
        fs::write(&path, r#"{"modelProviders":{"openai":[{"id":"external","baseUrl":"https://external"}]},"future":true}"#).unwrap();
        let file = &prepare_apply(
            "qwen-code",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
            true,
        )
        .unwrap()[0];
        let value: Value = serde_json::from_str(file.content.as_deref().unwrap()).unwrap();
        assert_eq!(value["future"], true);
        assert_eq!(value["model"]["name"], "vendor/model");
        assert_eq!(value["security"]["auth"]["selectedType"], "openai");
        assert_eq!(
            value["modelProviders"]["openai"].as_array().unwrap().len(),
            2
        );
        fs::write(&path, file.content.as_deref().unwrap()).unwrap();
        let openai = profile(ModelProtocol::OpenaiCompletions);
        let mut anthropic = profile(ModelProtocol::AnthropicMessages);
        anthropic.id = "other".into();
        let profiles = std::collections::BTreeMap::from([
            (openai.id.clone(), openai),
            (anthropic.id.clone(), anthropic),
        ]);
        assert_eq!(
            observe_active("qwen-code", &[path.clone()], &profiles),
            ObservedActiveModel::Managed("work".into())
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn qwen_migrates_the_exact_legacy_mux_wrapper_to_the_stable_array_shape() {
        let path = temp_path("qwen-legacy", "json");
        fs::write(
            &path,
            r#"{"modelProviders":{"openai":{"protocol":"openai","models":[{"id":"external","baseUrl":"https://external"}]}},"future":true}"#,
        )
        .unwrap();
        let file = &prepare_apply(
            "qwen-code",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
            false,
        )
        .unwrap()[0];
        let value: Value = serde_json::from_str(file.content.as_deref().unwrap()).unwrap();
        assert!(value["modelProviders"]["openai"].is_array());
        assert_eq!(
            value["modelProviders"]["openai"].as_array().unwrap().len(),
            2
        );
        assert_eq!(value["future"], true);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn qwen_rejects_an_unrecognized_provider_wrapper_without_overwriting_it() {
        let path = temp_path("qwen-unknown-wrapper", "json");
        let original =
            r#"{"modelProviders":{"openai":{"protocol":"openai","models":[],"future":true}}}"#;
        fs::write(&path, original).unwrap();
        let error = prepare_apply(
            "qwen-code",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
            false,
        )
        .unwrap_err();
        assert!(error.contains("neither the stable array shape nor the exact legacy MUX wrapper"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn crush_only_changes_primary_slot_and_keeps_small() {
        let path = temp_path("crush", "json");
        fs::write(
            &path,
            r#"{"models":{"small":{"provider":"external","model":"tiny"}},"future":true}"#,
        )
        .unwrap();
        let file = &prepare_apply(
            "crush",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
            true,
        )
        .unwrap()[0];
        let value: Value = serde_json::from_str(file.content.as_deref().unwrap()).unwrap();
        assert_eq!(value["models"]["small"]["model"], "tiny");
        assert_eq!(value["models"]["large"]["model"], "vendor/model");
        assert_eq!(
            value["providers"]["mux_776f726b"]["api_key"],
            "$WORK_API_KEY"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn vibe_preserves_comments_and_auxiliary_toml() {
        let path = temp_path("vibe", "toml");
        fs::write(&path, "# keep me\n[telemetry]\nenabled = false\n").unwrap();
        let file = &prepare_apply(
            "mistral-vibe",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
            true,
        )
        .unwrap()[0];
        let content = file.content.as_deref().unwrap();
        assert!(content.contains("# keep me"));
        assert!(content.contains("active_model = \"mux_776f726b\""));
        assert!(content.contains("api_key_env_var = \"WORK_API_KEY\""));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn factory_preserves_other_custom_models_and_uses_env_ref() {
        let path = temp_path("factory", "json");
        fs::write(&path, r#"{"customModels":[{"model":"external","baseUrl":"https://external"}],"mission":{"model":"worker"}}"#).unwrap();
        let file = &prepare_apply(
            "factory-droid",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiResponses),
            true,
        )
        .unwrap()[0];
        let value: Value = serde_json::from_str(file.content.as_deref().unwrap()).unwrap();
        assert_eq!(value["customModels"].as_array().unwrap().len(), 2);
        assert_eq!(value["customModels"][1]["apiKey"], "${WORK_API_KEY}");
        assert_eq!(value["mission"]["model"], "worker");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn clearing_an_absent_json_profile_is_a_noop() {
        for (agent_id, label) in [
            ("qwen-code", "qwen-clear"),
            ("factory-droid", "factory-clear"),
        ] {
            let path = temp_path(label, "json");
            let original = "{\n  // keep me\n  \"unrelated\": true\n}\n";
            fs::write(&path, original).unwrap();
            let prepared = prepare_clear(
                agent_id,
                &[path.clone()],
                &profile(ModelProtocol::OpenaiCompletions),
            )
            .unwrap();
            assert!(cleared_profile_absent(Ok(prepared.clone())).unwrap());
            assert_eq!(prepared[0].content.as_deref(), Some(original));
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn hermes_yaml_is_lossless_outside_owned_fields() {
        let path = temp_path("hermes", "yaml");
        fs::write(
            &path,
            r#"# keep me
auxiliary:
  compression:
    model: tiny
providers:
  external:
    api: https://external.example/v1 # keep external provider
model_aliases:
  fast:
    model: tiny
    provider: openrouter
model:
  temperature: 0.2 # keep model policy
"#,
        )
        .unwrap();
        let file = &prepare_apply(
            "hermes",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
            true,
        )
        .unwrap()[0];
        let content = file.content.as_deref().unwrap();
        assert!(content.contains("# keep me"));
        assert!(content.contains("compression:"));
        assert!(
            content.contains("api: https://gateway.example/v1"),
            "{content}"
        );
        assert!(content.contains("key_env: WORK_API_KEY"), "{content}");
        assert!(
            content.contains("provider: custom:mux_776f726b"),
            "{content}"
        );
        assert!(!content.contains("model:\n  base_url:"), "{content}");
        assert!(content.contains("# keep external provider"), "{content}");
        assert!(content.contains("# keep model policy"), "{content}");
        let value: Value = serde_yaml::from_str(content).unwrap();
        assert_eq!(
            value["providers"]["external"]["api"],
            "https://external.example/v1"
        );
        assert_eq!(value["model_aliases"]["fast"]["model"], "tiny");
        assert_eq!(value["model"]["temperature"], 0.2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn hermes_clear_resets_only_auxiliary_slots_that_reference_the_profile() {
        let path = temp_path("hermes-aux", "yaml");
        fs::write(
            &path,
            r#"auxiliary:
  compression:
    provider: custom:mux_776f726b
    model: gpt-custom
    timeout: 90 # keep task policy
  vision:
    provider: openrouter
    model: google/gemini-flash
providers:
  external:
    api: https://external.example/v1 # keep external provider
  mux_776f726b:
    api: https://gateway.example.test/v1
model_aliases:
  mux_776f726b:
    model: gpt-custom
    provider: custom:mux_776f726b
model:
  default: gpt-custom
  provider: custom:mux_776f726b
  temperature: 0.2 # keep model policy
"#,
        )
        .unwrap();
        let content = prepare_clear(
            "hermes",
            &[path.clone()],
            &profile(ModelProtocol::OpenaiCompletions),
        )
        .unwrap()[0]
            .content
            .clone()
            .unwrap();
        let value: Value = serde_yaml::from_str(&content).unwrap();
        assert_eq!(
            value["auxiliary"]["compression"]["timeout"], 90,
            "{content}"
        );
        assert_eq!(value["auxiliary"]["vision"]["provider"], "openrouter");
        assert_eq!(
            value["providers"]["external"]["api"],
            "https://external.example/v1"
        );
        assert!(value.get("model_aliases").is_none());
        assert_eq!(value["model"]["temperature"], 0.2);
        assert!(content.contains("# keep task policy"));
        assert!(content.contains("# keep external provider"));
        assert!(content.contains("# keep model policy"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn goose_writes_declarative_provider_and_keeps_config_comments() {
        let root = temp_path("goose-root", "dir");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.yaml");
        fs::write(&path, "# keep me\nextensions: {}\n").unwrap();
        let files = prepare_apply(
            "goose",
            &[path.clone()],
            &profile(ModelProtocol::AnthropicMessages),
            true,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].content.as_deref().unwrap().contains("# keep me"));
        let provider: Value = serde_json::from_str(files[1].content.as_deref().unwrap()).unwrap();
        assert_eq!(provider["engine"], "anthropic");
        assert_eq!(provider["api_key_env"], "WORK_API_KEY");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn goose_rejects_an_unsafe_adopted_provider_filename() {
        let path = temp_path("goose-unsafe-provider", "yaml");
        let mut profile = profile(ModelProtocol::OpenaiCompletions);
        profile
            .native_ids
            .insert("goose".into(), "../../outside".into());
        let error = prepare_apply("goose", &[path], &profile, true).unwrap_err();
        assert!(error.contains("unsafe provider filename"));
    }

    #[test]
    fn goose_clear_preserves_external_providers_and_config_comments() {
        let root = temp_path("goose-clear-root", "dir");
        fs::create_dir_all(root.join("custom_providers")).unwrap();
        let path = root.join("config.yaml");
        fs::write(
            &path,
            r#"# keep root comment
extensions: {}
providers:
  external:
    enabled: true # keep external provider
  mux_776f726b:
    enabled: true
    model: gpt-custom
    configured: true
active_provider: mux_776f726b
other_policy: strict # keep policy
"#,
        )
        .unwrap();
        let profile = profile(ModelProtocol::OpenaiCompletions);
        let provider_path = goose_provider_path(&path, &profile).unwrap();
        fs::write(&provider_path, "{\"name\":\"mux_776f726b\"}\n").unwrap();

        let files = prepare_clear("goose", &[path.clone()], &profile).unwrap();
        assert_eq!(files.len(), 2);
        let content = files[0].content.as_deref().unwrap();
        let value: Value = serde_yaml::from_str(content).unwrap();
        assert_eq!(value["providers"]["external"]["enabled"], true);
        assert!(value["providers"].get("mux_776f726b").is_none());
        assert!(value.get("active_provider").is_none());
        assert_eq!(value["other_policy"], "strict");
        assert!(content.contains("# keep root comment"));
        assert!(content.contains("# keep external provider"));
        assert!(content.contains("# keep policy"));
        assert_eq!(files[1].path, provider_path);
        assert!(files[1].original.is_some());
        assert!(files[1].content.is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
