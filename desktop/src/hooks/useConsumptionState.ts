import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelAssetOperation,
  commitAssetOperation,
  listConsumptionInventory,
  planDeleteCentralAsset,
  planSetAgentConsumption,
  planSetAssetConsumers,
  planUpdateCentralAsset,
} from "../lib/api";
import type {
  AgentConsumptionSelection,
  AssetCommandError,
  AssetOperationPlan,
  AssetRef,
  CentralAssetDraft,
  ConsumptionInventory,
} from "../lib/types";

export interface ConsumptionState {
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

export function useConsumptionState(): ConsumptionState {
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
      const next = await listConsumptionInventory();
      if (mounted.current && ownGeneration === generation.current) {
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
        return ownPlan(await planSetAgentConsumption(agentId, selection));
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
        return ownPlan(await planSetAssetConsumers(asset, agentIds));
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
        return ownPlan(await planUpdateCentralAsset(draft));
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
        return ownPlan(await planDeleteCentralAsset(asset, sourceId));
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
      const next = await commitAssetOperation(active, conflictConfirmation);
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
    await cancelAssetOperation(active.operation_id);
    if (mounted.current && planRef.current?.operation_id === active.operation_id) {
      planRef.current = null;
      setPlan(null);
    }
  }, []);

  return {
    inventory,
    loading,
    error,
    plan,
    committing,
    refresh,
    planForAgent,
    planForAsset,
    planUpdate,
    planDelete,
    commit,
    cancel,
  };
}
