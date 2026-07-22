import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import type { Transport } from "./lib/mcp";

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
  const [mcpEditor, setMcpEditor] = useState<{
    name: string | null;
    transport?: Transport;
  } | null>(null);
  const [migrationOpen, setMigrationOpen] = useState(false);
  const [migrationFocusId, setMigrationFocusId] = useState<string | null>(null);
  const [mcpMigrationCandidates, setMcpMigrationCandidates] = useState<McpAdoptionCandidate[]>([]);
  const [skillMigrationCandidates, setSkillMigrationCandidates] = useState<SkillInventoryItem[]>([]);
  const [modelMigrationCandidates, setModelMigrationCandidates] = useState<ModelAdoptionCandidate[]>([]);
  const [ignoredMigrations, setIgnoredMigrations] = useState(loadIgnoredMigrations);
  const nextResourceNavigationId = useRef(0);
  const state = useInstallState();
  const skillsState = useSkillsState();
  const consumptionState = useConsumptionState();
  const networkSettings = useNetworkSettings();
  const updater = useUpdater(networkSettings.settings.proxy_url);
  // 启动后静默安装/修复 ~/.local/bin/mux 软链（装 App 即带 CLI）。
  useCliTool();

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

  const refreshMigrations = useCallback(async () => {
    const [mcps, skills, models] = await Promise.all([
      listMcpAdoptionCandidates(),
      listSkillMigrationCandidates(),
      listModelAdoptionCandidates(),
    ]);
    setMcpMigrationCandidates(mcps);
    setSkillMigrationCandidates(skills);
    setModelMigrationCandidates(models);
  }, []);

  useEffect(() => {
    void refreshMigrations().catch(() => undefined);
  }, [refreshMigrations]);

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
      agents={state.agents}
      view={view}
      onSelectRegistry={() => setView({ kind: "registry" })}
      onSelectModels={() => setView({ kind: "models" })}
      onSelectSkills={() => setView({ kind: "skills" })}
      onSelectAgent={(id) => setView({ kind: "agent", id })}
      onAddAgent={() => setAddAgentOpen(true)}
      onRescan={refreshEverything}
      onOpenMigration={() => openMigration()}
      migrationCount={migrationCandidateCounts.all}
    >
      {view.kind === "skills" ? (
        <SkillsView
          state={skillsState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
          migrationCount={migrationCandidateCounts.skill}
          onOpenMigration={() => openMigration()}
        />
      ) : state.loading ? (
        <div
          className="flex items-center justify-center h-full text-sm"
          style={{ color: "var(--text-secondary)" }}
        >
          加载中…
        </div>
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
          onOpenResource={openResource}
          onOpenMigration={() => openMigration()}
          onManageExternalMcp={manageExternalMcp}
        />
      ) : (
        <RegistryView
          state={state}
          consumptionState={consumptionState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
          onEdit={(name, transport) => setMcpEditor({ name, transport })}
          onCreate={() => setMcpEditor({ name: null })}
          migrationCount={migrationCandidateCounts.mcp}
          onOpenMigration={() => openMigration()}
        />
      )}

      {addAgentOpen && (
        <AddAgentDialog
          onClose={() => setAddAgentOpen(false)}
          onAdded={state.refreshAgents}
        />
      )}

      {mcpEditor && (
        <RegistryEditPage
          state={state}
          consumptionState={consumptionState}
          name={mcpEditor.name}
          transport={mcpEditor.transport}
          onBack={() => setMcpEditor(null)}
        />
      )}

      {consumptionState.error?.code === "recovery_required" && (
        <div className="mux-asset-recovery-banner" role="alert">
          <strong>中央资产事务需要恢复</strong>
          <span>{consumptionState.error.message}</span>
        </div>
      )}

      {!migrationOpen &&
        consumptionState.error?.code !== "recovery_required" &&
        skillsState.error?.code !== "recovery_required" &&
        newMigrationCandidates.length > 0 && (
        <aside className="mux-migration-banner" role="status">
          <span>
            <strong>发现 {newMigrationCandidates.length} 项历史配置</strong>
            <small>可以导入 MUX 统一管理</small>
          </span>
          <button type="button" className="btn-ghost" onClick={ignoreCurrentMigrations}>稍后</button>
          <button type="button" className="btn-primary" onClick={() => openMigration()}>查看</button>
        </aside>
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
