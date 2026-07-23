import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelOperation,
  commitOperation,
  getWorkspaceSnapshot,
  planOperation,
} from "../lib/api";
import type {
  AgentConsumptionSelection,
  AgentCapabilityView,
  AssetCommandError,
  AssetOperationPlan,
  AssetRef,
  CentralAssetDraft,
  ConsumptionInventory,
  PlanOperationRequest,
} from "../lib/types";

export interface ConsumptionState {
  agents: AgentCapabilityView[];
  inventory: ConsumptionInventory | null;
  loading: boolean;
  error: AssetCommandError | null;
  plan: AssetOperationPlan | null;
  committing: boolean;
  refresh(): Promise<ConsumptionInventory>;
  planForAgent(
    agentId: string,
    selection: AgentConsumptionSelection,
  ): Promise<AssetOperationPlan>;
  planMcpEnabled(
    agentId: string,
    assetKey: string,
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
  planForAsset(asset: AssetRef, agentIds: string[]): Promise<AssetOperationPlan>;
  planUpdate(draft: CentralAssetDraft): Promise<AssetOperationPlan>;
  planDelete(asset: AssetRef, sourceId?: string): Promise<AssetOperationPlan>;
  commit(conflictConfirmation?: string): Promise<ConsumptionInventory>;
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

export function useConsumptionState(): ConsumptionState {
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

  const refresh = useCallback(async () => {
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
      return next;
    } catch (cause) {
      const nextError = commandError(cause);
      if (mounted.current && ownGeneration === generation.current) setError(nextError);
      throw cause;
    }
  }, []);

  useEffect(() => {
    refresh()
      .catch(() => undefined)
      .finally(() => {
        if (mounted.current) setLoading(false);
      });
  }, [refresh]);

  const ownPlan = useCallback((next: AssetOperationPlan) => {
    planRef.current = next;
    if (!mounted.current) return next;
    setPlan(next);
    setError(null);
    return next;
  }, []);

  const planForAgent = useCallback(
    async (agentId: string, selection: AgentConsumptionSelection) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "set_agent_consumption",
          request: { agent_id: agentId, selection },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const planForAsset = useCallback(
    async (asset: AssetRef, agentIds: string[]) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "set_asset_consumers",
          request: { asset, agent_ids: agentIds },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const planMcpEnabled = useCallback(
    async (agentId: string, assetKey: string, enabled: boolean) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "set_mcp_enabled",
          request: { agent_id: agentId, asset_key: assetKey, enabled },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const planModelEnabled = useCallback(
    async (agentId: string, profileId: string, enabled: boolean) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "set_model_enabled",
          request: { agent_id: agentId, profile_id: profileId, enabled },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const planActiveModel = useCallback(
    async (agentId: string, profileId: string) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "set_active_model",
          request: { agent_id: agentId, profile_id: profileId },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const planUpdate = useCallback(
    async (draft: CentralAssetDraft) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "update_central_asset",
          request: { draft },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const planDelete = useCallback(
    async (asset: AssetRef, sourceId?: string) => {
      if (planRef.current || planningRef.current) throw new Error("已有待确认的资产操作");
      planningRef.current = true;
      try {
        return ownPlan(await planAsset({
          operation: "delete_central_asset",
          request: { asset, source_id: sourceId },
        }));
      } catch (cause) {
        if (mounted.current) setError(commandError(cause));
        throw cause;
      } finally {
        planningRef.current = false;
      }
    },
    [ownPlan],
  );

  const commit = useCallback(async (conflictConfirmation?: string) => {
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
          conflict_confirmation: conflictConfirmation,
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
    planForAgent,
    planMcpEnabled,
    planModelEnabled,
    planActiveModel,
    planForAsset,
    planUpdate,
    planDelete,
    commit,
    cancel,
  };
}
