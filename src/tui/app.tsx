import { useState, useEffect } from "react";
import { Box, Text, useApp, useStdout } from "ink";
import Spinner from "ink-spinner";
import { McpList, type McpSelection, type ScopeTab } from "./mcp-list.js";
import { ConfigPanel } from "./config-panel.js";
import { ConfirmView } from "./confirm-view.js";
import { ScanView, type ScannedItem } from "./scan-view.js";
import { AddView } from "./add-view.js";
import { RegistryApplyPanel } from "./registry-apply-panel.js";
import { RegistryEditView } from "./registry-edit-view.js";
import { RegistryAddView } from "./registry-add-view.js";
import { RegistryDeleteConfirm } from "./registry-delete-confirm.js";
import { expandTilde } from "../utils/path.js";
import { MCP_HUB_DIR, BACKUPS_DIR } from "../constants.js";
import { readRegistry, writeRegistryEntry, writeDiscoveredEntry, removeRegistryEntry, keyOf, transportOf } from "../core/registry.js";
import { readAgents, getEnabledAgents, writeAgents } from "../core/agents.js";
import { loadSettings, mutateSettings } from "../core/settings.js";
import type { McpStdioConfig, McpHttpConfig, Scope, AgentsConfig } from "../types.js";
import { scanAgents } from "../core/scanner.js";
import { computeDiff } from "../core/differ.js";
import { applyDiffs } from "../core/applier.js";
import { writeState } from "../core/state.js";
import type { RegistryEntry, ActiveMcp, DiffEntry } from "../types.js";
import { join, resolve } from "node:path";

type View = "loading" | "first-scan" | "list" | "config" | "confirm" | "add" | "registry-apply" | "registry-edit" | "registry-add" | "registry-delete" | "done";

export function App() {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const [view, setView] = useState<View>("loading");
  const [entries, setEntries] = useState<RegistryEntry[]>([]);
  const [selections, setSelections] = useState<McpSelection[]>([]);
  const [enabledAgentNames, setEnabledTargetNames] = useState<string[]>([]);
  const [agentsConfigState, setAgentsConfigState] = useState<AgentsConfig>({ agents: {} });
  const [diffs, setDiffs] = useState<DiffEntry[]>([]);
  const [configMcp, setConfigMcp] = useState<string | null>(null);
  const [configTab, setConfigTab] = useState<ScopeTab>("project");
  const [activeTab, setActiveTab] = useState<ScopeTab>("project");
  const [addTab, setAddTab] = useState<ScopeTab>("project");
  const [listCursor, setListCursor] = useState(0);
  const [message, setMessage] = useState("");
  const [scannedItems, setScannedItems] = useState<ScannedItem[]>([]);
  const [registryMcp, setRegistryMcp] = useState<string | null>(null);
  const [returnToList, setReturnToList] = useState(false);

  useEffect(() => {
    const agentsConfig = readAgents();
    const projectDir = resolve(process.cwd());

    // Check if import has been completed before
    const hasImported = !!loadSettings().imported;

    // Scan ALL agents (including disabled) to discover MCPs
    const scanned = scanAgents(agentsConfig, projectDir, true);

    if (!hasImported && scanned.length > 0) {
      // Not yet imported — show scan view with import/skip choice
      const discoveredItems: ScannedItem[] = scanned.map((s) => {
        const raw = s.config as unknown as Record<string, unknown>;
        const mcpType: "stdio" | "http" = "command" in raw ? "stdio" : "http";
        return {
          name: s.name,
          type: mcpType,
          target: s.source.agent,
          scope: s.source.scope,
        };
      });
      // Deduplicate by name (keep first occurrence)
      const seen = new Set<string>();
      const uniqueItems = discoveredItems.filter((item) => {
        if (seen.has(item.name)) return false;
        seen.add(item.name);
        return true;
      });

      setScannedItems(uniqueItems);

      // Load existing registry + selections for the list view (used after skip)
      const reg = readRegistry().sort((a, b) => a.name.localeCompare(b.name));
      const enabled = getEnabledAgents(agentsConfig);
      const agentNames = Object.keys(enabled);
      const initial: McpSelection[] = reg.map((entry) => {
        const found = scanned.filter((s) => s.name === entry.name);
        const projectAgents = [...new Set(
          found.filter((f) => f.source.scope === "project").map((f) => f.source.agent)
        )];
        const globalAgents = [...new Set(
          found.filter((f) => f.source.scope === "global").map((f) => f.source.agent)
        )];
        return { name: entry.name, projectAgents, globalAgents };
      });
      setEntries(reg);
      setSelections(initial);
      setEnabledTargetNames(agentNames);
      setAgentsConfigState(agentsConfig);
      setView("first-scan");
      return;
    }

    // Already imported or nothing to scan — go straight to list
    const reg = readRegistry().sort((a, b) => a.name.localeCompare(b.name));
    const enabled = getEnabledAgents(agentsConfig);
    const agentNames = Object.keys(enabled);

    const initial: McpSelection[] = reg.map((entry) => {
      const found = scanned.filter((s) => s.name === entry.name);
      const projectAgents = [...new Set(
        found.filter((f) => f.source.scope === "project").map((f) => f.source.agent)
      )];
      const globalAgents = [...new Set(
        found.filter((f) => f.source.scope === "global").map((f) => f.source.agent)
      )];
      return { name: entry.name, projectAgents, globalAgents };
    });

    setEntries(reg);
    setSelections(initial);
    setEnabledTargetNames(agentNames);
    setAgentsConfigState(agentsConfig);
    setView("list");
  }, []);

  const handleAgentToggle = (name: string) => {
    setAgentsConfigState((prev) => {
      const updated: AgentsConfig = {
        agents: {
          ...prev.agents,
          [name]: { ...prev.agents[name], enabled: !prev.agents[name].enabled },
        },
      };
      writeAgents(updated);
      setEnabledTargetNames(Object.keys(updated.agents).filter((k) => updated.agents[k].enabled));
      return updated;
    });
  };

  const handleToggle = (name: string, tab: ScopeTab) => {
    if (tab === "registry") return; // Registry uses dedicated handlers
    setSelections((prev) =>
      prev.map((s) => {
        if (s.name !== name) return s;
        if (tab === "project") {
          const newAgents = s.projectAgents.length > 0 ? [] : [...enabledAgentNames];
          return { ...s, projectAgents: newAgents };
        } else {
          const newAgents = s.globalAgents.length > 0 ? [] : [...enabledAgentNames];
          return { ...s, globalAgents: newAgents };
        }
      })
    );
  };

  const handleApply = () => {
    const agentsConfig = readAgents();
    const projectDir = resolve(process.cwd());
    const scanned = scanAgents(agentsConfig, projectDir, true);

    // Convert selections to desired ActiveMcp list
    const desired: ActiveMcp[] = [];
    for (const sel of selections) {
      if (sel.projectAgents.length > 0) {
        desired.push({ name: sel.name, scope: "project", agents: sel.projectAgents });
      }
      if (sel.globalAgents.length > 0) {
        desired.push({ name: sel.name, scope: "global", agents: sel.globalAgents });
      }
    }

    const d = computeDiff(desired, scanned);
    setDiffs(d);

    if (d.length === 0) {
      setMessage("No changes to apply.");
      setView("done");
      setTimeout(() => exit(), 1500);
    } else {
      setView("confirm");
    }
  };

  const handleConfirm = () => {
    const hubDir = expandTilde(MCP_HUB_DIR);
    const agentsConfig = readAgents();
    const registry = readRegistry();
    const backupsDir = join(hubDir, BACKUPS_DIR);
    const projectDir = resolve(process.cwd());

    applyDiffs(diffs, agentsConfig, registry, backupsDir, projectDir);

    // Save state
    const active: ActiveMcp[] = [];
    for (const sel of selections) {
      if (sel.projectAgents.length > 0) {
        active.push({ name: sel.name, scope: "project", agents: sel.projectAgents });
      }
      if (sel.globalAgents.length > 0) {
        active.push({ name: sel.name, scope: "global", agents: sel.globalAgents });
      }
    }
    writeState({ active });

    setMessage(`✓ Applied ${diffs.length} changes successfully.`);
    setView("list");
    setTimeout(() => setMessage(""), 3000);
  };

  const handleOpenConfig = (mcpName: string, tab: ScopeTab) => {
    setConfigMcp(mcpName);
    setConfigTab(tab);
    setView("config");
  };

  const handleSaveConfig = (agents: string[]) => {
    setSelections((prev) =>
      prev.map((s) => {
        if (s.name !== configMcp) return s;
        if (configTab === "project") {
          return { ...s, projectAgents: agents };
        } else {
          return { ...s, globalAgents: agents };
        }
      })
    );
    setConfigMcp(null);
    setView("list");
  };

  const handleAdd = (tab: ScopeTab) => {
    setAddTab(tab);
    setView("add");
  };

  const handleAddSelect = (name: string) => {
    // After selecting an MCP to add, go to config panel to choose targets
    setConfigMcp(name);
    setConfigTab(addTab);
    setView("config");
  };

  const handleQuit = () => {
    exit();
  };

  // --- Registry handlers ---
  const handleRegistryApply = (name: string) => {
    setRegistryMcp(name);
    setView("registry-apply");
  };

  const handleRegistryApplySave = (scope: Scope, agents: string[]) => {
    setSelections((prev) =>
      prev.map((s) => {
        if (s.name !== registryMcp) return s;
        if (scope === "project" || scope === "both") {
          s = { ...s, projectAgents: agents };
        }
        if (scope === "global" || scope === "both") {
          s = { ...s, globalAgents: agents };
        }
        return s;
      })
    );
    setRegistryMcp(null);
    setView("list");
  };

  const handleRegistryEdit = (name: string) => {
    setRegistryMcp(name);
    setView("registry-edit");
  };

  const handleRegistryEditSave = (updatedEntry: RegistryEntry) => {
    // Preserve the recorded origin across edits.
    const prevOrigin = entries.find((e) => e.name === updatedEntry.name)?.origin;
    const entry = { ...updatedEntry, origin: updatedEntry.origin ?? prevOrigin };
    writeRegistryEntry(entry);
    setEntries((prev) =>
      prev.map((e) => (e.name === entry.name ? entry : e))
    );
    setRegistryMcp(null);
    setView("list");
  };

  const handleRegistryAdd = () => {
    setView("registry-add");
  };

  const handleRegistryAddSave = (newEntry: RegistryEntry) => {
    const entry: RegistryEntry = { ...newEntry, origin: newEntry.origin ?? { kind: "manual" } };
    writeRegistryEntry(entry);
    setEntries((prev) => [...prev, entry].sort((a, b) => a.name.localeCompare(b.name)));
    setSelections((prev) => [...prev, { name: entry.name, projectAgents: [], globalAgents: [] }]);
    setView("list");
  };

  const handleRegistryDelete = (name: string) => {
    setRegistryMcp(name);
    setView("registry-delete");
  };

  const handleRegistryDeleteConfirm = (alsoRemoveFromAgents: boolean) => {
    if (!registryMcp) return;
    const target = entries.find((e) => e.name === registryMcp);
    removeRegistryEntry(registryMcp, target ? transportOf(target) : "stdio");
    setEntries((prev) => prev.filter((e) => e.name !== registryMcp));

    if (alsoRemoveFromAgents) {
      // Clear from selections so apply will remove from targets
      setSelections((prev) =>
        prev.map((s) =>
          s.name === registryMcp ? { ...s, projectAgents: [], globalAgents: [] } : s
        )
      );
    } else {
      setSelections((prev) => prev.filter((s) => s.name !== registryMcp));
    }

    setRegistryMcp(null);
    setView("list");
  };

  if (view === "loading") {
    return (
      <Box>
        <Text color="cyan"><Spinner type="dots" /></Text>
        <Text> Scanning configurations...</Text>
      </Box>
    );
  }

  if (view === "first-scan") {
    return (
      <ScanView
        items={scannedItems}
        onComplete={(action) => {
          if (action === "import") {
            // Import scanned MCPs into registry
            const existingRegistry = readRegistry();
            const existingKeys = new Set(existingRegistry.map(keyOf));
            const agentsConfig = readAgents();
            const projectDir = resolve(process.cwd());
            const scanned = scanAgents(agentsConfig, projectDir, true);

            for (const s of scanned) {
              const entry: RegistryEntry = {
                name: s.name,
                description: "",
                tags: [],
                config: {},
                origin: { kind: "discovered", agent: s.source.agent, scope: s.source.scope },
              };
              const raw = s.config as unknown as Record<string, unknown>;
              if ("command" in raw) {
                entry.config.stdio = s.config as McpStdioConfig;
              } else {
                const url = (raw.url ?? raw.httpUrl ?? "") as string;
                const httpConfig: McpHttpConfig = {
                  type: (raw.type as "http" | "sse") ?? "http",
                  url,
                };
                if (raw.headers) httpConfig.headers = raw.headers as Record<string, string>;
                entry.config.http = httpConfig;
              }
              const k = keyOf(entry);
              if (existingKeys.has(k)) continue;
              writeDiscoveredEntry(entry);
              existingKeys.add(k);
            }

            // Mark as imported so we don't ask again
            mutateSettings((s) => {
              s.imported = new Date().toISOString();
            });

            // Refresh entries and selections
            const reg = readRegistry().sort((a, b) => a.name.localeCompare(b.name));
            const initial: McpSelection[] = reg.map((entry) => {
              const found = scanned.filter((sc) => sc.name === entry.name);
              const projectAgents = [...new Set(
                found.filter((f) => f.source.scope === "project").map((f) => f.source.agent)
              )];
              const globalAgents = [...new Set(
                found.filter((f) => f.source.scope === "global").map((f) => f.source.agent)
              )];
              return { name: entry.name, projectAgents, globalAgents };
            });
            setEntries(reg);
            setSelections(initial);
          }
          // Skip: don't write marker, so next launch will ask again
          setView("list");
        }}
      />
    );
  }

  if (view === "config" && configMcp) {
    const sel = selections.find((s) => s.name === configMcp);
    const currentAgents = sel
      ? (configTab === "project" ? sel.projectAgents : sel.globalAgents)
      : [];
    return (
      <ConfigPanel
        mcpName={configMcp}
        scopeLabel={configTab}
        currentAgents={currentAgents}
        availableAgents={enabledAgentNames}
        onSave={handleSaveConfig}
        onCancel={() => { setConfigMcp(null); setReturnToList(true); setView("list"); }}
      />
    );
  }

  if (view === "add") {
    const alreadyAdded = selections
      .filter((s) => (addTab === "project" ? s.projectAgents : s.globalAgents).length > 0)
      .map((s) => s.name);
    return (
      <AddView
        entries={entries}
        alreadyAdded={alreadyAdded}
        scopeLabel={addTab}
        onSelect={handleAddSelect}
        onCancel={() => setView("list")}
      />
    );
  }

  if (view === "registry-apply" && registryMcp) {
    return (
      <RegistryApplyPanel
        mcpName={registryMcp}
        availableAgents={enabledAgentNames}
        onSave={handleRegistryApplySave}
        onCancel={() => { setRegistryMcp(null); setReturnToList(true); setView("list"); }}
      />
    );
  }

  if (view === "registry-edit" && registryMcp) {
    const entry = entries.find((e) => e.name === registryMcp);
    if (entry) {
      return (
        <RegistryEditView
          entry={entry}
          onSave={handleRegistryEditSave}
          onCancel={() => { setRegistryMcp(null); setReturnToList(true); setView("list"); }}
        />
      );
    }
  }

  if (view === "registry-add") {
    return (
      <RegistryAddView
        existingNames={entries.map((e) => e.name)}
        onSave={handleRegistryAddSave}
        onCancel={() => setView("list")}
      />
    );
  }

  if (view === "registry-delete" && registryMcp) {
    return (
      <RegistryDeleteConfirm
        mcpName={registryMcp}
        onConfirm={handleRegistryDeleteConfirm}
        onCancel={() => { setRegistryMcp(null); setReturnToList(true); setView("list"); }}
      />
    );
  }

  if (view === "list") {
    const focus = returnToList ? "list" as const : undefined;
    if (returnToList) setReturnToList(false);
    return (
      <McpList
        entries={entries}
        selections={selections}
        enabledAgents={enabledAgentNames}
        activeTab={activeTab}
        cursor={listCursor}
        initialFocus={focus}
        message={message}
        agentsConfig={agentsConfigState}
        onCursorChange={setListCursor}
        onTabChange={setActiveTab}
        onToggle={handleToggle}
        onOpenConfig={handleOpenConfig}
        onAdd={handleAdd}
        onApply={handleApply}
        onQuit={handleQuit}
        onRegistryApply={handleRegistryApply}
        onRegistryEdit={handleRegistryEdit}
        onRegistryAdd={handleRegistryAdd}
        onRegistryDelete={handleRegistryDelete}
        onAgentToggle={handleAgentToggle}
      />
    );
  }

  if (view === "confirm") {
    return (
      <ConfirmView
        diffs={diffs}
        onConfirm={handleConfirm}
        onCancel={() => setView("list")}
      />
    );
  }

  return (
    <Box flexDirection="column" marginTop={1}>
      <Text color="green">{message || "Done."}</Text>
    </Box>
  );
}
