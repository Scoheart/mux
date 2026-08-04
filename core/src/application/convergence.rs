//! Unified desired/observed convergence for one Agent/asset relationship.

use super::operations::OperationPlan;
use crate::assets::{McpAdoptionStatus, ModelAdoptionStatus};
use crate::domain::assets::{
    AgentConsumptionSelection, AssetRef, ConvergenceAction, McpReapplyScope, OwnershipState,
    PlanConvergeConsumptionRequest, PlanReapplyMcpRequest, PlanReapplyModelRequest,
    PlanReapplySkillRequest, PlanRemoveAgentConsumptionRequest, PlanSetActiveModelRequest,
};
use crate::domain::error::{CoreError, CoreResult};
use crate::resources::skill::PlanImportRequest;
use std::collections::BTreeMap;

pub fn plan(request: PlanConvergeConsumptionRequest) -> CoreResult<OperationPlan> {
    let inventory = super::assets::list_inventory().map_err(super::error::from_legacy)?;
    if inventory.revision != request.observed_revision {
        return Err(CoreError::new(
            "observation_stale",
            "Agent configuration changed after this state was observed; refresh and review it again",
        )
        .with_detail("current_revision", inventory.revision));
    }
    let row = inventory
        .consumptions
        .iter()
        .chain(inventory.external.iter())
        .find(|row| row.agent_id == request.agent_id && row.asset == request.asset)
        .ok_or_else(|| {
            CoreError::new(
                "observation_stale",
                "the selected Agent asset observation no longer exists",
            )
        })?;
    if !row.available_actions.contains(&request.action) {
        return Err(CoreError::new(
            "convergence_action_unavailable",
            "the selected action is not safe for the current observation",
        ));
    }

    let plan = match request.action {
        ConvergenceAction::Detach => detach(&request.agent_id, &request.asset),
        ConvergenceAction::RestoreDesired => restore(&request.agent_id, &request.asset),
        ConvergenceAction::AdoptObserved => {
            adopt(&request.agent_id, &request.asset, row, &inventory)
        }
    }?;

    // Planning can stage private transaction material but must not weaken the
    // observation the user reviewed. Re-scan after the domain planner finishes
    // and discard the staged operation if any Agent input changed in between.
    let current = super::assets::list_inventory().map_err(super::error::from_legacy)?;
    if current.revision != request.observed_revision {
        cancel_staged(&plan);
        return Err(CoreError::new(
            "observation_stale",
            "Agent configuration changed while the convergence plan was being prepared; refresh and review it again",
        )
        .with_detail("current_revision", current.revision));
    }
    Ok(plan)
}

fn cancel_staged(plan: &OperationPlan) {
    match plan {
        OperationPlan::Asset { plan } => {
            let _ = super::assets::cancel_asset_operation(&plan.operation_id);
        }
        OperationPlan::Skill { plan } => {
            let _ = super::skills::cancel_operation(&plan.operation_id);
        }
    }
}

fn detach(agent_id: &str, asset: &AssetRef) -> CoreResult<OperationPlan> {
    let selection = selection_for(asset);
    let plan = super::assets::plan_remove_agent_consumption(PlanRemoveAgentConsumptionRequest {
        agent_id: agent_id.to_string(),
        selection,
    })
    .map_err(super::error::from_legacy)?;
    Ok(OperationPlan::Asset {
        plan: Box::new(plan),
    })
}

fn restore(agent_id: &str, asset: &AssetRef) -> CoreResult<OperationPlan> {
    let plan = match asset {
        AssetRef::Mcp { key } => super::assets::plan_reapply_mcp(PlanReapplyMcpRequest {
            asset_key: key.clone(),
            scope: McpReapplyScope::Agent {
                agent_id: agent_id.to_string(),
            },
        }),
        AssetRef::Model { profile_id } => {
            super::assets::plan_reapply_model(PlanReapplyModelRequest {
                agent_id: agent_id.to_string(),
                profile_id: profile_id.clone(),
            })
        }
        AssetRef::Skill { name } => super::assets::plan_reapply_skill(PlanReapplySkillRequest {
            agent_id: agent_id.to_string(),
            name: name.clone(),
        }),
        AssetRef::ModelProvider { .. } => Err(
            "convergence_action_unavailable: Model Provider is not an Agent relationship".into(),
        ),
    }
    .map_err(super::error::from_legacy)?;
    Ok(OperationPlan::Asset {
        plan: Box::new(plan),
    })
}

fn adopt(
    agent_id: &str,
    asset: &AssetRef,
    row: &crate::domain::assets::ConsumptionView,
    inventory: &crate::domain::assets::ConsumptionInventory,
) -> CoreResult<OperationPlan> {
    match asset {
        AssetRef::Mcp { key } => {
            let candidate = super::assets::list_mcp_adoption_candidates()
                .map_err(super::error::from_legacy)?
                .into_iter()
                .find(|candidate| {
                    candidate.agent_id == agent_id
                        && candidate.asset_key == *key
                        && matches!(
                            candidate.status,
                            McpAdoptionStatus::ExternalAdded | McpAdoptionStatus::ExternalChanged
                        )
                })
                .ok_or_else(|| {
                    CoreError::new(
                        "observation_stale",
                        "the MCP observation is no longer adoptable",
                    )
                })?;
            let plan = super::assets::plan_mcp_adoption(crate::assets::PlanMcpAdoptionRequest {
                asset_key: key.clone(),
                agent_id: agent_id.to_string(),
                candidate_fingerprint: candidate.fingerprint,
            })
            .map_err(super::error::from_legacy)?;
            Ok(OperationPlan::Asset {
                plan: Box::new(plan),
            })
        }
        AssetRef::Model { profile_id } => {
            if row.ownership == OwnershipState::Managed
                && row.reason.as_deref() == Some("model_active_state_drift")
            {
                let observed_active = inventory.consumptions.iter().find_map(|candidate| {
                    if candidate.agent_id == agent_id && candidate.active == Some(true) {
                        if let AssetRef::Model { profile_id } = &candidate.asset {
                            return Some(profile_id.clone());
                        }
                    }
                    None
                });
                if let Some(observed_active) = observed_active {
                    let plan = super::assets::plan_set_active_model(PlanSetActiveModelRequest {
                        agent_id: agent_id.to_string(),
                        profile_id: observed_active,
                    })
                    .map_err(super::error::from_legacy)?;
                    return Ok(OperationPlan::Asset {
                        plan: Box::new(plan),
                    });
                }
            }
            let candidate = super::assets::list_model_adoption_candidates()
                .map_err(super::error::from_legacy)?
                .into_iter()
                .filter(|candidate| candidate.agent_id == agent_id)
                .find(|candidate| {
                    candidate.status == ModelAdoptionStatus::Adoptable
                        && (candidate.managed_profile_id.as_deref() == Some(profile_id)
                            || (row.ownership == OwnershipState::External
                                && profile_id
                                    .strip_prefix("external-")
                                    .is_some_and(|id| id == candidate.candidate_id)))
                })
                .ok_or_else(|| {
                    CoreError::new(
                        "convergence_action_unavailable",
                        "this Model observation needs credential or identity repair before it can be adopted",
                    )
                })?;
            let plan =
                super::assets::plan_model_adoption(crate::assets::PlanModelAdoptionRequest {
                    candidate_fingerprints: BTreeMap::from([(
                        candidate.candidate_id,
                        candidate.fingerprint,
                    )]),
                })
                .map_err(super::error::from_legacy)?;
            Ok(OperationPlan::Asset {
                plan: Box::new(plan),
            })
        }
        AssetRef::Skill { name: _ } if row.ownership == OwnershipState::External => {
            let identity = row.observation_id.clone().ok_or_else(|| {
                CoreError::new(
                    "observation_stale",
                    "the Skill observation identity is missing",
                )
            })?;
            let plan = super::skills::plan_import(PlanImportRequest {
                identity,
                agent_ids: row.affected_agent_ids.clone(),
                replace_conflicts: false,
            })
            .map_err(super::error::from_skill)?;
            Ok(OperationPlan::Skill { plan })
        }
        AssetRef::Skill { name } => {
            let plan = super::assets::plan_adopt_observed_skill(agent_id, name)
                .map_err(super::error::from_legacy)?;
            Ok(OperationPlan::Asset {
                plan: Box::new(plan),
            })
        }
        AssetRef::ModelProvider { .. } => Err(CoreError::new(
            "convergence_action_unavailable",
            "Model Provider is not an Agent relationship",
        )),
    }
}

fn selection_for(asset: &AssetRef) -> AgentConsumptionSelection {
    match asset {
        AssetRef::Mcp { key } => AgentConsumptionSelection::Mcp {
            asset_keys: vec![key.clone()],
        },
        AssetRef::Model { profile_id } => AgentConsumptionSelection::Model {
            profile_ids: vec![profile_id.clone()],
        },
        AssetRef::Skill { name } => AgentConsumptionSelection::Skill {
            names: vec![name.clone()],
        },
        AssetRef::ModelProvider { provider_id } => AgentConsumptionSelection::Model {
            profile_ids: vec![provider_id.clone()],
        },
    }
}
