import { useCallback, useEffect, useMemo, useRef, useState, memo } from "react";
import { listRegistry, listAgents, scanInstalled, applyInstall, uninstall, cellKey } from "../lib/api";
import type { RegistryEntry, AgentInfo, InstalledMcp } from "../lib/types";
import { keyOf, transportOf, transportLabel, installedKey } from "../lib/mcp";
import { SearchIcon, CheckIcon, HalfDotIcon } from "./icons";
import { useToast } from "./Toast";
import { ServerDetail } from "./ServerDetail";

// ─── Cell state ────────────────────────────────────────────────────────────
type CellState = "disabled" | "empty" | "installed" | "customized";

interface CellProps {
  state: CellState;
  pending: boolean;
  onClick: () => void;
}

const Cell = memo(function Cell({ state, pending, onClick }: CellProps) {
  const isDisabled = state === "disabled";
  const base: React.CSSProperties = {
    width: 36,
    height: 36,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 8,
    cursor: isDisabled || pending ? "default" : "pointer",
    opacity: pending ? 0.4 : 1,
    pointerEvents: pending ? "none" : undefined,
    transition: "background 0.12s",
    border: "none",
    background: "transparent",
    margin: "auto",
  };

  if (state === "disabled") {
    return (
      <div style={base}>
        <span style={{ fontSize: 10, color: "var(--text-secondary)", opacity: 0.3 }}>•</span>
      </div>
    );
  }

  const handleClick = isDisabled ? undefined : onClick;

  if (state === "installed") {
    return (
      <button
        style={{ ...base, background: "color-mix(in srgb, #34C759 12%, transparent)" }}
        onClick={handleClick}
        title="已安装 — 点击卸载"
      >
        <CheckIcon className="w-4 h-4" style={{ color: "#34C759" }} />
      </button>
    );
  }

  if (state === "customized") {
    return (
      <button
        style={{ ...base, background: "color-mix(in srgb, #007AFF 12%, transparent)" }}
        onClick={handleClick}
        title="已安装（有覆写）— 点击卸载"
      >
        <HalfDotIcon className="w-4 h-4" style={{ color: "#007AFF" }} />
      </button>
    );
  }

  // empty
  return (
    <button
      style={base}
      onClick={handleClick}
      title="点击安装"
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "color-mix(in srgb, #007AFF 8%, transparent)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
      }}
    >
      <span style={{ fontSize: 14, color: "var(--text-secondary)", opacity: 0.4 }}>—</span>
    </button>
  );
});

// ─── MatrixView ─────────────────────────────────────────────────────────────
export function MatrixView() {
  const [entries, setEntries] = useState<RegistryEntry[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [installed, setInstalled] = useState<InstalledMcp[]>([]);
  const [loading, setLoading] = useState(true);
  const [q, setQ] = useState("");
  const [tagFilter, setTagFilter] = useState("");
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<RegistryEntry | null>(null);
  const toast = useToast();
  // ref to prevent stale closure issues with pending state
  const pendingRef = useRef(pending);
  pendingRef.current = pending;

  // ─── Load data ──────────────────────────────────────────────────────
  const doScan = useCallback(async () => {
    const data = await scanInstalled();
    setInstalled(data);
    return data;
  }, []);

  useEffect(() => {
    Promise.all([
      listRegistry().then(setEntries).catch(console.error),
      listAgents().then(setAgents).catch(console.error),
      doScan().catch(console.error),
    ]).finally(() => setLoading(false));
  }, [doScan]);

  // ─── Derived maps ───────────────────────────────────────────────────
  /** Map from cellKey → customized */
  const installedMap = useMemo(() => {
    const m = new Map<string, boolean>();
    for (const item of installed) {
      if (item.scope === "global") {
        m.set(cellKey(installedKey(item), item.agent), item.customized ?? false);
      }
    }
    return m;
  }, [installed]);

  // ─── Filter logic (reuse RegistryGrid approach) ──────────────────────
  const allTags = useMemo(() => {
    const tagSet = new Set<string>();
    entries.forEach((e) => e.tags.forEach((t) => tagSet.add(t)));
    return Array.from(tagSet).sort();
  }, [entries]);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    return entries.filter((e) => {
      const matchQ =
        !s ||
        e.name.toLowerCase().includes(s) ||
        e.description.toLowerCase().includes(s);
      const matchTag = !tagFilter || e.tags.includes(tagFilter);
      return matchQ && matchTag;
    });
  }, [entries, q, tagFilter]);

  // ─── Toggle handler ──────────────────────────────────────────────────
  const handleToggle = useCallback(
    async (entry: RegistryEntry, agentId: string) => {
      const serverName = entry.name;
      const transport = transportOf(entry);
      const key = cellKey(keyOf(entry), agentId);
      if (pendingRef.current.has(key)) return;

      const wasInstalled = installedMap.has(key);

      // Mark pending
      setPending((prev) => new Set(prev).add(key));

      // Optimistic update
      if (wasInstalled) {
        setInstalled((prev) =>
          prev.filter(
            (item) =>
              !(item.name === serverName && item.transport === transport &&
                item.agent === agentId && item.scope === "global")
          )
        );
      } else {
        setInstalled((prev) => [
          ...prev,
          { name: serverName, agent: agentId, scope: "global", file_path: "", transport, customized: false, enabled: true },
        ]);
      }

      try {
        const req = {
          server_name: serverName,
          transport,
          scope: "global" as const,
          agents: [agentId],
          project_dir: undefined,
          overrides: {},
        };
        if (wasInstalled) {
          await uninstall(req);
        } else {
          await applyInstall(req);
        }
        // Authoritative re-scan
        await doScan();
      } catch (err) {
        // Revert via re-scan
        await doScan().catch(console.error);
        const msg = Array.isArray(err) ? err.join("; ") : String(err);
        toast.show({ kind: "error", msg: `操作失败: ${msg}` });
      } finally {
        setPending((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [installedMap, doScan, toast]
  );

  // ─── Sticky table styles ─────────────────────────────────────────────
  const surfaceRaised = "var(--surface-raised)";
  const borderColor = "var(--border-hairline)";
  const borderStyle = `1px solid ${borderColor}`;

  const thCornerStyle: React.CSSProperties = {
    position: "sticky",
    top: 0,
    left: 0,
    zIndex: 30,
    background: surfaceRaised,
    border: borderStyle,
    padding: "8px 12px",
    textAlign: "left",
    whiteSpace: "nowrap",
    minWidth: 180,
    maxWidth: 220,
  };

  const thHeaderStyle: React.CSSProperties = {
    position: "sticky",
    top: 0,
    zIndex: 20,
    background: surfaceRaised,
    border: borderStyle,
    padding: "4px 0",
    width: 72,
    minWidth: 72,
    maxWidth: 72,
    textAlign: "center",
    verticalAlign: "bottom",
  };

  const tdFirstColStyle: React.CSSProperties = {
    position: "sticky",
    left: 0,
    zIndex: 10,
    background: surfaceRaised,
    border: borderStyle,
    padding: "0 12px",
    whiteSpace: "nowrap",
    minWidth: 180,
    maxWidth: 220,
  };

  const tdBodyStyle: React.CSSProperties = {
    border: borderStyle,
    padding: 0,
    textAlign: "center",
    width: 72,
    minWidth: 72,
    maxWidth: 72,
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16" style={{ color: "var(--text-secondary)" }}>
        <span className="text-sm">加载中…</span>
      </div>
    );
  }

  return (
    <div style={{ position: "relative" }}>
      {/* ── Search bar ── */}
      <div className="relative mb-3">
        <SearchIcon
          className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 pointer-events-none"
          style={{ color: "var(--text-secondary)" }}
        />
        <input
          className="w-full pl-9 pr-4 py-2.5 text-sm rounded-mac border outline-none transition-shadow"
          style={{
            background: surfaceRaised,
            border: borderStyle,
            color: "var(--text-primary)",
          }}
          placeholder="搜索服务器…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onFocus={(e) => {
            e.currentTarget.style.boxShadow = "0 0 0 3px rgba(0,122,255,.2)";
            e.currentTarget.style.borderColor = "#007AFF";
          }}
          onBlur={(e) => {
            e.currentTarget.style.boxShadow = "";
            e.currentTarget.style.borderColor = borderColor;
          }}
        />
      </div>

      {/* ── Tag pills ── */}
      {allTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5 mb-4">
          <button
            onClick={() => setTagFilter("")}
            className="px-3 py-1 text-xs rounded-pill border transition-colors"
            style={{
              background: !tagFilter ? "#007AFF" : surfaceRaised,
              color: !tagFilter ? "#fff" : "var(--text-secondary)",
              border: `1px solid ${!tagFilter ? "#007AFF" : borderColor}`,
            }}
          >
            全部
          </button>
          {allTags.map((tag) => (
            <button
              key={tag}
              onClick={() => setTagFilter(tagFilter === tag ? "" : tag)}
              className="px-3 py-1 text-xs rounded-pill border transition-colors"
              style={{
                background: tagFilter === tag ? "#007AFF" : surfaceRaised,
                color: tagFilter === tag ? "#fff" : "var(--text-secondary)",
                border: `1px solid ${tagFilter === tag ? "#007AFF" : borderColor}`,
              }}
            >
              {tag}
            </button>
          ))}
        </div>
      )}

      {/* ── Matrix table ── */}
      <div
        style={{
          overflowX: "auto",
          overflowY: "auto",
          maxHeight: "calc(100vh - 260px)",
          border: borderStyle,
          borderRadius: 10,
        }}
      >
        <table
          className="border-separate"
          style={{ borderSpacing: 0, fontSize: 12 }}
        >
          <thead>
            <tr>
              {/* Corner cell — sticky both axes */}
              <th style={thCornerStyle}>
                <span style={{ fontSize: 11, color: "var(--text-secondary)", fontWeight: 500 }}>
                  服务器 / Agent
                </span>
              </th>
              {agents.map((agent) => (
                <th key={agent.id} style={thHeaderStyle}>
                  <div
                    style={{
                      writingMode: "vertical-rl",
                      textOrientation: "mixed",
                      transform: "rotate(180deg)",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      maxHeight: 100,
                      padding: "4px 2px",
                      fontSize: 11,
                      fontWeight: 500,
                      color: agent.has_global
                        ? "var(--text-primary)"
                        : "var(--text-secondary)",
                      opacity: agent.has_global ? 1 : 0.4,
                      cursor: "default",
                    }}
                    title={agent.id}
                  >
                    {agent.id}
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {filtered.length === 0 ? (
              <tr>
                <td
                  colSpan={agents.length + 1}
                  style={{ ...tdBodyStyle, padding: "32px 0", color: "var(--text-secondary)" }}
                >
                  未找到匹配的服务器
                </td>
              </tr>
            ) : (
              filtered.map((entry) => (
                <tr key={keyOf(entry)}>
                  {/* Server name — sticky left */}
                  <td style={tdFirstColStyle}>
                    <button
                      style={{
                        border: "none",
                        background: "transparent",
                        padding: "8px 0",
                        cursor: "pointer",
                        textAlign: "left",
                        fontSize: 12,
                        fontWeight: 500,
                        color: "#007AFF",
                        width: "100%",
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                      }}
                      onClick={() => setDetail(entry)}
                      title={entry.description}
                    >
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{entry.name}</span>
                      <span
                        style={{
                          flexShrink: 0,
                          padding: "1px 7px",
                          borderRadius: 999,
                          fontSize: 9,
                          fontWeight: 600,
                          textTransform: "uppercase",
                          letterSpacing: "0.04em",
                          background: "var(--color-gray-150)",
                          color: "var(--color-gray-600)",
                          fontFamily: "var(--font-mono)",
                        }}
                      >
                        {transportLabel(entry)}
                      </span>
                    </button>
                  </td>
                  {agents.map((agent) => {
                    const key = cellKey(keyOf(entry), agent.id);
                    const isPending = pending.has(key);
                    let state: CellState = "disabled";
                    if (agent.has_global) {
                      if (installedMap.has(key)) {
                        state = installedMap.get(key) ? "customized" : "installed";
                      } else {
                        state = "empty";
                      }
                    }
                    return (
                      <td key={agent.id} style={tdBodyStyle}>
                        <Cell
                          state={state}
                          pending={isPending}
                          onClick={() => handleToggle(entry, agent.id)}
                        />
                      </td>
                    );
                  })}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* ── Server detail drawer ── */}
      {detail && (
        <ServerDetail
          entry={detail}
          agents={agents}
          installedMap={installedMap}
          onApplied={doScan}
          onClose={() => setDetail(null)}
        />
      )}
    </div>
  );
}
