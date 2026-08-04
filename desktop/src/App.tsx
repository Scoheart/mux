import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Layout } from "./components/Layout";
import { RegistryView } from "./components/RegistryView";
import { RegistryEditPage } from "./components/RegistryEditPage";
import { AddAgentDialog } from "./components/AddAgentDialog";
import { useInstallState } from "./hooks/useInstallState";
import { useSkillsState } from "./hooks/useSkillsState";
import { useConsumptionState } from "./hooks/useConsumptionState";
import { useUpdater } from "./hooks/useUpdater";
import { useCliTool } from "./hooks/useCliTool";
import { useNetworkSettings } from "./hooks/useNetworkSettings";
import { UpdateBanner } from "./components/UpdateBanner";
import {
  getBackendStatus,
  getMigrationReview,
  listModelAdoptionCandidates,
  resolveMigration,
} from "./lib/api";
import { ModelMigrationReviewDialog } from "./components/ModelMigrationReviewDialog";
import type {
  AssetCommandError,
  BackendStatus,
  MigrationResolutionStrategy,
  MigrationReview,
  ModelAdoptionCandidate,
  ResourceNavigationRequest,
  View,
} from "./lib/types";
import {
  clearResourceIntent,
  createResourceNavigationIntent,
  viewForResourceIntent,
} from "./lib/resourceNavigation";
import { mergeAgentInfos } from "./lib/agentCapabilities";
import { useStartupSync, type StartupTask } from "./hooks/useStartupSync";
import { RefreshIcon } from "./components/icons";
import { useTranslation } from "react-i18next";

const AgentView = lazy(() =>
  import("./components/AgentView").then((module) => ({
    default: module.AgentView,
  })),
);
const ModelsView = lazy(() =>
  import("./components/ModelsView").then((module) => ({
    default: module.ModelsView,
  })),
);
const SkillsView = lazy(() =>
  import("./components/SkillsView").then((module) => ({
    default: module.SkillsView,
  })),
);

function ViewLoading() {
  return (
    <div className="mux-view-loading" role="status">
      <RefreshIcon data-spinning="true" />
      正在打开视图…
    </div>
  );
}

function App() {
  const { t } = useTranslation();
  const [view, setView] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const [mcpEditorOpen, setMcpEditorOpen] = useState(false);
  const [externalModelCandidates, setExternalModelCandidates] = useState<ModelAdoptionCandidate[]>([]);
  const [migrationReview, setMigrationReview] = useState<MigrationReview | null>(null);
  const [backendStatus, setBackendStatus] = useState<BackendStatus | null>(null);
  const [migrationReviewOpen, setMigrationReviewOpen] = useState(false);
  const [migrationBusy, setMigrationBusy] = useState(false);
  const [migrationError, setMigrationError] = useState<AssetCommandError | null>(null);
  const nextResourceNavigationId = useRef(0);
  const state = useInstallState({ autoLoad: false });
  const skillsState = useSkillsState({ autoLoad: false });
  const consumptionState = useConsumptionState({ autoLoad: false });
  const networkSettings = useNetworkSettings();
  const updater = useUpdater(networkSettings.settings.proxy_url, { autoCheck: false });
  useEffect(() => {
    Promise.all([getBackendStatus(), getMigrationReview()])
      .then(([status, review]) => {
        setBackendStatus(status);
        setMigrationReview(review);
      })
      .catch(() => undefined);
  }, []);
  const agents = useMemo(
    () => mergeAgentInfos(state.agents, consumptionState.agents),
    [consumptionState.agents, state.agents],
  );
  const refreshExternalModels = useCallback(async () => {
    setExternalModelCandidates(await listModelAdoptionCandidates());
  }, []);
  const refreshWorkspace = useCallback(async () => {
    const snapshot = await consumptionState.refreshWorkspace();
    skillsState.hydrate(snapshot.assets.skills);
    return snapshot;
  }, [consumptionState.refreshWorkspace, skillsState.hydrate]);

  const foregroundStartupTasks = useMemo<StartupTask[]>(
    () => [
      { id: "workspace", label: "workspace", run: refreshWorkspace },
      { id: "registry", label: "registry", run: state.refreshRegistry },
      { id: "sources", label: "sources", run: state.refreshSources },
    ],
    [refreshWorkspace, state.refreshRegistry, state.refreshSources],
  );

  const deferredStartupTasks = useMemo<StartupTask[]>(
    () => [
      { id: "external-models", label: "external-models", run: refreshExternalModels },
      {
        id: "updates",
        label: "updates",
        run: async () => {
          if (await updater.checkNow() === "error") {
            throw new Error("update_check_failed");
          }
        },
      },
    ],
    [refreshExternalModels, updater.checkNow],
  );

  const startupSync = useStartupSync({
    foreground: foregroundStartupTasks,
    deferred: deferredStartupTasks,
    foregroundConcurrency: 2,
  });
  // CLI link repair is useful but irrelevant to the first interaction. Run it
  // only after all fresh read-only startup work has settled.
  useCliTool({ start: startupSync.settled });

  const refreshEverything = useCallback(async () => {
    await Promise.all([
      state.refreshRegistry(),
      state.refreshSources(),
      refreshWorkspace(),
      refreshExternalModels(),
    ]);
  }, [refreshExternalModels, refreshWorkspace, state.refreshRegistry, state.refreshSources]);

  const handleMigrationResolution = useCallback(async (
    strategy: MigrationResolutionStrategy,
    candidateHash?: string,
  ) => {
    const review = migrationReview;
    if (!review || migrationBusy) return;
    setMigrationBusy(true);
    setMigrationError(null);
    try {
      const outcome = await resolveMigration(review, strategy, candidateHash);
      setBackendStatus(outcome.status);
      setMigrationReview(outcome.review ?? null);
      if (!outcome.review) {
        setMigrationReviewOpen(false);
        await refreshEverything();
      }
    } catch (cause) {
      if (typeof cause === "object" && cause !== null && "code" in cause) {
        setMigrationError({
          code: String(cause.code),
          message: "message" in cause ? String(cause.message) : String(cause.code),
          details: "details" in cause && typeof cause.details === "object"
            ? cause.details as Record<string, unknown>
            : undefined,
        });
      } else {
        setMigrationError({ code: "migration_resolution_failed", message: String(cause) });
      }
      try {
        const [status, currentReview] = await Promise.all([
          getBackendStatus(),
          getMigrationReview(),
        ]);
        setBackendStatus(status);
        setMigrationReview(currentReview);
        if (!currentReview) setMigrationReviewOpen(false);
      } catch {
        // Preserve the actionable error already shown by the dialog.
      }
    } finally {
      setMigrationBusy(false);
    }
  }, [migrationBusy, migrationReview, refreshEverything]);

  const openResource = useCallback((request: ResourceNavigationRequest) => {
    const id = ++nextResourceNavigationId.current;
    setView(viewForResourceIntent(createResourceNavigationIntent(id, request)));
  }, []);

  const consumeResourceIntent = useCallback((id: number) => {
    setView((current) => clearResourceIntent(current, id));
  }, []);

  return (
    <Layout
      updater={updater}
      proxyUrl={networkSettings.settings.proxy_url}
      proxySettingsLoading={networkSettings.loading}
      onSaveProxy={networkSettings.save}
      agents={agents}
      view={view}
      onSelectRegistry={() => setView({ kind: "registry" })}
      onSelectModels={() => setView({ kind: "models" })}
      onSelectSkills={() => setView({ kind: "skills" })}
      onSelectAgent={(id) => setView({ kind: "agent", id })}
      onAddAgent={() => setAddAgentOpen(true)}
      onRescan={refreshEverything}
      startupSync={startupSync}
    >
      <Suspense fallback={<ViewLoading />}>
        {view.kind === "skills" ? (
          <SkillsView
            state={skillsState}
            intent={view.intent}
            onIntentConsumed={consumeResourceIntent}
          />
        ) : view.kind === "models" ? (
          <ModelsView
            consumptionState={consumptionState}
            intent={view.intent}
            onIntentConsumed={consumeResourceIntent}
          />
        ) : view.kind === "agent" ? (
          <AgentView
            state={state}
            skillsState={skillsState}
            consumptionState={consumptionState}
            agentId={view.id}
            externalModelCandidates={externalModelCandidates}
            onOpenResource={openResource}
          />
        ) : (
          <RegistryView
            state={state}
            consumptionState={consumptionState}
            intent={view.intent}
            onIntentConsumed={consumeResourceIntent}
            onCreate={() => setMcpEditorOpen(true)}
            onRetryLoad={startupSync.retryFailed}
            retryLoadDisabled={startupSync.syncing}
          />
        )}
      </Suspense>

      {addAgentOpen && (
        <AddAgentDialog
          onClose={() => setAddAgentOpen(false)}
          onAdded={async () => {
            await Promise.all([state.refreshAgents(), refreshWorkspace()]);
          }}
        />
      )}

      {mcpEditorOpen && (
        <RegistryEditPage
          state={state}
          consumptionState={consumptionState}
          name={null}
          onBack={() => setMcpEditorOpen(false)}
        />
      )}

      {migrationReview && (
        <div className="mux-asset-recovery-banner mux-migration-review-banner" role="alert">
          <div>
            <strong>{t("migration.title")}</strong>
            <span>
              {t("migration.bannerIntro", {
                agent: migrationReview.blockers[0]?.agent_name ?? "Agent",
              })}
            </span>
          </div>
          <button type="button" className="btn-primary" onClick={() => setMigrationReviewOpen(true)}>
            {t("migration.viewContinue")}
          </button>
        </div>
      )}

      {!migrationReview && (
        backendStatus?.state === "read_only"
        || consumptionState.error?.code === "recovery_required"
      ) && (
        <div className="mux-asset-recovery-banner" role="alert">
          <strong>{t("migration.recoveryTitle")}</strong>
          <span>{t("migration.recoveryMessage", {
            stage: backendStatus?.state === "read_only" ? backendStatus.stage : "durable_recovery",
          })}</span>
        </div>
      )}

      {!migrationReview && backendStatus?.state === "capability_unavailable" && (
        <div className="mux-asset-recovery-banner mux-capability-warning-banner" role="alert">
          <strong>{t("migration.capabilityTitle")}</strong>
          <span>{t("migration.capabilityMessage", { stage: backendStatus.stage })}</span>
        </div>
      )}

      {migrationReview && migrationReviewOpen && (
        <ModelMigrationReviewDialog
          review={migrationReview}
          busy={migrationBusy}
          error={migrationError}
          onResolve={handleMigrationResolution}
          onLater={() => {
            if (!migrationBusy) setMigrationReviewOpen(false);
          }}
        />
      )}

      <UpdateBanner updater={updater} />
    </Layout>
  );
}

export default App;
