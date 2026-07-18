import { useCallback, useEffect, useMemo, useState } from "react";
import {
  addLocalSourceDialog,
  subscribeSource,
  importPastedConfig,
  listAgents,
  listCustomRegistryKeys,
  listRegistry,
  listRegistryAll,
  listSources,
  refreshSource,
  removeSource,
  scanInstalled,
  setSourceEnabled,
} from "../lib/api";
import type {
  AgentInfo,
  CatalogItem,
  InstalledMcp,
  RegistryEntry,
  SourceView,
} from "../lib/types";
import { installedKey } from "../lib/mcp";

/**
 * Read model for the MCP asset library and observed Agent files. Mutating an
 * Agent's MCP usage is intentionally absent: all relationships go through
 * useConsumptionState's plan/review/commit lifecycle.
 */
export interface InstallState {
  entries: RegistryEntry[];
  catalog: CatalogItem[];
  agents: AgentInfo[];
  installed: InstalledMcp[];
  loading: boolean;
  agentsForServer(serverKey: string): string[];
  customKeys: Set<string>;
  rescan(): Promise<InstalledMcp[]>;
  refreshAll(): Promise<void>;
  refreshRegistry(): Promise<RegistryEntry[]>;
  refreshAgents(): Promise<AgentInfo[]>;
  sources: SourceView[];
  refreshSources(): Promise<SourceView[]>;
  subscribe(url: string, name?: string): Promise<SourceView>;
  pickLocalSource(): Promise<SourceView | null>;
  rescanDiscovered(): Promise<void>;
  refreshOneSource(id: string): Promise<void>;
  toggleSource(id: string, enabled: boolean): Promise<void>;
  deleteSource(id: string): Promise<void>;
  importPaste(text: string): Promise<string[]>;
}

export function useInstallState(): InstallState {
  const [entries, setEntries] = useState<RegistryEntry[]>([]);
  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [installed, setInstalled] = useState<InstalledMcp[]>([]);
  const [loading, setLoading] = useState(true);
  const [customKeys, setCustomKeys] = useState<Set<string>>(new Set());
  const [sources, setSources] = useState<SourceView[]>([]);

  const rescan = useCallback(async () => {
    const next = await scanInstalled();
    setInstalled(next);
    return next;
  }, []);

  const refreshAgents = useCallback(async () => {
    const next = await listAgents();
    setAgents(next);
    return next;
  }, []);

  const refreshRegistry = useCallback(async () => {
    const next = await listRegistry();
    setEntries(next);
    await Promise.all([
      listRegistryAll().then(setCatalog),
      listCustomRegistryKeys().then((keys) => setCustomKeys(new Set(keys))),
    ]);
    return next;
  }, []);

  const refreshSources = useCallback(async () => {
    const next = await listSources();
    setSources(next);
    return next;
  }, []);

  const refreshAll = useCallback(async () => {
    await Promise.all([
      refreshRegistry().catch(console.error),
      refreshSources().catch(console.error),
      rescan().catch(console.error),
    ]);
  }, [refreshRegistry, refreshSources, rescan]);

  useEffect(() => {
    Promise.all([
      refreshRegistry().catch(console.error),
      refreshSources().catch(console.error),
      refreshAgents().catch(console.error),
      rescan().catch(console.error),
    ]).finally(() => setLoading(false));
  }, [refreshAgents, refreshRegistry, refreshSources, rescan]);

  const serverToAgents = useMemo(() => {
    const result = new Map<string, string[]>();
    for (const item of installed) {
      if (item.scope !== "global" || !item.enabled) continue;
      const rows = result.get(installedKey(item)) ?? [];
      rows.push(item.agent);
      result.set(installedKey(item), rows);
    }
    return result;
  }, [installed]);
  const agentsForServer = useCallback(
    (serverKey: string) => serverToAgents.get(serverKey) ?? [],
    [serverToAgents],
  );

  const afterSourceChange = useCallback(async () => {
    await Promise.all([refreshSources(), refreshRegistry()]);
  }, [refreshRegistry, refreshSources]);

  const subscribe = useCallback(async (url: string, name?: string) => {
    const source = await subscribeSource(url, name);
    await afterSourceChange();
    return source;
  }, [afterSourceChange]);

  const pickLocalSource = useCallback(async () => {
    const source = await addLocalSourceDialog();
    if (source) await afterSourceChange();
    return source;
  }, [afterSourceChange]);

  const rescanDiscovered = useCallback(async () => {
    await rescan();
  }, [rescan]);

  const refreshOneSource = useCallback(async (id: string) => {
    await refreshSource(id);
    await afterSourceChange();
  }, [afterSourceChange]);

  const toggleSource = useCallback(async (id: string, enabled: boolean) => {
    await setSourceEnabled(id, enabled);
    await afterSourceChange();
  }, [afterSourceChange]);

  const deleteSource = useCallback(async (id: string) => {
    await removeSource(id);
    await afterSourceChange();
  }, [afterSourceChange]);

  const importPaste = useCallback(async (text: string) => {
    const names = await importPastedConfig(text);
    await afterSourceChange();
    return names;
  }, [afterSourceChange]);

  return {
    entries,
    catalog,
    agents,
    installed,
    loading,
    agentsForServer,
    customKeys,
    rescan,
    refreshAll,
    refreshRegistry,
    refreshAgents,
    sources,
    refreshSources,
    subscribe,
    pickLocalSource,
    rescanDiscovered,
    refreshOneSource,
    toggleSource,
    deleteSource,
    importPaste,
  };
}
