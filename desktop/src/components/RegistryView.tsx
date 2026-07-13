import { useState, useMemo, useCallback } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry, RegistryOrigin, CatalogItem } from "../lib/types";
import { keyOf, transportOf, type Transport } from "../lib/mcp";
import { exportEffectiveDialog, forgetEntry } from "../lib/api";
import { formatError } from "../lib/format";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SourcesSidebar } from "./SourcesSidebar";
import { AgentGlyph, agentName } from "./brandIcons";
import {
  CopyIcon,
  EditIcon,
  PlusIcon,
  LinkIcon,
  TerminalIcon,
  XIcon,
  CloudIcon,
  DownloadIcon,
  FolderIcon,
  LayersIcon,
  TrashIcon,
} from "./icons";
import { Avatar, Badge, IconButton, SearchBar, Modal, TransportPill, stickyHeaderStyle } from "./ui";
import { useToast } from "./Toast";
import { PasteConfigDialog } from "./PasteConfigDialog";

interface RegistryViewProps {
  state: InstallState;
  onEdit: (name: string, transport: Transport) => void;
  onCreate: () => void;
}

/** Origin buckets — still used to decide which entries are user-deletable. */
type OriginBucket = "remote" | "local" | "manual" | "discovered";
type StatusFilter = "all" | "shadowed";

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

export function RegistryView({ state, onEdit, onCreate }: RegistryViewProps) {
  const { catalog, entries, agentsForServer, sources } = state;
  const toast = useToast();

  const [q, setQ] = useState("");
  // Source and status are separate filters: the sidebar owns provenance, while
  // the compact status control appears only when duplicate copies need review.
  const [selectedSource, setSelectedSource] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [detail, setDetail] = useState<CatalogItem | null>(null);
  const [pasteOpen, setPasteOpen] = useState(false);

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

  const scoped = useMemo(() => {
    const s = q.trim().toLowerCase();
    let list = catalog;
    if (selectedSource !== null) list = list.filter((it) => inSource(it.entry, selectedSource));
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
  }, [catalog, q, selectedSource]);

  const shadowedCount = useMemo(
    () => scoped.filter((item) => !item.in_effect).length,
    [scoped]
  );

  const filtered = useMemo(() => {
    if (statusFilter === "shadowed") return scoped.filter((item) => !item.in_effect);
    return scoped;
  }, [scoped, statusFilter]);

  const showShadowedFilter = shadowedCount > 0 || statusFilter === "shadowed";

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
      if (
        !window.confirm(
          `删除「${entry.name}」（${t}）？将从目录移除并从所有 agent 卸载（有备份）。`
        )
      )
        return;
      try {
        await forgetEntry(entry.name, t);
        await Promise.all([state.refreshRegistry(), state.rescan()]);
        setDetail(null);
        toast.show({ kind: "success", msg: `已删除 ${entry.name}` });
      } catch (e) {
        toast.show({ kind: "error", msg: `删除失败：${String(e)}` });
      }
    },
    [deletable, state, toast]
  );

  return (
    <div className="flex h-full min-h-0">
      <SourcesSidebar
        state={state}
        selectedId={selectedSource}
        onSelect={(id) => {
          setSelectedSource(id);
          setStatusFilter("all");
        }}
      />

      <div className="flex-1 min-w-0 min-h-0 overflow-y-auto">
      {/* Sticky header: search/actions + catalog-state filter. */}
      <div className="sticky top-0 z-10 px-6 py-3" style={stickyHeaderStyle}>
        <div className="max-w-[1280px] mx-auto">
          <div className="flex items-center gap-3">
            <div className="flex-1 min-w-[160px]">
              <SearchBar value={q} onChange={setQ} placeholder="搜索 MCP…" />
            </div>
            {showShadowedFilter && (
              <button
                type="button"
                className="mux-shadowed-filter flex-shrink-0"
                data-active={statusFilter === "shadowed" ? "true" : undefined}
                aria-pressed={statusFilter === "shadowed"}
                title={statusFilter === "shadowed" ? "显示全部配置" : "只看被覆盖配置"}
                onClick={() => setStatusFilter(statusFilter === "shadowed" ? "all" : "shadowed")}
              >
                <LayersIcon className="w-3.5 h-3.5" />
                <span>被覆盖</span>
                <span className="tabular-nums opacity-70">{shadowedCount}</span>
              </button>
            )}
            <button
              onClick={() => setPasteOpen(true)}
              className="btn-ghost flex-shrink-0"
              title="粘贴 MCP 配置"
            >
              粘贴配置
            </button>
            <IconButton title="导出生效配置" onClick={doExport} disabled={entries.length === 0}>
              <DownloadIcon className="w-4 h-4" />
            </IconButton>
            <button onClick={onCreate} className="btn-primary flex-shrink-0">
              <PlusIcon className="w-4 h-4" />
              新建 MCP
            </button>
          </div>

        </div>
      </div>

      <div className="max-w-[1280px] mx-auto px-6 pt-5 pb-8">
        {filtered.length === 0 ? (
          <div className="py-16 text-sm text-center" style={{ color: "var(--text-secondary)" }}>
            {catalog.length === 0
              ? "暂无配置，请从左侧添加来源。"
              : statusFilter === "shadowed"
                ? "没有被覆盖项"
                : "没有匹配项"}
          </div>
        ) : (
          <div className="mux-grid">
            {filtered.map((item) => (
              <RegistryCard
                key={`${keyOf(item.entry)}@${item.entry.origin?.source ?? item.entry.origin?.kind ?? ""}`}
                item={item}
                installedAgents={agentsForServer(keyOf(item.entry))}
                sourceName={sourceName}
                overriddenBy={
                  item.in_effect
                    ? undefined
                    : originLabel(winningOriginByKey.get(keyOf(item.entry)), sourceName)
                }
                editable={editable(item.entry)}
                deletable={deletable(item.entry)}
                onOpen={() => setDetail(item)}
                onCopy={() => copyConfig(item.entry)}
                onEdit={() => onEdit(item.entry.name, transportOf(item.entry))}
                onDelete={() => deleteEntry(item.entry)}
              />
            ))}
          </div>
        )}
      </div>

      {pasteOpen && <PasteConfigDialog state={state} onClose={() => setPasteOpen(false)} />}

      {detail && (
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
          onDelete={deletable(detail.entry) ? () => deleteEntry(detail.entry) : undefined}
        />
      )}
      </div>
    </div>
  );
}

/** Default entries stay visually quiet; only a shadowed copy carries state UI. */
function RegistryCard({
  item,
  installedAgents,
  sourceName,
  overriddenBy,
  editable,
  deletable,
  onOpen,
  onCopy,
  onEdit,
  onDelete,
}: {
  item: CatalogItem;
  installedAgents: string[];
  sourceName: (id: string) => string;
  /** Label of the source that takes effect instead — presence marks this copy as shadowed. */
  overriddenBy?: string;
  editable: boolean;
  deletable: boolean;
  onOpen: () => void;
  onCopy: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { entry } = item;
  const usedBy = installedAgents.length;
  const ep = endpointOf(entry);
  const overridden = !!overriddenBy;

  return (
    <div
      className="mux-tile p-3"
      data-state={overridden ? "shadowed" : undefined}
      onClick={onOpen}
    >
      {/* Header: identity, provenance, and an explicit catalog state. */}
      <div className="flex items-start gap-2.5">
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
            {overridden && (
              <span
                className="mux-state-badge"
                data-state="shadowed"
                title={`已被覆盖：当前以「${overriddenBy}」为准`}
              >
                <LayersIcon className="w-3 h-3" />
                被覆盖
              </span>
            )}
          </div>
          <div className="flex items-center gap-1.5 mt-1 min-w-0">
            <TransportPill entry={entry} />
            <OriginTag entry={entry} installedAgents={installedAgents} sourceName={sourceName} />
          </div>
        </div>
      </div>

      {/* Endpoint as an inset code strip */}
      <div
        className="flex items-center gap-1.5 mt-2.5 px-2 py-1.5 rounded-mac min-w-0"
        style={{ background: "var(--surface-app)", border: "1px solid var(--border-hairline)" }}
      >
        {ep.link ? (
          <LinkIcon className="w-3 h-3 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
        ) : (
          <TerminalIcon className="w-3 h-3 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
        )}
        <span
          className="text-[11px] truncate"
          style={{ color: ep.link ? "var(--color-blue)" : "var(--text-secondary)", fontFamily: "var(--font-mono)" }}
          title={ep.text}
        >
          {ep.text}
        </span>
      </div>

      {/* Footer: usage dot + hover actions — pinned to the card bottom so every
          card in a row lines up (grid stretches them to equal height). */}
      <div className="flex items-center justify-between mt-3 pt-2.5" style={{ borderTop: "1px solid var(--border-hairline)" }}>
        {overridden ? (
          <span
            className="mux-shadowed-source min-w-0 truncate text-[11px]"
            title={`当前使用「${overriddenBy}」的配置`}
          >
            以 {overriddenBy} 为准
          </span>
        ) : (
          <span
            className="inline-flex items-center gap-1.5 text-[11px]"
            style={{ color: usedBy > 0 ? "var(--color-green)" : "var(--text-secondary)" }}
          >
            <span
              className="w-1.5 h-1.5 rounded-full flex-shrink-0"
              style={{ background: usedBy > 0 ? "var(--color-green)" : "var(--text-secondary)", opacity: usedBy > 0 ? 1 : 0.4 }}
            />
            {usedBy > 0 ? `${usedBy} 个 Agent` : "未使用"}
          </span>
        )}

        <div
          className="mux-toolbar flex items-center gap-0.5 rounded-mac px-0.5"
          style={{ background: "var(--surface-raised)" }}
          onClick={(e) => e.stopPropagation()}
        >
          {entry.repo && (
            <IconButton title={`打开仓库：${entry.repo}`} onClick={() => openUrl(entry.repo!)}>
              <LinkIcon className="w-4 h-4" />
            </IconButton>
          )}
          <IconButton title="复制配置 JSON" onClick={onCopy}>
            <CopyIcon className="w-4 h-4" />
          </IconButton>
          {editable && (
            <IconButton title="编辑配置" onClick={onEdit}>
              <EditIcon className="w-4 h-4" />
            </IconButton>
          )}
          {deletable && (
            <IconButton title="删除条目（并从所有 agent 卸载）" onClick={onDelete}>
              <TrashIcon className="w-4 h-4" />
            </IconButton>
          )}
        </div>
      </div>
    </div>
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
  return (
    <Modal width={560} onClose={onClose}>
        <div className="flex items-center gap-3 px-6 py-5" style={{ borderBottom: "1px solid var(--border-hairline)" }}>
          <Avatar seed={entry.name} size={40} />
          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold m-0 truncate" style={{ color: "var(--text-primary)" }}>
              {entry.name}
            </h2>
            <div className="flex items-center gap-1.5 mt-1">
              <TransportPill entry={entry} />
              <OriginTag entry={entry} installedAgents={installedAgents} sourceName={sourceName} />
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="关闭详情"
            title="关闭"
            className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer"
            style={{ background: "var(--border-hairline)", color: "var(--text-secondary)" }}
          >
            <XIcon className="w-3.5 h-3.5" />
          </button>
        </div>

        {overriddenBy && (
          <div className="px-6 pt-4">
            <div className="mux-detail-warning">
              <LayersIcon className="w-4 h-4 flex-shrink-0" />
              <div className="min-w-0">
                <div className="text-xs font-semibold">已被覆盖</div>
                <div className="text-[11px] mt-0.5 leading-relaxed">
                  当前使用「{overriddenBy}」，此副本不参与安装。
                </div>
              </div>
            </div>
          </div>
        )}

        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
          {entry.description && (
            <p className="text-sm leading-relaxed m-0" style={{ color: "var(--text-secondary)" }}>
              {entry.description}
            </p>
          )}
          {entry.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {entry.tags.map((t) => (
                <Badge key={t} tone="info">{t}</Badge>
              ))}
            </div>
          )}
          {entry.repo && (
            <div>
              <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
                仓库 / 主页
              </label>
              <button
                onClick={() => openUrl(entry.repo!)}
                className="inline-flex items-center gap-1.5 text-sm border-0 bg-transparent cursor-pointer p-0 break-all text-left"
                style={{ color: "var(--color-blue)" }}
                title="在浏览器中打开"
              >
                <LinkIcon className="w-3.5 h-3.5 flex-shrink-0" />
                {entry.repo}
              </button>
            </div>
          )}
          <div>
            <label className="text-xs font-medium block mb-2" style={{ color: "var(--text-secondary)" }}>
              配置
            </label>
            <pre
              className="text-xs overflow-x-auto m-0 p-3 rounded-mac"
              style={{
                background: "var(--surface-app)",
                border: "1px solid var(--border-hairline)",
                fontFamily: "var(--font-mono)",
                color: "var(--text-primary)",
              }}
            >
              {JSON.stringify(entry.config, null, 2)}
            </pre>
          </div>
        </div>

        <div className="flex items-center gap-2 px-6 py-4" style={{ borderTop: "1px solid var(--border-hairline)" }}>
          {onDelete && (
            <button
              onClick={onDelete}
              className="flex items-center gap-1.5 px-3 py-2 text-sm rounded-mac border-0 cursor-pointer"
              style={{ background: "transparent", color: "#FF3B30" }}
              title="删除条目（并从所有 agent 卸载）"
            >
              <TrashIcon className="w-4 h-4" />
              删除
            </button>
          )}
          <div className="flex-1" />
          <button
            onClick={onCopy}
            className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-mac cursor-pointer"
            style={{ background: "transparent", border: "1px solid var(--border-hairline)", color: "var(--text-primary)" }}
          >
            <CopyIcon className="w-4 h-4" />
            复制 JSON
          </button>
          {onEdit && (
            <button
              onClick={onEdit}
              className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-mac border-0 cursor-pointer font-medium"
              style={{ background: "#007AFF", color: "#fff" }}
            >
              <EditIcon className="w-4 h-4" />
              编辑
            </button>
          )}
        </div>
    </Modal>
  );
}
