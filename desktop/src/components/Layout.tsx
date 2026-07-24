import { ReactNode, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import type { AgentInfo, ProxySettings, View } from "../lib/types";
import {
  DownloadIcon,
  ChevronDownIcon,
  LanguageIcon,
  LayersIcon,
  MoonIcon,
  NetworkIcon,
  PackageIcon,
  RefreshIcon,
  SparklesIcon,
  SunIcon,
} from "./icons";
import { applyTheme, getInitialTheme, type Theme } from "../lib/theme";
import { formatError } from "../lib/format";
import { useLocale } from "../i18n/LocaleProvider";
import type { LocalePreference } from "../i18n";
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
  const [languageMenuOpen, setLanguageMenuOpen] = useState(false);
  const scanMenuRef = useRef<HTMLDivElement>(null);
  const languageMenuRef = useRef<HTMLDivElement>(null);
  const toast = useToast();
  const { t } = useTranslation();
  const localeState = useLocale();

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

  useEffect(() => {
    if (!languageMenuOpen) return;
    const close = (event: PointerEvent) => {
      if (!languageMenuRef.current?.contains(event.target as Node)) setLanguageMenuOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [languageMenuOpen]);

  const checkingUpdate = updater?.phase.kind === "checking";
  const handleCheckUpdate = async () => {
    if (!updater || checkingUpdate) return;
    const result = await updater.checkNow({ manual: true });
    // "available"/"error" both surface via the UpdateBanner; only the quiet
    // outcome needs feedback here.
    if (result === "latest") toast.show({ kind: "success", msg: t("layout.latest") });
  };

  const toggleTheme = () => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
  };

  const selectLocale = async (preference: LocalePreference) => {
    setLanguageMenuOpen(false);
    try {
      await localeState.setPreference(preference);
    } catch (error) {
      toast.show({
        kind: "error",
        msg: t("layout.languageSaveFailed", { error: formatError(error) }),
      });
    }
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
          background: "var(--surface-workspace)",
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
            data-active={view.kind === "models" ? "true" : undefined}
            onClick={onSelectModels}
          >
            <span className="flex items-center gap-1.5">
              <LayersIcon className="w-3.5 h-3.5" />
              <span className="mux-resource-label">Models</span>
            </span>
          </button>
          <button
            className="mux-seg-item"
            data-active={view.kind === "registry" ? "true" : undefined}
            onClick={onSelectRegistry}
          >
            <span className="flex items-center gap-1.5">
              <PackageIcon className="w-3.5 h-3.5" />
              <span className="mux-resource-label">MCPs</span>
            </span>
          </button>
          <button
            className="mux-seg-item"
            data-active={view.kind === "skills" ? "true" : undefined}
            onClick={onSelectSkills}
          >
            <span className="flex items-center gap-1.5">
              <SparklesIcon className="w-3.5 h-3.5" />
              <span className="mux-resource-label">Skills</span>
            </span>
          </button>
        </div>

        {/* The middle lane absorbs narrow widths. Pinned Agents scroll inside
            this lane while every control keeps its normal hit target. */}
        <div className="mux-topbar-navigation-lane">
          <AgentNavigation
            agents={agents}
            selectedAgentId={view.kind === "agent" ? view.id : null}
            onSelectAgent={onSelectAgent}
            onAddAgent={onAddAgent}
          />
        </div>

        {/* Right action group */}
        <button
          type="button"
          className="mux-icon-btn mux-network-button flex-shrink-0"
          data-active={proxyUrl ? "true" : undefined}
          title={proxyUrl ? `${t("layout.networkProxy")} · ${proxyUrl}` : t("layout.networkProxy")}
          aria-label={proxyUrl ? t("layout.networkProxyEnabled") : t("layout.configureNetworkProxy")}
          disabled={proxySettingsLoading}
          onClick={() => setProxySettingsOpen(true)}
        >
          <NetworkIcon className="w-4 h-4" />
          {proxyUrl && <span className="mux-network-status-dot" aria-hidden="true" />}
        </button>

        <button
          type="button"
          className="mux-icon-btn flex-shrink-0"
          title={theme === "dark" ? t("layout.lightTheme") : t("layout.darkTheme")}
          aria-label={t("layout.switchTheme")}
          onClick={toggleTheme}
        >
          {theme === "dark" ? <SunIcon className="w-4 h-4" /> : <MoonIcon className="w-4 h-4" />}
        </button>

        <div className="mux-language-menu-wrap flex-shrink-0" ref={languageMenuRef}>
          <button
            type="button"
            className="mux-icon-btn"
            title={t("layout.language")}
            aria-label={t("layout.languageMenu")}
            aria-expanded={languageMenuOpen}
            onClick={() => setLanguageMenuOpen((open) => !open)}
          >
            <LanguageIcon className="w-4 h-4" />
          </button>
          {languageMenuOpen && (
            <div className="mux-language-menu" role="menu" aria-label={t("layout.languageMenu")}>
              {([
                [null, t("layout.followSystem")],
                ["zh-CN", t("layout.simplifiedChinese")],
                ["en-US", t("layout.english")],
              ] as Array<[LocalePreference, string]>).map(([value, label]) => (
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={localeState.preference === value}
                  data-active={localeState.preference === value ? "true" : undefined}
                  disabled={localeState.saving}
                  key={value ?? "system"}
                  onClick={() => void selectLocale(value)}
                >
                  <span>{label}</span>
                  {localeState.preference === value && <span aria-hidden="true">✓</span>}
                </button>
              ))}
            </div>
          )}
        </div>

        {onRescan && (
          <div className="mux-scan-menu-wrap flex-shrink-0" ref={scanMenuRef}>
            <button
              type="button"
              className="mux-icon-btn mux-scan-menu-trigger"
              title={t("layout.scanAndMigrate")}
              aria-label={t("layout.scanAndMigrate")}
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
                  <span><strong>{rescanning ? t("layout.scanning") : t("layout.rescan")}</strong><small>{t("layout.rescanDetail")}</small></span>
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
                    <span><strong>{t("layout.migrateHistory")}</strong><small>{migrationCount > 0 ? t("layout.pendingCount", { count: migrationCount }) : t("layout.noMigration")}</small></span>
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
          title={version ? t("layout.currentVersion", { version }) : t("layout.checkUpdate")}
          aria-label={version ? t("layout.checkUpdateVersion", { version }) : t("layout.checkUpdate")}
          disabled={checkingUpdate}
          onClick={() => void handleCheckUpdate()}
        >
          <span
            className="mux-update-check-icon"
            data-busy={checkingUpdate ? "true" : undefined}
            aria-hidden="true"
          >
            {checkingUpdate
              ? <RefreshIcon className="w-full h-full" />
              : <DownloadIcon className="w-full h-full" />}
          </span>
          <span className="mux-update-check-label">{checkingUpdate ? t("layout.checking") : t("layout.checkUpdate")}</span>
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
