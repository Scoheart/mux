//! Central asset consumption contracts.
//!
//! A central MCP, Model, or Skill owns its lifecycle independently. This
//! module projects the desired relationship and observed Agent state without
//! flattening the domain-specific asset formats into an untyped payload.

pub mod compatibility;
pub mod inventory;
pub mod lifecycle;
pub mod migration;
pub mod model_migration;
pub mod planner;
pub mod transaction;
pub mod types;

pub use compatibility::{compatibility_for, CompatibilityReason, CompatibilityView};
pub use inventory::list_consumption_inventory;
pub use lifecycle::{
    migrate_model_profiles_v2_if_needed, plan_delete_central_asset, plan_model_schema_v2_migration,
    plan_update_central_asset,
};
pub use migration::{
    list_mcp_adoption_candidates, plan_mcp_adoption, McpAdoptionCandidate, McpAdoptionStatus,
    PlanMcpAdoptionRequest,
};
pub use model_migration::{
    list_model_adoption_candidates, plan_model_adoption, ModelAdoptionCandidate,
    ModelAdoptionStatus, ModelCredentialKind, PlanModelAdoptionRequest,
};
pub use planner::{
    plan_set_active_model, plan_set_agent_consumption, plan_set_asset_consumers,
    plan_set_mcp_enabled, plan_set_model_enabled, plan_update_agent_configuration,
};
pub use transaction::{
    cancel_asset_operation, commit_asset_operation, recover_pending_asset_operations,
};
pub use types::{
    AgentConsumptionSelection, AssetCommitRequest, AssetOperationKind, AssetOperationPlan,
    AssetRef, CentralAssetAction, CentralAssetChange, CentralAssetDraft, ConsumptionInventory,
    ConsumptionStatus, ConsumptionTarget, ConsumptionView, DomainPlan, McpConsumptionRecord,
    ModelAgentSelection, ModelConsumptionRecord, ModelStateChange, ModelStateSnapshot,
    PlanDeleteCentralAssetRequest, PlanSetActiveModelRequest, PlanSetAgentConsumptionRequest,
    PlanSetAssetConsumersRequest, PlanSetMcpEnabledRequest, PlanSetModelEnabledRequest,
    PlanUpdateAgentConfigurationRequest, PlanUpdateCentralAssetRequest, RelationshipAction,
    RelationshipChange, SelectionError,
};
