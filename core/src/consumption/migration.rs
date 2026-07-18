use crate::ops::scan_installed;
use crate::paths::settings_file;
use crate::registry::read_registry;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum McpAdoptionStatus {
    Adoptable,
    Drifted,
    External,
}

/// Read-only migration evidence. Hashes bind a later plan to the settings and
/// target bytes that were reviewed; no observed config or private path is
/// serialized into the candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpAdoptionCandidate {
    pub agent_id: String,
    pub asset_key: String,
    pub enabled: bool,
    pub status: McpAdoptionStatus,
    pub settings_hash: String,
    pub target_hash: String,
    pub candidate_hash: String,
}

pub fn list_mcp_adoption_candidates() -> Vec<McpAdoptionCandidate> {
    let central: BTreeSet<String> = read_registry()
        .into_iter()
        .map(|entry| entry.key())
        .collect();
    let settings_hash = hash_optional(fs::read(settings_file()).ok().as_deref());
    let mut candidates: Vec<_> = scan_installed(None)
        .into_iter()
        .filter(|item| item.scope == "global")
        .map(|item| {
            let asset_key = format!("{}::{}", item.name, item.transport);
            let status = if !central.contains(&asset_key) {
                McpAdoptionStatus::External
            } else if item.customized {
                McpAdoptionStatus::Drifted
            } else {
                McpAdoptionStatus::Adoptable
            };
            let target_hash = if item.file_path.is_empty() {
                hash_optional(None)
            } else {
                hash_optional(fs::read(&item.file_path).ok().as_deref())
            };
            let candidate_hash = hash_fields(&[
                item.agent.as_bytes(),
                asset_key.as_bytes(),
                if item.enabled {
                    b"enabled"
                } else {
                    b"disabled"
                },
                settings_hash.as_bytes(),
                target_hash.as_bytes(),
            ]);
            McpAdoptionCandidate {
                agent_id: item.agent,
                asset_key,
                enabled: item.enabled,
                status,
                settings_hash: settings_hash.clone(),
                target_hash,
                candidate_hash,
            }
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.agent_id
            .cmp(&right.agent_id)
            .then_with(|| left.asset_key.cmp(&right.asset_key))
    });
    candidates
}

fn hash_optional(bytes: Option<&[u8]>) -> String {
    match bytes {
        Some(bytes) => hex::encode(Sha256::digest(bytes)),
        None => "missing".into(),
    }
}

fn hash_fields(fields: &[&[u8]]) -> String {
    let mut hash = Sha256::new();
    for field in fields {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field);
    }
    hex::encode(hash.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::install;
    use crate::registry::write_manual_entry;
    use crate::testenv::TestHome;
    use crate::types::{RegistryConfig, RegistryEntry, StdioConfig};
    use std::collections::HashMap;

    #[test]
    fn exact_observation_is_adoptable_without_creating_a_relationship() {
        let home = TestHome::new("mcp-adopt");
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: Some(StdioConfig {
                    command: "local-server".into(),
                    args: None,
                    env: None,
                    cwd: None,
                }),
                http: None,
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        install(
            "local",
            "stdio",
            "global",
            &["claude-code".into()],
            None,
            &HashMap::new(),
        )
        .unwrap();
        let before = fs::read(home.home.join(".mux/settings.json")).unwrap();

        let candidates = list_mcp_adoption_candidates();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].status, McpAdoptionStatus::Adoptable);
        assert_eq!(
            fs::read(home.home.join(".mux/settings.json")).unwrap(),
            before
        );
        assert!(crate::settings::load_settings().mcp_consumptions.is_none());
    }
}
