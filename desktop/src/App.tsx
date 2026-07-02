import { useState } from "react";
import { Layout } from "./components/Layout";
import { RegistryView } from "./components/RegistryView";
import { SourcesView } from "./components/SourcesView";
import { RegistryEditPage } from "./components/RegistryEditPage";
import { AgentView } from "./components/AgentView";
import { AddAgentDialog } from "./components/AddAgentDialog";
import { useInstallState } from "./hooks/useInstallState";
import type { View } from "./lib/types";

function App() {
  const [view, setView] = useState<View>({ kind: "registry" });
  const [addAgentOpen, setAddAgentOpen] = useState(false);
  const state = useInstallState();

  // Full-page MCP editor — rendered without the top tab bar.
  if (view.kind === "mcp-edit") {
    return (
      <RegistryEditPage
        state={state}
        name={view.name}
        transport={view.transport}
        onBack={() => setView({ kind: "registry" })}
      />
    );
  }

  return (
    <Layout
      agents={state.agents}
      view={view}
      onSelectRegistry={() => setView({ kind: "registry" })}
      onSelectSources={() => setView({ kind: "sources" })}
      onSelectAgent={(id) => setView({ kind: "agent", id })}
      onAddAgent={() => setAddAgentOpen(true)}
      onRescan={state.refreshAll}
    >
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
          onEdit={(name, transport) => setView({ kind: "mcp-edit", name, transport })}
          onCreate={() => setView({ kind: "mcp-edit", name: null })}
        />
      ) : view.kind === "sources" ? (
        <SourcesView state={state} />
      ) : (
        <AgentView state={state} agentId={view.id} />
      )}

      {addAgentOpen && (
        <AddAgentDialog
          onClose={() => setAddAgentOpen(false)}
          onAdded={state.refreshAgents}
        />
      )}
    </Layout>
  );
}

export default App;
