import { useCallback, useRef, useState } from "react";
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
import type {
  ResourceNavigationRequest,
  View,
} from "./lib/types";
import {
  clearResourceIntent,
  createResourceNavigationIntent,
  viewForResourceIntent,
} from "./lib/resourceNavigation";
import type { Transport } from "./lib/mcp";

function App() {
  const [view, setView] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const [mcpEditor, setMcpEditor] = useState<{
    name: string | null;
    transport?: Transport;
  } | null>(null);
  const nextResourceNavigationId = useRef(0);
  const state = useInstallState();
  const skillsState = useSkillsState();
  const consumptionState = useConsumptionState();
  const networkSettings = useNetworkSettings();
  const updater = useUpdater(networkSettings.settings.proxy_url);
  // 启动后静默安装/修复 ~/.local/bin/mux 软链（装 App 即带 CLI）。
  useCliTool();

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
      onRescan={state.refreshAll}
    >
      {view.kind === "skills" ? (
        <SkillsView
          state={skillsState}
          consumptionState={consumptionState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
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
        />
      ) : view.kind === "agent" ? (
        <AgentView
          state={state}
          skillsState={skillsState}
          consumptionState={consumptionState}
          agentId={view.id}
          onOpenResource={openResource}
        />
      ) : (
        <RegistryView
          state={state}
          consumptionState={consumptionState}
          intent={view.intent}
          onIntentConsumed={consumeResourceIntent}
          onEdit={(name, transport) => setMcpEditor({ name, transport })}
          onCreate={() => setMcpEditor({ name: null })}
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

      <UpdateBanner updater={updater} />
    </Layout>
  );
}

export default App;
