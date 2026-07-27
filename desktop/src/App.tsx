import { useCallback, useMemo, useRef, useState } from "react";
import { Layout } from "./components/Layout";
import { RegistryView } from "./components/RegistryView";
import { RegistryEditPage } from "./components/RegistryEditPage";
import { AgentView } from "./components/AgentView";
import { AddAgentDialog } from "./components/AddAgentDialog";
import { ModelsView } from "./components/ModelsView";
import { SkillsView } from "./components/SkillsView";
import { useInstallState } from "./hooks/useInstallState";
import { useSkillsState } from "./hooks/useSkillsState";
import { useConsumptionState } from "./hooks/useConsumptionState";
import { useUpdater } from "./hooks/useUpdater";
import { useCliTool } from "./hooks/useCliTool";
import { useNetworkSettings } from "./hooks/useNetworkSettings";
import { UpdateBanner } from "./components/UpdateBanner";
import { listModelAdoptionCandidates } from "./lib/api";
import type {
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

function App() {
  const [view, setView] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const [mcpEditorOpen, setMcpEditorOpen] = useState(false);
  const [externalModelCandidates, setExternalModelCandidates] = useState<ModelAdoptionCandidate[]>([]);
  const nextResourceNavigationId = useRef(0);
  const state = useInstallState({ autoLoad: false });
  const skillsState = useSkillsState({ autoLoad: false });
  const consumptionState = useConsumptionState({ autoLoad: false });
  const networkSettings = useNetworkSettings();
  const updater = useUpdater(networkSettings.settings.proxy_url, { autoCheck: false });
  const agents = useMemo(
    () => mergeAgentInfos(state.agents, consumptionState.agents),
    [consumptionState.agents, state.agents],
  );
  const refreshExternalModels = useCallback(async () => {
    setExternalModelCandidates(await listModelAdoptionCandidates());
  }, []);

  const foregroundStartupTasks = useMemo<StartupTask[]>(() => [
    { id: "registry", label: "registry", run: state.refreshRegistry },
    { id: "agents", label: "agents", run: state.refreshAgents },
    { id: "sources", label: "sources", run: state.refreshSources },
    { id: "workspace", label: "workspace", run: consumptionState.refresh },
    { id: "skills", label: "skills", run: skillsState.refresh },
    { id: "installed", label: "installed", run: state.rescan },
  ], [
    consumptionState.refresh,
    skillsState.refresh,
    state.refreshAgents,
    state.refreshRegistry,
    state.refreshSources,
    state.rescan,
  ]);

  const deferredStartupTasks = useMemo<StartupTask[]>(() => [
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
  ], [
    refreshExternalModels,
    updater.checkNow,
  ]);

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
      state.refreshAll(),
      skillsState.refresh(),
      consumptionState.refresh(),
      refreshExternalModels(),
    ]);
  }, [consumptionState.refresh, refreshExternalModels, skillsState.refresh, state.refreshAll]);

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

      {addAgentOpen && (
        <AddAgentDialog
          onClose={() => setAddAgentOpen(false)}
          onAdded={state.refreshAgents}
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

      {consumptionState.error?.code === "recovery_required" && (
        <div className="mux-asset-recovery-banner" role="alert">
          <strong>资源更改需要恢复</strong>
          <span>{consumptionState.error.message}</span>
        </div>
      )}

      <UpdateBanner updater={updater} />
    </Layout>
  );
}

export default App;
