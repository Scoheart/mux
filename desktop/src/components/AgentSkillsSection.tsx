import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  normalizeSkillCommandError,
  type SkillsState,
} from "../hooks/useSkillsState";
import { planSkillAssignment } from "../lib/api";
import type {
  OperationPlan,
  SkillCommandError,
  SkillInventoryItem,
  SkillNavigationRequest,
  SkillTargetView,
  SkillsInventory,
} from "../lib/types";
import { SkillRiskBadge } from "./SkillCard";
import {
  SkillReviewDialog,
  type SkillAssignmentContext,
} from "./SkillReviewDialog";
import { Badge, Switch } from "./ui";

interface AgentSkillsSectionProps {
  agentId: string;
  state: SkillsState;
  onOpenSkills(request: SkillNavigationRequest): void;
}

interface AssignmentRow {
  item: SkillInventoryItem;
  targets: SkillTargetView[];
  actualItems: SkillInventoryItem[];
}

interface AssignmentReview {
  plan: OperationPlan;
  context: SkillAssignmentContext;
}

function rowReality(row: AssignmentRow) {
  const actualStates = new Set(row.actualItems.flatMap((item) => item.states));
  if (row.item.source === null) {
    return { text: "已分配 · 来源异常，需处理", readOnly: true };
  }
  if (row.item.states.includes("locally_modified") || actualStates.has("locally_modified")) {
    return { text: "已分配 · 本地已修改，需在 Skills 中审阅", readOnly: true };
  }
  if (row.item.states.includes("broken_link") || actualStates.has("broken_link")) {
    return { text: "已分配 · 当前未生效（链接损坏）", readOnly: true };
  }
  if (row.item.states.includes("conflicting_link") || actualStates.has("conflicting_link")) {
    return { text: "已分配 · 当前未生效（链接冲突）", readOnly: true };
  }
  if (row.actualItems.length === 0 || actualStates.has("missing")) {
    return { text: "已分配 · 当前未生效（目标缺失）", readOnly: false };
  }
  if (actualStates.has("assigned")) {
    return { text: "已分配 · 当前生效", readOnly: false };
  }
  return { text: "已分配 · 当前未生效（状态异常）", readOnly: true };
}

function assignedRows(
  inventory: NonNullable<SkillsState["inventory"]>,
  agentId: string,
): AssignmentRow[] {
  const targets = new Map(inventory.targets.map((target) => [target.target_id, target]));
  return inventory.items
    .filter((item) => item.location.kind === "central")
    .map((item) => {
      const retainedTargets = item.assigned_target_ids
        .map((targetId) => targets.get(targetId))
        .filter(
          (target): target is SkillTargetView =>
            Boolean(target?.affected_agent_ids.includes(agentId)),
        );
      const retainedIds = new Set(retainedTargets.map((target) => target.target_id));
      return {
        item,
        targets: retainedTargets,
        actualItems: inventory.items.filter(
          (candidate) =>
            candidate.name === item.name &&
            candidate.location.kind === "agent_target" &&
            retainedIds.has(candidate.location.target_id),
        ),
      };
    })
    .filter((row) => row.targets.length > 0);
}

export function AgentSkillsSection({
  agentId,
  state,
  onOpenSkills,
}: AgentSkillsSectionProps) {
  const [assignmentReview, setAssignmentReview] = useState<AssignmentReview | null>(null);
  const [planningName, setPlanningName] = useState<string | null>(null);
  const [assignmentError, setAssignmentError] = useState<SkillCommandError | null>(null);
  const [localRecovery, setLocalRecovery] = useState<string | null>(null);
  const planningRef = useRef(false);
  const generationRef = useRef(0);
  const planRef = useRef<OperationPlan | null>(null);
  const committedRef = useRef(new Set<string>());
  const recoveryRef = useRef(new Set<string>());
  const cancellationsRef = useRef(new Map<string, Promise<void>>());
  const commitRef = useRef<{
    operationId: string;
    promise: Promise<SkillsInventory>;
  } | null>(null);
  const cancelRef = useRef(state.cancel);
  const mountedRef = useRef(true);
  const currentAgentRef = useRef(agentId);
  cancelRef.current = state.cancel;
  const rows = useMemo(
    () => state.inventory ? assignedRows(state.inventory, agentId) : [],
    [agentId, state.inventory],
  );
  const agentNames = new Map(
    state.inventory?.agents.map((agent) => [agent.id, agent.name]) ?? [],
  );
  const verifiedAgent = state.inventory?.agents.some((agent) => agent.id === agentId) ?? false;
  const recoveryError =
    localRecovery ??
    state.inventory?.recovery_error ??
    (state.error?.code === "recovery_required" ? state.error.message : null);
  const readOnly = recoveryError !== null;
  const assignmentBusy =
    planningName !== null ||
    assignmentReview !== null ||
    state.pendingOperation !== null;
  const addSkill = () => onOpenSkills({ kind: "install", agentId });
  const retry = () => {
    void state.refresh().catch(() => undefined);
  };
  const cancelOnce = useCallback((operationId: string, reportError: boolean) => {
    const existing = cancellationsRef.current.get(operationId);
    if (existing) return existing;
    const pending = cancelRef.current(operationId).catch((reason: unknown) => {
      if (reportError && mountedRef.current) {
        setAssignmentError(normalizeSkillCommandError(reason));
      }
    });
    cancellationsRef.current.set(operationId, pending);
    return pending;
  }, []);
  const cleanupPlan = useCallback(async (plan: OperationPlan, reportError: boolean) => {
    const committing = commitRef.current;
    if (committing?.operationId === plan.operation_id) {
      try {
        await committing.promise;
      } catch {
        // Failed commits leave a staged plan unless recovery owns the journal.
      }
    }
    if (
      !committedRef.current.has(plan.operation_id) &&
      !recoveryRef.current.has(plan.operation_id)
    ) {
      await cancelOnce(plan.operation_id, reportError);
    }
  }, [cancelOnce]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      planningRef.current = false;
      const plan = planRef.current;
      planRef.current = null;
      if (plan) void cleanupPlan(plan, false);
    };
  }, [cleanupPlan]);

  useEffect(() => {
    if (currentAgentRef.current === agentId) return;
    currentAgentRef.current = agentId;
    generationRef.current += 1;
    planningRef.current = false;
    const plan = planRef.current;
    planRef.current = null;
    setAssignmentReview(null);
    setPlanningName(null);
    setAssignmentError(null);
    setLocalRecovery(null);
    if (plan) void cleanupPlan(plan, false);
  }, [agentId, cleanupPlan]);

  const planDisable = async (row: AssignmentRow) => {
    if (
      planningRef.current ||
      planRef.current ||
      state.pendingOperation !== null ||
      recoveryError
    ) {
      return;
    }
    planningRef.current = true;
    const generation = ++generationRef.current;
    const context: SkillAssignmentContext = {
      enabled: false,
      agentIds: [agentId],
      targetIds: row.targets.map((target) => target.target_id),
    };
    setAssignmentError(null);
    setPlanningName(row.item.name);
    try {
      const plan = await planSkillAssignment({
        skill_name: row.item.name,
        agent_ids: [agentId],
        enabled: false,
      });
      if (!mountedRef.current || generationRef.current !== generation) {
        await cancelOnce(plan.operation_id, false);
        return;
      }
      planRef.current = plan;
      setAssignmentReview({ plan, context });
    } catch (reason) {
      if (mountedRef.current && generationRef.current === generation) {
        const error = normalizeSkillCommandError(reason);
        if (error.code === "recovery_required") setLocalRecovery(error.message);
        else setAssignmentError(error);
      }
    } finally {
      if (generationRef.current === generation) {
        planningRef.current = false;
        if (mountedRef.current) setPlanningName(null);
      }
    }
  };
  const closeReview = () => {
    const plan = planRef.current;
    planRef.current = null;
    setAssignmentReview(null);
    if (plan) return cleanupPlan(plan, true);
  };
  const commitAssignment: SkillsState["commit"] = (plan, confirmation) => {
    const pending = state.commit(plan, confirmation);
    commitRef.current = { operationId: plan.operation_id, promise: pending };
    void pending
      .then(
        () => {
          committedRef.current.add(plan.operation_id);
        },
        (reason: unknown) => {
          if (normalizeSkillCommandError(reason).code === "recovery_required") {
            recoveryRef.current.add(plan.operation_id);
          }
          throw reason;
        },
      )
      .finally(() => {
        if (commitRef.current?.promise === pending) commitRef.current = null;
      })
      .catch(() => undefined);
    return pending;
  };
  const assignmentCommitted = () => {
    const plan = planRef.current;
    if (plan) committedRef.current.add(plan.operation_id);
    planRef.current = null;
    setAssignmentReview(null);
  };
  const enterRecovery = (message: string) => {
    const plan = planRef.current;
    if (plan) recoveryRef.current.add(plan.operation_id);
    planRef.current = null;
    setAssignmentReview(null);
    setLocalRecovery(message);
  };

  return (
    <section className="mux-agent-section mux-agent-skills" aria-labelledby="agent-skills-title">
      <div className="mux-agent-section-head">
        <div>
          <h3 id="agent-skills-title">Skills</h3>
          <p>管理当前 Agent 使用的用户级 Skills。</p>
        </div>
        {state.inventory && verifiedAgent && (
          <button type="button" className="btn-primary" disabled={readOnly} onClick={addSkill}>
            添加 Skill
          </button>
        )}
      </div>
      {!state.inventory && state.loading ? (
        <div className="mux-agent-skill-state" role="status">正在读取 Skills…</div>
      ) : !state.inventory && state.error && !recoveryError ? (
        <div className="mux-agent-skill-state" role="alert">
          <strong>读取 Skills 失败</strong>
          <span>{state.error.message}</span>
          <button type="button" className="btn-secondary" onClick={retry}>重试</button>
        </div>
      ) : state.inventory && !verifiedAgent ? (
        <div className="mux-agent-skill-state">
          <strong>此 Agent 暂不支持 Skills</strong>
          <span>MUX 尚未核验可用的用户级 Skill 目录。</span>
        </div>
      ) : state.inventory && rows.length === 0 ? (
        <div className="mux-agent-skill-state">
          <strong>还没有分配 Skill</strong>
          <span>添加后会通过 MUX 管理的目标目录向此 Agent 提供 Skill。</span>
        </div>
      ) : null}
      {recoveryError ? (
        <div className="mux-agent-skill-notice" data-tone="recovery" role="status">
          <strong>Skills 已进入只读恢复状态</strong>
          <span>{recoveryError}</span>
        </div>
      ) : state.inventory && state.error ? (
        <div className="mux-agent-skill-notice" data-tone="error" role="status">
          <strong>最近一次 Skill 操作未完成</strong>
          <span>{state.error.message}</span>
        </div>
      ) : null}
      {assignmentError && <div className="mux-agent-skill-notice" data-tone="error" role="alert">{assignmentError.message}</div>}
      {planningName && (
        <div className="mux-agent-skill-progress" role="status" aria-live="polite">
          正在生成 {planningName} 分配计划…
        </div>
      )}
      {state.inventory && verifiedAgent && rows.length > 0 && <ul className="mux-agent-skill-list">
        {rows.map(({ item, targets, actualItems }) => {
          const row = { item, targets, actualItems };
          const affectedNames = [
            ...new Set(targets.flatMap((target) => target.affected_agent_ids)),
          ].map((id) => agentNames.get(id) ?? id);
          const reality = rowReality(row);
          return (
            <li
              key={item.identity}
              className="mux-agent-skill-row"
              data-attention={reality.readOnly ? "true" : undefined}
              aria-busy={assignmentBusy || undefined}
              aria-label={`${item.name} Skill`}
            >
              <div className="mux-agent-skill-copy">
                <div className="mux-agent-skill-heading">
                  <div className="mux-agent-skill-identity">
                    <strong>{item.name}</strong>
                    <p>{item.description}</p>
                  </div>
                  <div className="mux-agent-skill-badges">
                    <SkillRiskBadge level={item.risk?.level ?? null} />
                    {item.update.available && <Badge tone="warning">有更新</Badge>}
                  </div>
                </div>
                <span className="mux-agent-skill-reality">{reality.text}</span>
                <div className="mux-agent-skill-target">
                  <span>真实目标</span>
                  <div>
                    {targets.map((target) => (
                      <code className="mux-agent-skill-path" key={target.target_id}>
                        {target.global_dir}
                      </code>
                    ))}
                  </div>
                  <small>共享影响：{affectedNames.join("、")}</small>
                </div>
              </div>
              <div className="mux-agent-skill-actions">
                {!reality.readOnly && (
                  <Switch
                    checked
                    disabled={
                      readOnly ||
                      assignmentBusy
                    }
                    title={`停用 ${item.name}`}
                    onChange={(enabled) => {
                      if (!enabled) void planDisable(row);
                    }}
                  />
                )}
                <button
                  type="button"
                  className="btn-ghost"
                  onClick={() => onOpenSkills({ kind: "detail", skillName: item.name })}
                >
                  查看 {item.name} 详情
                </button>
              </div>
            </li>
          );
        })}
      </ul>}
      {assignmentReview && (
        <SkillReviewDialog
          plan={assignmentReview.plan}
          assignmentContext={assignmentReview.context}
          onCommit={commitAssignment}
          onClose={closeReview}
          onCommitted={assignmentCommitted}
          onRecoveryRequired={enterRecovery}
        />
      )}
    </section>
  );
}
