import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { SourceView } from "../lib/types";
import { IconButton } from "./ui";
import { CloudIcon, FolderIcon, PackageIcon, RefreshIcon, TrashIcon, LayersIcon, EditIcon, SearchIcon, DownloadIcon, CheckIcon } from "./icons";
import { SubscribeDialog, OFFICIAL_SOURCE } from "./SubscribeDialog";
import { useToast } from "./Toast";
import { formatError } from "../lib/format";
import { exportManualDialog } from "../lib/api";

type SubscribePreset = { url?: string; name?: string } | null;

/** Sentinel `selectedId` for the "生效中" filter (the deduped effective catalog).
 *  `null` = 全部 (all copies); a real source id = that source's copies. */
export const EFFECTIVE_ID = "__effective__";

/** Sort rank: remote (0) → user local (1) → managed manual/discovered (2). */
function rank(s: SourceView): number {
  if (s.kind === "remote") return 0;
  return s.managed ? 2 : 1;
}

function kindIconOf(s: SourceView) {
  if (s.kind === "remote") return <CloudIcon className="w-3.5 h-3.5" />;      // 订阅（远程 URL）
  if (s.managed) {
    // 手动添加（用户手建/编辑）vs 自动探索（扫描各 agent 发现）
    return s.id === "discovered" ? <SearchIcon className="w-3.5 h-3.5" /> : <EditIcon className="w-3.5 h-3.5" />;
  }
  return <FolderIcon className="w-3.5 h-3.5" />;                              // 本地文件
}

/**
 * Left column that both *organizes* the catalog (click a source to filter the
 * grid) and *manages* sources (subscribe / import / official, plus per-source
 * enable / refresh / remove). Replaces the old standalone 来源 page.
 */
export function SourcesSidebar({
  state,
  selectedId,
  onSelect,
}: {
  state: InstallState;
  /** null = 全部 (all sources). */
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  const { sources, entries, catalog } = state;
  const toast = useToast();
  const [subscribe, setSubscribe] = useState<SubscribePreset>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const sorted = useMemo(
    () =>
      [...sources].sort(
        (a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
      ),
    [sources]
  );

  const pickLocal = async () => {
    try {
      const v = await state.pickLocalSource();
      if (v) toast.show({ kind: "success", msg: `已导入本地来源：${v.name}（${v.server_count} 个 server）` });
    } catch (e) {
      toast.show({ kind: "error", msg: "导入失败：" + formatError(e) });
    }
  };

  const doExport = async () => {
    try {
      const path = await exportManualDialog();
      if (path) toast.show({ kind: "success", msg: `已导出手动添加的 MCP → ${path}` });
    } catch (e) {
      toast.show({ kind: "error", msg: "导出失败：" + formatError(e) });
    }
  };

  const doRefresh = async (s: SourceView) => {
    setBusyId(s.id);
    try {
      if (s.id === "discovered") {
        await state.rescanDiscovered();
        toast.show({ kind: "success", msg: "已重新探索各 Agent 配置" });
      } else {
        await state.refreshOneSource(s.id);
        toast.show({ kind: "success", msg: `已刷新：${s.name}` });
      }
    } catch (e) {
      toast.show({ kind: "error", msg: "刷新失败：" + formatError(e) });
    } finally {
      setBusyId(null);
    }
  };

  const doRemove = async (s: SourceView) => {
    if (!window.confirm(`删除来源「${s.name}」？缓存一并删除，不影响已装配置。`)) return;
    setBusyId(s.id);
    try {
      await state.deleteSource(s.id);
      if (selectedId === s.id) onSelect(null);
      toast.show({ kind: "success", msg: `已删除来源：${s.name}` });
    } catch (e) {
      toast.show({ kind: "error", msg: "删除失败：" + formatError(e) });
    } finally {
      setBusyId(null);
    }
  };

  return (
    <aside
      className="flex-shrink-0 flex flex-col h-full min-h-0"
      style={{ width: 232, borderRight: "1px solid var(--border-hairline)", background: "var(--surface-sidebar)" }}
    >
      {/* Header: title + add actions */}
      <div className="flex items-center gap-1.5 px-3 pt-3.5 pb-2">
        <span className="text-xs font-semibold uppercase flex-1" style={{ color: "var(--text-secondary)", letterSpacing: "0.06em" }}>
          来源
        </span>
        <IconButton title="订阅官方精选合集" onClick={() => setSubscribe({ url: OFFICIAL_SOURCE.url, name: OFFICIAL_SOURCE.name })}>
          <PackageIcon className="w-4 h-4" />
        </IconButton>
        <IconButton title="导入本地配置文件" onClick={pickLocal}>
          <FolderIcon className="w-4 h-4" />
        </IconButton>
        <IconButton title="导出手动添加的 MCP 为配置文件" onClick={doExport}>
          <DownloadIcon className="w-4 h-4" />
        </IconButton>
        <IconButton title="订阅远程配置 URL" onClick={() => setSubscribe({})}>
          <CloudIcon className="w-4 h-4" />
        </IconButton>
      </div>

      {/* List */}
      <div className="flex-1 min-h-0 overflow-y-auto px-2 pb-3 mux-noscroll">
        {/* 全部（所有来源的全部副本）+ 生效中（去重后胜出的） */}
        <Row
          active={selectedId === null}
          icon={<LayersIcon className="w-3.5 h-3.5" />}
          name="全部"
          count={catalog.length}
          onClick={() => onSelect(null)}
        />
        <Row
          active={selectedId === EFFECTIVE_ID}
          icon={<CheckIcon className="w-3.5 h-3.5" />}
          name="生效中"
          count={entries.length}
          onClick={() => onSelect(EFFECTIVE_ID)}
        />

        <div className="my-1.5 mx-2 h-px" style={{ background: "var(--border-hairline)" }} />

        {sorted.length === 0 ? (
          <div className="text-[11px] px-3 py-4 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
            还没有来源。用上方按钮订阅、导入或添加官方精选合集。
          </div>
        ) : (
          sorted.map((s) => (
            <Row
              key={s.id}
              active={selectedId === s.id}
              icon={kindIconOf(s)}
              name={s.name}
              count={s.server_count}
              busy={busyId === s.id}
              onClick={() => onSelect(s.id)}
              actions={
                <>
                  {(s.kind === "remote" || !s.managed || s.id === "discovered") && (
                    <IconButton
                      title={s.id === "discovered" ? "重新探索各 Agent 配置" : "刷新（重新抓取 / 读取）"}
                      onClick={() => doRefresh(s)}
                      disabled={busyId === s.id}
                    >
                      <RefreshIcon className="w-3.5 h-3.5" style={busyId === s.id ? { animation: "spin 0.8s linear infinite" } : undefined} />
                    </IconButton>
                  )}
                  {!s.managed && (
                    <IconButton title="删除来源" onClick={() => doRemove(s)} disabled={busyId === s.id}>
                      <TrashIcon className="w-3.5 h-3.5" />
                    </IconButton>
                  )}
                </>
              }
            />
          ))
        )}
      </div>

      {subscribe && (
        <SubscribeDialog
          state={state}
          initialUrl={subscribe.url}
          initialName={subscribe.name}
          onClose={() => setSubscribe(null)}
        />
      )}
    </aside>
  );
}

/** One source row: clickable body (selects/filters) with a trailing toggle and
 *  hover-revealed refresh/remove actions. */
function Row({
  active,
  icon,
  name,
  count,
  dimmed,
  busy,
  onClick,
  toggle,
  actions,
}: {
  active: boolean;
  icon: React.ReactNode;
  name: string;
  count: number;
  dimmed?: boolean;
  busy?: boolean;
  onClick: () => void;
  toggle?: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <div
      className="mux-src-row group flex items-center gap-2 px-2.5 rounded-mac cursor-pointer"
      data-active={active ? "true" : undefined}
      style={{ opacity: dimmed ? 0.5 : 1 }}
      onClick={onClick}
      title={name}
    >
      <span className="flex-shrink-0" style={{ color: active ? "var(--color-blue)" : "var(--text-secondary)" }}>
        {icon}
      </span>
      <span className="text-[13px] truncate flex-1" style={{ color: "var(--text-primary)", fontWeight: active ? 600 : 400 }}>
        {name}
      </span>
      {/* count — hidden while hovering to make room for actions */}
      <span
        className={`text-[11px] tabular-nums flex-shrink-0 ${actions ? "group-hover:hidden" : ""}`}
        style={{ color: "var(--text-secondary)" }}
      >
        {count}
      </span>
      {actions && (
        <span className="hidden group-hover:flex items-center gap-0.5 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
          {actions}
        </span>
      )}
      {toggle && (
        <span className="flex-shrink-0" onClick={(e) => e.stopPropagation()}>
          {toggle}
        </span>
      )}
      {busy && !actions && <span />}
    </div>
  );
}
