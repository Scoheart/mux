import { useState, useMemo, useCallback } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { keyOf, transportOf, transportLabel, type Transport } from "../lib/mcp";
import { AgentGlyph, agentName } from "./brandIcons";
import { CopyIcon, EditIcon, PlusIcon, LinkIcon, TerminalIcon, XIcon } from "./icons";
import { Avatar, Badge, IconButton, SearchBar } from "./ui";
import { useToast } from "./Toast";

interface RegistryViewProps {
  state: InstallState;
  onEdit: (name: string, transport: Transport) => void;
  onCreate: () => void;
}

function endpointOf(entry: RegistryEntry): { text: string; link: boolean } {
  if (entry.config.http?.url) return { text: entry.config.http.url, link: true };
  if (entry.config.stdio) {
    const { command, args } = entry.config.stdio;
    return { text: [command, ...(args ?? [])].join(" "), link: false };
  }
  return { text: entry.description || "—", link: false };
}

/** Small pill showing an entry's transport (stdio / http / sse). Sized to sit
 *  on the same baseline as the origin Badge in the unified label row. */
function TransportTag({ entry }: { entry: RegistryEntry }) {
  return (
    <span
      className="inline-flex items-center px-2.5 py-1 rounded-full text-[10px] font-medium uppercase tracking-wide whitespace-nowrap flex-shrink-0"
      style={{
        background: "var(--color-gray-150)",
        color: "var(--color-gray-600)",
        fontFamily: "var(--font-mono)",
      }}
    >
      {transportLabel(entry)}
    </span>
  );
}

/** Origin indicator: 内置 (builtin) / 机器探索 (discovered, with source app icon) /
 *  手动添加 (manual). Falls back to deriving the source app from where the server is
 *  currently installed for legacy entries that predate the recorded origin. */
function OriginTag({
  entry,
  isCustom,
  installedAgents,
}: {
  entry: RegistryEntry;
  isCustom: boolean;
  installedAgents: string[];
}) {
  if (!isCustom) return <Badge tone="neutral">内置</Badge>;
  const origin = entry.origin;
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
  // Custom entry whose source app is unknown (no recorded origin, not currently
  // installed anywhere) — it still came from a machine scan, so label it 机器探索.
  return <Badge tone="neutral">机器探索</Badge>;
}

export function RegistryView({ state, onEdit, onCreate }: RegistryViewProps) {
  const { entries, agentsForServer, customKeys } = state;
  const toast = useToast();

  const [q, setQ] = useState("");
  const [detail, setDetail] = useState<RegistryEntry | null>(null);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    const list = s
      ? entries.filter(
          (e) => e.name.toLowerCase().includes(s) || e.description.toLowerCase().includes(s)
        )
      : entries;
    // Alphabetical order by name (case-insensitive).
    return [...list].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
    );
  }, [entries, q]);

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
      {/* Sticky header: search + new */}
      <div
        className="sticky top-0 z-10 px-6 py-4"
        style={{
          background: "var(--header-bg)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          borderBottom: "1px solid color-mix(in srgb, var(--glass-border) 55%, transparent)",
        }}
      >
        <div className="max-w-[1280px] mx-auto flex items-center gap-3">
          <div className="flex-1">
            <SearchBar value={q} onChange={setQ} placeholder="搜索 MCP Registry…" />
          </div>
          <span className="text-xs flex-shrink-0" style={{ color: "var(--text-secondary)" }}>
            {filtered.length} 个
          </span>
          <button onClick={onCreate} className="btn-primary flex-shrink-0">
            <PlusIcon className="w-4 h-4" />
            新建 MCP
          </button>
        </div>
      </div>

      <div className="max-w-[1280px] mx-auto px-6 pt-5 pb-8">
        {filtered.length === 0 ? (
          <div className="py-16 text-sm text-center" style={{ color: "var(--text-secondary)" }}>
            未找到匹配的 MCP
          </div>
        ) : (
          <div className="mux-grid">
            {filtered.map((entry) => {
              const sKey = keyOf(entry);
              const installedAgents = agentsForServer(sKey);
              const usedBy = installedAgents.length;
              const isCustom = customKeys.has(sKey);
              const ep = endpointOf(entry);

              return (
                <div
                  key={sKey}
                  className="mux-tile p-3.5"
                  onClick={() => setDetail(entry)}
                >
                  {/* Header: avatar + name; meta labels (transport + origin) on a
                      single tidy row beneath the name. */}
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
                        <TransportTag entry={entry} />
                        <OriginTag entry={entry} isCustom={isCustom} installedAgents={installedAgents} />
                      </div>
                    </div>
                  </div>

                  {/* Endpoint / command */}
                  <div className="flex items-center gap-1.5 mt-2.5 min-w-0">
                    {ep.link ? (
                      <LinkIcon
                        className="w-3.5 h-3.5 flex-shrink-0"
                        style={{ color: "var(--color-blue)" }}
                      />
                    ) : (
                      <TerminalIcon
                        className="w-3.5 h-3.5 flex-shrink-0"
                        style={{ color: "var(--text-secondary)" }}
                      />
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

                  {/* Footer: usage status + hover toolbar */}
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

      {/* Read-only detail modal */}
      {detail && (
        <RegistryDetail
          entry={detail}
          isCustom={customKeys.has(keyOf(detail))}
          installedAgents={agentsForServer(keyOf(detail))}
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
  isCustom,
  installedAgents,
  onClose,
  onCopy,
  onEdit,
}: {
  entry: RegistryEntry;
  isCustom: boolean;
  installedAgents: string[];
  onClose: () => void;
  onCopy: () => void;
  onEdit: () => void;
}) {
  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-40"
      style={{ background: "rgba(0,0,0,.3)", backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)" }}
      onClick={onClose}
    >
      <div
        className="flex flex-col w-[560px] max-h-[82vh] rounded-mac-lg overflow-hidden"
        style={{
          background: "var(--surface-overlay)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          border: "1px solid var(--glass-border)",
          boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          className="flex items-center gap-3 px-6 py-5"
          style={{ borderBottom: "1px solid var(--border-hairline)" }}
        >
          <Avatar seed={entry.name} size={40} />
          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold m-0 truncate" style={{ color: "var(--text-primary)" }}>
              {entry.name}
            </h2>
            <div className="flex items-center gap-1.5 mt-1">
              <TransportTag entry={entry} />
              <OriginTag entry={entry} isCustom={isCustom} installedAgents={installedAgents} />
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

        {/* Body */}
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

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-2 px-6 py-4"
          style={{ borderTop: "1px solid var(--border-hairline)" }}
        >
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
      </div>
    </div>
  );
}
