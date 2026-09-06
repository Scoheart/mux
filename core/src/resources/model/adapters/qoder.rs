//! Qoder Desktop 0.1.8 native custom-endpoint registry.
//! See docs/qoder-desktop-models.md for the audited storage contract.

use super::*;

const AGENT: &str = "qoder-desktop";

pub(crate) fn read_registry(path: &Path) -> Result<Option<Value>, String> {
    let (root, original) = read_jsonc(path)
        .map_err(|_| "qoder_config_conflicted: invalid JSON/JSONC".to_string())?;
    if original.is_none() { return Ok(None); }
    fn validate(node: jsonc_parser::cst::CstNode, path: &Path) -> Result<(), String> {
        if let Some(object) = node.as_object() {
            ensure_unique(&object, path, "Qoder config")?;
            for property in object.properties() {
                if let Some(value) = property.value() { validate(value, path)?; }
            }
        } else if let Some(array) = node.as_array() {
            for value in array.elements() { validate(value, path)?; }
        }
        Ok(())
    }
    let top = root.object_value().ok_or("qoder_config_conflicted: root must be an object")?;
    for property in top.properties() {
        if let Some(value) = property.value() { validate(value, path)?; }
    }
    ensure_unique(&top, path, "$root")?;
    Ok(root.to_serde_value())
}

fn object(parent: &CstObject, key: &str, path: &Path) -> Result<CstObject, String> {
    let value = parent.object_value_or_create(key).ok_or_else(|| {
        format!("qoder_config_conflicted: {}: {key} must be an object", path.display())
    })?;
    ensure_unique(&value, path, key)?;
    Ok(value)
}

fn validate_string(value: &str) -> Result<(), String> {
    // Qoder expands variables recursively, including literal credentials.
    if value.as_bytes().windows(2).any(|pair| {
        pair[0] == b'$' && (pair[1].is_ascii_alphanumeric() || matches!(pair[1], b'_' | b'{'))
    }) {
        return Err("qoder_interpolation_unsupported: literal values containing environment substitutions cannot be preserved; use an environment credential reference".into());
    }
    Ok(())
}

pub(super) fn prepare(
    path: &Path,
    profile: &ModelProfile,
    literal_key: Option<&str>,
) -> Result<PreparedModelFile, String> {
    for value in [&profile.base_url, &profile.model, &profile.name] {
        validate_string(value)?;
    }
    if let Some(key) = literal_key {
        validate_string(key)?;
    }
    let (root, original) = read_jsonc(path).map_err(|_| "qoder_config_conflicted: configuration must be valid JSON/JSONC".to_string())?;
    let top = root_object(&root, path)?;
    ensure_unique(&top, path, "$root")?;
    let providers = object(&top, "providers", path)?;
    let id = provider_id_for(AGENT, profile);
    if !safe_provider_id(&id) || !id.as_bytes()[0].is_ascii_alphanumeric() || id == "qoder" {
        return Err("qoder_provider_id_invalid: unsupported provider identity".into());
    }
    let provider = object(&providers, &id, path)?;
    if provider.get("models").and_then(|value| value.array_value()).is_some_and(|models| {
        models.elements().iter().any(|entry| {
            entry.to_serde_value().and_then(|value| value.get("model").cloned())
                .is_some_and(|id| id != json!(profile.model))
        })
    }) {
        return Err("qoder_shared_provider_read_only: provider connection is shared with another model".into());
    }
    let protocol = match profile.protocol {
        ModelProtocol::OpenaiCompletions => "openai",
        ModelProtocol::OpenaiResponses => "openai-responses",
        ModelProtocol::AnthropicMessages => "anthropic",
        _ => return Err("qoder_protocol_unsupported".into()),
    };
    let api_key = literal_key.map(str::to_string)
        .or_else(|| profile.env_key.as_ref().map(|key| format!("${{{key}}}")))
        .unwrap_or_default();
    for (key, value) in [
        ("type", "openai-compatible"),
        ("protocol", protocol),
        ("authType", if protocol == "anthropic" { "api-key" } else { "bearer" }),
        ("baseUrl", profile.base_url.as_str()),
        ("apiKey", api_key.as_str()),
    ] {
        set_json(&provider, key, Some(json!(value)));
    }
    // Each MUX-created provider owns one model. Adopted providers retain sibling
    // models and their provider-level default; no conversation selection is written.
    if provider.get("displayName").is_none() {
        set_json(&provider, "displayName", Some(json!(profile.name)));
    }
    if provider.get("model").is_none() {
        set_json(&provider, "model", Some(json!(profile.model)));
    }
    let models = provider.array_value_or_create("models").ok_or_else(|| {
        "qoder_config_conflicted: provider models must be an array".to_string()
    })?;
    let mut ids = BTreeSet::new();
    let mut existing = None;
    for element in models.elements() {
        let entry = element.as_object().ok_or("qoder_config_conflicted: model must be an object")?;
        ensure_unique(&entry, path, "models[]")?;
        let model_id = entry.get("model").and_then(|p| p.value()).and_then(|v| v.to_serde_value())
            .and_then(|v| v.as_str().map(str::to_string)).ok_or("qoder_config_conflicted: model id missing")?;
        if !ids.insert(model_id.clone()) {
            return Err("qoder_config_conflicted: duplicate model id".into());
        }
        if model_id == profile.model { existing = Some(entry); }
    }
    let entry = match existing {
        Some(entry) => entry,
        None => {
            models.append(input_value(json!({"model": profile.model})));
            models.elements().last().and_then(|node| node.as_object())
                .ok_or("qoder_config_conflicted: cannot create model")?
        }
    };
    set_json(&entry, "displayName", Some(json!(profile.name)));
    set_json(&entry, "contextWindow", profile.context_window.map(|value| json!(value)));
    set_json(&entry, "maxOutputTokens", profile.max_output_tokens.map(|value| json!(value)));
    if let Some(reasoning) = profile.reasoning {
        let capabilities = object(&entry, "capabilities", path)?;
        let thinking = object(&capabilities, "thinking", path)?;
        set_json(&thinking, "modes", Some(if reasoning { json!(["enabled"]) } else { json!([]) }));
    }
    Ok(prepared_json(path, original, root))
}

pub(super) fn clear(path: &Path, profile: &ModelProfile) -> Result<PreparedModelFile, String> {
    let (root, original) = read_jsonc(path).map_err(|_| "qoder_config_conflicted: configuration must be valid JSON/JSONC".to_string())?;
    if original.is_none() { return Ok(PreparedModelFile { path: path.into(), original, content: None }); }
    let top = root_object(&root, path)?;
    ensure_unique(&top, path, "$root")?;
    if top.get("providers").is_none() { return Ok(prepared_json(path, original, root)); }
    let providers = object(&top, "providers", path)?;
    let id = provider_id_for(AGENT, profile);
    if providers.get(&id).is_none() { return Ok(prepared_json(path, original, root)); }
    let provider = object(&providers, &id, path)?;
    let models = provider.get("models").and_then(|p| p.array_value())
        .ok_or("qoder_config_conflicted: provider models must be an array")?;
    let mut remaining = Vec::new();
    let mut removed = false;
    let mut ids = BTreeSet::new();
    for element in models.elements() {
        let value = element.to_serde_value().ok_or("qoder_config_conflicted: invalid model")?;
        let id = value.get("model").and_then(Value::as_str).ok_or("qoder_config_conflicted: model id missing")?;
        if !ids.insert(id.to_string()) { return Err("qoder_config_conflicted: duplicate model id".into()); }
        if id == profile.model { element.remove(); removed = true; } else { remaining.push(id.to_string()); }
    }
    if !removed { return Ok(prepared_json(path, original, root)); }
    if remaining.is_empty() {
        set_json(&providers, &id, None);
    } else if provider.get("model").and_then(|p| p.value()).and_then(|v| v.to_serde_value()).as_ref() == Some(&json!(profile.model)) {
        set_json(&provider, "model", Some(json!(remaining[0])));
    }
    Ok(prepared_json(path, original, root))
}

pub(super) fn clear_all(path: &Path) -> Result<PreparedModelFile, String> {
    // Discovery and deletion must agree on which entries were reviewed.
    read_registry(path)?;
    let (root, original) = read_jsonc(path).map_err(|_| "qoder_config_conflicted: configuration must be valid JSON/JSONC".to_string())?;
    if original.is_none() { return Ok(PreparedModelFile { path: path.into(), original, content: None }); }
    let top = root_object(&root, path)?;
    ensure_unique(&top, path, "$root")?;
    if top.get("providers").is_some() {
        let providers = object(&top, "providers", path)?;
        // Delete only records represented by the custom-endpoint observer.
        for property in providers.properties() {
            let Some(value) = property.value().and_then(|v| v.to_serde_value()) else { continue; };
            if value.get("baseUrl").and_then(Value::as_str).is_none() { continue; }
            let Some(models) = value.get("models").and_then(Value::as_array) else { continue; };
            let mut ids = BTreeSet::new();
            for model in models {
                let id = model.get("model").and_then(Value::as_str)
                    .ok_or("qoder_config_conflicted: invalid model id")?;
                if !ids.insert(id) { return Err("qoder_config_conflicted: duplicate model id".into()); }
            }
            if !matches!(value.get("protocol").and_then(Value::as_str).unwrap_or("openai"), "openai" | "openai-responses" | "anthropic") {
                return Err("qoder_config_conflicted: unsupported provider protocol".into());
            }
            if !models.is_empty() { property.remove(); }
        }
    }
    Ok(prepared_json(path, original, root))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("mux-qoder-models-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for TempDir {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    fn profile() -> ModelProfile {
        serde_json::from_value(json!({
            "id": "qoder-fixture", "name": "Fixture model", "model": "fixture-model",
            "protocol": "openai-completions", "base_url": "https://example.invalid/v1",
            "env_key": "MUX_QODER_FIXTURE_KEY", "context_window": 128000
        })).unwrap()
    }

    #[test]
    fn round_trip_preserves_mcp_comments_and_cli_selection_for_each_protocol() {
        let dir = TempDir::new();
        let path = dir.path().join("settings.json");
        let original = "{\n  // keep this MCP and CLI selection\n  \"mcpServers\": {\"keep\": {\"command\": \"fixture\"}},\n  \"model\": {\"name\": \"cli-selection\"},\n  \"hooks\": {}\n}\n";
        for (protocol, wire) in [
            (ModelProtocol::OpenaiCompletions, "openai"),
            (ModelProtocol::OpenaiResponses, "openai-responses"),
            (ModelProtocol::AnthropicMessages, "anthropic"),
        ] {
            fs::write(&path, original).unwrap();
            let mut profile = profile();
            profile.protocol = protocol;
            let prepared = prepare(&path, &profile, None).unwrap();
            let content = prepared.content.unwrap();
            assert!(content.contains("// keep this MCP and CLI selection"));
            fs::write(&path, &content).unwrap();
            let root = read_jsonc(&path).unwrap().0.to_serde_value().unwrap();
            let provider = &root["providers"][provider_id_for(AGENT, &profile)];
            assert_eq!(provider["protocol"], wire);
            assert_eq!(provider["apiKey"], "${MUX_QODER_FIXTURE_KEY}");
            assert_eq!(root["model"]["name"], "cli-selection");
            assert_eq!(root["mcpServers"]["keep"]["command"], "fixture");
            assert_eq!(prepare(&path, &profile, None).unwrap().content.as_deref(), Some(content.as_str()));
            let cleared = clear(&path, &profile).unwrap().content.unwrap();
            fs::write(&path, cleared).unwrap();
            let root = read_jsonc(&path).unwrap().0.to_serde_value().unwrap();
            assert!(root["providers"].as_object().unwrap().is_empty());
            assert_eq!(root["model"]["name"], "cli-selection");
            assert_eq!(root["mcpServers"]["keep"]["command"], "fixture");
        }
    }

    #[test]
    fn ambiguous_configs_and_interpolated_literal_keys_fail_closed() {
        let dir = TempDir::new();
        let path = dir.path().join("settings.json");
        let original = "{\"providers\": {}, \"providers\": {}}";
        fs::write(&path, original).unwrap();
        assert!(prepare(&path, &profile(), None).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        fs::write(&path, "{}").unwrap();
        assert!(prepare(&path, &profile(), Some("fixture-$SUBSTITUTION")).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
    }

    #[test]
    fn clear_preserves_external_sibling_models() {
        let dir = TempDir::new();
        let path = dir.path().join("settings.json");
        let mut profile = profile();
        profile.native_ids.insert(AGENT.into(), "external".into());
        fs::write(&path, r#"{"providers":{"external":{"baseUrl":"https://example.invalid/v1","apiKey":"${FIXTURE_KEY}","model":"fixture-model","models":[{"model":"fixture-model"},{"model":"sibling","custom":true}]}},"mcpServers":{}}"#).unwrap();
        let content = clear(&path, &profile).unwrap().content.unwrap();
        fs::write(&path, content).unwrap();
        let root = read_jsonc(&path).unwrap().0.to_serde_value().unwrap();
        assert_eq!(root["providers"]["external"]["model"], "sibling");
        assert_eq!(root["providers"]["external"]["models"][0]["custom"], true);
        assert_eq!(root["providers"]["external"]["apiKey"], "${FIXTURE_KEY}");
    }
}
