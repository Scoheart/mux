use crate::scanner::collapse_home;
use crate::settings::{load_settings, mutate_settings};
use crate::types::AgentDefinition;
use serde::Serialize;
use std::collections::BTreeMap;

/// An agent definition as surfaced to a UI: its stored config plus derived
/// has-path flags. `global`/`project` keep the raw stored `~/…` paths so the UI
/// can display and prefill the path editor.
#[derive(Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub format: String,
    pub key: String,
    pub has_global: bool,
    pub has_project: bool,
    pub enabled: bool,
    pub global: Option<String>,
    pub project: Option<String>,
}

/// 内置 agent 定义：编译期内嵌 root agents.json（与 TS CLI 共用的单一数据源）
const BUILTIN_AGENTS_JSON: &str = include_str!("../../data/agents.json");

pub fn builtin_agents() -> BTreeMap<String, AgentDefinition> {
    serde_json::from_str(BUILTIN_AGENTS_JSON).expect("agents.json must be valid")
}

/// 优先读 settings.agents（与 CLI 共用），缺失或为空时用内置。
pub fn load_agents() -> BTreeMap<String, AgentDefinition> {
    match load_settings().agents {
        Some(map) if !map.is_empty() => map,
        _ => builtin_agents(),
    }
}

/// 将完整 agent map 写入 settings.agents（保留其它分区不动）。
pub fn save_agents(map: &BTreeMap<String, AgentDefinition>) -> std::io::Result<()> {
    mutate_settings(|s| {
        s.agents = Some(map.clone());
    })
}

/// Validate + normalize an agent definition, then persist it (merged over
/// builtin/existing defs in `settings.agents`). `allow_overwrite` distinguishes
/// create (errors on an existing id) from edit (replaces in place). Global paths
/// are collapsed to `~/…`. The legacy project field is retained for backward
/// compatibility, but every usable definition must have a global path.
pub fn put(id: String, mut def: AgentDefinition, allow_overwrite: bool) -> Result<(), String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("agent id 不能为空".into());
    }
    if def.key.trim().is_empty() {
        return Err("配置 key 不能为空".into());
    }
    def.global = def
        .global
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(collapse_home);
    def.project = def
        .project
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if def.global.is_none() {
        return Err("全局配置路径不能为空".into());
    }
    let mut agents = load_agents();
    if !allow_overwrite && agents.contains_key(&id) {
        return Err(format!("agent 已存在: {}", id));
    }
    agents.insert(id, def);
    save_agents(&agents).map_err(|e| e.to_string())
}

/// List all agent definitions as `AgentInfo` view rows.
pub fn list_infos() -> Vec<AgentInfo> {
    load_agents()
        .into_iter()
        .map(|(id, d)| AgentInfo {
            id,
            format: d.format,
            key: d.key,
            has_global: d.global.is_some(),
            has_project: d.project.is_some(),
            enabled: d.enabled,
            global: d.global,
            project: d.project,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_loads_18_plus() {
        let a = builtin_agents();
        assert!(a.len() >= 18);
        assert_eq!(a["claude-code"].key, "mcpServers");
        assert_eq!(a["codex"].format, "toml");
    }
}
