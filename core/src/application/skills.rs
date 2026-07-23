//! Managed Skill use cases.

pub use crate::resources::skill::{
    GithubEndpoints, OperationPlan, PlanAssignmentRequest, PlanImportRequest, PlanRemoveRequest,
    PlanRepairRequest, PlanSkillAssetImportRequest, PlanSkillAssetInstallRequest,
    PlanUpdateRequest, SkillAgentView, SkillCommitRequest, SkillDetail, SkillError,
    SkillInventoryItem, SkillLocation, SkillOperationKind, SkillSourceInput, SkillSourceResolution,
    SkillsInventory, UpdateCheckOutcome,
};

pub fn list_inventory() -> Result<SkillsInventory, SkillError> {
    super::gate::read(crate::resources::skill::list_inventory)
}

pub fn list_migration_candidates() -> Result<Vec<SkillInventoryItem>, SkillError> {
    super::gate::read(crate::resources::skill::list_migration_candidates)
}

pub fn list_skill_agents() -> Result<Vec<SkillAgentView>, SkillError> {
    super::gate::read(crate::resources::skill::list_skill_agents)
}

pub fn get_skill_detail(identity: &str) -> Result<SkillDetail, SkillError> {
    super::gate::read(|| crate::resources::skill::get_skill_detail(identity))
}

pub fn resolve_source(
    input: SkillSourceInput,
    endpoints: GithubEndpoints,
) -> Result<SkillSourceResolution, SkillError> {
    super::gate::read(|| crate::resources::skill::resolve_source(input, endpoints))
}

pub fn plan_asset_install(
    request: PlanSkillAssetInstallRequest,
) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_asset_install(request))
}

pub fn plan_import(request: PlanImportRequest) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_import(request))
}

pub fn plan_asset_import(
    request: PlanSkillAssetImportRequest,
) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_asset_import(request))
}

pub fn plan_assignment(request: PlanAssignmentRequest) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_assignment(request))
}

pub fn plan_update(request: PlanUpdateRequest) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_update(request))
}

pub fn plan_remove(request: PlanRemoveRequest) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_remove(request))
}

pub fn plan_repair(request: PlanRepairRequest) -> Result<OperationPlan, SkillError> {
    super::gate::read(|| crate::resources::skill::plan_repair(request))
}

pub fn commit_install(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    super::gate::write(|| crate::resources::skill::commit_install(request))
}

pub fn commit_import(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    super::gate::write(|| crate::resources::skill::commit_import(request))
}

pub fn commit_assignment(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    super::gate::write(|| crate::resources::skill::commit_assignment(request))
}

pub fn commit_update(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    super::gate::write(|| crate::resources::skill::commit_update(request))
}

pub fn commit_remove(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    super::gate::write(|| crate::resources::skill::commit_remove(request))
}

pub fn commit_repair(request: SkillCommitRequest) -> Result<SkillsInventory, SkillError> {
    super::gate::write(|| crate::resources::skill::commit_repair(request))
}

pub fn cancel_operation(operation_id: &str) -> Result<(), SkillError> {
    super::gate::write(|| crate::resources::skill::cancel_operation(operation_id))
}

pub fn check_updates(manual: bool) -> Result<UpdateCheckOutcome, SkillError> {
    super::gate::write(|| crate::resources::skill::check_updates(manual))
}

pub fn check_updates_if_due() -> Result<UpdateCheckOutcome, SkillError> {
    super::gate::write(crate::resources::skill::check_updates_if_due)
}
