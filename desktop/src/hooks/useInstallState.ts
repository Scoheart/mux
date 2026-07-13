import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  listRegistry,
  listRegistryAll,
  listAgents,
  scanInstalled,
  applyInstall,
  uninstall,
  enableMcp,
  disableMcp,
  deleteMcp,
  cellKey,
  listCustomRegistryKeys,
  importDiscovered,
  listSources,
  subscribeSource,
  addLocalSourceDialog,
  refreshSource,
  setSourceEnabled,
  removeSource,
  importPastedConfig,
} from "../lib/api";
import type { RegistryEntry, CatalogItem, AgentInfo, InstalledMcp, SourceView } from "../lib/types";
import { keyOf, transportOf, installedKey } from "../lib/mcp";
import { formatError } from "../lib/format";
import { useToast } from "../components/Toast";

export interface InstallState {
  entries: RegistryEntry[];
  /** Every entry copy across all sources (not deduped), each flagged in_effect.
   *  Drives the Registry so shadowed copies stay visible. `entries` remains the
   *  deduped effective set used for installs/scans. */
  catalog: CatalogItem[];
  agents: AgentInfo[];
  installed: InstalledMcp[];
  loading: boolean;
  pending: Set<string>;
  /** Agents a server is actively installed in. Keyed by the composite registry
   *  key (`name::transport`, see keyOf/installedKey), so stdio and http variants
   *  of the same server are tracked independently. */
  agentsForServer(serverKey: string): string[];
  /** Composite keys (`name::transport`) of registry entries that have a user
   *  override (vs builtin). */
  customKeys: Set<string>;
  toggle(entry: RegistryEntry, agentId: string): Promise<void>;
  /** Enable (write back from the disabled snapshot) or disable (remove from the
   *  file but remember) a server for an agent. */
  setEnabled(entry: RegistryEntry, agentId: string, on: boolean): Promise<void>;
  /** Hard-delete a server from an agent (file + disabled snapshot). */
  remove(entry: RegistryEntry, agentId: string): Promise<void>;
  rescan(): Promise<InstalledMcp[]>;
  /** Full sync: import newly-discovered agent MCPs into the registry, then
   *  refresh the registry + the install scan. Used by the toolbar refresh. */
  refreshAll(): Promise<void>;
  refreshRegistry(): Promise<RegistryEntry[]>;
  refreshAgents(): Promise<AgentInfo[]>;
  /** Catalog sources: subscribed remote URLs + local files. */
  sources: SourceView[];
  refreshSources(): Promise<SourceView[]>;
  subscribe(url: string, name?: string): Promise<SourceView>;
  pickLocalSource(): Promise<SourceView | null>;
  /** Re-run agent discovery (the 自动探索 source's refresh). */
  rescanDiscovered(): Promise<void>;
  refreshOneSource(id: string): Promise<void>;
  toggleSource(id: string, enabled: boolean): Promise<void>;
  deleteSource(id: string): Promise<void>;
  /** Parse a pasted config blob and add its servers to the manual source. */
  importPaste(text: string): Promise<string[]>;
}

export function useInstallState(): InstallState {
  const [entries, setEntries] = useState<RegistryEntry[]>([]);
  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [installed, setInstalled] = useState<InstalledMcp[]>([]);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [customKeys, setCustomKeys] = useState<Set<string>>(new Set());
  const [sources, setSources] = useState<SourceView[]>([]);
  const toast = useToast();

  // Ref to prevent stale closure issues with pending state
  const pendingRef = useRef(pending);
  pendingRef.current = pending;

  const doScan = useCallback(async () => {
    const data = await scanInstalled();
    setInstalled(data);
    return data;
  }, []);

  const refreshAgents = useCallback(async () => {
    const data = await listAgents();
    setAgents(data);
    return data;
  }, []);

  const refreshRegistry = useCallback(async () => {
    const [data] = await Promise.all([
      listRegistry().then((d) => {
        setEntries(d);
        return d;
      }),
      listRegistryAll()
        .then(setCatalog)
        .catch(console.error),
      listCustomRegistryKeys()
        .then((keys) => setCustomKeys(new Set(keys)))
        .catch(console.error),
    ]);
    return data;
  }, []);

  const refreshSources = useCallback(async () => {
    const data = await listSources();
    setSources(data);
    return data;
  }, []);

  // Full sync (toolbar refresh): detect any newly-added agent MCPs into the
  // registry, then refresh the registry + the install scan together.
  const refreshAll = useCallback(async () => {
    await importDiscovered().catch(console.error);
    await Promise.all([
      refreshRegistry().catch(console.error),
      refreshSources().catch(console.error),
      doScan().catch(console.error),
    ]);
  }, [refreshRegistry, refreshSources, doScan]);

  useEffect(() => {
    Promise.all([
      refreshRegistry().catch(console.error),
      refreshSources().catch(console.error),
      listAgents().then(setAgents).catch(console.error),
      doScan().catch(console.error),
    ]).finally(() => setLoading(false));
  }, [doScan, refreshRegistry, refreshSources]);

  // These lookups care about *active* installs only, so they filter to enabled
  // rows. Disabled servers (remembered in ~/.mux/disabled.json, enabled === false)
  // are surfaced separately in the Agent view via `installed`.

  // Cell keys (serverKey + agent) of active installs — drives toggle()'s
  // install-vs-uninstall decision.
  const installedCells = useMemo(() => {
    const cells = new Set<string>();
    for (const item of installed) {
      if (item.scope === "global" && item.enabled) {
        cells.add(cellKey(installedKey(item), item.agent));
      }
    }
    return cells;
  }, [installed]);

  // Map: serverKey → agentId[] (global, enabled)
  const serverToAgents = useMemo(() => {
    const m = new Map<string, string[]>();
    for (const item of installed) {
      if (item.scope === "global" && item.enabled) {
        const arr = m.get(installedKey(item)) ?? [];
        arr.push(item.agent);
        m.set(installedKey(item), arr);
      }
    }
    return m;
  }, [installed]);



  const agentsForServer = useCallback(
    (serverKey: string) => serverToAgents.get(serverKey) ?? [],
    [serverToAgents]
  );

  const toggle = useCallback(
    async (entry: RegistryEntry, agentId: string) => {
      const serverName = entry.name;
      const transport = transportOf(entry);
      const serverKey = keyOf(entry);
      const key = cellKey(serverKey, agentId);
      if (pendingRef.current.has(key)) return;

      const wasInstalled = installedCells.has(key);

      // Mark pending
      setPending((prev) => new Set(prev).add(key));

      // Optimistic update
      if (wasInstalled) {
        setInstalled((prev) =>
          prev.filter(
            (item) =>
              !(item.name === serverName && item.transport === transport &&
                item.agent === agentId && item.scope === "global")
          )
        );
      } else {
        setInstalled((prev) => [
          ...prev,
          { name: serverName, agent: agentId, scope: "global", file_path: "", transport, customized: false, enabled: true },
        ]);
      }

      try {
        const req = {
          server_name: serverName,
          transport,
          agents: [agentId],
          overrides: {},
        };
        if (wasInstalled) {
          await uninstall(req);
        } else {
          await applyInstall(req);
        }
        // Authoritative re-scan
        await doScan();
      } catch (err) {
        // Revert via re-scan
        await doScan().catch(console.error);
        const msg = formatError(err);
        toast.show({ kind: "error", msg: `操作失败: ${msg}` });
      } finally {
        setPending((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [installedCells, doScan, toast]
  );

  // Enable/disable an installed server. Disabling removes it from the agent file
  // but keeps the row (flipped off); enabling writes the remembered config back.
  const setEnabled = useCallback(
    async (entry: RegistryEntry, agentId: string, on: boolean) => {
      const serverName = entry.name;
      const transport = transportOf(entry);
      const key = cellKey(keyOf(entry), agentId);
      if (pendingRef.current.has(key)) return;

      setPending((prev) => new Set(prev).add(key));
      // Optimistic: flip the matching row's enabled flag in place.
      setInstalled((prev) =>
        prev.map((item) =>
          item.name === serverName && item.transport === transport &&
          item.agent === agentId && item.scope === "global"
            ? { ...item, enabled: on }
            : item
        )
      );

      try {
        const req = {
          server_name: serverName,
          transport,
          agents: [agentId],
          overrides: {},
        };
        if (on) {
          await enableMcp(req);
        } else {
          await disableMcp(req);
        }
        await doScan();
      } catch (err) {
        await doScan().catch(console.error);
        const msg = formatError(err);
        toast.show({ kind: "error", msg: `操作失败: ${msg}` });
      } finally {
        setPending((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [doScan, toast]
  );

  // Hard-delete a server from an agent: gone from the file AND the disabled store.
  const remove = useCallback(
    async (entry: RegistryEntry, agentId: string) => {
      const serverName = entry.name;
      const transport = transportOf(entry);
      const key = cellKey(keyOf(entry), agentId);
      if (pendingRef.current.has(key)) return;

      setPending((prev) => new Set(prev).add(key));
      // Optimistic: drop the row entirely.
      setInstalled((prev) =>
        prev.filter(
          (item) =>
            !(item.name === serverName && item.transport === transport &&
              item.agent === agentId && item.scope === "global")
        )
      );

      try {
        await deleteMcp({
          server_name: serverName,
          transport,
          agents: [agentId],
          overrides: {},
        });
        await doScan();
      } catch (err) {
        await doScan().catch(console.error);
        const msg = formatError(err);
        toast.show({ kind: "error", msg: `操作失败: ${msg}` });
      } finally {
        setPending((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [doScan, toast]
  );

  // ── Source actions: each mutation refreshes the source list AND the catalog
  // (the aggregated registry changes when a source is added/toggled/removed).
  const afterSourceChange = useCallback(async () => {
    await Promise.all([
      refreshSources().catch(console.error),
      refreshRegistry().catch(console.error),
    ]);
  }, [refreshSources, refreshRegistry]);

  const subscribe = useCallback(
    async (url: string, name?: string) => {
      const v = await subscribeSource(url, name);
      await afterSourceChange();
      return v;
    },
    [afterSourceChange]
  );

  const pickLocalSource = useCallback(async () => {
    const v = await addLocalSourceDialog();
    if (v) await afterSourceChange();
    return v;
  }, [afterSourceChange]);

  const rescanDiscovered = useCallback(async () => {
    await importDiscovered().catch(console.error);
    await Promise.all([
      refreshRegistry().catch(console.error),
      refreshSources().catch(console.error),
      doScan().catch(console.error),
    ]);
  }, [refreshRegistry, refreshSources, doScan]);

  const refreshOneSource = useCallback(
    async (id: string) => {
      await refreshSource(id);
      await afterSourceChange();
    },
    [afterSourceChange]
  );

  const toggleSource = useCallback(
    async (id: string, enabled: boolean) => {
      await setSourceEnabled(id, enabled);
      await afterSourceChange();
    },
    [afterSourceChange]
  );

  const deleteSource = useCallback(
    async (id: string) => {
      await removeSource(id);
      await afterSourceChange();
    },
    [afterSourceChange]
  );

  const importPaste = useCallback(
    async (text: string) => {
      const names = await importPastedConfig(text);
      // New entries land in the manual source → refresh the catalog + source list.
      await Promise.all([refreshRegistry().catch(console.error), refreshSources().catch(console.error)]);
      return names;
    },
    [refreshRegistry, refreshSources]
  );

  return {
    entries,
    catalog,
    agents,
    installed,
    loading,
    pending,
    agentsForServer,
    customKeys,
    toggle,
    setEnabled,
    remove,
    rescan: doScan,
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
