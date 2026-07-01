export interface StdioConfig { command: string; args?: string[]; env?: Record<string, string>; }
export interface HttpConfig { type: "http" | "sse"; url: string; headers?: Record<string, string>; }
/** Provenance of a custom registry entry. `kind` is "discovered" (scanned from
 *  a local app config) or "manual" (created by the user). For discovered entries
 *  `agent`/`scope` identify the source app; omitted for manual ones. Absent
 *  entirely on builtin entries (origin is inferred at runtime). */
export interface RegistryOrigin {
  kind: "discovered" | "manual";
  agent?: string;
  scope?: string;
}
export interface RegistryEntry {
  name: string; description: string; tags: string[];
  config: { stdio?: StdioConfig; http?: HttpConfig };
  origin?: RegistryOrigin;
}
export interface AgentInfo {
  id: string; format: string; key: string;
  has_global: boolean; has_project: boolean; enabled: boolean;
  /** Raw stored config paths (e.g. `~/Library/Application Support/…/mcp.json`). */
  global: string | null; project: string | null;
}
/** Payload for creating a custom agent (mirrors Rust AgentDefinition). */
export interface AgentDefinitionInput {
  global: string | null;
  project: string | null;
  format: "json" | "toml";
  key: string;
  enabled: boolean;
  builtin?: boolean;
}
export interface InstalledMcp {
  name: string; agent: string; scope: string; file_path: string; transport: string;
  customized?: boolean;
  /** Whether the server is active in the agent's config (true) or merely
   *  remembered in MUX's disabled store (false). */
  enabled: boolean;
}
export interface PlannedWrite { agent: string; file_path: string; config_json: string; }
export interface PatchInput {
  args?: string[]; env?: Record<string, string>; url?: string; headers?: Record<string, string>;
}
/** Top-level GUI view: registry overview, a single agent's page, or the
 *  full-page MCP editor (name === null means creating a new entry). */
export type View =
  | { kind: "registry" }
  | { kind: "agent"; id: string }
  | { kind: "mcp-edit"; name: string | null; transport?: "stdio" | "http" };

export interface InstallRequest {
  server_name: string; transport: "stdio" | "http"; scope: "global" | "project"; agents: string[];
  project_dir?: string; overrides: Record<string, PatchInput>;
}
