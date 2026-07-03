import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  listRegistry,
  listAgents,
  scanInstalled,
  applyInstall,
  uninstall,
  enableMcp,
  disableMcp,
  deleteMcp,
  cellKey,
  listCustomRegistryKeys,
  upsertRegistry,
  importDiscovered,
  listSources,
  subscribeSource,
  addLocalSourceDialog,
  refreshSource,
  setSourceEnabled,
  removeSource,
  importPastedConfig,
} from "../lib/api";
import type { RegistryEntry, AgentInfo, InstalledMcp, SourceView } from "../lib/types";
import { keyOf, transportOf, installedKey } from "../lib/mcp";
import { useToast } from "../components/Toast";

export interface InstallState {
  entries: RegistryEntry[];
  agents: AgentInfo[];
  installed: InstalledMcp[];
  loading: boolean;
  pending: Set<string>;
  /** All cell/server lookups are keyed by the composite registry key
   *  (`name::transport`, see keyOf/installedKey), so stdio and http variants of
   *  the same server are tracked independently. */
  isInstalled(serverKey: string, agentId: string): boolean;
  customizedOf(serverKey: string, agentId: string): boolean;
  agentsForServer(serverKey: string): string[];
  serversForAgent(agentId: string): string[];
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

  // Guards the one-shot origin backfill so it runs at most once per mount.
  const backfilledRef = useRef(false);

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

  // The Matrix/Registry views care about *active* installs only, so these maps
  // filter to enabled rows. Disabled servers (remembered in ~/.mux/disabled.json,
  // enabled === false) are surfaced separately in the Agent view via `installed`.

  // Map from cellKey(serverKey, agent) → customized (global, enabled)
  const installedMap = useMemo(() => {
    const m = new Map<string, boolean>();
    for (const item of installed) {
      if (item.scope === "global" && item.enabled) {
        m.set(cellKey(installedKey(item), item.agent), item.customized ?? false);
      }
    }
    return m;
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

  // Map: agentId → serverKey[] (global, enabled)
  const agentToServers = useMemo(() => {
    const m = new Map<string, string[]>();
    for (const item of installed) {
      if (item.scope === "global" && item.enabled) {
        const arr = m.get(item.agent) ?? [];
        arr.push(installedKey(item));
        m.set(item.agent, arr);
      }
    }
    return m;
  }, [installed]);

  // One-shot origin backfill: legacy custom entries imported before the `origin`
  // field existed have no recorded source. Once the initial scan has landed, for
  // any such entry that IS currently installed in an agent, stamp its source app
  // (discovered + that agent) and persist it so the「来自 X」label survives even if
  // the app later removes the server. Orphaned (未使用) entries are left untouched —
  // we can't know their source, so they render the generic 机器探索 fallback rather
  // than being mislabeled durably.
  useEffect(() => {
    if (loading || backfilledRef.current) return;
    if (entries.length === 0 || installed.length === 0) return;
    const toStamp = entries.filter(
      (e) => customKeys.has(keyOf(e)) && !e.origin && (serverToAgents.get(keyOf(e))?.length ?? 0) > 0
    );
    backfilledRef.current = true;
    if (toStamp.length === 0) return;
    (async () => {
      for (const e of toStamp) {
        const agent = serverToAgents.get(keyOf(e))![0];
        await upsertRegistry({ ...e, origin: { kind: "discovered", agent, scope: "global" } }).catch(
          console.error
        );
      }
      await refreshRegistry().catch(console.error);
    })();
  }, [loading, entries, installed, customKeys, serverToAgents, refreshRegistry]);

  const isInstalled = useCallback(
    (serverKey: string, agentId: string) => installedMap.has(cellKey(serverKey, agentId)),
    [installedMap]
  );

  const customizedOf = useCallback(
    (serverKey: string, agentId: string) => installedMap.get(cellKey(serverKey, agentId)) ?? false,
    [installedMap]
  );

  const agentsForServer = useCallback(
    (serverKey: string) => serverToAgents.get(serverKey) ?? [],
    [serverToAgents]
  );

  const serversForAgent = useCallback(
    (agentId: string) => agentToServers.get(agentId) ?? [],
    [agentToServers]
  );

  const toggle = useCallback(
    async (entry: RegistryEntry, agentId: string) => {
      const serverName = entry.name;
      const transport = transportOf(entry);
      const serverKey = keyOf(entry);
      const key = cellKey(serverKey, agentId);
      if (pendingRef.current.has(key)) return;

      const wasInstalled = installedMap.has(key);

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
          scope: "global" as const,
          agents: [agentId],
          project_dir: undefined,
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
        const msg = Array.isArray(err) ? err.join("; ") : String(err);
        toast.show({ kind: "error", msg: `操作失败: ${msg}` });
      } finally {
        setPending((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [installedMap, doScan, toast]
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
          scope: "global" as const,
          agents: [agentId],
          project_dir: undefined,
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
        const msg = Array.isArray(err) ? err.join("; ") : String(err);
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
          scope: "global" as const,
          agents: [agentId],
          project_dir: undefined,
          overrides: {},
        });
        await doScan();
      } catch (err) {
        await doScan().catch(console.error);
        const msg = Array.isArray(err) ? err.join("; ") : String(err);
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
    agents,
    installed,
    loading,
    pending,
    isInstalled,
    customizedOf,
    agentsForServer,
    serversForAgent,
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
