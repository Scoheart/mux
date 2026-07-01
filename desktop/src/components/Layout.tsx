import { ReactNode, useState } from "react";
import type { AgentInfo, View } from "../lib/types";
import { RefreshIcon, PackageIcon, PlusIcon, SunIcon, MoonIcon } from "./icons";
import { AgentGlyph, agentName } from "./brandIcons";
import { applyTheme, getInitialTheme, type Theme } from "../lib/theme";

interface LayoutProps {
  children: ReactNode;
  agents: AgentInfo[];
  view: View;
  onSelectRegistry: () => void;
  onSelectAgent: (id: string) => void;
  onAddAgent?: () => void;
  onRescan?: () => Promise<unknown> | void;
}

export function Layout({
  children,
  agents,
  view,
  onSelectRegistry,
  onSelectAgent,
  onAddAgent,
  onRescan,
}: LayoutProps) {
  const [rescanning, setRescanning] = useState(false);
  const [theme, setTheme] = useState<Theme>(getInitialTheme);

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
        }}
      >
        {/* MUX wordmark */}
        <span
          className="text-[22px] font-bold bg-gradient-to-r from-brand-gold via-brand-coral to-brand-magenta bg-clip-text select-none flex-shrink-0"
          style={{ WebkitTextFillColor: "transparent", letterSpacing: "-0.02em" }}
        >
          MUX
        </span>

        {/* Registry entry (isolated group) */}
        <div className="mux-seg flex-shrink-0">
          <button
            className="mux-seg-item"
            data-active={view.kind === "registry" ? "true" : undefined}
            onClick={onSelectRegistry}
          >
            <span className="flex items-center gap-1.5">
              <PackageIcon className="w-3.5 h-3.5" />
              Registry
            </span>
          </button>
        </div>

        {/* Spacer — pushes the agent group to the right */}
        <div className="flex-1" />

        {/* Agent brand-icon group (right-aligned, horizontally scrollable) */}
        <div className="min-w-0 overflow-x-auto mux-noscroll">
          <div className="mux-seg" style={{ width: "max-content" }}>
            {agents.map((a) => {
              const active = view.kind === "agent" && view.id === a.id;
              const warn = !a.has_global && !a.has_project;
              return (
                <button
                  key={a.id}
                  className="mux-agent-btn relative"
                  data-active={active ? "true" : undefined}
                  title={`${agentName(a.id)}${warn ? "（无配置路径）" : ""}`}
                  onClick={() => onSelectAgent(a.id)}
                >
                  <AgentGlyph id={a.id} size={26} />
                  {warn && (
                    <span
                      className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full"
                      style={{ background: "#FF9500", border: "1.5px solid var(--surface-sidebar)" }}
                    />
                  )}
                </button>
              );
            })}
          </div>
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

        {/* Add-agent button — prominent accent action */}
        {onAddAgent && (
          <button
            type="button"
            className="mux-add-btn flex-shrink-0"
            title="添加 Agent"
            aria-label="添加 Agent"
            onClick={onAddAgent}
          >
            <PlusIcon className="w-4 h-4" style={{ color: "#fff" }} />
          </button>
        )}

        <span className="text-[11px] flex-shrink-0" style={{ color: "var(--text-secondary)" }}>
          v0.1.1
        </span>
      </header>

      {/* Content — transparent so the body's tinted backdrop shows through the
          glass surfaces. min-h-0 is critical for overflow to work. */}
      <main className="flex-1 min-h-0 overflow-hidden" style={{ background: "transparent" }}>
        {children}
      </main>
    </div>
  );
}
