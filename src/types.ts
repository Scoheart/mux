export interface McpStdioConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

export interface McpHttpConfig {
  type: "http" | "sse";
  url: string;
  headers?: Record<string, string>;
}

export type McpConfig = McpStdioConfig | McpHttpConfig;

/** Provenance of a catalog entry:
 *  - "discovered" — scanned from a local app config (`agent`/`scope` set),
 *  - "manual"     — created by the user by hand,
 *  - "remote"     — from a subscribed remote source (`source` = its id),
 *  - "local"      — from a local file source (`source` = its id). */
export interface RegistryOrigin {
  kind: "discovered" | "manual" | "remote" | "local";
  agent?: string;
  scope?: "global" | "project";
  source?: string;
}

/** A catalog source: a subscribed remote URL or a local file. Servers are parsed
 *  from a cached copy under ~/.mux/sources/<kind>/<id>.<ext>. Shape matches the
 *  desktop's Rust SourceDef (snake_case) so both tools share settings.json. */
export interface SourceDef {
  id: string;
  kind: "remote" | "local";
  name: string;
  url?: string;
  path?: string;
  format: string;
  key: string;
  enabled: boolean;
  added_at?: string;
  synced_at?: string;
  server_count?: number;
  error?: string;
}

export interface RegistryEntry {
  name: string;
  description: string;
  tags: string[];
  config: {
    stdio?: McpStdioConfig;
    http?: McpHttpConfig;
  };
  origin?: RegistryOrigin;
}

export interface AgentDefinition {
  global: string | null;
  project: string | null;
  format: "json" | "toml";
  key: string;
  enabled: boolean;
  builtin?: boolean;
}

export interface AgentsConfig {
  agents: Record<string, AgentDefinition>;
}

export type Scope = "global" | "project" | "both";

export interface ActiveMcp {
  name: string;
  scope: Scope;
  projectPath?: string;
  agents: string[];
}

export interface StateConfig {
  active: ActiveMcp[];
}

export interface ScannedMcp {
  name: string;
  config: McpConfig;
  source: {
    agent: string;
    scope: "global" | "project";
    filePath: string;
  };
}

export type DiffAction = "add" | "remove" | "change";

export interface DiffEntry {
  action: DiffAction;
  mcpName: string;
  agent: string;
  scope: "global" | "project";
  config?: McpConfig;
}
