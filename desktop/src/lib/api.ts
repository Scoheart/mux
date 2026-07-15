import { invoke } from "@tauri-apps/api/core";
import type { RegistryEntry, CatalogItem, AgentInfo, AgentDefinitionInput, InstalledMcp, InstallRequest, SourceView, ResyncOutcome, ModelProfile, ModelProfileView, ModelAgentView, ModelApplyResult } from "./types";

export const listRegistry = () => invoke<RegistryEntry[]>("list_registry");
export const listModelProfiles = () =>
  invoke<ModelProfileView[]>("list_model_profiles");
export const saveModelProfile = (profile: ModelProfile, credential?: string) =>
  invoke<void>("save_model_profile", { profile, credential: credential ?? null });
export const deleteModelProfile = (id: string) =>
  invoke<void>("delete_model_profile", { id });
export const listModelAgents = () =>
  invoke<ModelAgentView[]>("list_model_agents");
export const applyModelProfile = (agentId: string, profileId: string) =>
  invoke<ModelApplyResult>("apply_model_profile", { agentId, profileId });
/** All entry copies across sources (not deduped), each flagged in_effect. */
export const listRegistryAll = () => invoke<CatalogItem[]>("list_registry_all");
/** 桌面包内 mux CLI 的安装状态（sidecar → ~/.local/bin 软链）。 */
export type CliStatus = { bundled: boolean; installed: boolean; link_path: string; in_path: boolean };
export const cliStatus = () => invoke<CliStatus>("cli_status");
export const installCli = () => invoke<CliStatus>("install_cli");
export type UpdateEnvironment = {
  canSelfUpdate: boolean;
  reason: "disk-image" | "app-translocation" | "read-only-volume" | null;
};
export const updateEnvironment = () =>
  invoke<UpdateEnvironment>("update_environment");
/** Save auto-syncs the new config to installed agents; returns their names. */
export const upsertRegistry = (entry: RegistryEntry) =>
  invoke<string[]>("upsert_registry_entry", { entry });
export const deleteRegistry = (name: string, transport: string) =>
  invoke<string[]>("delete_registry_entry", { name, transport });
/** Re-stamp an entry's current config into agents that have it installed (global).
 *  force=false skips hand-customized installs (reported in skipped_customized). */
export const resyncEntry = (name: string, transport: string, force: boolean) =>
  invoke<ResyncOutcome>("resync_entry", { name, transport, force });
/** Delete a manual/discovered catalog entry and uninstall it from all agents. */
export const forgetEntry = (name: string, transport: string) =>
  invoke<void>("forget_entry", { name, transport });
export const listCustomRegistryKeys = () =>
  invoke<string[]>("list_custom_registry_keys");
export const listAgents = () => invoke<AgentInfo[]>("list_agents");
export const addAgent = (id: string, def: AgentDefinitionInput) =>
  invoke<void>("add_agent", { id, def });
export const updateAgent = (id: string, def: AgentDefinitionInput) =>
  invoke<void>("update_agent", { id, def });
export const scanInstalled = () => invoke<InstalledMcp[]>("scan_installed");
/** Register any discovered-but-unregistered agent MCPs into the registry. Returns
 *  the number newly imported. */
export const importDiscovered = () => invoke<number>("import_discovered");
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

/** Parse a pasted config blob (JSON/TOML) and add its servers to the manual
 *  source. Returns the added server names. */
export const importPastedConfig = (text: string) =>
  invoke<string[]>("import_pasted_config", { text });

// ── Catalog sources (subscribe remote / add local) ────────────────────────
export const listSources = () => invoke<SourceView[]>("list_sources");
/** Subscribe to a remote config URL: fetch + cache + register as a source. */
export const subscribeSource = (url: string, name?: string) =>
  invoke<SourceView>("subscribe_source", { url, name: name ?? null });
/** Open a native file picker and add the chosen file as a local source.
 *  Resolves to null if the user cancels. */
export const addLocalSourceDialog = () =>
  invoke<SourceView | null>("add_local_source_dialog");
/** Export the complete effective catalog to JSON via a native save dialog.
 *  Resolves to the written path, or null if the user cancels. */
export const exportEffectiveDialog = () =>
  invoke<string | null>("export_effective_dialog");
/** Re-fetch (remote) / re-copy (local) a source's file. */
export const refreshSource = (id: string) =>
  invoke<SourceView>("refresh_source", { id });
export const setSourceEnabled = (id: string, enabled: boolean) =>
  invoke<void>("set_source_enabled", { id, enabled });
export const removeSource = (id: string) =>
  invoke<void>("remove_source", { id });

/** Stable key for an (server, agent) cell in the matrix. `serverKey` is the
 *  composite registry key (`name::transport`), so stdio and http variants of the
 *  same server get distinct cells. */
export const cellKey = (serverKey: string, agentId: string) =>
  `${serverKey}|${agentId}`;
