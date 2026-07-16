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
  SkillCommandError,
  SkillContentKind,
  SkillDetail,
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
import { SkillInspector } from "./SkillInspector";
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

export function SkillsView({ state }: { state: SkillsState }) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<SkillStatusFilter>("all");
  const [source, setSource] = useState<SkillSourceFilter>("all");
  const [contentKind, setContentKind] = useState<SkillContentFilter>("all");
  const [checking, setChecking] = useState(false);
  const [selectedIdentity, setSelectedIdentity] = useState<string | null>(null);
  const [detail, setDetail] = useState<SkillDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<SkillCommandError | null>(null);
  const detailGeneration = useRef(0);
  const mounted = useRef(true);
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
  const recoveryError = state.inventory?.recovery_error ?? null;
  const checkDisabled =
    checking || state.loading || state.pendingOperation !== null || recoveryError !== null;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      detailGeneration.current += 1;
    };
  }, []);

  const closeInspector = useCallback(() => {
    detailGeneration.current += 1;
    setSelectedIdentity(null);
    setDetail(null);
    setDetailLoading(false);
    setDetailError(null);
  }, []);

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

  const inventoryNotice = recoveryError ? (
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
              loading={detailLoading}
              error={detailError}
              onClose={closeInspector}
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
                  onOpen={() => setSelectedIdentity(item.identity)}
                />
              ))}
            </ResourceGrid>
          </>
        )}
      </ResourceWorkspace>
    </div>
  );
}
