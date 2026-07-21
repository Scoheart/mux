use super::compatibility::compatibility_for;
use super::types::{
    AssetRef, ConsumptionInventory, ConsumptionStatus, ConsumptionTarget, ConsumptionView,
};
use crate::models::{
    list_agents as list_model_agents, observe_active_model_for_settings, observe_external_model,
    observe_profile_consumption, ExternalModelObservedState, ModelObservedState,
    ObservedActiveModel,
};
use crate::ops::scan_installed;
use crate::registry::read_registry;
use crate::settings::load_settings_strict;
use crate::skills::{list_inventory as list_skills_inventory, InventoryState, SkillLocation};
use std::collections::{BTreeMap, BTreeSet};

pub fn list_consumption_inventory() -> Result<ConsumptionInventory, String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let mut inventory = ConsumptionInventory {
        recovery_error: super::transaction::pending_recovery_error(),
        ..Default::default()
    };
    project_mcps(&settings, &mut inventory)?;
    project_models(&settings, &mut inventory)?;
    project_skills(&settings, &mut inventory)?;
    sort_inventory(&mut inventory);
    Ok(inventory)
}

fn project_mcps(
    settings: &crate::settings::Settings,
    inventory: &mut ConsumptionInventory,
) -> Result<(), String> {
    let central: BTreeSet<String> = read_registry()
        .into_iter()
        .map(|entry| entry.key())
        .collect();
    let observed = scan_installed(None);
    let mut consumed_observations = BTreeSet::new();

    for (agent_id, records) in settings.mcp_consumptions.iter().flatten() {
        for (map_key, record) in records {
            let asset = AssetRef::Mcp {
                key: record.asset_key.clone(),
            };
            let compatibility = compatibility_for(agent_id, &asset)?;
            let identity_matches = map_key == &record.asset_key;
            let match_index = observed.iter().position(|item| {
                item.agent == *agent_id
                    && item.scope == "global"
                    && format!("{}::{}", item.name, item.transport) == record.asset_key
            });
            let matching = match_index.map(|index| &observed[index]);
            let (status, reason, is_observed) = if !identity_matches {
                (
                    ConsumptionStatus::Conflicted,
                    Some("mcp_record_identity_mismatch".into()),
                    matching.is_some(),
                )
            } else if !central.contains(&record.asset_key) {
                (
                    ConsumptionStatus::Conflicted,
                    Some("mcp_asset_missing".into()),
                    matching.is_some(),
                )
            } else if !compatibility.compatible {
                (
                    ConsumptionStatus::Unsupported,
                    compatibility.reason.map(|reason| reason.code),
                    matching.is_some(),
                )
            } else {
                match matching {
                    None => (
                        ConsumptionStatus::Drifted,
                        Some("mcp_target_missing".into()),
                        false,
                    ),
                    Some(item) if item.enabled != record.enabled => (
                        ConsumptionStatus::Drifted,
                        Some("mcp_enabled_state_drift".into()),
                        true,
                    ),
                    Some(item) if item.customized => (
                        ConsumptionStatus::Drifted,
                        Some("mcp_config_drift".into()),
                        true,
                    ),
                    Some(_) => (ConsumptionStatus::Synced, None, true),
                }
            };
            if let Some(index) = match_index {
                consumed_observations.insert(index);
            }
            inventory.consumptions.push(ConsumptionView {
                agent_id: agent_id.clone(),
                asset,
                desired: true,
                observed: is_observed,
                enabled: Some(record.enabled),
                active: None,
                desired_active: None,
                status,
                reason,
                affected_agent_ids: if compatibility.affected_agent_ids.is_empty() {
                    vec![agent_id.clone()]
                } else {
                    compatibility.affected_agent_ids
                },
                target: None,
            });
        }
    }

    for (index, item) in observed.into_iter().enumerate() {
        if consumed_observations.contains(&index) || item.scope != "global" {
            continue;
        }
        let key = format!("{}::{}", item.name, item.transport);
        inventory.external.push(ConsumptionView {
            agent_id: item.agent.clone(),
            asset: AssetRef::Mcp { key: key.clone() },
            desired: false,
            observed: true,
            enabled: Some(item.enabled),
            active: None,
            desired_active: None,
            status: ConsumptionStatus::External,
            reason: Some(
                if central.contains(&key) && !item.customized {
                    "mcp_adoptable"
                } else if central.contains(&key) {
                    "mcp_external_customized"
                } else {
                    "mcp_external_unmanaged"
                }
                .into(),
            ),
            affected_agent_ids: vec![item.agent],
            target: None,
        });
    }
    Ok(())
}

fn project_models(
    settings: &crate::settings::Settings,
    inventory: &mut ConsumptionInventory,
) -> Result<(), String> {
    let assigned_agents: BTreeSet<String> = settings
        .model_consumptions
        .iter()
        .flatten()
        .map(|(agent_id, _)| agent_id.clone())
        .chain(
            settings
                .model_assignments
                .iter()
                .flatten()
                .map(|(agent_id, _)| agent_id.clone()),
        )
        .collect();
    for agent_id in &assigned_agents {
        let selection = settings.model_selection(agent_id);
        let observed_active = observe_active_model_for_settings(settings, agent_id);
        let observed_active_profile = match &observed_active {
            ObservedActiveModel::Managed(profile_id) => Some(profile_id.as_str()),
            _ => None,
        };
        for (profile_id, record) in &selection.profiles {
            let asset = AssetRef::Model {
                profile_id: profile_id.clone(),
            };
            let compatibility = compatibility_for(agent_id, &asset)?;
            let Some(profile) = settings
                .model_profiles
                .as_ref()
                .and_then(|profiles| profiles.get(profile_id))
            else {
                inventory.consumptions.push(ConsumptionView {
                    agent_id: agent_id.clone(),
                    asset,
                    desired: true,
                    observed: false,
                    enabled: Some(record.enabled),
                    active: Some(observed_active_profile == Some(profile_id)),
                    desired_active: Some(
                        selection.active_profile_id.as_deref() == Some(profile_id.as_str()),
                    ),
                    status: ConsumptionStatus::Conflicted,
                    reason: Some("model_profile_missing".into()),
                    affected_agent_ids: vec![agent_id.clone()],
                    target: None,
                });
                continue;
            };
            let desired_active =
                selection.active_profile_id.as_deref() == Some(profile_id.as_str());
            let observed_is_active = observed_active_profile == Some(profile_id);
            let (observed, status, reason) = if !compatibility.compatible {
                (
                    false,
                    ConsumptionStatus::Unsupported,
                    compatibility.reason.map(|reason| reason.code),
                )
            } else {
                let state = match (
                    record.enabled,
                    observe_profile_consumption(agent_id, profile, observed_is_active)?,
                ) {
                    (true, ModelObservedState::Synced) => (true, ConsumptionStatus::Synced, None),
                    (true, ModelObservedState::Missing) => (
                        false,
                        ConsumptionStatus::Drifted,
                        Some("model_target_missing".into()),
                    ),
                    (true, ModelObservedState::Drifted) => (
                        true,
                        ConsumptionStatus::Drifted,
                        Some("model_owned_fields_drift".into()),
                    ),
                    (true, ModelObservedState::Conflicted) => (
                        true,
                        ConsumptionStatus::Conflicted,
                        Some("model_target_conflicted".into()),
                    ),
                    (false, ModelObservedState::Missing) => {
                        (false, ConsumptionStatus::Synced, None)
                    }
                    (false, ModelObservedState::Conflicted) => (
                        true,
                        ConsumptionStatus::Conflicted,
                        Some("model_target_conflicted".into()),
                    ),
                    (false, _) => (
                        true,
                        ConsumptionStatus::Drifted,
                        Some("model_disabled_state_drift".into()),
                    ),
                };
                if state.1 == ConsumptionStatus::Synced && desired_active != observed_is_active {
                    (
                        state.0,
                        ConsumptionStatus::Drifted,
                        Some("model_active_state_drift".into()),
                    )
                } else {
                    state
                }
            };
            inventory.consumptions.push(ConsumptionView {
                agent_id: agent_id.clone(),
                asset,
                desired: true,
                observed,
                enabled: Some(record.enabled),
                active: Some(observed_is_active),
                desired_active: Some(desired_active),
                status,
                reason,
                affected_agent_ids: vec![agent_id.clone()],
                target: None,
            });
        }
        let external_reason = match observed_active {
            ObservedActiveModel::External => {
                Some((ConsumptionStatus::External, "model_external_current"))
            }
            ObservedActiveModel::Conflicted => {
                Some((ConsumptionStatus::Conflicted, "model_active_conflicted"))
            }
            _ => None,
        };
        if let Some((status, reason)) = external_reason {
            inventory.external.push(ConsumptionView {
                agent_id: agent_id.clone(),
                asset: AssetRef::Model {
                    profile_id: format!("external-{agent_id}"),
                },
                desired: false,
                observed: true,
                enabled: None,
                active: Some(true),
                desired_active: Some(false),
                status,
                reason: Some(reason.into()),
                affected_agent_ids: vec![agent_id.clone()],
                target: None,
            });
        }
    }
    for agent in list_model_agents()
        .into_iter()
        .filter(|agent| agent.mode == "managed" && !assigned_agents.contains(&agent.id))
    {
        let (status, reason) = match observe_external_model(&agent.id)? {
            ExternalModelObservedState::Absent => continue,
            ExternalModelObservedState::Present => (
                ConsumptionStatus::External,
                Some("model_external_unmanaged".into()),
            ),
            ExternalModelObservedState::Conflicted => (
                ConsumptionStatus::Conflicted,
                Some("model_external_conflicted".into()),
            ),
        };
        inventory.external.push(ConsumptionView {
            agent_id: agent.id.clone(),
            asset: AssetRef::Model {
                profile_id: format!("external-{}", agent.id),
            },
            desired: false,
            observed: true,
            enabled: None,
            active: Some(true),
            desired_active: Some(false),
            status,
            reason,
            affected_agent_ids: vec![agent.id],
            target: None,
        });
    }
    Ok(())
}

fn project_skills(
    settings: &crate::settings::Settings,
    inventory: &mut ConsumptionInventory,
) -> Result<(), String> {
    let skills = list_skills_inventory().map_err(|error| format!("{error:?}"))?;
    let targets: BTreeMap<_, _> = skills
        .targets
        .iter()
        .map(|target| (target.target_id.as_str(), target))
        .collect();
    let target_items: BTreeMap<_, _> = skills
        .items
        .iter()
        .filter_map(|item| match &item.location {
            SkillLocation::AgentTarget { target_id, .. } => {
                Some(((target_id.as_str(), item.name.as_str()), item))
            }
            SkillLocation::Central => None,
        })
        .collect();
    let mut canonical_assignments = BTreeMap::<String, BTreeSet<String>>::new();
    for item in &skills.items {
        canonical_assignments
            .entry(item.name.clone())
            .or_default()
            .extend(item.assigned_target_ids.iter().cloned());
    }
    let mut desired_physical = BTreeSet::new();

    for (name, _) in settings.skill_assignments.iter().flatten() {
        let target_ids = canonical_assignments.get(name).cloned().unwrap_or_default();
        for target_id in target_ids {
            desired_physical.insert((target_id.clone(), name.clone()));
            let Some(target) = targets.get(target_id.as_str()) else {
                inventory.consumptions.push(ConsumptionView {
                    agent_id: target_id.clone(),
                    asset: AssetRef::Skill { name: name.clone() },
                    desired: true,
                    observed: false,
                    enabled: None,
                    active: None,
                    desired_active: None,
                    status: ConsumptionStatus::Conflicted,
                    reason: Some("skill_target_unknown".into()),
                    affected_agent_ids: Vec::new(),
                    target: None,
                });
                continue;
            };
            let physical = target_items.get(&(target_id.as_str(), name.as_str()));
            let (observed, status, reason) = match physical {
                None => (
                    false,
                    ConsumptionStatus::Drifted,
                    Some("skill_target_missing".into()),
                ),
                Some(item) if item.states.contains(&InventoryState::BrokenLink) => (
                    true,
                    ConsumptionStatus::Drifted,
                    Some("skill_broken_link".into()),
                ),
                Some(item) if item.states.contains(&InventoryState::ConflictingLink) => (
                    true,
                    ConsumptionStatus::Conflicted,
                    Some("skill_conflicting_link".into()),
                ),
                Some(item) if item.states.contains(&InventoryState::LocallyModified) => (
                    true,
                    ConsumptionStatus::Drifted,
                    Some("skill_local_modification".into()),
                ),
                Some(item) if item.states.contains(&InventoryState::Missing) => (
                    false,
                    ConsumptionStatus::Drifted,
                    Some("skill_target_missing".into()),
                ),
                Some(_) => (true, ConsumptionStatus::Synced, None),
            };
            let agents = if target.affected_agent_ids.is_empty() {
                target.primary_agent_ids.clone()
            } else {
                target.affected_agent_ids.clone()
            };
            for agent_id in &agents {
                inventory.consumptions.push(ConsumptionView {
                    agent_id: agent_id.clone(),
                    asset: AssetRef::Skill { name: name.clone() },
                    desired: true,
                    observed,
                    enabled: None,
                    active: None,
                    desired_active: None,
                    status: status.clone(),
                    reason: reason.clone(),
                    affected_agent_ids: agents.clone(),
                    target: Some(ConsumptionTarget {
                        target_id: target.target_id.clone(),
                        global_dir: target.global_dir.clone(),
                    }),
                });
            }
        }
    }

    for item in &skills.items {
        let SkillLocation::AgentTarget {
            target_id,
            global_dir,
        } = &item.location
        else {
            continue;
        };
        if desired_physical.contains(&(target_id.clone(), item.name.clone())) {
            continue;
        }
        let Some(target) = targets.get(target_id.as_str()) else {
            continue;
        };
        let agents = if target.affected_agent_ids.is_empty() {
            target.primary_agent_ids.clone()
        } else {
            target.affected_agent_ids.clone()
        };
        for agent_id in &agents {
            inventory.external.push(ConsumptionView {
                agent_id: agent_id.clone(),
                asset: AssetRef::Skill {
                    name: item.name.clone(),
                },
                desired: false,
                observed: true,
                enabled: None,
                active: None,
                desired_active: None,
                status: ConsumptionStatus::External,
                reason: Some("skill_external".into()),
                affected_agent_ids: agents.clone(),
                target: Some(ConsumptionTarget {
                    target_id: target_id.clone(),
                    global_dir: global_dir.clone(),
                }),
            });
        }
    }
    Ok(())
}

fn sort_inventory(inventory: &mut ConsumptionInventory) {
    let sort = |items: &mut Vec<ConsumptionView>| {
        items.sort_by(|left, right| {
            left.agent_id
                .cmp(&right.agent_id)
                .then_with(|| left.asset.cmp(&right.asset))
        });
        items.dedup();
    };
    sort(&mut inventory.consumptions);
    sort(&mut inventory.external);
}
