import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { SourceView } from "../lib/types";
import { IconButton } from "./ui";
import { CloudIcon, FolderIcon, RefreshIcon, TrashIcon, LayersIcon, EditIcon, SearchIcon } from "./icons";
import { SubscribeDialog } from "./SubscribeDialog";
import { useToast } from "./Toast";
import { formatError } from "../lib/format";

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
 * grid) and *manages* sources (subscribe / import, plus per-source
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
  const { sources, catalog } = state;
  const toast = useToast();
  const [subscribeOpen, setSubscribeOpen] = useState(false);
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
      if (v) toast.show({ kind: "success", msg: `已导入 ${v.name} · ${v.server_count} 项` });
    } catch (e) {
      toast.show({ kind: "error", msg: "导入失败：" + formatError(e) });
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
    <aside className="mux-feature-sidebar">
      {/* Header: title + add actions */}
      <div className="flex items-center gap-1.5 px-3 pt-3.5 pb-2">
        <span className="text-xs font-semibold uppercase flex-1" style={{ color: "var(--text-secondary)", letterSpacing: "0.06em" }}>
          来源
        </span>
        <IconButton title="添加订阅" onClick={() => setSubscribeOpen(true)}>
          <CloudIcon className="w-4 h-4" />
        </IconButton>
        <IconButton title="导入配置" onClick={pickLocal}>
          <FolderIcon className="w-4 h-4" />
        </IconButton>
      </div>

      {/* List */}
      <div className="flex-1 min-h-0 overflow-y-auto px-2 pb-3 mux-noscroll">
        {/* Status filtering lives in the content toolbar; this list is sources only. */}
        <Row
          active={selectedId === null}
          icon={<LayersIcon className="w-3.5 h-3.5" />}
          name="全部"
          count={catalog.length}
          onClick={() => onSelect(null)}
        />
        <div className="my-1.5 mx-2 h-px" style={{ background: "var(--border-hairline)" }} />

        {sorted.length === 0 ? (
          <div className="text-[11px] px-3 py-4 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
            暂无来源。添加订阅或导入配置。
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
                      title={s.id === "discovered" ? "重新探索" : "刷新来源"}
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

      {subscribeOpen && (
        <SubscribeDialog
          state={state}
          onClose={() => setSubscribeOpen(false)}
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
