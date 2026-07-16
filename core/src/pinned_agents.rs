use crate::agents::load_agents_from_settings;
use crate::settings::{load_settings_strict, mutate_settings_checked, Settings, UiSettings};
use std::collections::BTreeSet;
use std::io::Error;

pub const MAX_PINNED_AGENTS: usize = 6;

fn configurable_agent_ids(settings: &Settings) -> BTreeSet<String> {
    load_agents_from_settings(settings)
        .into_iter()
        .filter_map(|(id, definition)| definition.global.is_some().then_some(id))
        .collect()
}

fn normalize_loaded(ids: Vec<String>, configurable: &BTreeSet<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    ids.into_iter()
        .filter(|id| configurable.contains(id))
        .filter(|id| seen.insert(id.clone()))
        .take(MAX_PINNED_AGENTS)
        .collect()
}

fn validate_requested(ids: &[String], configurable: &BTreeSet<String>) -> Result<(), String> {
    if ids.len() > MAX_PINNED_AGENTS {
        return Err(format!("最多只能置顶 {MAX_PINNED_AGENTS} 个 Agent"));
    }
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(format!("置顶 Agent 不能重复: {id}"));
        }
        if !configurable.contains(id) {
            return Err(format!("Agent 不存在或没有全局配置能力: {id}"));
        }
    }
    Ok(())
}

pub fn get_pinned_agents() -> Result<Vec<String>, String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let configurable = configurable_agent_ids(&settings);
    let ids = settings.ui.unwrap_or_default().pinned_agents;
    Ok(normalize_loaded(ids, &configurable))
}

pub fn set_pinned_agents(ids: Vec<String>) -> Result<Vec<String>, String> {
    mutate_settings_checked(move |settings| set_pinned_agents_in_settings(settings, ids))
        .map_err(|error| error.to_string())
}

fn set_pinned_agents_in_settings(
    settings: &mut Settings,
    ids: Vec<String>,
) -> std::io::Result<Vec<String>> {
    let configurable = configurable_agent_ids(settings);
    validate_requested(&ids, &configurable).map_err(Error::other)?;
    let saved = ids.clone();
    settings
        .ui
        .get_or_insert_with(UiSettings::default)
        .pinned_agents = ids;
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::load_agents;
    use crate::settings::Settings;
    use crate::testenv::TestHome;
    use serde_json::Value;
    use std::fs;

    fn settings_path(home: &TestHome) -> std::path::PathBuf {
        home.home.join(".mux/settings.json")
    }

    #[test]
    fn valid_ids_roundtrip_in_input_order_and_preserve_unknown_fields() {
        let home = TestHome::new("pinned-roundtrip");
        let path = settings_path(&home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"ui":{"future_ui_key":{"keep":true}},"future_section":{"keep":true}}"#,
        )
        .unwrap();

        let saved = set_pinned_agents(vec!["codex".into(), "claude-code".into()]).unwrap();
        assert_eq!(saved, vec!["codex", "claude-code"]);
        assert_eq!(get_pinned_agents().unwrap(), saved);

        let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(value["ui"]["future_ui_key"]["keep"], true);
        assert_eq!(value["future_section"]["keep"], true);
        assert_eq!(value["ui"]["pinned_agents"][0], "codex");
    }

    #[test]
    fn write_rejects_limit_duplicates_unknown_and_read_only_agents() {
        let _home = TestHome::new("pinned-validation");
        let configurable: Vec<String> = load_agents()
            .into_iter()
            .filter_map(|(id, definition)| definition.global.is_some().then_some(id))
            .collect();
        assert!(configurable.len() >= MAX_PINNED_AGENTS + 1);
        assert!(set_pinned_agents(configurable[..MAX_PINNED_AGENTS + 1].to_vec()).is_err());
        assert!(set_pinned_agents(vec!["codex".into(), "codex".into()]).is_err());
        assert!(set_pinned_agents(vec!["missing-agent".into()]).is_err());

        let read_only = load_agents()
            .into_iter()
            .find_map(|(id, definition)| definition.global.is_none().then_some(id))
            .expect("catalog must retain at least one read-only Agent");
        assert!(set_pinned_agents(vec![read_only]).is_err());
    }

    #[test]
    fn read_normalizes_stale_duplicates_and_excess_without_rewriting_file() {
        let home = TestHome::new("pinned-normalize");
        let path = settings_path(&home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{
          "ui": {
            "pinned_agents": [
              "codex", "missing-agent", "codex", "claude-code",
              "qoder", "pi", "cursor", "gemini", "opencode"
            ]
          }
        }"#;
        fs::write(&path, original).unwrap();

        let loaded = get_pinned_agents().unwrap();
        assert_eq!(loaded.first().map(String::as_str), Some("codex"));
        assert_eq!(loaded.iter().filter(|id| id.as_str() == "codex").count(), 1);
        assert!(loaded.len() <= MAX_PINNED_AGENTS);
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn corrupt_settings_are_not_replaced() {
        let home = TestHome::new("pinned-corrupt");
        let path = settings_path(&home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"ui":{"pinned_agents":["#;
        fs::write(&path, original).unwrap();

        assert!(get_pinned_agents().is_err());
        assert!(set_pinned_agents(vec!["codex".into()]).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn validation_uses_agents_from_the_mutation_settings_snapshot() {
        let mut stale = Settings::default();
        stale.agents = Some(std::collections::BTreeMap::from([(
            "snapshot-agent".into(),
            crate::types::AgentDefinition {
                global: Some("~/.snapshot/mcp.json".into()),
                format: "json".into(),
                key: "mcpServers".into(),
                enabled: true,
                ..Default::default()
            },
        )]));
        assert!(configurable_agent_ids(&stale).contains("snapshot-agent"));

        let mut current = Settings::default();
        let result = set_pinned_agents_in_settings(&mut current, vec!["snapshot-agent".into()]);

        assert!(result.is_err());
        assert!(current.ui.is_none());
    }
}
