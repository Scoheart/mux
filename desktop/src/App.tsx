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
import { MigrationDialog } from "./components/MigrationDialog";
import { MigrationBanner } from "./components/MigrationBanner";
import { buildMigrationCandidates, mcpMigrationCandidateId, migrationCounts } from "./lib/migration";
import { listMcpAdoptionCandidates, listModelAdoptionCandidates, listSkillMigrationCandidates } from "./lib/api";
import type {
  McpAdoptionCandidate,
  ModelAdoptionCandidate,
  ResourceNavigationRequest,
  SkillInventoryItem,
  View,
} from "./lib/types";
import {
  clearResourceIntent,
  createResourceNavigationIntent,
  viewForResourceIntent,
} from "./lib/resourceNavigation";
import { mergeAgentInfos } from "./lib/agentCapabilities";
import { useStartupSync, type StartupTask } from "./hooks/useStartupSync";

const MIGRATION_IGNORED_KEY = "mux:migration-ignored:v2";

function loadIgnoredMigrations(): Set<string> {
  try {
    const value = JSON.parse(localStorage.getItem(MIGRATION_IGNORED_KEY) ?? "[]");
    return new Set(Array.isArray(value) ? value.filter((item) => typeof item === "string") : []);
  } catch {
    return new Set();
  }
}

function App() {
  const [view, setView] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const [mcpEditorOpen, setMcpEditorOpen] = useState(false);
  const [migrationOpen, setMigrationOpen] = useState(false);
  const [migrationFocusId, setMigrationFocusId] = useState<string | null>(null);
  const [mcpMigrationCandidates, setMcpMigrationCandidates] = useState<McpAdoptionCandidate[]>([]);
  const [skillMigrationCandidates, setSkillMigrationCandidates] = useState<SkillInventoryItem[]>([]);
  const [modelMigrationCandidates, setModelMigrationCandidates] = useState<ModelAdoptionCandidate[]>([]);
  const [ignoredMigrations, setIgnoredMigrations] = useState(loadIgnoredMigrations);
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
  const migrationCandidates = useMemo(
    () => buildMigrationCandidates(mcpMigrationCandidates, skillMigrationCandidates, modelMigrationCandidates),
    [mcpMigrationCandidates, modelMigrationCandidates, skillMigrationCandidates],
  );
  const newMigrationCandidates = useMemo(
    () => migrationCandidates.filter((candidate) => !ignoredMigrations.has(candidate.fingerprint)),
    [ignoredMigrations, migrationCandidates],
  );
  const migrationCandidateCounts = migrationCounts(migrationCandidates);
  const visibleMigrationCandidates = migrationFocusId
    ? migrationCandidates.filter((candidate) => candidate.id === migrationFocusId)
    : migrationCandidates;

  const openMigration = useCallback((focusId: string | null = null) => {
    setMigrationFocusId(focusId);
    setMigrationOpen(true);
  }, []);

  const closeMigration = useCallback(() => {
    setMigrationOpen(false);
    setMigrationFocusId(null);
  }, []);

  const manageExternalMcp = useCallback((assetKey: string) => {
    openMigration(mcpMigrationCandidateId(assetKey));
  }, [openMigration]);

  const refreshMcpMigrations = useCallback(async () => {
    setMcpMigrationCandidates(await listMcpAdoptionCandidates());
  }, []);

  const refreshSkillMigrations = useCallback(async () => {
    setSkillMigrationCandidates(await listSkillMigrationCandidates());
  }, []);

  const refreshModelMigrations = useCallback(async () => {
    setModelMigrationCandidates(await listModelAdoptionCandidates());
  }, []);

  const refreshMigrations = useCallback(async () => {
    await Promise.all([
      refreshMcpMigrations(),
      refreshSkillMigrations(),
      refreshModelMigrations(),
    ]);
  }, [refreshMcpMigrations, refreshModelMigrations, refreshSkillMigrations]);

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
    { id: "migration-mcp", label: "migration-mcp", run: refreshMcpMigrations },
    { id: "migration-skills", label: "migration-skills", run: refreshSkillMigrations },
    { id: "migration-models", label: "migration-models", run: refreshModelMigrations },
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
    refreshMcpMigrations,
    refreshModelMigrations,
    refreshSkillMigrations,
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
      refreshMigrations(),
    ]);
  }, [consumptionState.refresh, refreshMigrations, skillsState.refresh, state.refreshAll]);

  const ignoreCurrentMigrations = useCallback(() => {
    setIgnoredMigrations((current) => {
      const next = new Set(current);
      for (const candidate of newMigrationCandidates) next.add(candidate.fingerprint);
      localStorage.setItem(MIGRATION_IGNORED_KEY, JSON.stringify([...next]));
      return next;
    });
  }, [newMigrationCandidates]);

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
      onOpenMigration={() => openMigration()}
      migrationCount={migrationCandidateCounts.all}
      startupSync={startupSync}
    >
      {view.kind === "skills" ? (
        <SkillsView
          state={skillsState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
          migrationCount={migrationCandidateCounts.skill}
          onOpenMigration={() => openMigration()}
        />
      ) : view.kind === "models" ? (
        <ModelsView
          consumptionState={consumptionState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
          migrationCount={migrationCandidateCounts.model}
          onOpenMigration={() => openMigration()}
        />
      ) : view.kind === "agent" ? (
        <AgentView
          state={state}
          skillsState={skillsState}
          consumptionState={consumptionState}
          agentId={view.id}
          modelMigrationCandidates={modelMigrationCandidates}
          onOpenResource={openResource}
          onOpenMigration={(focusId) => openMigration(focusId ?? null)}
          onManageExternalMcp={manageExternalMcp}
        />
      ) : (
        <RegistryView
          state={state}
          consumptionState={consumptionState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
          onCreate={() => setMcpEditorOpen(true)}
          migrationCount={migrationCandidateCounts.mcp}
          onOpenMigration={() => openMigration()}
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

      {!migrationOpen &&
        consumptionState.error?.code !== "recovery_required" &&
        skillsState.error?.code !== "recovery_required" &&
        newMigrationCandidates.length > 0 && (
        <MigrationBanner
          candidates={newMigrationCandidates}
          onLater={ignoreCurrentMigrations}
          onOpen={() => openMigration()}
        />
      )}

      {migrationOpen && (
        <MigrationDialog
          candidates={visibleMigrationCandidates}
          onClose={closeMigration}
          onRefresh={refreshEverything}
        />
      )}

      <UpdateBanner updater={updater} />
    </Layout>
  );
}

export default App;
