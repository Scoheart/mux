import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { SourceView } from "../lib/types";
import { IconButton } from "./ui";
import { CheckIcon, CloudIcon, FolderIcon, RefreshIcon, TrashIcon, LayersIcon, EditIcon, SearchIcon, XIcon } from "./icons";
import { SubscribeDialog } from "./SubscribeDialog";
import { useToast } from "./Toast";
import { formatError } from "../lib/format";
import { SidebarItem, SidebarSection, WorkspaceSidebar } from "./ResourceWorkspace";

export type McpStatusFilter = "all" | "used" | "unused" | "shadowed";
export type McpStatusCounts = Record<McpStatusFilter, number>;

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
  statusFilter,
  statusCounts,
  onStatusFilter,
}: {
  state: InstallState;
  /** null = 全部 (all sources). */
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  statusFilter: McpStatusFilter;
  statusCounts: McpStatusCounts;
  onStatusFilter: (filter: McpStatusFilter) => void;
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
    <WorkspaceSidebar title="MCPs" count={catalog.length}>
      <SidebarSection title="状态">
        <SidebarItem
          active={statusFilter === "all"}
          icon={<LayersIcon className="w-3.5 h-3.5" />}
          label="全部"
          count={statusCounts.all}
          onClick={() => onStatusFilter("all")}
        />
        <SidebarItem
          active={statusFilter === "used"}
          icon={<CheckIcon className="w-3.5 h-3.5" />}
          label="使用中"
          count={statusCounts.used}
          onClick={() => onStatusFilter("used")}
        />
        <SidebarItem
          active={statusFilter === "unused"}
          icon={<XIcon className="w-3.5 h-3.5" />}
          label="未使用"
          count={statusCounts.unused}
          onClick={() => onStatusFilter("unused")}
        />
        {statusCounts.shadowed > 0 && (
          <SidebarItem
            active={statusFilter === "shadowed"}
            icon={<LayersIcon className="w-3.5 h-3.5" />}
            label="被覆盖"
            count={statusCounts.shadowed}
            onClick={() => onStatusFilter("shadowed")}
          />
        )}
      </SidebarSection>

      <SidebarSection
        title="来源"
        actions={
          <>
            <IconButton title="添加订阅" onClick={() => setSubscribeOpen(true)}>
              <CloudIcon className="w-4 h-4" />
            </IconButton>
            <IconButton title="导入配置" onClick={pickLocal}>
              <FolderIcon className="w-4 h-4" />
            </IconButton>
          </>
        }
      >
        <SidebarItem
          active={selectedId === null}
          icon={<LayersIcon className="w-3.5 h-3.5" />}
          label="全部来源"
          count={catalog.length}
          onClick={() => onSelect(null)}
        />
        {sorted.length === 0 ? (
          <div className="mux-sidebar-empty">暂无来源</div>
        ) : (
          sorted.map((source) => (
            <SidebarItem
              key={source.id}
              active={selectedId === source.id}
              icon={kindIconOf(source)}
              label={source.name}
              count={source.server_count}
              onClick={() => onSelect(source.id)}
              actions={
                <>
                  {(source.kind === "remote" || !source.managed || source.id === "discovered") && (
                    <IconButton
                      title={source.id === "discovered" ? "重新探索" : "刷新来源"}
                      onClick={() => doRefresh(source)}
                      disabled={busyId === source.id}
                    >
                      <RefreshIcon className="w-3.5 h-3.5" style={busyId === source.id ? { animation: "spin 0.8s linear infinite" } : undefined} />
                    </IconButton>
                  )}
                  {!source.managed && (
                    <IconButton title="删除来源" onClick={() => doRemove(source)} disabled={busyId === source.id}>
                      <TrashIcon className="w-3.5 h-3.5" />
                    </IconButton>
                  )}
                </>
              }
            />
          ))
        )}
      </SidebarSection>

      {subscribeOpen && (
        <SubscribeDialog
          state={state}
          onClose={() => setSubscribeOpen(false)}
        />
      )}
    </WorkspaceSidebar>
  );
}
