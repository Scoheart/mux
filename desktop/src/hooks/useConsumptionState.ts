import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelOperation,
  commitOperation,
  getWorkspaceSnapshot,
  planOperation,
} from "../lib/api";
import i18n from "../i18n";
import type {
  AgentConsumptionSelection,
  AgentCapabilityView,
  AssetCommandError,
  AssetOperationPlan,
  AssetRef,
  CentralAssetDraft,
  ConsumptionInventory,
  ConsumptionView,
  ConvergenceAction,
  PlanOperationRequest,
  UnifiedOperationPlan,
  WorkspaceSnapshot,
} from "../lib/types";

export interface ConsumptionState {
  agents: AgentCapabilityView[];
  inventory: ConsumptionInventory | null;
  loading: boolean;
  error: AssetCommandError | null;
  plan: AssetOperationPlan | null;
  committing: boolean;
  refresh(): Promise<ConsumptionInventory>;
  refreshWorkspace(): Promise<WorkspaceSnapshot>;
  planForAgent(
    agentId: string,
    selection: AgentConsumptionSelection,
  ): Promise<AssetOperationPlan>;
  planAdditionsForAgent(
    agentId: string,
    selection: AgentConsumptionSelection,
  ): Promise<AssetOperationPlan>;
  planMcpEnabled(
    agentId: string,
    assetKey: string,
    enabled: boolean,
  ): Promise<AssetOperationPlan>;
  planSkillEnabled(
    agentId: string,
    name: string,
    enabled: boolean,
  ): Promise<AssetOperationPlan>;
  planModelEnabled(
    agentId: string,
    profileId: string,
    enabled: boolean,
  ): Promise<AssetOperationPlan>;
  planActiveModel(
    agentId: string,
    profileId: string,
  ): Promise<AssetOperationPlan>;
  planConvergence(
    item: ConsumptionView,
    action: ConvergenceAction,
  ): Promise<UnifiedOperationPlan>;
  planForAsset(asset: AssetRef, agentIds: string[]): Promise<AssetOperationPlan>;
  planUpdate(draft: CentralAssetDraft): Promise<AssetOperationPlan>;
  planDelete(asset: AssetRef, sourceId?: string): Promise<AssetOperationPlan>;
  commit(): Promise<ConsumptionInventory>;
  cancel(): Promise<void>;
}

function commandError(error: unknown): AssetCommandError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error
  ) {
    return {
      code: String(error.code),
      message: String(error.message),
    };
  }
  return { code: "asset_operation_failed", message: String(error) };
}

async function planAsset(request: PlanOperationRequest): Promise<AssetOperationPlan> {
  const result = await planOperation(request);
  if (result.domain !== "asset") {
    throw new Error("Core returned a Skill plan for an asset request");
  }
  return result.plan;
}

export function useConsumptionState({ autoLoad = true }: { autoLoad?: boolean } = {}): ConsumptionState {
  const [agents, setAgents] = useState<AgentCapabilityView[]>([]);
  const [inventory, setInventory] = useState<ConsumptionInventory | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AssetCommandError | null>(null);
  const [plan, setPlan] = useState<AssetOperationPlan | null>(null);
  const [committing, setCommitting] = useState(false);
  const generation = useRef(0);
  const mounted = useRef(true);
  const planRef = useRef(plan);
  const planningRef = useRef(false);
  const committingRef = useRef(false);
  planRef.current = plan;

  useEffect(() => () => {
    mounted.current = false;
  }, []);

  const refreshWorkspace = useCallback(async () => {
    const ownGeneration = ++generation.current;
    try {
      const snapshot = await getWorkspaceSnapshot();
      const next = snapshot.relationships;
      if (mounted.current && ownGeneration === generation.current) {
        setAgents(snapshot.agents);
        setInventory(next);
        setError(next.recovery_error
          ? { code: "recovery_required", message: next.recovery_error }
          : null);
      }
      return snapshot;
    } catch (cause) {
      const nextError = commandError(cause);
      if (mounted.current && ownGeneration === generation.current) setError(nextError);
      throw cause;
    } finally {
      if (mounted.current && ownGeneration === generation.current) setLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    const snapshot = await refreshWorkspace();
    return snapshot.relationships;
  }, [refreshWorkspace]);

  useEffect(() => {
    if (!autoLoad) return;
    refresh().catch(() => undefined);
  }, [autoLoad, refresh]);

  const ownPlan = useCallback((next: AssetOperationPlan) => {
    planRef.current = next;
    if (!mounted.current) return next;
    setPlan(next);
    setError(null);
    return next;
  }, []);

  const startPlan = useCallback(async (request: PlanOperationRequest) => {
    if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
    planningRef.current = true;
    try {
      return ownPlan(await planAsset(request));
    } catch (cause) {
      if (mounted.current) setError(commandError(cause));
      throw cause;
    } finally {
      planningRef.current = false;
    }
  }, [ownPlan]);

  const planForAgent = useCallback(
    (agentId: string, selection: AgentConsumptionSelection) => startPlan({
      operation: "set_agent_consumption",
      request: { agent_id: agentId, selection },
    }),
    [startPlan],
  );

  const planAdditionsForAgent = useCallback(
    (agentId: string, selection: AgentConsumptionSelection) => startPlan({
      operation: "ensure_agent_consumption",
      request: { agent_id: agentId, selection },
    }),
    [startPlan],
  );

  const planForAsset = useCallback(
    (asset: AssetRef, agentIds: string[]) => startPlan({
      operation: "set_asset_consumers",
      request: { asset, agent_ids: agentIds },
    }),
    [startPlan],
  );

  const planMcpEnabled = useCallback(
    (agentId: string, assetKey: string, enabled: boolean) => startPlan({
      operation: "set_mcp_enabled",
      request: { agent_id: agentId, asset_key: assetKey, enabled },
    }),
    [startPlan],
  );

  const planSkillEnabled = useCallback(
    (agentId: string, name: string, enabled: boolean) => startPlan({
      operation: "set_skill_enabled",
      request: { agent_id: agentId, name, enabled },
    }),
    [startPlan],
  );

  const planModelEnabled = useCallback(
    (agentId: string, profileId: string, enabled: boolean) => startPlan({
      operation: "set_model_enabled",
      request: { agent_id: agentId, profile_id: profileId, enabled },
    }),
    [startPlan],
  );

  const planActiveModel = useCallback(
    (agentId: string, profileId: string) => startPlan({
      operation: "set_active_model",
      request: { agent_id: agentId, profile_id: profileId },
    }),
    [startPlan],
  );

  const planConvergence = useCallback(async (
    item: ConsumptionView,
    action: ConvergenceAction,
  ) => {
    if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
    const observedRevision = inventory?.revision;
    if (!observedRevision) throw new Error(i18n.t("observations.revisionUnavailable"));
    planningRef.current = true;
    try {
      const result = await planOperation({
        operation: "converge_consumption",
        request: {
          agent_id: item.agent_id,
          asset: item.asset,
          action,
          observed_revision: observedRevision,
        },
      });
      if (result.domain === "asset") ownPlan(result.plan);
      return result;
    } catch (cause) {
      if (mounted.current) setError(commandError(cause));
      throw cause;
    } finally {
      planningRef.current = false;
    }
  }, [inventory?.revision, ownPlan]);

  const planUpdate = useCallback(
    (draft: CentralAssetDraft) => startPlan({
      operation: "update_central_asset",
      request: { draft },
    }),
    [startPlan],
  );

  const planDelete = useCallback(
    (asset: AssetRef, sourceId?: string) => startPlan({
      operation: "delete_central_asset",
      request: { asset, source_id: sourceId },
    }),
    [startPlan],
  );

  const commit = useCallback(async () => {
    const active = planRef.current;
    if (!active || committingRef.current) throw new Error("没有可提交的资产操作");
    committingRef.current = true;
    setCommitting(true);
    try {
      const committed = await commitOperation({
        domain: "asset",
        request: {
          operation_id: active.operation_id,
          candidate_hash: active.candidate_hash,
        },
      });
      if (committed.domain !== "asset") {
        throw new Error("Core returned a Skill inventory for an asset commit");
      }
      const next = committed.inventory;
      ++generation.current;
      if (mounted.current && planRef.current?.operation_id === active.operation_id) {
        setInventory(next);
        planRef.current = null;
        setPlan(null);
        setError(next.recovery_error
          ? { code: "recovery_required", message: next.recovery_error }
          : null);
      }
      return next;
    } catch (cause) {
      if (mounted.current) setError(commandError(cause));
      throw cause;
    } finally {
      committingRef.current = false;
      if (mounted.current) setCommitting(false);
    }
  }, []);

  const cancel = useCallback(async () => {
    const active = planRef.current;
    if (!active || committingRef.current) return;
    await cancelOperation({ domain: "asset", operation_id: active.operation_id });
    if (mounted.current && planRef.current?.operation_id === active.operation_id) {
      planRef.current = null;
      setPlan(null);
    }
  }, []);

  return {
    agents,
    inventory,
    loading,
    error,
    plan,
    committing,
    refresh,
    refreshWorkspace,
    planForAgent,
    planAdditionsForAgent,
    planMcpEnabled,
    planSkillEnabled,
    planModelEnabled,
    planActiveModel,
    planConvergence,
    planForAsset,
    planUpdate,
    planDelete,
    commit,
    cancel,
  };
}
