import { useState, useMemo, useCallback, useEffect, useRef } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry, RegistryOrigin, CatalogItem, ResourceNavigationIntent } from "../lib/types";
import { keyOf, transportOf, type Transport } from "../lib/mcp";
import { exportEffectiveDialog, forgetEntry } from "../lib/api";
import { formatError } from "../lib/format";
import { redactSensitiveConfig } from "../lib/resourceWorkspace";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SourcesSidebar } from "./SourcesSidebar";
import { AgentGlyph, agentName } from "./brandIcons";
import {
  CopyIcon,
  EditIcon,
  PlusIcon,
  LinkIcon,
  TerminalIcon,
  CloudIcon,
  DownloadIcon,
  FolderIcon,
  LayersIcon,
  PackageIcon,
  TrashIcon,
} from "./icons";
import { Avatar, Badge, IconButton, TransportPill } from "./ui";
import { ResourceCard } from "./ResourceCard";
import { ResourceState } from "./ResourceState";
import { ReviewDialog } from "./ReviewDialog";
import { useToast } from "./Toast";
import { PasteConfigDialog } from "./PasteConfigDialog";
import {
  AgentStack,
  InspectorField,
  InspectorSection,
  ResourceGrid,
  ResourceInspector,
  ResourceTabs,
  ResourceWorkspace,
} from "./ResourceWorkspace";

interface RegistryViewProps {
  state: InstallState;
  intent?: Extract<ResourceNavigationIntent, { domain: "mcp" }>;
  onIntentConsumed?(id: number): void;
  onEdit: (name: string, transport: Transport) => void;
  onCreate: () => void;
}

/** Origin buckets — still used to decide which entries are user-deletable. */
type OriginBucket = "remote" | "local" | "manual" | "discovered";
type McpStatusFilter = "all" | "used" | "unused" | "shadowed";
type McpStatusCounts = Record<McpStatusFilter, number>;
/** Classify an entry's origin into a bucket. Entries with no origin, or a
 *  legacy/unknown kind, fall into "discovered" (scanned-from-machine). */
function bucketOf(entry: RegistryEntry): OriginBucket {
  const k = entry.origin?.kind;
  if (k === "remote") return "remote";
  if (k === "local") return "local";
  if (k === "manual") return "manual";
  return "discovered";
}

/** User-owned entries (手动添加 / 自动探索) can be edited and deleted here.
 *  Remote/local subscription entries belong to a source and are read-only — edit
 *  them at their upstream, or override via a new manual entry. */
function isUserOwned(entry: RegistryEntry): boolean {
  const b = bucketOf(entry);
  return b === "manual" || b === "discovered";
}

/** Does `entry` belong to the sidebar-selected source? Managed sources match by
 *  origin kind ("manual" / "discovered"); remote/local match by origin.source id. */
function inSource(entry: RegistryEntry, sourceId: string): boolean {
  if (sourceId === "manual") return entry.origin?.kind === "manual";
  if (sourceId === "discovered") return entry.origin?.kind === "discovered";
  return entry.origin?.source === sourceId;
}

function endpointOf(entry: RegistryEntry): { text: string; link: boolean } {
  if (entry.config.http?.url) return { text: entry.config.http.url, link: true };
  if (entry.config.stdio) {
    const { command, args } = entry.config.stdio;
    return { text: [command, ...(args ?? [])].join(" "), link: false };
  }
  return { text: entry.description || "—", link: false };
}

/** Provenance indicator: 订阅:X (remote source) / 本地:X (local source) / 手动添加 /
 *  来自 {agent} (discovered). There is no built-in bucket anymore. */
function OriginTag({
  entry,
  installedAgents,
  sourceName,
}: {
  entry: RegistryEntry;
  installedAgents: string[];
  sourceName: (id: string) => string;
}) {
  const origin = entry.origin;
  if (origin?.kind === "remote") {
    return (
      <span className="inline-flex items-center gap-1 min-w-0" title={`订阅：${origin.source ? sourceName(origin.source) : ""}`}>
        <CloudIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {origin.source ? sourceName(origin.source) : "订阅"}
        </span>
      </span>
    );
  }
  if (origin?.kind === "local") {
    return (
      <span className="inline-flex items-center gap-1 min-w-0" title={`本地：${origin.source ? sourceName(origin.source) : ""}`}>
        <FolderIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {origin.source ? sourceName(origin.source) : "本地"}
        </span>
      </span>
    );
  }
  if (origin?.kind === "manual") return <Badge tone="info">手动</Badge>;
  // discovered (recorded) or legacy custom: show the source app when known.
  const agent = origin?.agent ?? installedAgents[0];
  if (agent) {
    return (
      <span className="inline-flex items-center gap-1 min-w-0" title={`来自 ${agentName(agent)}`}>
        <span className="flex-shrink-0 inline-flex"><AgentGlyph id={agent} size={16} /></span>
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {agentName(agent)}
        </span>
      </span>
    );
  }
  return <Badge tone="neutral">探索</Badge>;
}

/** Short human label for an origin, for the "被『X』覆盖" tooltip. */
function originLabel(origin: RegistryOrigin | undefined, sourceName: (id: string) => string): string {
  if (!origin) return "其它来源";
  if (origin.kind === "manual") return "手动添加";
  if (origin.kind === "discovered") return origin.agent ? agentName(origin.agent) : "自动探索";
  const label = origin.source ? sourceName(origin.source) : "";
  return label || (origin.kind === "remote" ? "订阅" : "本地");
}

export function RegistryView({ state, intent, onIntentConsumed, onEdit, onCreate }: RegistryViewProps) {
  const { catalog, entries, agentsForServer, sources } = state;
  const toast = useToast();

  const [q, setQ] = useState("");
  // Source and status are separate filters: the sidebar owns provenance, while
  // status stays visible above the grid.
  const [selectedSource, setSelectedSource] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<McpStatusFilter>("all");
  const [detail, setDetail] = useState<CatalogItem | null>(null);
  const [pasteOpen, setPasteOpen] = useState(false);
  const [deleteReview, setDeleteReview] = useState<RegistryEntry | null>(null);
  const lastConsumedIntentId = useRef<number | null>(null);

  const sourceName = useCallback(
    (id: string) => sources.find((s) => s.id === id)?.name ?? id,
    [sources]
  );

  // For each composite key, the origin of the in-effect (winning) copy — used to
  // tell an overridden card which source actually takes effect.
  const winningOriginByKey = useMemo(() => {
    const m = new Map<string, RegistryOrigin | undefined>();
    for (const item of catalog) {
      if (item.in_effect) m.set(keyOf(item.entry), item.entry.origin);
    }
    return m;
  }, [catalog]);

  const sourceScoped = useMemo(() => {
    if (selectedSource === null) return catalog;
    return catalog.filter((item) => inSource(item.entry, selectedSource));
  }, [catalog, selectedSource]);

  const statusCounts = useMemo<McpStatusCounts>(() => {
    let used = 0;
    let unused = 0;
    let shadowed = 0;
    for (const item of sourceScoped) {
      if (!item.in_effect) shadowed += 1;
      if (agentsForServer(keyOf(item.entry)).length > 0 && item.in_effect) used += 1;
      else if (item.in_effect) unused += 1;
    }
    return { all: sourceScoped.length, used, unused, shadowed };
  }, [agentsForServer, sourceScoped]);

  const scoped = useMemo(() => {
    const s = q.trim().toLowerCase();
    let list = sourceScoped;
    if (s)
      list = list.filter(
        (it) => it.entry.name.toLowerCase().includes(s) || it.entry.description.toLowerCase().includes(s)
      );
    // Alphabetical by name, then transport; in-effect copy first within a group.
    return [...list].sort(
      (a, b) =>
        a.entry.name.localeCompare(b.entry.name, undefined, { sensitivity: "base" }) ||
        transportOf(a.entry).localeCompare(transportOf(b.entry)) ||
        Number(b.in_effect) - Number(a.in_effect)
    );
  }, [q, sourceScoped]);

  const filtered = useMemo(() => {
    if (statusFilter === "shadowed") return scoped.filter((item) => !item.in_effect);
    if (statusFilter === "used") {
      return scoped.filter(
        (item) => item.in_effect && agentsForServer(keyOf(item.entry)).length > 0
      );
    }
    if (statusFilter === "unused") {
      return scoped.filter(
        (item) => item.in_effect && agentsForServer(keyOf(item.entry)).length === 0
      );
    }
    return scoped;
  }, [agentsForServer, scoped, statusFilter]);

  useEffect(() => {
    if (!intent || state.loading || lastConsumedIntentId.current === intent.id) return;
    lastConsumedIntentId.current = intent.id;
    if (intent.kind === "create") {
      setDetail(null);
      onCreate();
      onIntentConsumed?.(intent.id);
      return;
    }
    const item = catalog.find(
      (candidate) =>
        candidate.entry.name === intent.name &&
        transportOf(candidate.entry) === intent.transport &&
        candidate.in_effect,
    ) ?? catalog.find(
      (candidate) => candidate.entry.name === intent.name && transportOf(candidate.entry) === intent.transport,
    );
    setQ("");
    setSelectedSource(null);
    setStatusFilter("all");
    setDetail(item ?? null);
    if (!item) toast.show({ kind: "error", msg: `未找到 MCP“${intent.name}”。` });
    onIntentConsumed?.(intent.id);
  }, [catalog, intent, onCreate, onIntentConsumed, state.loading, toast]);

  const changeQuery = (value: string) => {
    setDetail(null);
    setQ(value);
  };

  const changeSource = (sourceId: string | null) => {
    setDetail(null);
    setSelectedSource(sourceId);
  };

  const changeStatus = (status: McpStatusFilter) => {
    setDetail(null);
    setStatusFilter(status);
  };

  const copyConfig = useCallback(
    (entry: RegistryEntry) => {
      navigator.clipboard
        .writeText(JSON.stringify(entry.config, null, 2))
        .then(() => toast.show({ kind: "success", msg: `已复制 ${entry.name} 配置` }))
        .catch(() => toast.show({ kind: "error", msg: "复制失败" }));
    },
    [toast]
  );

  const doExport = useCallback(async () => {
    try {
      const path = await exportEffectiveDialog();
      if (path) toast.show({ kind: "success", msg: `已导出 ${entries.length} 项 → ${path}` });
    } catch (e) {
      toast.show({ kind: "error", msg: "导出失败：" + formatError(e) });
    }
  }, [entries.length, toast]);

  // Only user-owned entries (手动添加 / 探索) can be edited/deleted; subscription/
  // local entries belong to a source and are managed on the 来源 page.
  const deletable = useCallback((entry: RegistryEntry) => isUserOwned(entry), []);
  const editable = useCallback((entry: RegistryEntry) => isUserOwned(entry), []);

  const deleteEntry = useCallback(
    async (entry: RegistryEntry) => {
      if (!deletable(entry)) return;
      const t = transportOf(entry);
      try {
        await forgetEntry(entry.name, t);
        await Promise.all([state.refreshRegistry(), state.rescan()]);
        setDetail(null);
        setDeleteReview(null);
        toast.show({ kind: "success", msg: `已删除 ${entry.name}` });
      } catch (e) {
        toast.show({ kind: "error", msg: `删除失败：${String(e)}` });
        throw e;
      }
    },
    [deletable, state, toast]
  );

  return (
    <ResourceWorkspace
      sidebar={
        <SourcesSidebar
          state={state}
          selectedId={selectedSource}
          onSelect={changeSource}
        />
      }
      query={q}
      onQueryChange={changeQuery}
      searchPlaceholder="搜索 MCP"
      filters={
        <ResourceTabs
          label="MCP 状态"
          value={statusFilter}
          options={[
            { value: "all", label: "全部", count: statusCounts.all },
            { value: "used", label: "使用中", count: statusCounts.used },
            { value: "unused", label: "未使用", count: statusCounts.unused },
            { value: "shadowed", label: "被覆盖", count: statusCounts.shadowed },
          ]}
          onChange={changeStatus}
        />
      }
      toolbarActions={
        <>
          <button
            onClick={() => {
              setDetail(null);
              setPasteOpen(true);
            }}
            className="btn-ghost"
            title="粘贴 MCP 配置"
          >
            粘贴配置
          </button>
          <IconButton title="导出生效配置" onClick={doExport} disabled={entries.length === 0}>
            <DownloadIcon className="w-4 h-4" />
          </IconButton>
          <button
            onClick={() => {
              setDetail(null);
              onCreate();
            }}
            className="btn-primary"
          >
            <PlusIcon className="w-4 h-4" />
            新建 MCP
          </button>
        </>
      }
      inspector={
        detail ? (
          <RegistryDetail
            entry={detail.entry}
            overriddenBy={
              detail.in_effect
                ? undefined
                : originLabel(winningOriginByKey.get(keyOf(detail.entry)), sourceName)
            }
            installedAgents={agentsForServer(keyOf(detail.entry))}
            sourceName={sourceName}
            onClose={() => setDetail(null)}
            onCopy={() => copyConfig(detail.entry)}
            onEdit={
              editable(detail.entry)
                ? () => {
                    const { name } = detail.entry;
                    const transport = transportOf(detail.entry);
                    setDetail(null);
                    onEdit(name, transport);
                  }
                : undefined
            }
            onDelete={deletable(detail.entry) ? () => setDeleteReview(detail.entry) : undefined}
          />
        ) : undefined
      }
      onInspectorClose={() => setDetail(null)}
    >
      {filtered.length === 0 ? (
        <ResourceState
          kind={catalog.length === 0 ? "empty" : "no-match"}
          icon={<PackageIcon className="w-6 h-6" />}
          title={catalog.length === 0 ? "暂无 MCP" : "没有匹配项"}
          detail={catalog.length === 0 ? "添加订阅、导入配置或新建 MCP" : "调整搜索、来源或状态筛选后重试。"}
          action={catalog.length === 0 ? undefined : (
            <button type="button" className="btn-secondary" onClick={() => {
              setQ("");
              setSelectedSource(null);
              setStatusFilter("all");
            }}>清除筛选</button>
          )}
        />
      ) : (
        <ResourceGrid>
          {filtered.map((item) => (
            <RegistryCard
              key={`${keyOf(item.entry)}@${item.entry.origin?.source ?? item.entry.origin?.kind ?? ""}`}
              item={item}
              selected={detail === item}
              installedAgents={agentsForServer(keyOf(item.entry))}
              sourceName={sourceName}
              overriddenBy={
                item.in_effect
                  ? undefined
                  : originLabel(winningOriginByKey.get(keyOf(item.entry)), sourceName)
              }
              onOpen={() => setDetail(item)}
            />
          ))}
        </ResourceGrid>
      )}

      {pasteOpen && <PasteConfigDialog state={state} onClose={() => setPasteOpen(false)} />}
      {deleteReview && (
        <ReviewDialog
          title="删除 MCP"
          subtitle={`${deleteReview.name} · ${transportOf(deleteReview)}`}
          confirmLabel="删除 MCP"
          onClose={() => setDeleteReview(null)}
          onConfirm={() => deleteEntry(deleteReview)}
        >
          <p>将从目录移除并从所有关联 Agent 卸载。写入前会创建备份，Agent 的其他配置保持不变。</p>
        </ReviewDialog>
      )}
    </ResourceWorkspace>
  );
}

/** Default entries stay visually quiet; only a shadowed copy carries state UI. */
function RegistryCard({
  item,
  selected,
  installedAgents,
  sourceName,
  overriddenBy,
  onOpen,
}: {
  item: CatalogItem;
  selected: boolean;
  installedAgents: string[];
  sourceName: (id: string) => string;
  /** Label of the source that takes effect instead — presence marks this copy as shadowed. */
  overriddenBy?: string;
  onOpen: () => void;
}) {
  const { entry } = item;
  const ep = endpointOf(entry);
  const overridden = !!overriddenBy;

  return (
    <ResourceCard
      selected={selected}
      attention={overridden ? "shadowed" : undefined}
      ariaLabel={`打开 MCP ${entry.name} 详情`}
      onOpen={onOpen}
      identity={
        <>
        <span className="mux-card-avatar flex-shrink-0">
          <Avatar seed={entry.name} size={34} />
        </span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <span
              className="text-[13px] font-semibold truncate leading-tight"
              style={{ color: "var(--text-primary)" }}
              title={entry.name}
            >
              {entry.name}
            </span>
          </div>
          <div className="flex items-center gap-1.5 mt-1 min-w-0">
            <TransportPill entry={entry} />
            <OriginTag entry={entry} installedAgents={installedAgents} sourceName={sourceName} />
          </div>
        </div>
        </>
      }
      configuration={
        <div className="mux-resource-endpoint">
        {ep.link ? (
          <LinkIcon className="w-3 h-3 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
        ) : (
          <TerminalIcon className="w-3 h-3 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
        )}
        <span style={{ color: ep.link ? "var(--color-blue)" : undefined }} title={ep.text}>
          {ep.text}
        </span>
        </div>
      }
      state={
        <>
          {overridden ? (
            <Badge tone="warning"><LayersIcon className="w-3 h-3" />被覆盖</Badge>
          ) : (
            <Badge tone="success">生效中</Badge>
          )}
          {installedAgents.length > 0 ? (
            <Badge tone="info">使用中</Badge>
          ) : (
            <Badge tone="neutral">未使用</Badge>
          )}
        </>
      }
      impact={
        <>
          <AgentStack ids={installedAgents} />
          {overridden && (
          <span
            className="mux-shadowed-source ml-auto min-w-0 truncate text-[10px]"
            title={`当前使用「${overriddenBy}」的配置`}
          >
            以 {overriddenBy} 为准
          </span>
          )}
        </>
      }
    />
  );
}

function RegistryDetail({
  entry,
  overriddenBy,
  installedAgents,
  sourceName,
  onClose,
  onCopy,
  onEdit,
  onDelete,
}: {
  entry: RegistryEntry;
  overriddenBy?: string;
  installedAgents: string[];
  sourceName: (id: string) => string;
  onClose: () => void;
  onCopy: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  const endpoint = endpointOf(entry);
  return (
    <ResourceInspector
      title={entry.name}
      avatar={<Avatar seed={entry.name} size={40} />}
      subtitle={
        <div className="flex items-center gap-1.5">
          <TransportPill entry={entry} />
          <OriginTag entry={entry} installedAgents={installedAgents} sourceName={sourceName} />
        </div>
      }
      onClose={onClose}
      footer={
        <>
          {onDelete && (
            <button onClick={onDelete} className="btn-danger" title="删除条目并从所有 Agent 卸载">
              <TrashIcon className="w-4 h-4" />
              删除
            </button>
          )}
          <div className="flex-1" />
          <button onClick={onCopy} className="btn-ghost">
            <CopyIcon className="w-4 h-4" />
            复制
          </button>
          {onEdit && (
            <button onClick={onEdit} className="btn-primary">
              <EditIcon className="w-4 h-4" />
              编辑
            </button>
          )}
        </>
      }
    >
      {overriddenBy && (
        <div className="mux-detail-warning">
          <LayersIcon className="w-4 h-4 flex-shrink-0" />
          <div className="min-w-0">
            <div className="text-xs font-semibold">已被覆盖</div>
            <div className="text-[11px] mt-0.5 leading-relaxed">
              当前使用「{overriddenBy}」，此副本不参与安装。
            </div>
          </div>
        </div>
      )}

      {entry.description && <p className="mux-inspector-description">{entry.description}</p>}

      <InspectorSection title="连接">
        <InspectorField label="地址" mono>{endpoint.text}</InspectorField>
        <InspectorField label="Agent"><AgentStack ids={installedAgents} /></InspectorField>
        {entry.repo && (
          <InspectorField label="主页">
            <button onClick={() => openUrl(entry.repo!)} className="mux-inline-link" title="在浏览器中打开">
              <LinkIcon className="w-3.5 h-3.5" />
              {entry.repo}
            </button>
          </InspectorField>
        )}
      </InspectorSection>

      {entry.tags.length > 0 && (
        <InspectorSection title="标签">
          <div className="flex flex-wrap gap-1.5">
            {entry.tags.map((tag) => <Badge key={tag} tone="info">{tag}</Badge>)}
          </div>
        </InspectorSection>
      )}

      <InspectorSection title="配置">
        <pre className="mux-config-preview">
          {JSON.stringify(redactSensitiveConfig(entry.config), null, 2)}
        </pre>
      </InspectorSection>
    </ResourceInspector>
  );
}
