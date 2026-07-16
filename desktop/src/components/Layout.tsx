import { ReactNode, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import {
  DownloadIcon,
  RefreshIcon,
  SunIcon,
  MoonIcon,
} from "./icons";
import { applyTheme, getInitialTheme, type Theme } from "../lib/theme";
import { useToast } from "./Toast";
import type { UpdaterState } from "../hooks/useUpdater";

interface LayoutProps {
  children: ReactNode;
  onRescan?: () => Promise<unknown> | void;
  updater?: UpdaterState;
}

export function Layout({ children, onRescan, updater }: LayoutProps) {
  const [rescanning, setRescanning] = useState(false);
  const [theme, setTheme] = useState<Theme>(getInitialTheme);
  const [version, setVersion] = useState("");
  const toast = useToast();

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

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
      {/* Top bar — brand + window actions only */}
      <header
        className="flex-shrink-0 flex items-center gap-3 px-5"
        style={{
          height: 52,
          background: "var(--glass-fill-strong)",
          borderBottom: "1px solid var(--glass-border)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          boxShadow: "var(--glass-highlight)",
          position: "relative",
          zIndex: 100,
        }}
      >
        <span
          className="text-[22px] font-bold bg-gradient-to-r from-brand-gold via-brand-coral to-brand-magenta bg-clip-text select-none flex-shrink-0"
          style={{ WebkitTextFillColor: "transparent", letterSpacing: 0 }}
        >
          MUX
        </span>

        <div className="flex-1" />

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

      {/* Content — sidebar (when present) reaches this edge under the header.
          min-h-0 is critical for overflow to work. */}
      <main className="flex-1 min-h-0 overflow-hidden" style={{ background: "transparent" }}>
        {children}
      </main>
    </div>
  );
}
