import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { StartupSyncState } from "../hooks/useStartupSync";
import { RefreshIcon } from "./icons";

export function StartupSyncBar({ state }: { state: StartupSyncState }) {
  const { t } = useTranslation();
  const [showSettled, setShowSettled] = useState(false);

  useEffect(() => {
    if (!state.settled || state.failed > 0) {
      setShowSettled(false);
      return;
    }
    setShowSettled(true);
    const timer = setTimeout(() => setShowSettled(false), 1_400);
    return () => clearTimeout(timer);
  }, [state.failed, state.settled]);

  if (state.settled && state.failed === 0 && !showSettled) return null;

  const activeLabel = (() => {
    switch (state.activeLabel) {
      case "registry": return t("startup.tasks.registry");
      case "agents": return t("startup.tasks.agents");
      case "sources": return t("startup.tasks.sources");
      case "workspace": return t("startup.tasks.workspace");
      case "skills": return t("startup.tasks.skills");
      case "installed": return t("startup.tasks.installed");
      case "external-models": return t("startup.tasks.externalModels");
      case "updates": return t("startup.tasks.updates");
      default: return null;
    }
  })();
  const detail = state.failed > 0
    ? t("startup.failed", { count: state.failed })
    : state.settled
      ? t("startup.complete")
      : state.slow
        ? t("startup.slow")
        : activeLabel ?? t("startup.waiting");

  return (
    <div
      className="mux-startup-sync"
      data-slow={state.slow ? "true" : undefined}
      data-settled={state.settled ? "true" : undefined}
      role={state.failed > 0 ? "alert" : "status"}
      aria-live="polite"
    >
      <RefreshIcon
        className="mux-startup-sync-icon"
        data-spinning={state.syncing ? "true" : undefined}
      />
      <strong>{t("startup.title")}</strong>
      <span className="mux-startup-sync-detail">{detail}</span>
      <span className="mux-startup-sync-count">
        {state.completed}/{state.total}
      </span>
      {state.failed > 0 && (
        <button
          type="button"
          className="mux-startup-sync-retry"
          disabled={state.syncing}
          onClick={() => void state.retryFailed()}
        >
          {t("common.retry")}
        </button>
      )}
    </div>
  );
}
