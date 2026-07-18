//! Central asset consumption contracts.
//!
//! A central MCP, Model, or Skill owns its lifecycle independently. This
//! module projects the desired relationship and observed Agent state without
//! flattening the domain-specific asset formats into an untyped payload.

pub mod compatibility;
pub mod inventory;
pub mod lifecycle;
pub mod migration;
pub mod planner;
pub mod transaction;
pub mod types;

pub use compatibility::{compatibility_for, CompatibilityReason, CompatibilityView};
pub use inventory::list_consumption_inventory;
pub use lifecycle::{plan_delete_central_asset, plan_update_central_asset};
pub use migration::{list_mcp_adoption_candidates, McpAdoptionCandidate, McpAdoptionStatus};
pub use planner::{plan_set_agent_consumption, plan_set_asset_consumers};
pub use transaction::{
    cancel_asset_operation, commit_asset_operation, recover_pending_asset_operations,
};
pub use types::{
    AgentConsumptionSelection, AssetCommitRequest, AssetOperationKind, AssetOperationPlan,
    AssetRef, CentralAssetAction, CentralAssetChange, CentralAssetDraft, ConsumptionInventory,
    ConsumptionStatus, ConsumptionView, DomainPlan, McpConsumptionRecord,
    PlanDeleteCentralAssetRequest, PlanSetAgentConsumptionRequest, PlanSetAssetConsumersRequest,
    PlanUpdateCentralAssetRequest, RelationshipAction, RelationshipChange, SelectionError,
};
