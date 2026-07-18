use super::compatibility::compatibility_for;
use super::inventory::list_consumption_inventory;
use super::types::{
    AgentConsumptionSelection, AssetOperationKind, AssetOperationPlan, AssetRef,
    CentralAssetChange, ConsumptionStatus, DomainPlan, PlanSetAgentConsumptionRequest,
    PlanSetAssetConsumersRequest, RelationshipAction, RelationshipChange,
};
use crate::agents::load_agents;
use crate::paths::{mux_dir, settings_file};
use crate::scanner::expand_tilde;
use crate::settings::{load_settings_strict, Settings};
use crate::skills::list_inventory as list_skills_inventory;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const OPERATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PersistedAssetOperation {
    pub schema_version: u32,
    pub plan: AssetOperationPlan,
    pub settings_hash: String,
    pub target_hashes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "kebab-case")]
pub(crate) enum LifecycleBinding {
    McpUpsert {
        key: String,
        draft_hash: String,
    },
    McpDelete {
        key: String,
        source_id: String,
        fallback_exists: bool,
        effective_before: bool,
    },
    ModelUpsert {
        profile_id: String,
        draft_hash: String,
        credential_action: CredentialAction,
    },
    ModelDelete {
        profile_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CredentialAction {
    Keep,
    Set,
    Clear,
}

pub fn plan_set_agent_consumption(
    request: PlanSetAgentConsumptionRequest,
) -> Result<AssetOperationPlan, String> {
    validate_agent_id(&request.agent_id)?;
    let selection = request
        .selection
        .normalize()
        .map_err(|error| error.to_string())?;
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let domain_plan = match selection {
        AgentConsumptionSelection::Mcp { asset_keys } => {
            validate_unique_mcp_names(&asset_keys)?;
            for key in &asset_keys {
                require_compatible(&request.agent_id, &AssetRef::Mcp { key: key.clone() })?;
            }
            let before = current_mcp_selection(&settings, &request.agent_id);
            DomainPlan::Mcp {
                before: BTreeMap::from([(request.agent_id.clone(), before)]),
                after: BTreeMap::from([(request.agent_id, asset_keys)]),
            }
        }
        AgentConsumptionSelection::Model { profile_ids } => {
            let after_profile = profile_ids.into_iter().next();
            if let Some(profile_id) = &after_profile {
                require_compatible(
                    &request.agent_id,
                    &AssetRef::Model {
                        profile_id: profile_id.clone(),
                    },
                )?;
            }
            let before = settings
                .model_assignments
                .as_ref()
                .and_then(|assignments| assignments.get(&request.agent_id))
                .cloned();
            DomainPlan::Model {
                before: BTreeMap::from([(request.agent_id.clone(), before)]),
                after: BTreeMap::from([(request.agent_id, after_profile)]),
            }
        }
        AgentConsumptionSelection::Skill { names } => {
            for name in &names {
                require_compatible(&request.agent_id, &AssetRef::Skill { name: name.clone() })?;
            }
            let before = current_skill_selection(&request.agent_id)?;
            skill_plan_for_agent(&request.agent_id, before, names)?
        }
    };
    finalize_plan(domain_plan)
}

pub fn plan_set_asset_consumers(
    request: PlanSetAssetConsumersRequest,
) -> Result<AssetOperationPlan, String> {
    request
        .asset
        .validate()
        .map_err(|error| error.to_string())?;
    let selected: BTreeSet<String> = request.agent_ids.into_iter().collect();
    for agent_id in &selected {
        validate_agent_id(agent_id)?;
        require_compatible(agent_id, &request.asset)?;
    }
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let inventory = list_consumption_inventory()?;
    let current: BTreeSet<String> = inventory
        .consumptions
        .iter()
        .filter(|item| item.asset == request.asset && item.desired)
        .map(|item| item.agent_id.clone())
        .collect();
    let affected: BTreeSet<String> = current.union(&selected).cloned().collect();

    let domain_plan = match &request.asset {
        AssetRef::Mcp { key } => {
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for agent_id in affected {
                let existing = current_mcp_selection(&settings, &agent_id);
                let mut desired: BTreeSet<String> = existing.iter().cloned().collect();
                if selected.contains(&agent_id) {
                    desired.insert(key.clone());
                } else {
                    desired.remove(key);
                }
                validate_unique_mcp_names(&desired.iter().cloned().collect::<Vec<_>>())?;
                before.insert(agent_id.clone(), existing);
                after.insert(agent_id, desired.into_iter().collect());
            }
            DomainPlan::Mcp { before, after }
        }
        AssetRef::Model { profile_id } => {
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for agent_id in affected {
                let existing = settings
                    .model_assignments
                    .as_ref()
                    .and_then(|assignments| assignments.get(&agent_id))
                    .cloned();
                let desired = if selected.contains(&agent_id) {
                    Some(profile_id.clone())
                } else if existing.as_deref() == Some(profile_id) {
                    None
                } else {
                    existing.clone()
                };
                before.insert(agent_id.clone(), existing);
                after.insert(agent_id, desired);
            }
            DomainPlan::Model { before, after }
        }
        AssetRef::Skill { name } => {
            validate_closed_skill_consumers(name, &selected)?;
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for agent_id in affected {
                let existing = current_skill_selection(&agent_id)?;
                let mut desired: BTreeSet<String> = existing.iter().cloned().collect();
                if selected.contains(&agent_id) {
                    desired.insert(name.clone());
                } else {
                    desired.remove(name);
                }
                before.insert(agent_id.clone(), existing);
                after.insert(agent_id, desired.into_iter().collect());
            }
            DomainPlan::Skill { before, after }
        }
    };
    finalize_plan(domain_plan)
}

fn validate_unique_mcp_names(keys: &[String]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for key in keys {
        let name = key
            .rsplit_once("::")
            .map(|(name, _)| name)
            .ok_or_else(|| format!("invalid MCP asset key: {key}"))?;
        if !names.insert(name) {
            return Err(format!(
                "mcp_identity_conflict: one Agent cannot consume two transport variants named {name}"
            ));
        }
    }
    Ok(())
}

fn skill_plan_for_agent(
    agent_id: &str,
    current: Vec<String>,
    desired: Vec<String>,
) -> Result<DomainPlan, String> {
    let current_set: BTreeSet<String> = current.iter().cloned().collect();
    let desired_set: BTreeSet<String> = desired.iter().cloned().collect();
    let changed: Vec<String> = current_set
        .symmetric_difference(&desired_set)
        .cloned()
        .collect();
    let mut affected = BTreeSet::from([agent_id.to_string()]);
    for name in &changed {
        let compatibility = compatibility_for(agent_id, &AssetRef::Skill { name: name.clone() })?;
        affected.extend(compatibility.affected_agent_ids);
    }
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for affected_agent in affected {
        let existing = current_skill_selection(&affected_agent)?;
        let mut next: BTreeSet<String> = existing.iter().cloned().collect();
        for name in &changed {
            if desired_set.contains(name) {
                next.insert(name.clone());
            } else {
                next.remove(name);
            }
        }
        before.insert(affected_agent.clone(), existing);
        after.insert(affected_agent, next.into_iter().collect());
    }
    Ok(DomainPlan::Skill { before, after })
}

fn validate_closed_skill_consumers(name: &str, selected: &BTreeSet<String>) -> Result<(), String> {
    for agent_id in selected {
        let compatibility = compatibility_for(
            agent_id,
            &AssetRef::Skill {
                name: name.to_string(),
            },
        )?;
        let missing: Vec<_> = compatibility
            .affected_agent_ids
            .into_iter()
            .filter(|affected| !selected.contains(affected))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "skill_shared_target_conflict: {agent_id} shares one physical Skill target with {}",
                missing.join(", ")
            ));
        }
    }
    Ok(())
}

fn require_compatible(agent_id: &str, asset: &AssetRef) -> Result<(), String> {
    let view = compatibility_for(agent_id, asset)?;
    if view.compatible {
        Ok(())
    } else {
        let reason = view.reason.expect("incompatible view has reason");
        Err(format!("{}: {}", reason.code, reason.message))
    }
}

fn validate_agent_id(agent_id: &str) -> Result<(), String> {
    if agent_id.trim().is_empty() || !load_agents().contains_key(agent_id) {
        return Err(format!("unknown Agent: {agent_id}"));
    }
    Ok(())
}

fn current_mcp_selection(settings: &Settings, agent_id: &str) -> Vec<String> {
    settings
        .mcp_consumptions
        .as_ref()
        .and_then(|consumptions| consumptions.get(agent_id))
        .map(|records| records.keys().cloned().collect())
        .unwrap_or_default()
}

fn current_skill_selection(agent_id: &str) -> Result<Vec<String>, String> {
    let names: BTreeSet<String> = list_consumption_inventory()?
        .consumptions
        .into_iter()
        .filter_map(|item| match item.asset {
            AssetRef::Skill { name } if item.agent_id == agent_id && item.desired => Some(name),
            _ => None,
        })
        .collect();
    Ok(names.iter().cloned().collect())
}

fn finalize_plan(domain_plan: DomainPlan) -> Result<AssetOperationPlan, String> {
    finalize_plan_with(
        AssetOperationKind::SetConsumption,
        domain_plan,
        Vec::new(),
        Vec::new(),
        None,
    )
}

pub(crate) fn finalize_plan_with(
    kind: AssetOperationKind,
    domain_plan: DomainPlan,
    central_changes: Vec<CentralAssetChange>,
    extra_target_files: Vec<String>,
    lifecycle: Option<LifecycleBinding>,
) -> Result<AssetOperationPlan, String> {
    if let Some(error) = super::transaction::pending_recovery_error() {
        return Err(format!("recovery_required: {error}"));
    }
    let relationship_changes = relationship_changes(&domain_plan);
    let affected_agent_ids: Vec<String> = agents_for_plan(&domain_plan).into_iter().collect();
    let mut target_files = target_files(&domain_plan)?;
    target_files.extend(extra_target_files);
    target_files.sort();
    target_files.dedup();
    let current_inventory = list_consumption_inventory()?;
    let effects = effect_assets(&domain_plan, &central_changes, &relationship_changes);
    let mut blocked: Vec<_> = current_inventory
        .consumptions
        .iter()
        .filter(|item| {
            effects.contains(&(item.agent_id.clone(), item.asset.clone()))
                && matches!(
                    item.status,
                    ConsumptionStatus::Drifted | ConsumptionStatus::Conflicted
                )
        })
        .map(|item| {
            format!(
                "{}: {}",
                item.agent_id,
                item.reason.as_deref().unwrap_or("unresolved_drift")
            )
        })
        .collect();
    for (agent_id, asset) in &effects {
        if !asset_desired_after(&domain_plan, agent_id, asset) {
            continue;
        }
        for item in current_inventory
            .external
            .iter()
            .filter(|item| external_blocks_selection(agent_id, asset, item))
        {
            blocked.push(format!(
                "{}: {}",
                item.agent_id,
                item.reason.as_deref().unwrap_or("external_asset_conflict")
            ));
        }
    }
    blocked.sort();
    blocked.dedup();
    let warnings = blocked.clone();
    let requires_conflict_confirmation =
        !blocked.is_empty() && kind == AssetOperationKind::UpdateAsset;
    let can_commit = blocked.is_empty() || requires_conflict_confirmation;
    let settings_hash = hash_file(&settings_file());
    let target_hashes = hash_targets(&target_files);
    let operation_id = Uuid::new_v4().hyphenated().to_string();
    let candidate_material = serde_json::to_vec(&(
        &kind,
        &domain_plan,
        &central_changes,
        &relationship_changes,
        &target_files,
        &affected_agent_ids,
        &warnings,
        can_commit,
        requires_conflict_confirmation,
        &settings_hash,
        &target_hashes,
        &lifecycle,
    ))
    .map_err(|error| error.to_string())?;
    let candidate_hash = hex::encode(Sha256::digest(candidate_material));
    let plan = AssetOperationPlan {
        operation_id,
        kind,
        domain_plan,
        central_changes,
        relationship_changes,
        target_files,
        affected_agent_ids,
        warnings,
        can_commit,
        requires_conflict_confirmation,
        candidate_hash,
    };
    persist_operation(&PersistedAssetOperation {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan: plan.clone(),
        settings_hash,
        target_hashes,
        lifecycle,
    })?;
    Ok(plan)
}

fn relationship_changes(plan: &DomainPlan) -> Vec<RelationshipChange> {
    let mut changes = Vec::new();
    match plan {
        DomainPlan::Mcp { before, after } => {
            for agent_id in union_keys(before, after) {
                diff_many(
                    agent_id,
                    before.get(agent_id).cloned().unwrap_or_default(),
                    after.get(agent_id).cloned().unwrap_or_default(),
                    |key| AssetRef::Mcp { key },
                    &mut changes,
                );
            }
        }
        DomainPlan::Skill { before, after } => {
            for agent_id in union_keys(before, after) {
                diff_many(
                    agent_id,
                    before.get(agent_id).cloned().unwrap_or_default(),
                    after.get(agent_id).cloned().unwrap_or_default(),
                    |name| AssetRef::Skill { name },
                    &mut changes,
                );
            }
        }
        DomainPlan::Model { before, after } => {
            for agent_id in union_keys(before, after) {
                let left = before.get(agent_id).cloned().flatten();
                let right = after.get(agent_id).cloned().flatten();
                if left == right {
                    continue;
                }
                if let Some(profile_id) = left {
                    changes.push(RelationshipChange {
                        agent_id: agent_id.clone(),
                        asset: AssetRef::Model { profile_id },
                        action: RelationshipAction::Remove,
                    });
                }
                if let Some(profile_id) = right {
                    changes.push(RelationshipChange {
                        agent_id: agent_id.clone(),
                        asset: AssetRef::Model { profile_id },
                        action: RelationshipAction::Add,
                    });
                }
            }
        }
    }
    changes.sort_by(|left, right| {
        left.agent_id
            .cmp(&right.agent_id)
            .then_with(|| left.asset.cmp(&right.asset))
            .then_with(|| format!("{:?}", left.action).cmp(&format!("{:?}", right.action)))
    });
    changes
}

fn diff_many<F>(
    agent_id: &str,
    before: Vec<String>,
    after: Vec<String>,
    asset: F,
    out: &mut Vec<RelationshipChange>,
) where
    F: Fn(String) -> AssetRef,
{
    let before: BTreeSet<String> = before.into_iter().collect();
    let after: BTreeSet<String> = after.into_iter().collect();
    for identity in before.difference(&after) {
        out.push(RelationshipChange {
            agent_id: agent_id.to_owned(),
            asset: asset(identity.clone()),
            action: RelationshipAction::Remove,
        });
    }
    for identity in after.difference(&before) {
        out.push(RelationshipChange {
            agent_id: agent_id.to_owned(),
            asset: asset(identity.clone()),
            action: RelationshipAction::Add,
        });
    }
}

fn union_keys<'a, T>(
    left: &'a BTreeMap<String, T>,
    right: &'a BTreeMap<String, T>,
) -> BTreeSet<&'a String> {
    left.keys().chain(right.keys()).collect()
}

fn agents_for_plan(plan: &DomainPlan) -> BTreeSet<String> {
    match plan {
        DomainPlan::Mcp { before, after } | DomainPlan::Skill { before, after } => {
            before.keys().chain(after.keys()).cloned().collect()
        }
        DomainPlan::Model { before, after } => before.keys().chain(after.keys()).cloned().collect(),
    }
}

fn domain_matches(plan: &DomainPlan, asset: &AssetRef) -> bool {
    matches!(
        (plan, asset),
        (DomainPlan::Mcp { .. }, AssetRef::Mcp { .. })
            | (DomainPlan::Model { .. }, AssetRef::Model { .. })
            | (DomainPlan::Skill { .. }, AssetRef::Skill { .. })
    )
}

fn effect_assets(
    plan: &DomainPlan,
    central_changes: &[CentralAssetChange],
    relationship_changes: &[RelationshipChange],
) -> BTreeSet<(String, AssetRef)> {
    let mut effects: BTreeSet<(String, AssetRef)> = relationship_changes
        .iter()
        .map(|change| (change.agent_id.clone(), change.asset.clone()))
        .collect();
    let agents = agents_for_plan(plan);
    for change in central_changes {
        if !domain_matches(plan, &change.asset) {
            continue;
        }
        effects.extend(
            agents
                .iter()
                .cloned()
                .map(|agent_id| (agent_id, change.asset.clone())),
        );
    }
    effects
}

fn asset_desired_after(plan: &DomainPlan, agent_id: &str, asset: &AssetRef) -> bool {
    match (plan, asset) {
        (DomainPlan::Mcp { after, .. }, AssetRef::Mcp { key }) => {
            after.get(agent_id).is_some_and(|keys| keys.contains(key))
        }
        (DomainPlan::Model { after, .. }, AssetRef::Model { profile_id }) => after
            .get(agent_id)
            .and_then(Option::as_deref)
            .is_some_and(|desired| desired == profile_id),
        (DomainPlan::Skill { after, .. }, AssetRef::Skill { name }) => after
            .get(agent_id)
            .is_some_and(|names| names.contains(name)),
        _ => false,
    }
}

fn external_blocks_selection(
    agent_id: &str,
    asset: &AssetRef,
    external: &super::types::ConsumptionView,
) -> bool {
    if external.agent_id != agent_id {
        return false;
    }
    match (asset, &external.asset) {
        (AssetRef::Mcp { key }, AssetRef::Mcp { key: external_key }) => {
            key == external_key && external.reason.as_deref() != Some("mcp_adoptable")
        }
        (AssetRef::Model { .. }, AssetRef::Model { .. }) => true,
        (
            AssetRef::Skill { name },
            AssetRef::Skill {
                name: external_name,
            },
        ) => name == external_name,
        _ => false,
    }
}

fn target_files(plan: &DomainPlan) -> Result<Vec<String>, String> {
    let mut files = BTreeSet::new();
    match plan {
        DomainPlan::Mcp { .. } => {
            let agents = load_agents();
            for agent_id in agents_for_plan(plan) {
                if let Some(path) = agents.get(&agent_id).and_then(|agent| agent.global.clone()) {
                    files.insert(path);
                }
            }
        }
        DomainPlan::Model { .. } => {
            for agent_id in agents_for_plan(plan) {
                match agent_id.as_str() {
                    "claude-code" => {
                        files.insert("~/.claude/settings.json".into());
                    }
                    "codex" => {
                        files.insert("~/.codex/config.toml".into());
                    }
                    "pi" => {
                        files.insert("~/.pi/agent/models.json".into());
                        files.insert("~/.pi/agent/settings.json".into());
                    }
                    _ => {}
                }
            }
        }
        DomainPlan::Skill { before, after } => {
            let skills = list_skills_inventory().map_err(|error| format!("{error:?}"))?;
            for agent_id in agents_for_plan(plan) {
                let names: BTreeSet<String> = before
                    .get(&agent_id)
                    .into_iter()
                    .flatten()
                    .chain(after.get(&agent_id).into_iter().flatten())
                    .cloned()
                    .collect();
                for target in &skills.targets {
                    if target.affected_agent_ids.contains(&agent_id)
                        || target.primary_agent_ids.contains(&agent_id)
                    {
                        for name in &names {
                            files.insert(format!("{}/{}", target.global_dir, name));
                        }
                    }
                }
            }
        }
    }
    Ok(files.into_iter().collect())
}

pub(crate) fn hash_targets(targets: &[String]) -> BTreeMap<String, String> {
    targets
        .iter()
        .map(|target| (target.clone(), hash_path(&expand_tilde(target))))
        .collect()
}

pub(crate) fn hash_file(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => hex::encode(Sha256::digest(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => "missing".into(),
        Err(error) => format!("error:{:?}", error.kind()),
    }
}

fn hash_path(path: &Path) -> String {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::read_link(path)
            .map(|target| hex::encode(Sha256::digest(target.as_os_str().as_encoded_bytes())))
            .unwrap_or_else(|error| format!("error:{:?}", error.kind())),
        Ok(metadata) if metadata.is_file() => hash_file(path),
        Ok(metadata) if metadata.is_dir() => "directory".into(),
        Ok(_) => "other".into(),
        Err(error) if error.kind() == ErrorKind::NotFound => "missing".into(),
        Err(error) => format!("error:{:?}", error.kind()),
    }
}

pub(crate) fn operation_root(operation_id: &str) -> PathBuf {
    mux_dir().join("staging/consumption").join(operation_id)
}

fn persist_operation(operation: &PersistedAssetOperation) -> Result<(), String> {
    let root = operation_root(&operation.plan.operation_id);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    let path = root.join("plan.json");
    let bytes = serde_json::to_vec_pretty(operation).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn load_operation(operation_id: &str) -> Result<PersistedAssetOperation, String> {
    Uuid::parse_str(operation_id).map_err(|_| "invalid asset operation id".to_string())?;
    let bytes = fs::read(operation_root(operation_id).join("plan.json"))
        .map_err(|_| "asset operation is unavailable or expired".to_string())?;
    let operation: PersistedAssetOperation = serde_json::from_slice(&bytes)
        .map_err(|_| "asset operation plan is invalid".to_string())?;
    if operation.schema_version != OPERATION_SCHEMA_VERSION
        || operation.plan.operation_id != operation_id
    {
        return Err("asset operation plan is incompatible".into());
    }
    Ok(operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::write_manual_entry;
    use crate::settings::mutate_settings;
    use crate::testenv::TestHome;
    use crate::types::{HttpConfig, RegistryConfig, RegistryEntry, StdioConfig};

    #[test]
    fn mcp_plan_has_stable_typed_diff_and_private_persistence() {
        let home = TestHome::new("consume-plan");
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
        mutate_settings(|settings| settings.mcp_consumptions = None).unwrap();

        let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();

        assert!(plan.can_commit);
        assert_eq!(plan.relationship_changes.len(), 1);
        assert_eq!(plan.relationship_changes[0].action, RelationshipAction::Add);
        let persisted = fs::read_to_string(
            home.home
                .join(".mux/staging/consumption")
                .join(&plan.operation_id)
                .join("plan.json"),
        )
        .unwrap();
        assert!(!persisted.contains(home.home.to_string_lossy().as_ref()));
    }

    #[test]
    fn agent_and_asset_entrypoints_produce_the_same_candidate() {
        let _home = TestHome::new("consume-plan-equivalent");
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
        let agent = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap();
        let asset = plan_set_asset_consumers(PlanSetAssetConsumersRequest {
            asset: AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            agent_ids: vec!["claude-code".into()],
        })
        .unwrap();
        assert_eq!(agent.domain_plan, asset.domain_plan);
        assert_eq!(agent.relationship_changes, asset.relationship_changes);
        assert_eq!(agent.target_files, asset.target_files);
        assert_eq!(agent.candidate_hash, asset.candidate_hash);
    }

    #[test]
    fn one_agent_cannot_select_two_transports_with_the_same_mcp_name() {
        let _home = TestHome::new("consume-mcp-name-conflict");
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
        write_manual_entry(&RegistryEntry {
            name: "local".into(),
            description: String::new(),
            tags: Vec::new(),
            config: RegistryConfig {
                stdio: None,
                http: Some(HttpConfig {
                    kind: "http".into(),
                    url: "https://example.invalid/mcp".into(),
                    headers: None,
                }),
            },
            origin: None,
            repo: None,
        })
        .unwrap();
        let error = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into(), "local::http".into()],
            },
        })
        .unwrap_err();
        assert!(error.starts_with("mcp_identity_conflict:"));
    }
}
