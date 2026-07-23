//! Cross-domain central asset use cases.

pub use crate::assets::{
    CompatibilityReason, CompatibilityView, McpAdoptionCandidate, McpAdoptionStatus,
    ModelAdoptionCandidate, ModelAdoptionStatus, ModelCredentialKind, PlanMcpAdoptionRequest,
    PlanModelAdoptionRequest,
};
pub use crate::domain::assets::*;

pub fn compatibility_for(agent_id: &str, asset: &AssetRef) -> Result<CompatibilityView, String> {
    super::gate::read(|| crate::assets::compatibility_for(agent_id, asset))
}

/// Return the read-only cross-domain desired/observed projection. Storage
/// migration and Model pointer reconciliation belong to [`super::bootstrap`].
pub fn list_inventory() -> Result<ConsumptionInventory, String> {
    super::gate::read(crate::assets::list_consumption_inventory)
}

pub fn list_mcp_adoption_candidates() -> Result<Vec<McpAdoptionCandidate>, String> {
    super::gate::read(crate::assets::list_mcp_adoption_candidates)
}

pub fn list_model_adoption_candidates() -> Result<Vec<ModelAdoptionCandidate>, String> {
    super::gate::read(crate::assets::list_model_adoption_candidates)
}

pub fn plan_update_central_asset(
    request: PlanUpdateCentralAssetRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_update_central_asset(request))
}

pub fn plan_delete_central_asset(
    request: PlanDeleteCentralAssetRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_delete_central_asset(request))
}

pub fn plan_mcp_adoption(request: PlanMcpAdoptionRequest) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_mcp_adoption(request))
}

pub fn plan_model_adoption(
    request: PlanModelAdoptionRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_model_adoption(request))
}

pub fn plan_model_schema_v2_migration() -> Result<Option<AssetOperationPlan>, String> {
    super::gate::read(crate::assets::plan_model_schema_v2_migration)
}

pub fn plan_set_agent_consumption(
    request: PlanSetAgentConsumptionRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_set_agent_consumption(request))
}

pub fn plan_ensure_agent_consumption(
    request: PlanEnsureAgentConsumptionRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_ensure_agent_consumption(request))
}

pub fn plan_set_asset_consumers(
    request: PlanSetAssetConsumersRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_set_asset_consumers(request))
}

pub fn plan_update_asset_consumers(
    request: PlanUpdateAssetConsumersRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_update_asset_consumers(request))
}

pub fn plan_set_mcp_enabled(
    request: PlanSetMcpEnabledRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_set_mcp_enabled(request))
}

pub fn plan_reapply_mcp(request: PlanReapplyMcpRequest) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_reapply_mcp(request))
}

pub fn plan_set_model_enabled(
    request: PlanSetModelEnabledRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_set_model_enabled(request))
}

pub fn plan_set_active_model(
    request: PlanSetActiveModelRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_set_active_model(request))
}

pub fn plan_update_agent_capabilities(
    request: PlanUpdateAgentCapabilitiesRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_update_agent_capabilities(request))
}

pub fn plan_update_agent_configuration(
    request: PlanUpdateAgentConfigurationRequest,
) -> Result<AssetOperationPlan, String> {
    super::gate::read(|| crate::assets::plan_update_agent_configuration(request))
}

pub fn commit_asset_operation(request: AssetCommitRequest) -> Result<ConsumptionInventory, String> {
    super::gate::write(|| crate::assets::commit_asset_operation(request))
}

pub fn cancel_asset_operation(operation_id: &str) -> Result<(), String> {
    super::gate::write(|| crate::assets::cancel_asset_operation(operation_id))
}

pub fn recover_pending_asset_operations() -> Result<Vec<String>, String> {
    super::gate::write(crate::assets::recover_pending_asset_operations)
}

pub fn migrate_model_profiles_v2_if_needed() -> Result<bool, String> {
    super::gate::write(crate::assets::migrate_model_profiles_v2_if_needed)
}
