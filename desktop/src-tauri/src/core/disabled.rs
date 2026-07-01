use crate::core::settings::{load_settings, mutate_settings};
use crate::core::types::McpConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// MUX 记住的「已禁用」MCP（存于 `~/.mux/settings.json` 的 `disabled` 分区）。
///
/// 禁用一个 MCP 时，MUX 会把它从 agent 的真实配置文件里移除（这样 agent 不再加载它），
/// 同时把它当时的配置快照存到这里，以便之后原样恢复。一个 in-file 的 `disabled` 标记
/// 不行——大多数 agent（如 Claude Code 的 `.claude.json`）并不认识这种标记，仍会照常加载。
///
/// 一条被禁用的 MCP 记录。`config` 是禁用那一刻的配置快照，因此重新启用时能原样写回
/// （包括用户做过的自定义改动）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisabledEntry {
    pub name: String,
    pub transport: String,
    pub scope: String,
    pub config: McpConfig,
}

/// 读取禁用库：agent_id -> 该 agent 下被禁用的 MCP 列表。缺失时返回空 map。
pub fn load_disabled() -> BTreeMap<String, Vec<DisabledEntry>> {
    load_settings().disabled.unwrap_or_default()
}

/// 将完整禁用库写入 settings.disabled（保留其它分区不动）。
pub fn save_disabled(map: &BTreeMap<String, Vec<DisabledEntry>>) -> std::io::Result<()> {
    mutate_settings(|s| {
        s.disabled = Some(map.clone());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{HttpConfig, StdioConfig};

    #[test]
    fn entry_roundtrips_through_json() {
        let mut store: BTreeMap<String, Vec<DisabledEntry>> = BTreeMap::new();
        store.insert(
            "claude-code".into(),
            vec![DisabledEntry {
                name: "figma".into(),
                transport: "stdio".into(),
                scope: "global".into(),
                config: McpConfig::Stdio(StdioConfig {
                    command: "npx".into(),
                    args: Some(vec!["-y".into(), "figma".into()]),
                    env: None,
                }),
            }],
        );
        let json = serde_json::to_string(&store).unwrap();
        let back: BTreeMap<String, Vec<DisabledEntry>> = serde_json::from_str(&json).unwrap();
        assert_eq!(store, back);
    }

    #[test]
    fn http_entry_roundtrips() {
        let entry = DisabledEntry {
            name: "amap".into(),
            transport: "http".into(),
            scope: "global".into(),
            config: McpConfig::Http(HttpConfig {
                kind: "http".into(),
                url: "https://example.com".into(),
                headers: None,
            }),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: DisabledEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }
}
