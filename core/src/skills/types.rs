use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub allowed_tools: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillFileKind {
    File,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillFile {
    pub path: String,
    pub kind: SkillFileKind,
    pub size: u64,
    pub executable: bool,
    pub link_target: Option<String>,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Added,
    Modified,
    Removed,
    ModeChanged,
    LinkChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillFileChange {
    pub path: String,
    pub kind: FileChangeKind,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub unified_diff: Option<String>,
    pub diff_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatedSkill {
    pub manifest: SkillManifest,
    pub content_kind: SkillContentKind,
    pub files: Vec<SkillFile>,
    pub content_hash: String,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillContentKind {
    Automation,
    Assets,
    Reference,
    Instructions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum SkillError {
    InvalidManifest {
        message: String,
        path: String,
    },
    UnsafePath {
        message: String,
        path: String,
    },
    LimitExceeded {
        limit: String,
        actual: u64,
        allowed: u64,
    },
    InvalidSource {
        message: String,
    },
    Network {
        message: String,
        retry_at: Option<String>,
    },
    Conflict {
        message: String,
        path: String,
    },
    PlanStale {
        message: String,
    },
    ConfirmationRequired {
        message: String,
        findings_hash: String,
    },
    RecoveryRequired {
        message: String,
    },
    Io {
        message: String,
        path: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillSource {
    Github {
        owner: String,
        repo: String,
        subpath: String,
        requested_ref: String,
        pinned: bool,
    },
    Local {
        path: String,
        subpath: String,
    },
    Imported {
        original_path: String,
        backup_path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskFinding {
    pub rule_id: String,
    pub rule_version: u32,
    pub level: RiskLevel,
    pub path: String,
    pub line: Option<u32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRiskSummary {
    pub level: RiskLevel,
    #[serde(default)]
    pub findings: Vec<RiskFinding>,
    #[serde(default)]
    pub finding_count: u64,
    #[serde(default)]
    pub findings_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SkillUpdateState {
    pub available: bool,
    pub checked_at: Option<String>,
    pub resolved_revision: Option<String>,
    pub etag: Option<String>,
    pub error: Option<String>,
    pub retry_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedSkillRecord {
    pub name: String,
    pub description: String,
    pub content_kind: SkillContentKind,
    pub source: SkillSource,
    pub resolved_revision: Option<String>,
    pub content_hash: String,
    pub installed_at: String,
    pub updated_at: String,
    pub risk: SkillRiskSummary,
    #[serde(default)]
    pub update: SkillUpdateState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillAgentView {
    pub id: String,
    pub name: String,
    pub target_id: String,
    pub global_dir: String,
    pub affected_agent_ids: Vec<String>,
    pub docs: String,
    pub evidence: String,
    pub verified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillTargetView {
    pub target_id: String,
    pub global_dir: String,
    pub primary_agent_ids: Vec<String>,
    pub affected_agent_ids: Vec<String>,
    pub assignable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InventoryState {
    Managed,
    Assigned,
    External,
    LocallyModified,
    BrokenLink,
    ConflictingLink,
    Missing,
    UpdateAvailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillLocation {
    Central,
    AgentTarget {
        target_id: String,
        global_dir: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInventoryItem {
    pub identity: String,
    pub name: String,
    pub description: String,
    pub content_kind: SkillContentKind,
    pub states: BTreeSet<InventoryState>,
    pub location: SkillLocation,
    pub source: Option<SkillSource>,
    pub resolved_revision: Option<String>,
    pub content_hash: Option<String>,
    pub risk: Option<SkillRiskSummary>,
    pub update: SkillUpdateState,
    pub assigned_target_ids: Vec<String>,
    pub affected_agent_ids: Vec<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillsInventory {
    pub items: Vec<SkillInventoryItem>,
    pub agents: Vec<SkillAgentView>,
    pub targets: Vec<SkillTargetView>,
    pub recovery_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillDetail {
    pub item: SkillInventoryItem,
    pub files: Vec<SkillFile>,
    pub skill_md: String,
    pub skill_md_truncated: bool,
}

pub(crate) fn capped_message(message: impl AsRef<str>) -> String {
    message.as_ref().chars().take(512).collect()
}

pub(crate) fn normalized_error_path(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized.to_string_lossy().replace('\\', "/")
}

pub(crate) fn io_error(path: &Path, error: std::io::Error) -> SkillError {
    SkillError::Io {
        message: capped_message(error.to_string()),
        path: Some(normalized_error_path(path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn managed_skill_records_are_typed_and_absent_settings_remain_compatible() {
        let absent: Settings = serde_json::from_str("{}").unwrap();
        assert!(absent.managed_skills.is_none());

        let settings: Settings = serde_json::from_str(
            r#"{
              "managed_skills": {
                "safe": {
                  "name": "safe",
                  "description": "Safe fixture",
                  "content_kind": "reference",
                  "source": {"kind":"local","path":"~/fixture","subpath":"safe"},
                  "resolved_revision": null,
                  "content_hash": "abc",
                  "installed_at": "2026-07-16T00:00:00Z",
                  "updated_at": "2026-07-16T00:00:00Z",
                  "risk": {"level":"low"}
                }
              }
            }"#,
        )
        .unwrap();
        let record = &settings.managed_skills.as_ref().unwrap()["safe"];
        assert_eq!(record.content_kind, SkillContentKind::Reference);
        assert_eq!(record.risk.level, RiskLevel::Low);
        assert!(record.risk.findings.is_empty());
        assert_eq!(record.risk.finding_count, 0);
        assert!(!record.risk.findings_truncated);
        assert_eq!(record.update, Default::default());
        assert!(matches!(record.source, SkillSource::Local { .. }));
    }
}
