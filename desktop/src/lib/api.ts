import { invoke } from "@tauri-apps/api/core";
import type { RegistryEntry, AgentInfo, AgentDefinitionInput, InstalledMcp, PlannedWrite, InstallRequest } from "./types";

export const listRegistry = () => invoke<RegistryEntry[]>("list_registry");
export const upsertRegistry = (entry: RegistryEntry) =>
  invoke<void>("upsert_registry_entry", { entry });
export const deleteRegistry = (name: string, transport: string) =>
  invoke<void>("delete_registry_entry", { name, transport });
export const listCustomRegistryKeys = () =>
  invoke<string[]>("list_custom_registry_keys");
export const listAgents = () => invoke<AgentInfo[]>("list_agents");
export const addAgent = (id: string, def: AgentDefinitionInput) =>
  invoke<void>("add_agent", { id, def });
export const updateAgent = (id: string, def: AgentDefinitionInput) =>
  invoke<void>("update_agent", { id, def });
export const scanInstalled = (projectDir?: string) =>
  invoke<InstalledMcp[]>("scan_installed", { projectDir: projectDir ?? null });
/** Register any discovered-but-unregistered agent MCPs into the registry. Returns
 *  the number newly imported. */
export const importDiscovered = (projectDir?: string) =>
  invoke<number>("import_discovered", { projectDir: projectDir ?? null });
export const previewInstall = (req: InstallRequest) =>
  invoke<PlannedWrite[]>("preview_install", { req });
export const applyInstall = (req: InstallRequest) =>
  invoke<void>("apply_install", { req });
export const uninstall = (req: InstallRequest) =>
  invoke<void>("uninstall", { req });
/** Remove a server from the agent file but remember its config so it can be
 *  re-enabled later (the row stays in the UI as an "off" toggle). */
export const disableMcp = (req: InstallRequest) =>
  invoke<void>("disable_mcp", { req });
/** Restore a previously disabled server from its remembered config snapshot. */
export const enableMcp = (req: InstallRequest) =>
  invoke<void>("enable_mcp", { req });
/** Hard-delete: remove from the agent file AND forget any disabled snapshot. */
export const deleteMcp = (req: InstallRequest) =>
  invoke<void>("delete_mcp", { req });

/** Stable key for an (server, agent) cell in the matrix. `serverKey` is the
 *  composite registry key (`name::transport`), so stdio and http variants of the
 *  same server get distinct cells. */
export const cellKey = (serverKey: string, agentId: string) =>
  `${serverKey}|${agentId}`;
