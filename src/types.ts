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

/** Provenance of a custom registry entry. `kind` is "discovered" (scanned from
 *  a local app config) or "manual" (created by the user). For discovered entries
 *  `agent`/`scope` identify the source app; omitted for manual ones. Absent
 *  entirely on builtin entries (origin is inferred at runtime). */
export interface RegistryOrigin {
  kind: "discovered" | "manual";
  agent?: string;
  scope?: "global" | "project";
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
