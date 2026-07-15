import { ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import type { AgentInfo, View } from "../lib/types";
import {
  ChevronDownIcon,
  CheckIcon,
  DownloadIcon,
  RefreshIcon,
  PackageIcon,
  LayersIcon,
  PlusIcon,
  SearchIcon,
  SunIcon,
  MoonIcon,
  XIcon,
} from "./icons";
import { AgentGlyph } from "./brandIcons";
import { applyTheme, getInitialTheme, type Theme } from "../lib/theme";
import { useToast } from "./Toast";
import type { UpdaterState } from "../hooks/useUpdater";

interface LayoutProps {
  children: ReactNode;
  agents: AgentInfo[];
  view: View;
  onSelectRegistry: () => void;
  onSelectModels: () => void;
  onSelectAgent: (id: string) => void;
  onAddAgent?: () => void;
  onRescan?: () => Promise<unknown> | void;
  updater?: UpdaterState;
}

export function Layout({
  children,
  agents,
  view,
  onSelectRegistry,
  onSelectModels,
  onSelectAgent,
  onAddAgent,
  onRescan,
  updater,
}: LayoutProps) {
  const [rescanning, setRescanning] = useState(false);
  const [theme, setTheme] = useState<Theme>(getInitialTheme);
  const [version, setVersion] = useState("");
  const [agentPickerOpen, setAgentPickerOpen] = useState(false);
  const [agentQuery, setAgentQuery] = useState("");
  const agentPickerRef = useRef<HTMLDivElement>(null);
  const toast = useToast();

  const selectedAgent =
    view.kind === "agent" ? agents.find((agent) => agent.id === view.id) ?? null : null;
  const writableCount = agents.filter((agent) => agent.has_global).length;
  const visibleAgents = useMemo(() => {
    const query = agentQuery.trim().toLocaleLowerCase();
    return agents
      .filter((agent) => agent.has_global)
      .filter((agent) => {
        if (!query) return true;
        return [agent.name, agent.id, agent.category]
          .join(" ")
          .toLocaleLowerCase()
          .includes(query);
      })
      .sort((left, right) =>
        left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
      );
  }, [agentQuery, agents]);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  useEffect(() => {
    if (!agentPickerOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAgentPickerOpen(false);
    };
    const closeOnPointerDown = (event: PointerEvent) => {
      if (!agentPickerRef.current?.contains(event.target as Node)) {
        setAgentPickerOpen(false);
      }
    };
    document.addEventListener("keydown", closeOnEscape);
    document.addEventListener("pointerdown", closeOnPointerDown);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
      document.removeEventListener("pointerdown", closeOnPointerDown);
    };
  }, [agentPickerOpen]);

  const checkingUpdate = updater?.phase.kind === "checking";
  const handleCheckUpdate = async () => {
    if (!updater || checkingUpdate) return;
    const result = await updater.checkNow({ manual: true });
    // "available"/"error" both surface via the UpdateBanner; only the quiet
    // outcome needs feedback here.
    if (result === "latest") toast.show({ kind: "success", msg: "已是最新版本" });
  };

  const toggleTheme = () => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
  };

  const handleRescan = async () => {
    if (!onRescan || rescanning) return;
    setRescanning(true);
    try {
      await onRescan();
    } finally {
      setRescanning(false);
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Top bar */}
      <header
        className="flex-shrink-0 flex items-center gap-3 px-5"
        style={{
          height: 56,
          background: "var(--glass-fill-strong)",
          borderBottom: "1px solid var(--glass-border)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          boxShadow: "var(--glass-highlight)",
          position: "relative",
          zIndex: 100,
        }}
      >
        {/* MUX wordmark */}
        <span
          className="text-[22px] font-bold bg-gradient-to-r from-brand-gold via-brand-coral to-brand-magenta bg-clip-text select-none flex-shrink-0"
          style={{ WebkitTextFillColor: "transparent", letterSpacing: 0 }}
        >
          MUX
        </span>

        {/* MCPs (also the way back from an agent view) */}
        <div className="mux-seg flex-shrink-0">
          <button
            className="mux-seg-item"
            data-active={view.kind === "registry" ? "true" : undefined}
            onClick={onSelectRegistry}
          >
            <span className="flex items-center gap-1.5">
              <PackageIcon className="w-3.5 h-3.5" />
              MCPs
            </span>
          </button>
          <button
            className="mux-seg-item"
            data-active={view.kind === "models" ? "true" : undefined}
            onClick={onSelectModels}
          >
            <span className="flex items-center gap-1.5">
              <LayersIcon className="w-3.5 h-3.5" />
              Models
              <span className="mux-seg-beta">Beta</span>
            </span>
          </button>
        </div>

        {/* Spacer — pushes Agent navigation to the right */}
        <div className="flex-1" />

        <div className="mux-agent-picker-anchor flex-shrink-0" ref={agentPickerRef}>
          <button
            type="button"
            className="mux-agent-picker-trigger"
            data-active={selectedAgent ? "true" : undefined}
            data-open={agentPickerOpen ? "true" : undefined}
            aria-haspopup="listbox"
            aria-expanded={agentPickerOpen}
            onClick={() => {
              setAgentPickerOpen((open) => !open);
              setAgentQuery("");
            }}
          >
            {selectedAgent ? (
              <AgentGlyph id={selectedAgent.id} name={selectedAgent.name} size={24} />
            ) : (
              <PackageIcon className="w-5 h-5 flex-shrink-0" />
            )}
            <span className="mux-agent-picker-trigger-copy">
              <span className="mux-agent-picker-trigger-name">
                {selectedAgent?.name ?? "选择 Agent"}
              </span>
              <span className="mux-agent-picker-trigger-meta">
                {selectedAgent?.id ?? `${writableCount} 个可配置 Agent`}
              </span>
            </span>
            <ChevronDownIcon className="mux-agent-picker-chevron" />
          </button>
          {agentPickerOpen && (
            <section
              className="mux-agent-picker"
              aria-label="选择 Agent"
            >
              <div className="mux-agent-picker-search">
                <SearchIcon className="w-4 h-4 flex-shrink-0" />
                <input
                  type="search"
                  autoFocus
                  spellCheck={false}
                  value={agentQuery}
                  onChange={(event) => setAgentQuery(event.target.value)}
                  placeholder="按名称或 ID 搜索"
                  aria-label="搜索 Agent"
                />
                <button
                  type="button"
                  className="mux-agent-picker-search-clear"
                  data-visible={agentQuery ? "true" : undefined}
                  disabled={!agentQuery}
                  tabIndex={agentQuery ? 0 : -1}
                  aria-label="清除搜索"
                  title="清除搜索"
                  onPointerDown={(event) => event.preventDefault()}
                  onClick={() => setAgentQuery("")}
                >
                  <XIcon className="w-3.5 h-3.5" />
                </button>
              </div>

              <div className="mux-agent-picker-list" role="listbox">
                {visibleAgents.length === 0 ? (
                  <div className="mux-agent-picker-empty">未找到匹配项</div>
                ) : (
                  visibleAgents.map((agent) => {
                    const active = selectedAgent?.id === agent.id;
                    return (
                      <button
                        type="button"
                        role="option"
                        aria-selected={active}
                        key={agent.id}
                        className="mux-agent-picker-row"
                        data-active={active ? "true" : undefined}
                        onClick={() => {
                          onSelectAgent(agent.id);
                          setAgentPickerOpen(false);
                        }}
                      >
                        <AgentGlyph id={agent.id} name={agent.name} size={32} />
                        <span className="min-w-0 flex-1">
                          <span className="mux-agent-picker-name">{agent.name}</span>
                          <span className="mux-agent-picker-meta">
                            {agent.format.toUpperCase()} · {agent.id}
                          </span>
                        </span>
                        {active && <CheckIcon className="mux-agent-picker-check" />}
                      </button>
                    );
                  })
                )}
              </div>

              {onAddAgent && (
                <div className="mux-agent-picker-footer">
                  <button
                    type="button"
                    onClick={() => {
                      setAgentPickerOpen(false);
                      onAddAgent();
                    }}
                  >
                    <PlusIcon className="w-4 h-4" />
                    添加自定义 Agent
                  </button>
                </div>
              )}
            </section>
          )}
        </div>

        {/* Divider */}
        <div className="h-5 w-px flex-shrink-0" style={{ background: "var(--border-hairline)" }} />

        {/* Right action group */}
        <button
          type="button"
          className="mux-icon-btn flex-shrink-0"
          title={theme === "dark" ? "切换到浅色" : "切换到深色"}
          aria-label="切换主题"
          onClick={toggleTheme}
        >
          {theme === "dark" ? <SunIcon className="w-4 h-4" /> : <MoonIcon className="w-4 h-4" />}
        </button>

        {onRescan && (
          <button
            type="button"
            className="mux-icon-btn flex-shrink-0"
            title="重新扫描"
            aria-label="重新扫描"
            disabled={rescanning}
            onClick={handleRescan}
          >
            <RefreshIcon
              className="w-4 h-4"
              style={rescanning ? { animation: "spin 0.8s linear infinite" } : undefined}
            />
          </button>
        )}

        {/* Explicit update action: keep the installed version visible without
            relying on users to discover that a bare version label is clickable. */}
        <button
          type="button"
          className="mux-update-check flex-shrink-0"
          title={version ? `当前版本 v${version}，点击检查更新` : "检查更新"}
          aria-label={version ? `检查更新，当前版本 v${version}` : "检查更新"}
          disabled={checkingUpdate}
          onClick={() => void handleCheckUpdate()}
        >
          <DownloadIcon
            className="w-3.5 h-3.5"
            style={checkingUpdate ? { animation: "spin 0.8s linear infinite" } : undefined}
          />
          <span>{checkingUpdate ? "检查中…" : "检查更新"}</span>
          {version && <span className="mux-update-version">v{version}</span>}
        </button>
      </header>

      {/* Content — transparent so the body's tinted backdrop shows through the
          glass surfaces. min-h-0 is critical for overflow to work. */}
      <main className="flex-1 min-h-0 overflow-hidden" style={{ background: "transparent" }}>
        {children}
      </main>
    </div>
  );
}
