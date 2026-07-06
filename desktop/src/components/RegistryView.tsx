import { useState, useMemo, useCallback } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { keyOf, transportOf, type Transport } from "../lib/mcp";
import { AgentGlyph, agentName } from "./brandIcons";
import { CopyIcon, EditIcon, PlusIcon, LinkIcon, TerminalIcon, XIcon, CloudIcon, FolderIcon } from "./icons";
import { Avatar, Badge, IconButton, SearchBar, Modal, TransportPill, stickyHeaderStyle } from "./ui";
import { useToast } from "./Toast";
import { PasteConfigDialog } from "./PasteConfigDialog";

interface RegistryViewProps {
  state: InstallState;
  onEdit: (name: string, transport: Transport) => void;
  onCreate: () => void;
}

/** Origin buckets used by the source filter. */
type OriginBucket = "all" | "remote" | "local" | "manual" | "discovered";

const FILTERS: { value: OriginBucket; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "remote", label: "订阅" },
  { value: "local", label: "本地" },
  { value: "manual", label: "手动" },
  { value: "discovered", label: "探索" },
];

/** Classify an entry's origin into a filter bucket. Entries with no origin, or a
 *  legacy/unknown kind, fall into "discovered" (scanned-from-machine). */
function bucketOf(entry: RegistryEntry): Exclude<OriginBucket, "all"> {
  const k = entry.origin?.kind;
  if (k === "remote") return "remote";
  if (k === "local") return "local";
  if (k === "manual") return "manual";
  return "discovered";
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
      <span className="inline-flex items-center gap-1" title={`订阅：${origin.source ? sourceName(origin.source) : ""}`}>
        <CloudIcon className="w-3.5 h-3.5" style={{ color: "var(--color-blue)" }} />
        <span className="text-[11px]" style={{ color: "var(--text-secondary)" }}>
          订阅{origin.source ? `：${sourceName(origin.source)}` : ""}
        </span>
      </span>
    );
  }
  if (origin?.kind === "local") {
    return (
      <span className="inline-flex items-center gap-1" title={`本地：${origin.source ? sourceName(origin.source) : ""}`}>
        <FolderIcon className="w-3.5 h-3.5" style={{ color: "var(--text-secondary)" }} />
        <span className="text-[11px]" style={{ color: "var(--text-secondary)" }}>
          本地{origin.source ? `：${sourceName(origin.source)}` : ""}
        </span>
      </span>
    );
  }
  if (origin?.kind === "manual") return <Badge tone="info">手动添加</Badge>;
  // discovered (recorded) or legacy custom: show the source app when known.
  const agent = origin?.agent ?? installedAgents[0];
  if (agent) {
    return (
      <span className="inline-flex items-center gap-1" title={`来自 ${agentName(agent)}`}>
        <AgentGlyph id={agent} size={16} />
        <span className="text-[11px]" style={{ color: "var(--text-secondary)" }}>
          来自 {agentName(agent)}
        </span>
      </span>
    );
  }
  return <Badge tone="neutral">机器探索</Badge>;
}

export function RegistryView({ state, onEdit, onCreate }: RegistryViewProps) {
  const { entries, agentsForServer, sources } = state;
  const toast = useToast();

  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<OriginBucket>("all");
  const [detail, setDetail] = useState<RegistryEntry | null>(null);
  const [pasteOpen, setPasteOpen] = useState(false);

  const sourceName = useCallback(
    (id: string) => sources.find((s) => s.id === id)?.name ?? id,
    [sources]
  );

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    let list = s
      ? entries.filter(
          (e) => e.name.toLowerCase().includes(s) || e.description.toLowerCase().includes(s)
        )
      : entries;
    if (filter !== "all") list = list.filter((e) => bucketOf(e) === filter);
    return [...list].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
    );
  }, [entries, q, filter]);

  const copyConfig = useCallback(
    (entry: RegistryEntry) => {
      navigator.clipboard
        .writeText(JSON.stringify(entry.config, null, 2))
        .then(() => toast.show({ kind: "success", msg: `已复制 ${entry.name} 配置` }))
        .catch(() => toast.show({ kind: "error", msg: "复制失败" }));
    },
    [toast]
  );

  return (
    <div className="h-full min-h-0 overflow-y-auto">
      {/* Sticky header: search + new, then a source filter row */}
      <div className="sticky top-0 z-10 px-6 py-4" style={stickyHeaderStyle}>
        <div className="max-w-[1280px] mx-auto flex items-center gap-3">
          <div className="flex-1">
            <SearchBar value={q} onChange={setQ} placeholder="搜索 MCP Registry…" />
          </div>
          <span className="text-xs flex-shrink-0" style={{ color: "var(--text-secondary)" }}>
            {filtered.length} 个
          </span>
          <button
            onClick={() => setPasteOpen(true)}
            className="btn-ghost flex-shrink-0"
            title="粘贴 mcpServers 配置，自动识别并加入手动来源"
          >
            粘贴配置
          </button>
          <button onClick={onCreate} className="btn-primary flex-shrink-0">
            <PlusIcon className="w-4 h-4" />
            新建 MCP
          </button>
        </div>
        <div className="max-w-[1280px] mx-auto mt-3 flex items-center gap-2">
          <div className="mux-seg">
            {FILTERS.map((f) => (
              <button
                key={f.value}
                className="mux-seg-item"
                data-active={filter === f.value ? "true" : undefined}
                onClick={() => setFilter(f.value)}
              >
                {f.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="max-w-[1280px] mx-auto px-6 pt-5 pb-8">
        {filtered.length === 0 ? (
          <div className="py-16 text-sm text-center" style={{ color: "var(--text-secondary)" }}>
            {entries.length === 0
              ? "目录为空 —— 到「来源」页订阅远程配置或导入本地配置。"
              : "未找到匹配的 MCP"}
          </div>
        ) : (
          <div className="mux-grid">
            {filtered.map((entry) => {
              const sKey = keyOf(entry);
              const installedAgents = agentsForServer(sKey);
              const usedBy = installedAgents.length;
              const ep = endpointOf(entry);

              return (
                <div key={sKey} className="mux-tile p-3.5" onClick={() => setDetail(entry)}>
                  <div className="flex items-center gap-2.5">
                    <Avatar seed={entry.name} size={34} />
                    <div className="flex-1 min-w-0">
                      <div
                        className="text-sm font-semibold truncate"
                        style={{ color: "var(--text-primary)" }}
                        title={entry.name}
                      >
                        {entry.name}
                      </div>
                      <div className="flex items-center gap-1.5 mt-1">
                        <TransportPill entry={entry} />
                        <OriginTag entry={entry} installedAgents={installedAgents} sourceName={sourceName} />
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-1.5 mt-2.5 min-w-0">
                    {ep.link ? (
                      <LinkIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
                    ) : (
                      <TerminalIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
                    )}
                    <span
                      className="text-[11px] truncate"
                      style={{
                        color: ep.link ? "var(--color-blue)" : "var(--text-secondary)",
                        fontFamily: "var(--font-mono)",
                      }}
                      title={ep.text}
                    >
                      {ep.text}
                    </span>
                  </div>

                  <div className="flex items-center justify-between mt-3 pt-3" style={{ borderTop: "1px solid var(--border-hairline)" }}>
                    {usedBy > 0 ? (
                      <Badge tone="success">{usedBy} 个 agent 使用</Badge>
                    ) : (
                      <Badge tone="neutral">未使用</Badge>
                    )}

                    <div className="mux-toolbar flex items-center gap-0.5" onClick={(e) => e.stopPropagation()}>
                      <IconButton title="复制配置 JSON" onClick={() => copyConfig(entry)}>
                        <CopyIcon className="w-4 h-4" />
                      </IconButton>
                      <IconButton title="编辑配置" onClick={() => onEdit(entry.name, transportOf(entry))}>
                        <EditIcon className="w-4 h-4" />
                      </IconButton>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {pasteOpen && <PasteConfigDialog state={state} onClose={() => setPasteOpen(false)} />}

      {detail && (
        <RegistryDetail
          entry={detail}
          installedAgents={agentsForServer(keyOf(detail))}
          sourceName={sourceName}
          onClose={() => setDetail(null)}
          onCopy={() => copyConfig(detail)}
          onEdit={() => {
            const { name } = detail;
            const transport = transportOf(detail);
            setDetail(null);
            onEdit(name, transport);
          }}
        />
      )}
    </div>
  );
}

function RegistryDetail({
  entry,
  installedAgents,
  sourceName,
  onClose,
  onCopy,
  onEdit,
}: {
  entry: RegistryEntry;
  installedAgents: string[];
  sourceName: (id: string) => string;
  onClose: () => void;
  onCopy: () => void;
  onEdit: () => void;
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
            onClick={onClose}
            className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer"
            style={{ background: "var(--border-hairline)", color: "var(--text-secondary)" }}
          >
            <XIcon className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
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

        <div className="flex items-center justify-end gap-2 px-6 py-4" style={{ borderTop: "1px solid var(--border-hairline)" }}>
          <button
            onClick={onCopy}
            className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-mac cursor-pointer"
            style={{ background: "transparent", border: "1px solid var(--border-hairline)", color: "var(--text-primary)" }}
          >
            <CopyIcon className="w-4 h-4" />
            复制 JSON
          </button>
          <button
            onClick={onEdit}
            className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-mac border-0 cursor-pointer font-medium"
            style={{ background: "#007AFF", color: "#fff" }}
          >
            <EditIcon className="w-4 h-4" />
            编辑
          </button>
        </div>
    </Modal>
  );
}
