//! Skill value objects persisted independently from the Skill resource engine.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillContentKind {
    Automation,
    Assets,
    Reference,
    Instructions,
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
    Archive {
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
#[serde(deny_unknown_fields)]
pub struct SkillSettingsSnapshot {
    pub managed_skills: Option<BTreeMap<String, ManagedSkillRecord>>,
    pub skill_assignments: Option<BTreeMap<String, BTreeSet<String>>>,
    pub skill_consumptions:
        Option<BTreeMap<String, BTreeMap<String, crate::domain::assets::SkillConsumptionRecord>>>,
    pub skill_update_checked_at: Option<String>,
}
