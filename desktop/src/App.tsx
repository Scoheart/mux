import { useState } from "react";
import { Layout } from "./components/Layout";
import { RegistryView } from "./components/RegistryView";
import { RegistryEditPage } from "./components/RegistryEditPage";
import { AgentView } from "./components/AgentView";
import { AddAgentDialog } from "./components/AddAgentDialog";
import { ModelsView } from "./components/ModelsView";
import { useInstallState } from "./hooks/useInstallState";
import { useUpdater } from "./hooks/useUpdater";
import { useCliTool } from "./hooks/useCliTool";
import { UpdateBanner } from "./components/UpdateBanner";
import type { View } from "./lib/types";
import type { Transport } from "./lib/mcp";

function App() {
  const [view, setView] = useState<View>({ kind: "registry" });
  /** Where to return after closing the full-page MCP editor. */
  const [editReturn, setEditReturn] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const [mcpEditor, setMcpEditor] = useState<{
    name: string | null;
    transport?: Transport;
  } | null>(null);
  const state = useInstallState();
  const updater = useUpdater();
  // 启动后静默安装/修复 ~/.local/bin/mux 软链（装 App 即带 CLI）。
  useCliTool();

  const openEditor = (name: string | null, transport?: "stdio" | "http") => {
    setEditReturn(view);
    setView({ kind: "mcp-edit", name, transport });
  };

  const goRegistry = () => setView({ kind: "registry" });
  const goModels = () => setView({ kind: "models" });

  // Full-page MCP editor — rendered without the top tab bar.
  if (view.kind === "mcp-edit") {
    return (
      <>
        <UpdateBanner updater={updater} />
        <RegistryEditPage
          state={state}
          name={view.name}
          transport={view.transport}
          onBack={() => setView(editReturn)}
        />
      </>
    );
  }

  return (
    <Layout updater={updater} onRescan={state.refreshAll}>
      {state.loading ? (
        <div
          className="flex items-center justify-center h-full text-sm"
          style={{ color: "var(--text-secondary)" }}
        >
          加载中…
        </div>
      ) : view.kind === "registry" ? (
        <RegistryView
          state={state}
          onEdit={(name, transport) => openEditor(name, transport)}
          onCreate={() => openEditor(null)}
          onSelectAgent={(id) => setView({ kind: "agent", id })}
          onAddAgent={() => setAddAgentOpen(true)}
          onSelectModels={goModels}
        />
      ) : view.kind === "models" ? (
        <ModelsView onSelectMcps={goRegistry} />
      ) : (
        <AgentView
          state={state}
          agentId={view.id}
          onSelectAgent={(id) => setView({ kind: "agent", id })}
          onAddAgent={() => setAddAgentOpen(true)}
          onSelectMcps={goRegistry}
          onSelectModels={goModels}
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
