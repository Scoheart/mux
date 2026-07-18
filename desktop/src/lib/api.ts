import { invoke } from "@tauri-apps/api/core";
import type {
  AgentDefinitionInput,
  AgentConsumptionSelection,
  AgentInfo,
  AssetCommandError,
  AssetOperationPlan,
  AssetRef,
  CentralAssetDraft,
  CatalogItem,
  ConsumptionInventory,
  InstalledMcp,
  ModelAgentView,
  ModelProfileView,
  OperationPlan,
  PlanRemoveRequest,
  PlanRepairRequest,
  PlanSkillAssetImportRequest,
  PlanSkillAssetInstallRequest,
  PlanUpdateRequest,
  RegistryEntry,
  SkillAgentView,
  SkillCommitRequest,
  SkillDetail,
  SkillSourceResolution,
  SkillsInventory,
  SourceView,
  UpdateCheckOutcome,
} from "./types";

export const listConsumptionInventory = () =>
  invoke<ConsumptionInventory>("list_consumption_inventory");
export const planSetAgentConsumption = (
  agentId: string,
  selection: AgentConsumptionSelection,
) =>
  invoke<AssetOperationPlan>("plan_set_agent_consumption", {
    request: { agent_id: agentId, selection },
  });
export const planSetAssetConsumers = (asset: AssetRef, agentIds: string[]) =>
  invoke<AssetOperationPlan>("plan_set_asset_consumers", {
    request: { asset, agent_ids: agentIds },
  });
export const planUpdateCentralAsset = (draft: CentralAssetDraft) =>
  invoke<AssetOperationPlan>("plan_update_central_asset", {
    request: { draft },
  });
export const planDeleteCentralAsset = (asset: AssetRef, sourceId?: string) =>
  invoke<AssetOperationPlan>("plan_delete_central_asset", {
    request: { asset, source_id: sourceId ?? null },
  });
export const commitAssetOperation = (
  plan: Pick<AssetOperationPlan, "operation_id" | "candidate_hash">,
  conflictConfirmation?: string,
) =>
  invoke<ConsumptionInventory>("commit_asset_operation", {
    request: {
      operation_id: plan.operation_id,
      candidate_hash: plan.candidate_hash,
      conflict_confirmation: conflictConfirmation ?? null,
    },
  });
export const cancelAssetOperation = (operationId: string) =>
  invoke<void>("cancel_asset_operation", { operationId });

export type { AssetCommandError };

export const listRegistry = () => invoke<RegistryEntry[]>("list_registry");
export const listModelProfiles = () =>
  invoke<ModelProfileView[]>("list_model_profiles");
export const listModelAgents = () =>
  invoke<ModelAgentView[]>("list_model_agents");
/** All entry copies across sources (not deduped), each flagged in_effect. */
export const listRegistryAll = () => invoke<CatalogItem[]>("list_registry_all");
/** 桌面包内 mux CLI 的安装状态（sidecar → ~/.local/bin 软链）。 */
export type CliStatus = { bundled: boolean; installed: boolean; link_path: string; in_path: boolean };
export const cliStatus = () => invoke<CliStatus>("cli_status");
export const installCli = () => invoke<CliStatus>("install_cli");
export type UpdateEnvironment = {
  canSelfUpdate: boolean;
  reason: "disk-image" | "app-translocation" | "read-only-volume" | null;
};
export const updateEnvironment = () =>
  invoke<UpdateEnvironment>("update_environment");
export const listCustomRegistryKeys = () =>
  invoke<string[]>("list_custom_registry_keys");
export const listAgents = () => invoke<AgentInfo[]>("list_agents");
export const getPinnedAgents = () =>
  invoke<string[]>("get_pinned_agents");
export const setPinnedAgents = (agentIds: string[]) =>
  invoke<string[]>("set_pinned_agents", { agentIds });
export const addAgent = (id: string, def: AgentDefinitionInput) =>
  invoke<void>("add_agent", { id, def });
export const updateAgent = (id: string, def: AgentDefinitionInput) =>
  invoke<void>("update_agent", { id, def });
export const scanInstalled = () => invoke<InstalledMcp[]>("scan_installed");
/** Parse a pasted config blob (JSON/TOML) and add its servers to the manual
 *  source. Returns the added server names. */
export const importPastedConfig = (text: string) =>
  invoke<string[]>("import_pasted_config", { text });

// ── Catalog sources (subscribe remote / add local) ────────────────────────
export const listSources = () => invoke<SourceView[]>("list_sources");
/** Subscribe to a remote config URL: fetch + cache + register as a source. */
export const subscribeSource = (url: string, name?: string) =>
  invoke<SourceView>("subscribe_source", { url, name: name ?? null });
/** Open a native file picker and add the chosen file as a local source.
 *  Resolves to null if the user cancels. */
export const addLocalSourceDialog = () =>
  invoke<SourceView | null>("add_local_source_dialog");
/** Export the complete effective catalog to JSON via a native save dialog.
 *  Resolves to the written path, or null if the user cancels. */
export const exportEffectiveDialog = () =>
  invoke<string | null>("export_effective_dialog");
/** Re-fetch (remote) / re-copy (local) a source's file. */
export const refreshSource = (id: string) =>
  invoke<SourceView>("refresh_source", { id });
export const setSourceEnabled = (id: string, enabled: boolean) =>
  invoke<void>("set_source_enabled", { id, enabled });
export const removeSource = (id: string) =>
  invoke<void>("remove_source", { id });

/** Stable key for an (server, agent) cell in the matrix. `serverKey` is the
 *  composite registry key (`name::transport`), so stdio and http variants of the
 *  same server get distinct cells. */
export const cellKey = (serverKey: string, agentId: string) =>
  `${serverKey}|${agentId}`;

export const listSkillsInventory = () =>
  invoke<SkillsInventory>("list_skills_inventory");
export const listSkillAgents = () =>
  invoke<SkillAgentView[]>("list_skill_agents");
export const getSkillDetail = (identity: string) =>
  invoke<SkillDetail>("get_skill_detail", { identity });
export const resolveGithubSkillSource = (value: string) =>
  invoke<SkillSourceResolution>("resolve_skill_source", { value });
export const resolveLocalSkillSourceDialog = () =>
  invoke<SkillSourceResolution | null>("resolve_local_skill_source_dialog");
export const planSkillAssetInstall = (request: PlanSkillAssetInstallRequest) =>
  invoke<OperationPlan>("plan_skill_asset_install", { request });
export const commitSkillInstall = (request: SkillCommitRequest) =>
  invoke<SkillsInventory>("commit_skill_install", { request });
export const planSkillAssetImport = (request: PlanSkillAssetImportRequest) =>
  invoke<OperationPlan>("plan_skill_asset_import", { request });
export const commitSkillImport = (request: SkillCommitRequest) =>
  invoke<SkillsInventory>("commit_skill_import", { request });
export const planSkillUpdate = (request: PlanUpdateRequest) =>
  invoke<OperationPlan>("plan_skill_update", { request });
export const commitSkillUpdate = (request: SkillCommitRequest) =>
  invoke<SkillsInventory>("commit_skill_update", { request });
export const planSkillRemove = (request: PlanRemoveRequest) =>
  invoke<OperationPlan>("plan_skill_remove", { request });
export const commitSkillRemove = (request: SkillCommitRequest) =>
  invoke<SkillsInventory>("commit_skill_remove", { request });
export const commitSkillAssignment = (request: SkillCommitRequest) =>
  invoke<SkillsInventory>("commit_skill_assignment", { request });
export const planSkillRepair = (request: PlanRepairRequest) =>
  invoke<OperationPlan>("plan_skill_repair", { request });
export const commitSkillRepair = (request: SkillCommitRequest) =>
  invoke<SkillsInventory>("commit_skill_repair", { request });
export const checkSkillUpdates = (manual: boolean) =>
  invoke<UpdateCheckOutcome>("check_skill_updates", { manual });
export const cancelSkillOperation = (operationId: string) =>
  invoke<void>("cancel_skill_operation", { operationId });
