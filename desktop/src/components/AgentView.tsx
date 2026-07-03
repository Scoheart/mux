import { useState, useMemo, useCallback } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { keyOf, transportLabel, installedKey } from "../lib/mcp";
import { XIcon, PlusIcon, EditIcon, PackageIcon } from "./icons";
import { Avatar, Badge, IconButton, SearchBar, Switch } from "./ui";
import { AgentGlyph, agentName } from "./brandIcons";
import { AddAgentDialog } from "./AddAgentDialog";
import { cellKey } from "../lib/api";

interface AgentViewProps {
  state: InstallState;
  agentId: string;
}

/** Build a minimal entry from a composite key when the installed server isn't in
 *  the registry — enough for the transport-aware uninstall request. */
function syntheticEntry(serverKey: string): RegistryEntry {
  const idx = serverKey.lastIndexOf("::");
  const name = idx >= 0 ? serverKey.slice(0, idx) : serverKey;
  const transport = idx >= 0 ? serverKey.slice(idx + 2) : "stdio";
  return {
    name,
    description: "",
    tags: [],
    config: transport === "http" ? { http: { type: "http", url: "" } } : { stdio: { command: "" } },
  };
}

/** Small pill showing an entry's transport (stdio / http / sse). */
function TransportPill({ entry }: { entry: RegistryEntry }) {
  return (
    <span
      className="px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide flex-shrink-0"
      style={{ background: "var(--color-gray-150)", color: "var(--color-gray-600)", fontFamily: "var(--font-mono)" }}
    >
      {transportLabel(entry)}
    </span>
  );
}

export function AgentView({ state, agentId }: AgentViewProps) {
  const { entries, agents, installed, pending, toggle, setEnabled, remove, refreshAgents, rescan } = state;

  const [showAddPopover, setShowAddPopover] = useState(false);
  const [addSearch, setAddSearch] = useState("");
  const [editingAgent, setEditingAgent] = useState(false);

  const agent = useMemo(() => agents.find((a) => a.id === agentId) ?? null, [agents, agentId]);

  // All global rows for this agent — includes disabled (enabled === false) rows
  // so they stay visible as an "off" toggle rather than vanishing.
  const agentRows = useMemo(
    () => installed.filter((i) => i.agent === agentId && i.scope === "global"),
    [installed, agentId]
  );

  // Composite keys (name::transport) of every server shown for this agent
  // (enabled or disabled) — drives the add-popover's "not installed" filter.
  const installedKeySet = useMemo(
    () => new Set(agentRows.map((r) => installedKey(r))),
    [agentRows]
  );

  // Resolve each row to its registry entry (or a synthetic stand-in) + enabled.
  // Sort alphabetically by name (transport as tiebreaker) so order is independent
  // of enabled/disabled state — a row keeps its slot when toggled rather than
  // jumping to the end (where scan_installed appends disabled rows).
  const installedEntries = useMemo(
    () =>
      agentRows
        .map((r) => {
          const k = installedKey(r);
          const entry = entries.find((e) => keyOf(e) === k) ?? syntheticEntry(k);
          return { entry, enabled: r.enabled };
        })
        .sort(
          (a, b) =>
            a.entry.name.localeCompare(b.entry.name, undefined, { sensitivity: "base" }) ||
            transportLabel(a.entry).localeCompare(transportLabel(b.entry))
        ),
    [agentRows, entries]
  );

  const notInstalledEntries = useMemo(() => {
    const s = addSearch.trim().toLowerCase();
    return entries
      .filter((e) => {
        const notInstalled = !installedKeySet.has(keyOf(e));
        if (!s) return notInstalled;
        return notInstalled && (e.name.toLowerCase().includes(s) || e.description.toLowerCase().includes(s));
      })
      .sort(
        (a, b) =>
          a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) ||
          transportLabel(a).localeCompare(transportLabel(b))
      );
  }, [entries, installedKeySet, addSearch]);

  const handleToggle = useCallback(
    (entry: RegistryEntry) => {
      const key = cellKey(keyOf(entry), agentId);
      if (pending.has(key)) return;
      toggle(entry, agentId);
    },
    [agentId, pending, toggle]
  );

  const borderColor = "var(--border-hairline)";
  const surfaceRaised = "var(--surface-raised)";

  if (!agent) {
    return (
      <div className="flex items-center justify-center h-full text-sm" style={{ color: "var(--text-secondary)" }}>
        未找到该 Agent
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 overflow-y-auto">
      <div className="max-w-4xl mx-auto px-6 py-6">
        {/* Header */}
        <div className="flex items-center gap-3 mb-5">
          <AgentGlyph id={agent.id} size={44} />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h2
                className="text-lg font-semibold m-0 truncate"
                style={{ color: "var(--text-primary)" }}
              >
                {agentName(agent.id)}
              </h2>
              {!agent.has_global && <Badge tone="warning">无全局路径</Badge>}
            </div>
            <div
              className="text-xs mt-0.5"
              style={{ color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}
            >
              {agent.id} · {agent.format}
            </div>
          </div>
        </div>

        {/* Config file path — always shown (the stored ~/… path), with an edit affordance */}
        <div className="mb-5 p-3 rounded-mac" style={{ background: surfaceRaised, border: `1px solid ${borderColor}` }}>
          <div className="flex items-center justify-between mb-1">
            <div className="text-xs font-semibold uppercase" style={{ color: "var(--text-secondary)", letterSpacing: "0.06em" }}>
              配置文件路径
            </div>
            <button
              onClick={() => setEditingAgent(true)}
              className="flex items-center gap-1 text-xs px-2 py-1 rounded-mac border-0 cursor-pointer font-medium"
              style={{ background: "transparent", color: "var(--color-blue)" }}
              title="编辑配置文件路径"
            >
              <EditIcon className="w-3.5 h-3.5" />
              编辑
            </button>
          </div>
          {agent.global ? (
            <span className="text-xs break-all" style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
              {agent.global}
            </span>
          ) : agent.project ? (
            <span className="text-xs break-all" style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
              项目： {agent.project}
            </span>
          ) : (
            <span className="text-xs" style={{ color: "var(--text-secondary)" }}>
              未设置路径，点击「编辑」添加
            </span>
          )}
        </div>

        {editingAgent && (
          <AddAgentDialog
            existing={agent}
            onClose={() => setEditingAgent(false)}
            onAdded={async () => {
              await refreshAgents();
              await rescan();
            }}
          />
        )}

        {/* Installed MCP header + add */}
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-semibold uppercase m-0" style={{ color: "var(--text-secondary)", letterSpacing: "0.06em" }}>
            已安装 MCP（{installedEntries.length}）
          </h3>

          <div style={{ position: "relative", zIndex: 50 }}>
            <button
              onClick={() => {
                if (!agent.has_global) return;
                setShowAddPopover((v) => !v);
                setAddSearch("");
              }}
              disabled={!agent.has_global}
              className="btn-primary"
              title={agent.has_global ? "添加 MCP" : "无全局配置路径，无法添加"}
            >
              <PlusIcon className="w-3.5 h-3.5" />
              添加 MCP
            </button>

            {showAddPopover && (
              <>
                <div
                  style={{ position: "fixed", inset: 0, zIndex: 40 }}
                  onClick={() => {
                    setShowAddPopover(false);
                    setAddSearch("");
                  }}
                />
                <div
                  style={{
                    position: "absolute",
                    top: "calc(100% + 6px)",
                    right: 0,
                    width: 340,
                    maxHeight: 380,
                    background: "var(--glass-fill-strong)",
                    backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
                    WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
                    border: `1px solid var(--glass-border)`,
                    borderRadius: 12,
                    boxShadow: "var(--glass-shadow), var(--glass-highlight)",
                    display: "flex",
                    flexDirection: "column",
                    overflow: "hidden",
                    zIndex: 50,
                  }}
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="p-2 flex-shrink-0" style={{ borderBottom: `1px solid ${borderColor}` }}>
                    <SearchBar value={addSearch} onChange={setAddSearch} placeholder="搜索 MCP…" autoFocus />
                  </div>
                  <div className="flex-1 overflow-y-auto">
                    {notInstalledEntries.length === 0 ? (
                      <div className="px-3 py-4 text-xs text-center" style={{ color: "var(--text-secondary)" }}>
                        {entries.length === installedEntries.length ? "所有 MCP 均已安装" : "未找到匹配的 MCP"}
                      </div>
                    ) : (
                      notInstalledEntries.map((entry) => {
                        const isPending = pending.has(cellKey(keyOf(entry), agentId));
                        return (
                          <button
                            key={keyOf(entry)}
                            onClick={() => {
                              handleToggle(entry);
                              setShowAddPopover(false);
                              setAddSearch("");
                            }}
                            disabled={isPending}
                            className="w-full text-left px-3 py-2.5 border-0 transition-colors flex items-center gap-2.5"
                            style={{
                              background: "transparent",
                              borderBottom: `1px solid ${borderColor}`,
                              opacity: isPending ? 0.5 : 1,
                              cursor: isPending ? "default" : "pointer",
                            }}
                            onMouseEnter={(e) => {
                              if (!isPending) e.currentTarget.style.background = "color-mix(in srgb, #007AFF 6%, transparent)";
                            }}
                            onMouseLeave={(e) => {
                              e.currentTarget.style.background = "transparent";
                            }}
                          >
                            <Avatar seed={entry.name} size={30} />
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-1.5">
                                <span className="text-xs font-medium truncate" style={{ color: "var(--text-primary)" }}>
                                  {entry.name}
                                </span>
                                <TransportPill entry={entry} />
                              </div>
                              {entry.description && (
                                <div className="text-xs truncate mt-0.5" style={{ color: "var(--text-secondary)" }}>
                                  {entry.description}
                                </div>
                              )}
                            </div>
                          </button>
                        );
                      })
                    )}
                  </div>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Installed list — compact responsive grid to use the horizontal space */}
        {installedEntries.length === 0 ? (
          <div
            className="flex flex-col items-center gap-2 py-12 text-center rounded-mac-lg"
            style={{ border: `1px dashed ${borderColor}` }}
          >
            <PackageIcon className="w-7 h-7" style={{ color: "var(--text-secondary)", opacity: 0.5 }} />
            <div className="text-sm font-medium" style={{ color: "var(--text-primary)" }}>
              还没有安装任何 MCP
            </div>
            <div className="text-xs" style={{ color: "var(--text-secondary)" }}>
              点右上角「添加 MCP」开始
            </div>
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))", gap: 8 }}>
            {installedEntries.map(({ entry, enabled }) => {
              const isPending = pending.has(cellKey(keyOf(entry), agentId));
              return (
                <div
                  key={keyOf(entry)}
                  className="mux-card flex items-center gap-2.5 px-3 py-2.5"
                  style={{ opacity: isPending ? 0.5 : enabled ? 1 : 0.55 }}
                >
                  <Avatar seed={entry.name} size={30} />
                  <span className="flex items-center gap-1.5 flex-1 min-w-0">
                    <span className="text-sm font-medium truncate" style={{ color: "var(--text-primary)" }}>
                      {entry.name}
                    </span>
                    <TransportPill entry={entry} />
                  </span>
                  <Switch
                    checked={enabled}
                    disabled={isPending}
                    title={enabled ? "禁用（从配置移除但保留记录）" : "启用（写回配置）"}
                    onChange={(on) => {
                      if (pending.has(cellKey(keyOf(entry), agentId))) return;
                      setEnabled(entry, agentId, on);
                    }}
                  />
                  <IconButton
                    title="删除（彻底移除）"
                    disabled={isPending}
                    onClick={() => {
                      if (pending.has(cellKey(keyOf(entry), agentId))) return;
                      remove(entry, agentId);
                    }}
                  >
                    <XIcon className="w-4 h-4" />
                  </IconButton>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
