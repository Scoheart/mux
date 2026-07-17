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
import { useUpdater } from "./hooks/useUpdater";
import { useCliTool } from "./hooks/useCliTool";
import { UpdateBanner } from "./components/UpdateBanner";
import type {
  SkillNavigationIntent,
  SkillNavigationRequest,
  View,
} from "./lib/types";
import type { Transport } from "./lib/mcp";

function App() {
  const [view, setView] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const [mcpEditor, setMcpEditor] = useState<{
    name: string | null;
    transport?: Transport;
  } | null>(null);
  const nextSkillNavigationId = useRef(0);
  const state = useInstallState();
  const skillsState = useSkillsState();
  const updater = useUpdater();
  // 启动后静默安装/修复 ~/.local/bin/mux 软链（装 App 即带 CLI）。
  useCliTool();

  const openSkills = useCallback((request: SkillNavigationRequest) => {
    const id = ++nextSkillNavigationId.current;
    const intent: SkillNavigationIntent =
      request.kind === "detail"
        ? { id, kind: "detail", skillName: request.skillName }
        : { id, kind: "install", agentId: request.agentId };
    setView({ kind: "skills", intent });
  }, []);

  const consumeSkillIntent = useCallback((id: number) => {
    setView((current) =>
      current.kind === "skills" && current.intent?.id === id
        ? { kind: "skills" }
        : current,
    );
  }, []);

  return (
    <Layout
      updater={updater}
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
          intent={view.intent}
          onIntentConsumed={consumeSkillIntent}
        />
      ) : state.loading ? (
        <div
          className="flex items-center justify-center h-full text-sm"
          style={{ color: "var(--text-secondary)" }}
        >
          加载中…
        </div>
      ) : view.kind === "models" ? (
        <ModelsView />
      ) : view.kind === "agent" ? (
        <AgentView
          state={state}
          skillsState={skillsState}
          agentId={view.id}
          onOpenModels={() => setView({ kind: "models" })}
          onOpenSkills={openSkills}
        />
      ) : (
        <RegistryView
          state={state}
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
          name={mcpEditor.name}
          transport={mcpEditor.transport}
          onBack={() => setMcpEditor(null)}
        />
      )}

      <UpdateBanner updater={updater} />
    </Layout>
  );
}

export default App;
