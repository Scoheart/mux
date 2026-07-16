use mux_core::skills::{
    InventoryState, ManagedSkillRecord, RiskLevel, SkillContentKind, SkillRiskSummary, SkillSource,
    SkillUpdateState, SkillsInventory,
};
use std::fs;
use std::path::{Path, PathBuf};

pub fn write_skill(root: &Path, name: &str, description: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nFixture body.\n"),
    )
    .unwrap();
}

pub fn managed_record(name: &str, content_hash: &str) -> ManagedSkillRecord {
    ManagedSkillRecord {
        name: name.into(),
        description: "Managed fixture".into(),
        content_kind: SkillContentKind::Instructions,
        source: SkillSource::Local {
            path: "~/fixtures".into(),
            subpath: name.into(),
        },
        resolved_revision: None,
        content_hash: content_hash.into(),
        installed_at: "2026-07-16T00:00:00Z".into(),
        updated_at: "2026-07-16T00:00:00Z".into(),
        risk: SkillRiskSummary {
            level: RiskLevel::Low,
            findings: Vec::new(),
            finding_count: 0,
            findings_truncated: false,
        },
        update: SkillUpdateState::default(),
    }
}

#[allow(dead_code)]
pub fn assert_managed_link(path: PathBuf, central: PathBuf) {
    assert!(fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(path).unwrap(),
        fs::canonicalize(central).unwrap()
    );
}

pub fn has_state(inventory: &SkillsInventory, name: &str, state: InventoryState) -> bool {
    inventory
        .items
        .iter()
        .any(|item| item.name == name && item.states.contains(&state))
}
