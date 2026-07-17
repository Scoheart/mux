import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  normalizeSkillCommandError,
  type SkillsState,
} from "../hooks/useSkillsState";
import * as api from "../lib/api";
import {
  filterSkills,
  type SkillContentFilter,
  type SkillSourceFilter,
  type SkillStatusFilter,
} from "../lib/skills";
import type {
  OperationPlan,
  SkillCommandError,
  SkillContentKind,
  SkillDetail,
  SkillNavigationIntent,
  SkillsInventory,
} from "../lib/types";
import {
  FolderIcon,
  LayersIcon,
  LinkIcon,
  PackageIcon,
  RefreshIcon,
  TerminalIcon,
} from "./icons";
import { SkillCard } from "./SkillCard";
import { SkillInstallDialog } from "./SkillInstallDialog";
import {
  SkillInspector,
  type SkillLifecycleIntent,
} from "./SkillInspector";
import {
  SkillReviewDialog,
  type SkillReviewDialogProps,
} from "./SkillReviewDialog";
import { useToast } from "./Toast";
import {
  ResourceEmpty,
  ResourceGrid,
  ResourceTabs,
  ResourceWorkspace,
  SidebarItem,
  SidebarSection,
  WorkspaceSidebar,
} from "./ResourceWorkspace";

const statusOptions: Array<{ value: SkillStatusFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "updates", label: "有更新" },
  { value: "needs_attention", label: "需处理" },
  { value: "external", label: "外部" },
];

const sourceOptions: Array<{
  value: SkillSourceFilter;
  label: string;
  icon: ReactNode;
}> = [
  { value: "all", label: "全部来源", icon: <LayersIcon className="w-3.5 h-3.5" /> },
  { value: "github", label: "GitHub", icon: <LinkIcon className="w-3.5 h-3.5" /> },
  { value: "local", label: "本地", icon: <FolderIcon className="w-3.5 h-3.5" /> },
];

const contentOptions: Array<{
  value: SkillContentFilter;
  label: string;
  icon: ReactNode;
}> = [
  { value: "all", label: "全部类型", icon: <PackageIcon className="w-3.5 h-3.5" /> },
  { value: "automation", label: "自动化", icon: <TerminalIcon className="w-3.5 h-3.5" /> },
  { value: "assets", label: "模板与素材", icon: <PackageIcon className="w-3.5 h-3.5" /> },
  { value: "reference", label: "参考资料", icon: <LayersIcon className="w-3.5 h-3.5" /> },
  { value: "instructions", label: "说明型", icon: <PackageIcon className="w-3.5 h-3.5" /> },
];

interface SkillsViewProps {
  state: SkillsState;
  intent?: SkillNavigationIntent;
  onIntentConsumed?(id: number): void;
}

type LifecycleAssignmentContext = NonNullable<
  SkillReviewDialogProps["assignmentContext"]
> & { operationId: string };

interface LifecycleReview {
  plan: OperationPlan;
  assignmentContext: LifecycleAssignmentContext | null;
}

function assignmentContextForPlan(
  intent: Extract<SkillLifecycleIntent, { kind: "assignment" }>,
  inventory: SkillsInventory | null,
  plan: OperationPlan,
): LifecycleAssignmentContext {
  const selectedAgentIds = new Set(intent.agentIds);
  const priorTargetIds = new Set(
    inventory?.items.find(
      (item) =>
        item.name === intent.skillName && item.location.kind === "central",
    )?.assigned_target_ids ?? [],
  );
  const targetIds = plan.targets
    .filter(
      (target) =>
        target.affected_agent_ids.some((agentId) =>
          selectedAgentIds.has(agentId),
        ) &&
        (!intent.enabled || !priorTargetIds.has(target.target_id)),
    )
    .map((target) => target.target_id);
  return {
    operationId: plan.operation_id,
    enabled: intent.enabled,
    agentIds: intent.agentIds,
    targetIds,
  };
}

export function SkillsView({
  state,
  intent,
  onIntentConsumed,
}: SkillsViewProps) {
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<SkillStatusFilter>("all");
  const [source, setSource] = useState<SkillSourceFilter>("all");
  const [contentKind, setContentKind] = useState<SkillContentFilter>("all");
  const [checking, setChecking] = useState(false);
  const [selectedIdentity, setSelectedIdentity] = useState<string | null>(null);
  const [detail, setDetail] = useState<SkillDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<SkillCommandError | null>(null);
  const [installOpen, setInstallOpen] = useState(false);
  const [installInitialAgentId, setInstallInitialAgentId] = useState<
    string | null
  >(null);
  const [navigationNotice, setNavigationNotice] = useState<string | null>(null);
  const [lifecycleReview, setLifecycleReview] =
    useState<LifecycleReview | null>(null);
  const [lifecyclePlanning, setLifecyclePlanning] = useState(false);
  const [recoveryRequired, setRecoveryRequired] = useState<string | null>(null);
  const detailGeneration = useRef(0);
  const lifecycleGeneration = useRef(0);
  const lifecyclePlanRef = useRef<OperationPlan | null>(null);
  const lifecyclePendingRef = useRef(false);
  const lifecycleCommittedRef = useRef(new Set<string>());
  const lifecycleRecoveryRef = useRef(new Set<string>());
  const lifecycleCancellationsRef = useRef(new Map<string, Promise<void>>());
  const lifecycleCommitRef = useRef<{
    operationId: string;
    promise: Promise<SkillsInventory>;
  } | null>(null);
  const cancelRef = useRef(state.cancel);
  const toastRef = useRef(toast);
  const mounted = useRef(true);
  const lastConsumedIntentId = useRef<number | null>(null);
  cancelRef.current = state.cancel;
  toastRef.current = toast;
  const items = state.inventory?.items ?? [];
  const filters = { status, source, contentKind, query };
  const filtered = useMemo(
    () => filterSkills(items, filters),
    [contentKind, items, query, source, status],
  );
  const selected = selectedIdentity
    ? items.find((item) => item.identity === selectedIdentity) ?? null
    : null;
  const countWith = (
    override: Partial<{
      status: SkillStatusFilter;
      source: SkillSourceFilter;
      contentKind: SkillContentKind | "all";
    }>,
  ) => filterSkills(items, { ...filters, ...override }).length;
  const recoveryError =
    recoveryRequired ??
    state.inventory?.recovery_error ??
    (state.error?.code === "recovery_required" ? state.error.message : null);
  const checkDisabled =
    checking ||
    lifecyclePlanning ||
    state.loading ||
    state.pendingOperation !== null ||
    recoveryError !== null;

  const cancelLifecycleOnce = useCallback(
    (operationId: string, reportError: boolean) => {
      const existing = lifecycleCancellationsRef.current.get(operationId);
      if (existing) return existing;
      const pending = cancelRef.current(operationId).catch((reason: unknown) => {
        if (reportError && mounted.current) {
          toastRef.current.show({
            kind: "error",
            msg: normalizeSkillCommandError(reason).message,
          });
        }
      });
      lifecycleCancellationsRef.current.set(operationId, pending);
      return pending;
    },
    [],
  );

  const cleanupLifecyclePlan = useCallback(
    async (plan: OperationPlan, reportError: boolean) => {
      const committing = lifecycleCommitRef.current;
      if (committing?.operationId === plan.operation_id) {
        try {
          await committing.promise;
        } catch {
          // A failed commit leaves this plan staged unless recovery owns it.
        }
      }
      if (
        !lifecycleCommittedRef.current.has(plan.operation_id) &&
        !lifecycleRecoveryRef.current.has(plan.operation_id)
      ) {
        await cancelLifecycleOnce(plan.operation_id, reportError);
      }
    },
    [cancelLifecycleOnce],
  );

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      detailGeneration.current += 1;
      lifecycleGeneration.current += 1;
      const plan = lifecyclePlanRef.current;
      if (plan) void cleanupLifecyclePlan(plan, false);
    };
  }, [cleanupLifecyclePlan]);

  const closeInspector = useCallback(() => {
    detailGeneration.current += 1;
    lifecycleGeneration.current += 1;
    setSelectedIdentity(null);
    setDetail(null);
    setDetailLoading(false);
    setDetailError(null);
  }, []);

  const planLifecycle = async (intent: SkillLifecycleIntent) => {
    if (
      lifecyclePendingRef.current ||
      lifecyclePlanRef.current ||
      state.pendingOperation !== null ||
      recoveryError
    ) {
      return;
    }

    const generation = ++lifecycleGeneration.current;
    const inventorySnapshot = state.inventory;
    lifecyclePendingRef.current = true;
    setLifecyclePlanning(true);
    try {
      const plan = await (() => {
        switch (intent.kind) {
          case "import":
            return api.planSkillImport({
              identity: intent.identity,
              agent_ids: intent.agentIds,
              replace_conflicts: false,
            });
          case "update":
            return api.planSkillUpdate({
              skill_name: intent.skillName,
              replace_local_changes: false,
            });
          case "remove":
            return api.planSkillRemove({ skill_name: intent.skillName });
          case "assignment":
            return api.planSkillAssignment({
              skill_name: intent.skillName,
              agent_ids: intent.agentIds,
              enabled: intent.enabled,
            });
          case "repair":
            return api.planSkillRepair({
              skill_name: intent.skillName,
              repair: intent.repair,
            });
        }
      })();

      if (!mounted.current || lifecycleGeneration.current !== generation) {
        await cancelLifecycleOnce(plan.operation_id, false);
        return;
      }
      lifecyclePlanRef.current = plan;
      setLifecycleReview({
        plan,
        assignmentContext:
          intent.kind === "assignment"
            ? assignmentContextForPlan(intent, inventorySnapshot, plan)
            : null,
      });
    } catch (reason) {
      if (mounted.current && lifecycleGeneration.current === generation) {
        const error = normalizeSkillCommandError(reason);
        if (error.code === "recovery_required") {
          setRecoveryRequired(error.message);
        } else {
          toast.show({ kind: "error", msg: error.message });
        }
      }
    } finally {
      lifecyclePendingRef.current = false;
      if (mounted.current) setLifecyclePlanning(false);
    }
  };

  const closeLifecycleReview = () => {
    const plan = lifecyclePlanRef.current;
    lifecyclePlanRef.current = null;
    setLifecycleReview(null);
    if (plan) return cleanupLifecyclePlan(plan, true);
  };

  const commitLifecycle: SkillsState["commit"] = (plan, confirmation) => {
    const pending = state.commit(plan, confirmation);
    lifecycleCommitRef.current = {
      operationId: plan.operation_id,
      promise: pending,
    };
    void pending
      .then(
        () => {
          lifecycleCommittedRef.current.add(plan.operation_id);
        },
        (reason: unknown) => {
          if (normalizeSkillCommandError(reason).code === "recovery_required") {
            lifecycleRecoveryRef.current.add(plan.operation_id);
          }
          throw reason;
        },
      )
      .finally(() => {
        if (lifecycleCommitRef.current?.promise === pending) {
          lifecycleCommitRef.current = null;
        }
      })
      .catch(() => undefined);
    return pending;
  };

  const lifecycleCommitted = (inventory: SkillsInventory) => {
    const plan = lifecyclePlanRef.current;
    if (plan) lifecycleCommittedRef.current.add(plan.operation_id);
    lifecyclePlanRef.current = null;
    setLifecycleReview(null);
    toast.show({ kind: "success", msg: "Skill 操作已完成。" });
    const selectedName = selected?.name;
    if (selectedName && !inventory.items.some((item) => item.name === selectedName)) {
      closeInspector();
    }
  };

  const enterRecovery = (message: string) => {
    const plan = lifecyclePlanRef.current;
    if (plan) lifecycleRecoveryRef.current.add(plan.operation_id);
    lifecyclePlanRef.current = null;
    setLifecycleReview(null);
    setInstallOpen(false);
    setRecoveryRequired(message);
  };

  const changeQuery = (value: string) => {
    closeInspector();
    setQuery(value);
  };

  const changeStatus = (value: SkillStatusFilter) => {
    closeInspector();
    setStatus(value);
  };

  const changeSource = (value: SkillSourceFilter) => {
    closeInspector();
    setSource(value);
  };

  const changeContentKind = (value: SkillContentFilter) => {
    closeInspector();
    setContentKind(value);
  };

  const openSkill = (identity: string) => {
    setNavigationNotice(null);
    setSelectedIdentity(identity);
  };

  useEffect(() => {
    if (!recoveryError) return;
    setInstallOpen(false);
    setInstallInitialAgentId(null);
  }, [recoveryError]);

  useEffect(() => {
    const inventory = state.inventory;
    if (
      !intent ||
      lastConsumedIntentId.current === intent.id ||
      (!inventory && !recoveryError)
    ) {
      return;
    }

    lastConsumedIntentId.current = intent.id;
    setNavigationNotice(null);
    if (!inventory) {
      closeInspector();
      setInstallInitialAgentId(null);
      setInstallOpen(false);
    } else if (intent.kind === "detail") {
      const item = inventory.items.find(
        (candidate) =>
          candidate.name === intent.skillName &&
          candidate.location.kind === "central" &&
          (candidate.source !== null ||
            candidate.assigned_target_ids.length > 0),
      );
      if (item) {
        setQuery("");
        setStatus("all");
        setSource("all");
        setContentKind("all");
        setSelectedIdentity(item.identity);
      } else {
        closeInspector();
        setNavigationNotice(
          `未找到可管理的 Skill“${intent.skillName}”。`,
        );
      }
    } else if (!recoveryError) {
      setInstallInitialAgentId(intent.agentId);
      setInstallOpen(true);
    } else {
      setInstallInitialAgentId(null);
      setInstallOpen(false);
    }
    onIntentConsumed?.(intent.id);
  }, [closeInspector, intent, onIntentConsumed, recoveryError, state.inventory]);

  useEffect(() => {
    if (
      selectedIdentity &&
      !filtered.some((item) => item.identity === selectedIdentity)
    ) {
      closeInspector();
    }
  }, [closeInspector, filtered, selectedIdentity]);

  useEffect(() => {
    if (!selected) return;

    const generation = ++detailGeneration.current;
    let active = true;
    setDetail(null);
    setDetailError(null);
    setDetailLoading(true);

    void api
      .getSkillDetail(selected.identity)
      .then((next) => {
        if (active && detailGeneration.current === generation) setDetail(next);
      })
      .catch((reason: unknown) => {
        if (active && detailGeneration.current === generation) {
          setDetailError(normalizeSkillCommandError(reason));
        }
      })
      .finally(() => {
        if (active && detailGeneration.current === generation) {
          setDetailLoading(false);
        }
      });

    return () => {
      active = false;
      if (detailGeneration.current === generation) detailGeneration.current += 1;
    };
  }, [selected?.identity]);

  const checkUpdates = async () => {
    if (checkDisabled) return;
    setChecking(true);
    try {
      await state.checkUpdates(true);
    } catch {
      // The app-owned hook retains and presents the structured error.
    } finally {
      if (mounted.current) setChecking(false);
    }
  };

  const retry = () => {
    void state.refresh().catch(() => undefined);
  };

  const stateNotice = recoveryError ? (
    <div className="mux-skill-notice" data-tone="recovery" role="status">
      <strong>Skills 已进入只读恢复状态</strong>
      <span>{recoveryError}</span>
    </div>
  ) : state.error && state.inventory ? (
    <div className="mux-skill-notice" data-tone="error" role="status">
      <strong>最近一次操作未完成</strong>
      <span>{state.error.message}</span>
      {state.error.retry_at && <code>可重试时间：{state.error.retry_at}</code>}
    </div>
  ) : null;
  const inventoryNotice = navigationNotice || stateNotice ? (
    <>
      {navigationNotice && (
        <div className="mux-skill-notice" data-tone="error" role="status">
          <strong>{navigationNotice}</strong>
        </div>
      )}
      {stateNotice}
    </>
  ) : null;

  return (
    <div className="mux-skill-workspace">
      <ResourceWorkspace
        sidebar={
          <WorkspaceSidebar title="Skills" count={items.length}>
            <SidebarSection title="来源">
              {sourceOptions.map((option) => (
                <SidebarItem
                  key={option.value}
                  active={source === option.value}
                  icon={option.icon}
                  label={option.label}
                  count={countWith({ source: option.value })}
                  onClick={() => changeSource(option.value)}
                />
              ))}
            </SidebarSection>
            <SidebarSection title="内容类型">
              {contentOptions.map((option) => (
                <SidebarItem
                  key={option.value}
                  active={contentKind === option.value}
                  icon={option.icon}
                  label={option.label}
                  count={countWith({ contentKind: option.value })}
                  onClick={() => changeContentKind(option.value)}
                />
              ))}
            </SidebarSection>
          </WorkspaceSidebar>
        }
        query={query}
        onQueryChange={changeQuery}
        searchPlaceholder="搜索 Skills"
        toolbarActions={
          <>
            <button
              className="btn-secondary"
              type="button"
              disabled={checkDisabled}
              onClick={() => void checkUpdates()}
            >
              <span
                className="mux-skill-check-icon"
                data-busy={checking ? "true" : undefined}
                aria-hidden="true"
              >
                <RefreshIcon className="w-4 h-4" />
              </span>
              {checking ? "检查中…" : "检查更新"}
            </button>
            <button
              className="btn-primary"
              type="button"
              disabled={checkDisabled}
              onClick={() => {
                setInstallInitialAgentId(null);
                setInstallOpen(true);
              }}
            >
              安装 Skill
            </button>
          </>
        }
        filters={
          <ResourceTabs
            label="Skill 状态"
            value={status}
            options={statusOptions.map((option) => ({
              ...option,
              count: countWith({ status: option.value }),
            }))}
            onChange={changeStatus}
          />
        }
        inspector={
          selected ? (
            <SkillInspector
              item={selected}
              detail={detail}
              agents={state.inventory?.agents ?? []}
              targets={state.inventory?.targets ?? []}
              loading={detailLoading}
              error={detailError}
              onClose={closeInspector}
              onPlan={(intent) => void planLifecycle(intent)}
              planning={lifecyclePlanning}
              readOnly={recoveryError !== null || state.pendingOperation !== null}
            />
          ) : undefined
        }
        onInspectorClose={closeInspector}
      >
        {!state.inventory && state.loading ? (
          <ResourceEmpty
            icon={<RefreshIcon className="w-6 h-6" />}
            title="正在读取 Skills…"
            detail="正在核对托管副本与 Agent 目录。"
          />
        ) : !state.inventory && recoveryError ? (
          <ResourceEmpty
            icon={<PackageIcon className="w-6 h-6" />}
            title="Skills 已进入只读恢复状态"
            detail={recoveryError}
          />
        ) : !state.inventory && state.error ? (
          <ResourceEmpty
            icon={<PackageIcon className="w-6 h-6" />}
            title="读取 Skills 失败"
            detail={state.error.message}
            action={
              <button className="btn-primary" type="button" onClick={retry}>
                重试
              </button>
            }
          />
        ) : filtered.length === 0 ? (
          <>
            {inventoryNotice}
            <ResourceEmpty
              icon={<PackageIcon className="w-6 h-6" />}
              title={items.length === 0 ? "暂无 Skills" : "没有匹配项"}
              detail={items.length === 0 ? "安装或导入后，Skills 会显示在这里。" : "调整搜索或筛选条件后重试。"}
            />
          </>
        ) : (
          <>
            {inventoryNotice}
            <ResourceGrid>
              {filtered.map((item) => (
                <SkillCard
                  key={item.identity}
                  item={item}
                  selected={item.identity === selectedIdentity}
                  onOpen={() => openSkill(item.identity)}
                />
              ))}
            </ResourceGrid>
          </>
        )}
      </ResourceWorkspace>
      {installOpen && state.inventory && !recoveryError && (
        <SkillInstallDialog
          agents={state.inventory.agents}
          initialAgentId={installInitialAgentId ?? undefined}
          commit={state.commit}
          cancel={state.cancel}
          onClose={() => {
            setInstallOpen(false);
            setInstallInitialAgentId(null);
          }}
          onCommitted={() => {
            setInstallOpen(false);
            setInstallInitialAgentId(null);
            toast.show({ kind: "success", msg: "Skill 已安装。" });
          }}
          onRecoveryRequired={enterRecovery}
        />
      )}
      {lifecycleReview && (
        <SkillReviewDialog
          plan={lifecycleReview.plan}
          assignmentContext={
            lifecycleReview.assignmentContext?.operationId ===
            lifecycleReview.plan.operation_id
              ? lifecycleReview.assignmentContext
              : undefined
          }
          onCommit={commitLifecycle}
          onClose={closeLifecycleReview}
          onCommitted={lifecycleCommitted}
          onRecoveryRequired={enterRecovery}
        />
      )}
    </div>
  );
}
