import { ReactNode, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import type { AgentInfo, ProxySettings, View } from "../lib/types";
import {
  DownloadIcon,
  ChevronDownIcon,
  LayersIcon,
  MoonIcon,
  NetworkIcon,
  PackageIcon,
  RefreshIcon,
  SparklesIcon,
  SunIcon,
} from "./icons";
import { applyTheme, getInitialTheme, type Theme } from "../lib/theme";
import { useToast } from "./Toast";
import type { UpdaterState } from "../hooks/useUpdater";
import { AgentNavigation } from "./AgentNavigation";
import { ProxySettingsDialog } from "./ProxySettingsDialog";

interface LayoutProps {
  children: ReactNode;
  agents: AgentInfo[];
  view: View;
  onSelectRegistry: () => void;
  onSelectModels: () => void;
  onSelectSkills: () => void;
  onSelectAgent: (id: string) => void;
  onAddAgent?: () => void;
  onRescan?: () => Promise<unknown> | void;
  onOpenMigration?: () => void;
  migrationCount?: number;
  updater?: UpdaterState;
  proxyUrl: string | null;
  proxySettingsLoading: boolean;
  onSaveProxy: (proxyUrl: string | null) => Promise<ProxySettings>;
}

export function Layout({
  children,
  agents,
  view,
  onSelectRegistry,
  onSelectModels,
  onSelectSkills,
  onSelectAgent,
  onAddAgent,
  onRescan,
  onOpenMigration,
  migrationCount = 0,
  updater,
  proxyUrl,
  proxySettingsLoading,
  onSaveProxy,
}: LayoutProps) {
  const [rescanning, setRescanning] = useState(false);
  const [theme, setTheme] = useState<Theme>(getInitialTheme);
  const [version, setVersion] = useState("");
  const [proxySettingsOpen, setProxySettingsOpen] = useState(false);
  const [scanMenuOpen, setScanMenuOpen] = useState(false);
  const scanMenuRef = useRef<HTMLDivElement>(null);
  const toast = useToast();

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  useEffect(() => {
    if (!scanMenuOpen) return;
    const close = (event: PointerEvent) => {
      if (!scanMenuRef.current?.contains(event.target as Node)) setScanMenuOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [scanMenuOpen]);

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
        className="mux-topbar flex-shrink-0 flex items-center gap-3 px-5"
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
          className="mux-wordmark text-[22px] font-bold bg-gradient-to-r from-brand-gold via-brand-coral to-brand-magenta bg-clip-text select-none flex-shrink-0"
          style={{ WebkitTextFillColor: "transparent", letterSpacing: 0 }}
        >
          MUX
        </span>

        {/* Top-level resources (also the way back from an Agent view) */}
        <div className="mux-seg mux-skill-seg flex-shrink-0">
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
          <button
            className="mux-seg-item"
            data-active={view.kind === "skills" ? "true" : undefined}
            onClick={onSelectSkills}
          >
            <span className="flex items-center gap-1.5">
              <SparklesIcon className="w-3.5 h-3.5" />
              Skills
            </span>
          </button>
        </div>

        {/* Spacer — pushes Agent navigation to the right */}
        <div className="flex-1" />

        <AgentNavigation
          agents={agents}
          selectedAgentId={view.kind === "agent" ? view.id : null}
          onSelectAgent={onSelectAgent}
          onAddAgent={onAddAgent}
        />

        {/* Divider */}
        <div className="h-5 w-px flex-shrink-0" style={{ background: "var(--border-hairline)" }} />

        {/* Right action group */}
        <button
          type="button"
          className="mux-icon-btn mux-network-button flex-shrink-0"
          data-active={proxyUrl ? "true" : undefined}
          title={proxyUrl ? `网络代理 · ${proxyUrl}` : "网络代理"}
          aria-label={proxyUrl ? "配置网络代理，当前已启用" : "配置网络代理"}
          disabled={proxySettingsLoading}
          onClick={() => setProxySettingsOpen(true)}
        >
          <NetworkIcon className="w-4 h-4" />
          {proxyUrl && <span className="mux-network-status-dot" aria-hidden="true" />}
        </button>

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
          <div className="mux-scan-menu-wrap flex-shrink-0" ref={scanMenuRef}>
            <button
              type="button"
              className="mux-icon-btn mux-scan-menu-trigger"
              title="扫描与迁移"
              aria-label="扫描与迁移"
              aria-expanded={scanMenuOpen}
              onClick={() => setScanMenuOpen((open) => !open)}
            >
              <RefreshIcon
                className="w-4 h-4"
                style={rescanning ? { animation: "spin 0.8s linear infinite" } : undefined}
              />
              <ChevronDownIcon className="mux-scan-menu-chevron" />
              {migrationCount > 0 && <span className="mux-scan-menu-dot" aria-hidden="true" />}
            </button>
            {scanMenuOpen && (
              <div className="mux-scan-menu" role="menu">
                <button
                  type="button"
                  role="menuitem"
                  disabled={rescanning}
                  onClick={() => {
                    setScanMenuOpen(false);
                    void handleRescan();
                  }}
                >
                  <RefreshIcon className="w-4 h-4" />
                  <span><strong>{rescanning ? "扫描中…" : "重新扫描"}</strong><small>重新读取 Agent 配置</small></span>
                </button>
                {onOpenMigration && (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setScanMenuOpen(false);
                      onOpenMigration();
                    }}
                  >
                    <PackageIcon className="w-4 h-4" />
                    <span><strong>迁移历史配置</strong><small>{migrationCount > 0 ? `${migrationCount} 项待处理` : "没有待迁移配置"}</small></span>
                  </button>
                )}
              </div>
            )}
          </div>
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

      {proxySettingsOpen && (
        <ProxySettingsDialog
          proxyUrl={proxyUrl}
          onClose={() => setProxySettingsOpen(false)}
          onSave={onSaveProxy}
        />
      )}
    </div>
  );
}
