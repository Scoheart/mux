use crate::settings::{load_settings, mutate_settings};
use crate::types::AgentDefinition;
use std::collections::BTreeMap;

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
