use crate::settings::{load_settings, mutate_settings};
use crate::types::McpConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// MUX 记住的「已禁用」MCP（存于 `~/.mux/settings.json` 的 `disabled` 分区）。
///
/// 禁用一个 MCP 时，MUX 会把它从 agent 的真实配置文件里移除（这样 agent 不再加载它），
/// 同时把它当时的配置快照存到这里，以便之后完整恢复其配置字段。一个 in-file 的 `disabled` 标记
/// 不行——大多数 agent（如 Claude Code 的 `.claude.json`）并不认识这种标记，仍会照常加载。
///
/// 一条被禁用的 MCP 记录。`config` 兼容旧版本并用于展示；`snapshot` 保存目标 server
/// 的完整语义值，因此重新启用时还能恢复 MUX 不建模的 Agent 策略字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisabledEntry {
    pub name: String,
    pub transport: String,
    pub scope: String,
    pub config: McpConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Value>,
}

/// 读取禁用库：agent_id -> 该 agent 下被禁用的 MCP 列表。缺失时返回空 map。
pub fn load_disabled() -> BTreeMap<String, Vec<DisabledEntry>> {
    load_settings().disabled.unwrap_or_default()
}

/// Insert or replace one disabled snapshot inside the settings mutation lock.
/// This avoids stale whole-map writes when the CLI and desktop act concurrently.
pub fn remember(agent: &str, entry: DisabledEntry) -> std::io::Result<()> {
    mutate_settings(|settings| {
        let store = settings.disabled.get_or_insert_with(BTreeMap::new);
        let list = store.entry(agent.to_string()).or_default();
        list.retain(|saved| {
            !(saved.name == entry.name
                && saved.transport == entry.transport
                && saved.scope == entry.scope)
        });
        list.push(entry);
    })
}

/// Remove exactly the snapshot that was restored. A concurrently replaced
/// snapshot is retained and reported as a conflict instead of being discarded.
pub fn remove_if_unchanged(agent: &str, expected: &DisabledEntry) -> std::io::Result<bool> {
    mutate_settings(|settings| {
        let Some(store) = settings.disabled.as_mut() else {
            return false;
        };
        let mut removed = false;
        let mut remove_agent = false;
        if let Some(list) = store.get_mut(agent) {
            if let Some(index) = list.iter().position(|entry| entry == expected) {
                list.remove(index);
                removed = true;
            }
            remove_agent = list.is_empty();
        }
        if remove_agent {
            store.remove(agent);
        }
        if store.is_empty() {
            settings.disabled = None;
        }
        removed
    })
}

/// Purge one disabled identity, regardless of snapshot contents. Used only by
/// explicit hard-delete operations.
pub fn purge(agent: &str, name: &str, transport: &str, scope: &str) -> std::io::Result<bool> {
    mutate_settings(|settings| {
        let Some(store) = settings.disabled.as_mut() else {
            return false;
        };
        let mut changed = false;
        let mut remove_agent = false;
        if let Some(list) = store.get_mut(agent) {
            let before = list.len();
            list.retain(|entry| {
                !(entry.name == name && entry.transport == transport && entry.scope == scope)
            });
            changed = list.len() != before;
            remove_agent = list.is_empty();
        }
        if remove_agent {
            store.remove(agent);
        }
        if store.is_empty() {
            settings.disabled = None;
        }
        changed
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HttpConfig, StdioConfig};

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
                    cwd: None,
                }),
                snapshot: Some(serde_json::json!({
                    "command": "npx",
                    "enabled": false,
                    "timeout": 120
                })),
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
            snapshot: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: DisabledEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn entry_mutations_preserve_unrelated_snapshots() {
        let _home = crate::testenv::TestHome::new("disabled-mutations");
        let first = DisabledEntry {
            name: "first".into(),
            transport: "stdio".into(),
            scope: "global".into(),
            config: McpConfig::Stdio(StdioConfig {
                command: "one".into(),
                args: None,
                env: None,
                cwd: None,
            }),
            snapshot: Some(serde_json::json!({"command": "one"})),
        };
        let second = DisabledEntry {
            name: "second".into(),
            transport: "stdio".into(),
            scope: "global".into(),
            config: McpConfig::Stdio(StdioConfig {
                command: "two".into(),
                args: None,
                env: None,
                cwd: None,
            }),
            snapshot: Some(serde_json::json!({"command": "two"})),
        };
        remember("agent-a", first.clone()).unwrap();
        remember("agent-b", second.clone()).unwrap();

        let mut stale = first.clone();
        stale.snapshot = Some(serde_json::json!({"command": "stale"}));
        assert!(!remove_if_unchanged("agent-a", &stale).unwrap());
        assert!(remove_if_unchanged("agent-a", &first).unwrap());
        assert_eq!(load_disabled()["agent-b"], vec![second]);

        assert!(purge("agent-b", "second", "stdio", "global").unwrap());
        assert!(load_disabled().is_empty());
    }
}
