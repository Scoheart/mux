import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "../lib/api";
import type {
  OperationPlan,
  SkillCommandError,
  SkillCommitRequest,
  SkillOperationKind,
  SkillsInventory,
  UpdateCheckOutcome,
} from "../lib/types";

export interface SkillsState {
  inventory: SkillsInventory | null;
  loading: boolean;
  pendingOperation: string | null;
  error: SkillCommandError | null;
  refresh(): Promise<SkillsInventory>;
  commit(
    plan: OperationPlan,
    findingsConfirmation: string | null,
  ): Promise<SkillsInventory>;
  cancel(operationId: string): Promise<void>;
  checkUpdates(manual: boolean): Promise<UpdateCheckOutcome>;
}

const unknownError = (): SkillCommandError => ({
  code: "unknown",
  message: "操作失败，请重试。",
});

export function normalizeSkillCommandError(value: unknown): SkillCommandError {
  if (typeof value !== "object" || value === null) return unknownError();
  const candidate = value as Record<string, unknown>;
  if (
    typeof candidate.code !== "string" ||
    typeof candidate.message !== "string"
  ) {
    return unknownError();
  }
  const normalized: SkillCommandError = {
    code: candidate.code,
    message: candidate.message,
  };
  if (typeof candidate.retry_at === "string") {
    normalized.retry_at = candidate.retry_at;
  }
  if (typeof candidate.findings_hash === "string") {
    normalized.findings_hash = candidate.findings_hash;
  }
  return normalized;
}

const committers = {
  install: api.commitSkillInstall,
  import: api.commitSkillImport,
  update: api.commitSkillUpdate,
  remove: api.commitSkillRemove,
  assignment: api.commitSkillAssignment,
  repair: api.commitSkillRepair,
} satisfies Record<
  SkillOperationKind,
  (request: SkillCommitRequest) => Promise<SkillsInventory>
>;

export function useSkillsState(): SkillsState {
  const [inventory, setInventory] = useState<SkillsInventory | null>(null);
  const [loading, setLoading] = useState(true);
  const [pendingOperation, setPendingOperation] = useState<string | null>(null);
  const [error, setError] = useState<SkillCommandError | null>(null);
  const mounted = useRef(true);
  const activeCommit = useRef<string | null>(null);
  const cacheGeneration = useRef(0);
  const loadingGeneration = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const loadInventory = useCallback(
    async (generation: number, clearError: boolean) => {
      const loadingRequest = ++loadingGeneration.current;
      if (mounted.current) {
        setLoading(true);
        if (clearError) setError(null);
      }
      try {
        const next = await api.listSkillsInventory();
        if (
          mounted.current &&
          cacheGeneration.current === generation
        ) {
          setInventory(next);
        }
        return next;
      } catch (reason) {
        const normalized = normalizeSkillCommandError(reason);
        if (
          mounted.current &&
          cacheGeneration.current === generation
        ) {
          setError(normalized);
        }
        throw normalized;
      } finally {
        if (
          mounted.current &&
          loadingGeneration.current === loadingRequest
        ) {
          setLoading(false);
        }
      }
    },
    [],
  );

  const refresh = useCallback(() => {
    const generation = ++cacheGeneration.current;
    return loadInventory(generation, true);
  }, [loadInventory]);

  useEffect(() => {
    void refresh().catch(() => undefined);
  }, [refresh]);

  const commit = useCallback(
    async (
      plan: OperationPlan,
      findingsConfirmation: string | null,
    ): Promise<SkillsInventory> => {
      if (activeCommit.current !== null) {
        throw {
          code: "operation_pending",
          message: "已有 Skill 操作正在进行。",
        } satisfies SkillCommandError;
      }

      const operationId = plan.operation_id;
      activeCommit.current = operationId;
      ++cacheGeneration.current;
      if (mounted.current) {
        setError(null);
        setPendingOperation(operationId);
      }
      const request: SkillCommitRequest = {
        operation_id: operationId,
        candidate_hash: plan.candidate_hash,
        findings_confirmation: findingsConfirmation,
      };

      try {
        const next = await committers[plan.kind](request);
        ++cacheGeneration.current;
        if (mounted.current) setInventory(next);
        return next;
      } catch (reason) {
        const normalized = normalizeSkillCommandError(reason);
        if (mounted.current) setError(normalized);
        throw normalized;
      } finally {
        if (activeCommit.current === operationId) {
          activeCommit.current = null;
          if (mounted.current) setPendingOperation(null);
        }
      }
    },
    [],
  );

  const cancel = useCallback(async (operationId: string) => {
    if (mounted.current) setError(null);
    try {
      await api.cancelSkillOperation(operationId);
    } catch (reason) {
      const normalized = normalizeSkillCommandError(reason);
      if (mounted.current) setError(normalized);
      throw normalized;
    }
  }, []);

  const checkUpdates = useCallback(
    async (manual: boolean) => {
      const generation = ++cacheGeneration.current;
      if (mounted.current) setError(null);
      try {
        const outcome = await api.checkSkillUpdates(manual);
        await loadInventory(generation, false);
        return outcome;
      } catch (reason) {
        const normalized = normalizeSkillCommandError(reason);
        if (
          mounted.current &&
          cacheGeneration.current === generation
        ) {
          setError(normalized);
        }
        throw normalized;
      }
    },
    [loadInventory],
  );

  return {
    inventory,
    loading,
    pendingOperation,
    error,
    refresh,
    commit,
    cancel,
    checkUpdates,
  };
}
