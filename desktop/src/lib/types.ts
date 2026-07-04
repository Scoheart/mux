export interface StdioConfig { command: string; args?: string[]; env?: Record<string, string>; }
export interface HttpConfig { type: string; url: string; headers?: Record<string, string>; }
/** Provenance of a catalog entry:
 *  - "discovered" — scanned from a local app config (`agent`/`scope` set),
 *  - "manual"     — created by the user by hand,
 *  - "remote"     — from a subscribed remote source (`source` = its id),
 *  - "local"      — from a local file source (`source` = its id). */
export interface RegistryOrigin {
  kind: "discovered" | "manual" | "remote" | "local";
  agent?: string;
  scope?: string;
  source?: string;
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
export interface PatchInput {
  args?: string[]; env?: Record<string, string>; url?: string; headers?: Record<string, string>;
}
/** Top-level GUI view: registry overview, a single agent's page, or the
 *  full-page MCP editor (name === null means creating a new entry). */
export type View =
  | { kind: "registry" }
  | { kind: "sources" }
  | { kind: "agent"; id: string }
  | { kind: "mcp-edit"; name: string | null; transport?: "stdio" | "http" };

/** A catalog source (mirrors Rust SourceView): a subscribed remote URL or a
 *  local file. Its servers are parsed from a cached copy under ~/.mux/sources/. */
export type SourceKind = "remote" | "local";
export interface SourceView {
  id: string;
  kind: SourceKind;
  name: string;
  url: string | null;
  path: string | null;
  format: string;
  enabled: boolean;
  added_at: string | null;
  synced_at: string | null;
  server_count: number;
  error: string | null;
  /** True for the auto-managed sources (手动添加 / 自动探索); the UI hides
   *  refresh/remove for these. */
  managed: boolean;
}

export interface InstallRequest {
  server_name: string; transport: "stdio" | "http"; scope: "global" | "project"; agents: string[];
  project_dir?: string; overrides: Record<string, PatchInput>;
}
