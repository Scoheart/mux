//! One plan/commit/cancel entry point for every resource domain.

use crate::assets::{PlanMcpAdoptionRequest, PlanModelAdoptionRequest};
use crate::domain::assets::{
    AssetCommitRequest, AssetOperationPlan, ConsumptionInventory, PlanDeleteCentralAssetRequest,
    PlanEnsureAgentConsumptionRequest, PlanReapplyMcpRequest, PlanSetActiveModelRequest,
    PlanSetAgentConsumptionRequest, PlanSetAssetConsumersRequest, PlanSetMcpEnabledRequest,
    PlanSetModelEnabledRequest, PlanUpdateAgentCapabilitiesRequest,
    PlanUpdateAgentConfigurationRequest, PlanUpdateAssetConsumersRequest,
    PlanUpdateCentralAssetRequest,
};
use crate::domain::error::CoreResult;
use crate::resources::skill::{
    OperationPlan as SkillOperationPlan, PlanAssignmentRequest, PlanImportRequest,
    PlanRemoveRequest, PlanRepairRequest, PlanSkillAssetImportRequest,
    PlanSkillAssetInstallRequest, PlanUpdateRequest, SkillCommitRequest, SkillOperationKind,
    SkillsInventory,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "request", rename_all = "snake_case")]
pub enum PlanOperationRequest {
    UpdateCentralAsset(PlanUpdateCentralAssetRequest),
    DeleteCentralAsset(PlanDeleteCentralAssetRequest),
    SetAgentConsumption(PlanSetAgentConsumptionRequest),
    EnsureAgentConsumption(PlanEnsureAgentConsumptionRequest),
    SetAssetConsumers(PlanSetAssetConsumersRequest),
    UpdateAssetConsumers(PlanUpdateAssetConsumersRequest),
    SetMcpEnabled(PlanSetMcpEnabledRequest),
    ReapplyMcp(PlanReapplyMcpRequest),
    SetModelEnabled(PlanSetModelEnabledRequest),
    SetActiveModel(PlanSetActiveModelRequest),
    UpdateAgentCapabilities(PlanUpdateAgentCapabilitiesRequest),
    /// Legacy full-form request retained for existing Desktop clients.
    UpdateAgentConfiguration(PlanUpdateAgentConfigurationRequest),
    AdoptMcp(PlanMcpAdoptionRequest),
    AdoptModel(PlanModelAdoptionRequest),
    AdoptSkill(PlanImportRequest),
    InstallSkill(PlanSkillAssetInstallRequest),
    ImportSkill(PlanSkillAssetImportRequest),
    AssignSkill(PlanAssignmentRequest),
    UpdateSkill(PlanUpdateRequest),
    RemoveSkill(PlanRemoveRequest),
    RepairSkill(PlanRepairRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum OperationPlan {
    Asset { plan: Box<AssetOperationPlan> },
    Skill { plan: SkillOperationPlan },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum CommitOperationRequest {
    Asset {
        request: AssetCommitRequest,
    },
    Skill {
        kind: SkillOperationKind,
        request: SkillCommitRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum CancelOperationRequest {
    Asset { operation_id: String },
    Skill { operation_id: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum OperationCommitResult {
    Asset { inventory: ConsumptionInventory },
    Skill { inventory: SkillsInventory },
}

pub fn plan(request: PlanOperationRequest) -> CoreResult<OperationPlan> {
    use PlanOperationRequest::*;

    let plan = match request {
        UpdateCentralAsset(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_update_central_asset(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        DeleteCentralAsset(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_delete_central_asset(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        SetAgentConsumption(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_set_agent_consumption(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        EnsureAgentConsumption(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_ensure_agent_consumption(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        SetAssetConsumers(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_set_asset_consumers(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        UpdateAssetConsumers(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_update_asset_consumers(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        SetMcpEnabled(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_set_mcp_enabled(request).map_err(super::error::from_legacy)?,
            ),
        },
        ReapplyMcp(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_reapply_mcp(request).map_err(super::error::from_legacy)?,
            ),
        },
        SetModelEnabled(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_set_model_enabled(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        SetActiveModel(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_set_active_model(request).map_err(super::error::from_legacy)?,
            ),
        },
        UpdateAgentCapabilities(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_update_agent_capabilities(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        UpdateAgentConfiguration(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_update_agent_configuration(request)
                    .map_err(super::error::from_legacy)?,
            ),
        },
        AdoptMcp(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_mcp_adoption(request).map_err(super::error::from_legacy)?,
            ),
        },
        AdoptModel(request) => OperationPlan::Asset {
            plan: Box::new(
                super::assets::plan_model_adoption(request).map_err(super::error::from_legacy)?,
            ),
        },
        AdoptSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_import(request).map_err(super::error::from_skill)?,
        },
        InstallSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_asset_install(request).map_err(super::error::from_skill)?,
        },
        ImportSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_asset_import(request).map_err(super::error::from_skill)?,
        },
        AssignSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_assignment(request).map_err(super::error::from_skill)?,
        },
        UpdateSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_update(request).map_err(super::error::from_skill)?,
        },
        RemoveSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_remove(request).map_err(super::error::from_skill)?,
        },
        RepairSkill(request) => OperationPlan::Skill {
            plan: super::skills::plan_repair(request).map_err(super::error::from_skill)?,
        },
    };
    Ok(plan)
}

pub fn commit(request: CommitOperationRequest) -> CoreResult<OperationCommitResult> {
    match request {
        CommitOperationRequest::Asset { request } => {
            let inventory = super::assets::commit_asset_operation(request)
                .map_err(super::error::from_legacy)?;
            Ok(OperationCommitResult::Asset { inventory })
        }
        CommitOperationRequest::Skill { kind, request } => {
            let result = match kind {
                SkillOperationKind::Install => super::skills::commit_install(request),
                SkillOperationKind::Import => super::skills::commit_import(request),
                SkillOperationKind::Update => super::skills::commit_update(request),
                SkillOperationKind::Remove => super::skills::commit_remove(request),
                SkillOperationKind::Assignment => super::skills::commit_assignment(request),
                SkillOperationKind::Repair => super::skills::commit_repair(request),
            };
            Ok(OperationCommitResult::Skill {
                inventory: result.map_err(super::error::from_skill)?,
            })
        }
    }
}

pub fn cancel(request: CancelOperationRequest) -> CoreResult<()> {
    match request {
        CancelOperationRequest::Asset { operation_id } => {
            super::assets::cancel_asset_operation(&operation_id).map_err(super::error::from_legacy)
        }
        CancelOperationRequest::Skill { operation_id } => {
            super::skills::cancel_operation(&operation_id).map_err(super::error::from_skill)
        }
    }
}
