export interface StdioConfig { command: string; args?: string[]; env?: Record<string, string>; cwd?: string; }
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
  /** Optional homepage / source repo URL (e.g. a GitHub repo), shown as a link. */
  repo?: string;
}
/** One entry copy from a source, plus whether it's the in-effect (winning) copy
 *  for its composite key. Copies with `in_effect === false` are shadowed by a
 *  higher-precedence source but still shown so nothing is hidden. */
export interface CatalogItem {
  entry: RegistryEntry;
  in_effect: boolean;
}
export interface AgentInfo {
  id: string; name: string; format: string; key: string;
  has_global: boolean; has_project: boolean; enabled: boolean;
  supported_transports: Array<"stdio" | "http">;
  /** Raw stored config paths (e.g. `~/Library/Application Support/…/mcp.json`). */
  global: string | null; project: string | null;
  docs: string | null;
  note: string | null;
  category: string;
  evidence: "official" | "official-source" | "catalog" | "custom" | string;
  verified_at: string | null;
  builtin: boolean;
}

export type ModelProtocol =
  | "anthropic-messages"
  | "openai-responses"
  | "openai-completions";

export interface ModelProfile {
  id: string;
  name: string;
  protocol: ModelProtocol;
  base_url: string;
  model: string;
  context_window?: number;
  max_output_tokens?: number;
  reasoning: boolean;
}

export interface ModelProfileView extends ModelProfile {
  credential_saved: boolean;
}

export interface ModelAgentView {
  id: "claude-code" | "codex" | "pi" | "qoder" | string;
  name: string;
  mode: "managed" | "guided";
  installed: boolean;
  config_path: string;
  docs: string;
  assigned_profile: string | null;
  supported_protocols: ModelProtocol[];
  note: string;
}

export interface ModelApplyResult {
  agent: string;
  profile: string;
  files: string[];
  restart_required: boolean;
  message: string;
}
/** Payload for creating a custom agent (mirrors Rust AgentDefinition). */
export interface AgentDefinitionInput {
  global: string | null;
  /** Legacy metadata retained when editing an existing definition. */
  project: string | null;
  format: "json" | "toml" | "yaml";
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
/** Top-level GUI view. Resource editors are overlays and intentionally remain
 *  outside navigation state so the app chrome never disappears. */
export type View =
  | { kind: "registry" }
  | { kind: "models" }
  | { kind: "agent"; id: string };

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
  server_name: string; transport: "stdio" | "http"; agents: string[];
  overrides: Record<string, PatchInput>;
}

/** Result of re-syncing an edited entry to its installed agents. */
export interface ResyncOutcome {
  /** Agent ids the current config was re-stamped into. */
  synced: string[];
  /** Agent ids skipped because their on-disk config was hand-customized
   *  (only populated when force = false). */
  skipped_customized: string[];
}
