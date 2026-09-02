import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelOperation,
  commitOperation,
  listAgentCapabilities,
  listConsumptionInventory,
  planOperation,
} from "../lib/api";
import i18n from "../i18n";
import { assetIdentity } from "../lib/consumption";
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
} from "../lib/types";

export interface ConsumptionState {
  agents: AgentCapabilityView[];
  inventory: ConsumptionInventory | null;
  loading: boolean;
  error: AssetCommandError | null;
  agentsError: AssetCommandError | null;
  plan: AssetOperationPlan | null;
  committing: boolean;
  refresh(): Promise<ConsumptionInventory>;
  refreshAgents(): Promise<AgentCapabilityView[]>;
  planForAgent(
    agentId: string,
    selection: AgentConsumptionSelection,
  ): Promise<AssetOperationPlan>;
  planAdditionsForAgent(
    agentId: string,
    selection: AgentConsumptionSelection,
  ): Promise<AssetOperationPlan>;
  clearAgentMcp(agentId: string): Promise<ConsumptionInventory>;
  planClearAgentModels(agentId: string): Promise<AssetOperationPlan>;
  setMcpEnabled(
    agentId: string,
    assetKey: string,
    enabled: boolean,
  ): Promise<ConsumptionInventory>;
  setAllMcpEnabled(
    agentId: string,
    enabled: boolean,
  ): Promise<ConsumptionInventory>;
  setSkillEnabled(
    agentId: string,
    name: string,
    enabled: boolean,
  ): Promise<ConsumptionInventory>;
  planModelEnabled(
    agentId: string,
    profileId: string,
    enabled: boolean,
  ): Promise<AssetOperationPlan>;
  setActiveModel(
    agentId: string,
    profileId: string,
  ): Promise<ConsumptionInventory>;
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
  const [agentsError, setAgentsError] = useState<AssetCommandError | null>(null);
  const [plan, setPlan] = useState<AssetOperationPlan | null>(null);
  const [committing, setCommitting] = useState(false);
  const inventoryGeneration = useRef(0);
  const agentsGeneration = useRef(0);
  const mounted = useRef(true);
  const planRef = useRef(plan);
  const planningRef = useRef(false);
  const committingRef = useRef(false);
  planRef.current = plan;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    const ownGeneration = ++inventoryGeneration.current;
    try {
      const next = await listConsumptionInventory();
      if (mounted.current && ownGeneration === inventoryGeneration.current) {
        setInventory(next);
        setError(null);
      }
      return next;
    } catch (cause) {
      const nextError = commandError(cause);
      if (mounted.current && ownGeneration === inventoryGeneration.current) setError(nextError);
      throw cause;
    } finally {
      if (mounted.current && ownGeneration === inventoryGeneration.current) setLoading(false);
    }
  }, []);

  const refreshAgents = useCallback(async () => {
    const ownGeneration = ++agentsGeneration.current;
    try {
      const next = await listAgentCapabilities();
      if (mounted.current && ownGeneration === agentsGeneration.current) {
        setAgents(next);
        setAgentsError(null);
      }
      return next;
    } catch (cause) {
      const nextError = commandError(cause);
      if (mounted.current && ownGeneration === agentsGeneration.current) {
        setAgentsError(nextError);
      }
      throw cause;
    }
  }, []);

  useEffect(() => {
    if (!autoLoad) return;
    void Promise.allSettled([refresh(), refreshAgents()]);
  }, [autoLoad, refresh, refreshAgents]);

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

  const executeImmediately = useCallback(async (request: PlanOperationRequest) => {
    if (planRef.current || planningRef.current || committingRef.current) {
      throw new Error("已有正在处理的资产操作");
    }
    planningRef.current = true;
    let active: AssetOperationPlan | null = null;
    try {
      active = await planAsset(request);
      if (!active.can_commit) {
        throw new Error(active.warnings[0] ?? "当前状态无法应用此更改");
      }
      planningRef.current = false;
      committingRef.current = true;
      if (mounted.current) setCommitting(true);
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
      ++inventoryGeneration.current;
      if (mounted.current) {
        setInventory(next);
        setError(null);
      }
      if (!committed.converged) {
        throw {
          code: "pending_convergence",
          message: i18n.t("observations.convergencePending"),
        } satisfies AssetCommandError;
      }
      return next;
    } catch (cause) {
      if (active) {
        await cancelOperation({ domain: "asset", operation_id: active.operation_id })
          .catch(() => undefined);
      }
      if (mounted.current) setError(commandError(cause));
      throw cause;
    } finally {
      planningRef.current = false;
      committingRef.current = false;
      if (mounted.current) setCommitting(false);
    }
  }, []);

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

  const clearAgentMcp = useCallback(
    (agentId: string) => executeImmediately({
      operation: "clear_agent_mcp",
      request: { agent_id: agentId },
    }),
    [executeImmediately],
  );

  const planClearAgentModels = useCallback(
    (agentId: string) => startPlan({
      operation: "clear_agent_models",
      request: { agent_id: agentId },
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

  const setMcpEnabled = useCallback(
    (agentId: string, assetKey: string, enabled: boolean) => executeImmediately({
      operation: "set_mcp_enabled",
      request: { agent_id: agentId, asset_key: assetKey, enabled },
    }),
    [executeImmediately],
  );

  const setAllMcpEnabled = useCallback(
    (agentId: string, enabled: boolean) => executeImmediately({
      operation: "set_all_mcp_enabled",
      request: { agent_id: agentId, enabled },
    }),
    [executeImmediately],
  );

  const setSkillEnabled = useCallback(
    (agentId: string, name: string, enabled: boolean) => executeImmediately({
      operation: "set_skill_enabled",
      request: { agent_id: agentId, name, enabled },
    }),
    [executeImmediately],
  );

  const planModelEnabled = useCallback(
    (agentId: string, profileId: string, enabled: boolean) => startPlan({
      operation: "set_model_enabled",
      request: { agent_id: agentId, profile_id: profileId, enabled },
    }),
    [startPlan],
  );

  const setActiveModel = useCallback(
    (agentId: string, profileId: string) => executeImmediately({
      operation: "set_active_model",
      request: { agent_id: agentId, profile_id: profileId },
    }),
    [executeImmediately],
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
      const planWithRevision = (revision: string) => planOperation({
        operation: "converge_consumption" as const,
        request: {
          agent_id: item.agent_id,
          asset: item.asset,
          action,
          observed_revision: revision,
        },
      });
      let result;
      try {
        result = await planWithRevision(observedRevision);
      } catch (cause) {
        if (commandError(cause).code !== "observation_stale") throw cause;
        const next = await refresh();
        const current = [...next.consumptions, ...next.external].find(
          (candidate) => candidate.agent_id === item.agent_id
            && candidate.asset.domain === item.asset.domain
            && assetIdentity(candidate.asset) === assetIdentity(item.asset)
            && candidate.available_actions.includes(action),
        );
        if (!current) throw cause;
        result = await planWithRevision(next.revision);
      }
      if (result.domain === "asset") ownPlan(result.plan);
      return result;
    } catch (cause) {
      if (mounted.current) setError(commandError(cause));
      throw cause;
    } finally {
      planningRef.current = false;
    }
  }, [inventory?.revision, ownPlan, refresh]);

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
      ++inventoryGeneration.current;
      if (mounted.current && planRef.current?.operation_id === active.operation_id) {
        setInventory(next);
        planRef.current = null;
        setPlan(null);
        setError(null);
      }
      if (!committed.converged) {
        throw {
          code: "pending_convergence",
          message: i18n.t("observations.convergencePending"),
        } satisfies AssetCommandError;
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
    agentsError,
    plan,
    committing,
    refresh,
    refreshAgents,
    planForAgent,
    planAdditionsForAgent,
    clearAgentMcp,
    planClearAgentModels,
    setMcpEnabled,
    setAllMcpEnabled,
    setSkillEnabled,
    planModelEnabled,
    setActiveModel,
    planConvergence,
    planForAsset,
    planUpdate,
    planDelete,
    commit,
    cancel,
  };
}
