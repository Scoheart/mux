import { useState, useCallback } from "react";
import type { RegistryEntry, AgentInfo, PatchInput } from "../lib/types";
import { applyInstall, cellKey } from "../lib/api";
import { keyOf, transportOf } from "../lib/mcp";
import { EnvEditor } from "./EnvEditor";
import { XIcon } from "./icons";
import { useToast } from "./Toast";

interface ServerDetailProps {
  entry: RegistryEntry;
  agents: AgentInfo[];
  installedMap: Map<string, boolean>;
  onApplied: () => Promise<unknown>;
  onClose: () => void;
}

export function ServerDetail({ entry, agents, installedMap, onApplied, onClose }: ServerDetailProps) {
  const toast = useToast();
  // Local override state per agentId
  const [overrides, setOverrides] = useState<Record<string, PatchInput>>({});
  const [applying, setApplying] = useState<Set<string>>(new Set());

  const globalAgents = agents.filter((a) => a.has_global);

  const applyOverride = useCallback(
    async (agentId: string) => {
      if (applying.has(agentId)) return;
      setApplying((prev) => new Set(prev).add(agentId));
      try {
        const agentOverride = overrides[agentId] ?? {};
        await applyInstall({
          server_name: entry.name,
          transport: transportOf(entry),
          scope: "global",
          agents: [agentId],
          project_dir: undefined,
          overrides: { [agentId]: agentOverride },
        });
        await onApplied();
        toast.show({ kind: "success", msg: `已应用覆写: ${agentId}` });
      } catch (err) {
        const msg = Array.isArray(err) ? err.join("; ") : String(err);
        toast.show({ kind: "error", msg: `应用失败: ${msg}` });
      } finally {
        setApplying((prev) => {
          const next = new Set(prev);
          next.delete(agentId);
          return next;
        });
      }
    },
    [applying, overrides, entry.name, onApplied, toast]
  );

  const monogram = entry.name[0]?.toUpperCase() ?? "?";

  return (
    <>
      {/* Backdrop */}
      <div
        style={{
          position: "fixed",
          inset: 0,
          zIndex: 40,
          background: "rgba(0,0,0,0.15)",
        }}
        onClick={onClose}
      />

      {/* Drawer */}
      <div
        style={{
          position: "fixed",
          top: 0,
          right: 0,
          bottom: 0,
          width: 380,
          zIndex: 50,
          background: "var(--surface-overlay)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          borderLeft: "1px solid var(--glass-border)",
          boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "flex-start",
            gap: 12,
            padding: "20px 20px 16px",
            borderBottom: "1px solid var(--border-hairline)",
            flexShrink: 0,
          }}
        >
          {/* Monogram */}
          <div
            style={{
              width: 40,
              height: 40,
              borderRadius: 10,
              flexShrink: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "#fff",
              fontSize: 16,
              fontWeight: 600,
              background: "linear-gradient(135deg, #007AFF, #5AC8FA)",
            }}
          >
            {monogram}
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2
              style={{
                margin: 0,
                fontSize: 16,
                fontWeight: 600,
                color: "var(--text-primary)",
                wordBreak: "break-all",
              }}
            >
              {entry.name}
            </h2>
            <p
              style={{
                margin: "4px 0 0",
                fontSize: 12,
                color: "var(--text-secondary)",
                lineHeight: 1.4,
              }}
            >
              {entry.description}
            </p>
          </div>
          <button
            onClick={onClose}
            style={{
              flexShrink: 0,
              border: "none",
              background: "transparent",
              cursor: "pointer",
              padding: 4,
              color: "var(--text-secondary)",
              display: "flex",
              alignItems: "center",
            }}
            title="关闭"
          >
            <XIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Tags */}
        {entry.tags.length > 0 && (
          <div
            style={{
              padding: "12px 20px",
              borderBottom: "1px solid var(--border-hairline)",
              flexShrink: 0,
              display: "flex",
              flexWrap: "wrap",
              gap: 6,
            }}
          >
            {entry.tags.map((tag) => (
              <span
                key={tag}
                style={{
                  padding: "2px 10px",
                  fontSize: 11,
                  borderRadius: 9999,
                  background: "color-mix(in srgb, #007AFF 10%, transparent)",
                  color: "#007AFF",
                }}
              >
                {tag}
              </span>
            ))}
          </div>
        )}

        {/* Per-agent override sections */}
        <div style={{ flex: 1, overflowY: "auto", padding: "12px 20px 24px" }}>
          <h3
            style={{
              margin: "0 0 12px",
              fontSize: 12,
              fontWeight: 600,
              color: "var(--text-secondary)",
              textTransform: "uppercase",
              letterSpacing: "0.06em",
            }}
          >
            环境变量覆写
          </h3>
          {globalAgents.length === 0 && (
            <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              无可用 Agent
            </p>
          )}
          {globalAgents.map((agent) => {
            const key = cellKey(keyOf(entry), agent.id);
            const isInstalled = installedMap.has(key);
            const isApplying = applying.has(agent.id);
            const agentOverride = overrides[agent.id] ?? {};
            const currentEnv = agentOverride.env ?? {};

            return (
              <div
                key={agent.id}
                style={{
                  marginBottom: 20,
                  paddingBottom: 20,
                  borderBottom: "1px solid var(--border-hairline)",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    marginBottom: 6,
                  }}
                >
                  <span
                    style={{
                      fontSize: 12,
                      fontWeight: 500,
                      color: "var(--text-primary)",
                      fontFamily: "var(--font-mono)",
                    }}
                  >
                    {agent.id}
                  </span>
                  <span
                    style={{
                      fontSize: 11,
                      color: isInstalled ? "#34C759" : "var(--text-secondary)",
                    }}
                  >
                    {isInstalled ? "已安装" : "未安装"}
                  </span>
                </div>

                <EnvEditor
                  value={currentEnv}
                  onChange={(env) =>
                    setOverrides((prev) => ({
                      ...prev,
                      [agent.id]: { ...prev[agent.id], env },
                    }))
                  }
                />

                <button
                  onClick={() => applyOverride(agent.id)}
                  disabled={isApplying}
                  style={{
                    marginTop: 8,
                    padding: "5px 14px",
                    fontSize: 12,
                    fontWeight: 500,
                    borderRadius: 8,
                    border: "none",
                    background: isApplying ? "var(--border-hairline)" : "#007AFF",
                    color: isApplying ? "var(--text-secondary)" : "#fff",
                    cursor: isApplying ? "default" : "pointer",
                  }}
                >
                  {isApplying ? "应用中…" : "应用覆写"}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}
