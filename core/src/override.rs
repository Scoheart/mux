use crate::types::{HttpConfig, McpConfig, StdioConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Partial override: contains only fields differing from canonical
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverridePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// effective = canonical ⊕ patch
pub fn apply_override(base: &McpConfig, patch: &OverridePatch) -> McpConfig {
    match base {
        McpConfig::Stdio(s) => McpConfig::Stdio(StdioConfig {
            command: s.command.clone(),
            args: patch.args.clone().or_else(|| s.args.clone()),
            env: patch.env.clone().or_else(|| s.env.clone()),
            cwd: s.cwd.clone(),
        }),
        McpConfig::Http(h) => McpConfig::Http(HttpConfig {
            kind: h.kind.clone(),
            url: patch.url.clone().unwrap_or_else(|| h.url.clone()),
            headers: patch.headers.clone().or_else(|| h.headers.clone()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_overrides_env_keeps_command() {
        let base = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into()]),
            env: Some(HashMap::from([("T".into(), "a".into())])),
            cwd: None,
        });
        let mut env = HashMap::new();
        env.insert("T".to_string(), "b".to_string());
        let patch = OverridePatch {
            env: Some(env),
            ..Default::default()
        };
        if let McpConfig::Stdio(eff) = apply_override(&base, &patch) {
            assert_eq!(eff.command, "npx");
            assert_eq!(eff.env.unwrap().get("T").unwrap(), "b");
        } else {
            panic!("expected stdio");
        }
    }
}
