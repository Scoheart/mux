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

export type SkillNavigationRequest =
  | { kind: "detail"; skillName: string }
  | { kind: "install"; agentId: string };

export type SkillNavigationIntent =
  | { id: number; kind: "detail"; skillName: string }
  | { id: number; kind: "install"; agentId: string };

/** Top-level GUI view. Resource editors are overlays and intentionally remain
 *  outside navigation state so the app chrome never disappears. */
export type View =
  | { kind: "registry" }
  | { kind: "models" }
  | { kind: "skills"; intent?: SkillNavigationIntent }
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

export type RiskLevel = "low" | "medium" | "high";
export type SkillContentKind =
  | "automation"
  | "assets"
  | "reference"
  | "instructions";
export type InventoryState =
  | "managed"
  | "assigned"
  | "external"
  | "locally_modified"
  | "broken_link"
  | "conflicting_link"
  | "missing"
  | "update_available";
export type SkillFileKind = "file" | "symlink";
export type FileChangeKind =
  | "added"
  | "modified"
  | "removed"
  | "mode_changed"
  | "link_changed";
export type PlannedLinkState =
  | "missing"
  | "managed"
  | "broken"
  | "directory"
  | "unknown_symlink";
export type SkillOperationKind =
  | "install"
  | "import"
  | "update"
  | "remove"
  | "assignment"
  | "repair";

export interface SkillRiskFinding {
  rule_id: string;
  rule_version: number;
  level: RiskLevel;
  path: string;
  line: number | null;
  reason: string;
}

export interface SkillRiskSummary {
  level: RiskLevel;
  findings: SkillRiskFinding[];
  finding_count: number;
  findings_truncated: boolean;
}

export type SkillSource =
  | {
      kind: "github";
      owner: string;
      repo: string;
      subpath: string;
      requested_ref: string;
      pinned: boolean;
    }
  | { kind: "local"; path: string; subpath: string }
  | { kind: "imported"; original_path: string; backup_path: string };

export interface SkillUpdateState {
  available: boolean;
  checked_at: string | null;
  resolved_revision: string | null;
  etag: string | null;
  error: string | null;
  retry_at: string | null;
}

export interface ManagedSkillRecord {
  name: string;
  description: string;
  content_kind: SkillContentKind;
  source: SkillSource;
  resolved_revision: string | null;
  content_hash: string;
  installed_at: string;
  updated_at: string;
  risk: SkillRiskSummary;
  update: SkillUpdateState;
}

export interface SkillFile {
  path: string;
  kind: SkillFileKind;
  size: number;
  executable: boolean;
  link_target: string | null;
  sha256: string;
}

export interface SkillFileChange {
  path: string;
  kind: FileChangeKind;
  before_hash: string | null;
  after_hash: string | null;
  unified_diff: string | null;
  diff_truncated: boolean;
}

export interface SkillAgentView {
  id: string;
  name: string;
  target_id: string;
  global_dir: string;
  affected_agent_ids: string[];
  docs: string;
  evidence: string;
  verified_at: string;
}

export interface SkillTargetView {
  target_id: string;
  global_dir: string;
  primary_agent_ids: string[];
  affected_agent_ids: string[];
  assignable: boolean;
}

export type SkillLocation =
  | { kind: "central" }
  | { kind: "agent_target"; target_id: string; global_dir: string };

export interface SkillInventoryItem {
  identity: string;
  name: string;
  description: string;
  content_kind: SkillContentKind;
  states: InventoryState[];
  location: SkillLocation;
  source: SkillSource | null;
  resolved_revision: string | null;
  content_hash: string | null;
  risk: SkillRiskSummary | null;
  update: SkillUpdateState;
  assigned_target_ids: string[];
  affected_agent_ids: string[];
  installed_at: string | null;
  updated_at: string | null;
}

export interface SkillsInventory {
  items: SkillInventoryItem[];
  agents: SkillAgentView[];
  targets: SkillTargetView[];
  recovery_error: string | null;
}

export interface SkillDetail {
  item: SkillInventoryItem;
  files: SkillFile[];
  skill_md: string;
  skill_md_truncated: boolean;
}

export interface SkillCandidateSummary {
  name: string;
  description: string;
  relative_path: string;
  content_kind: SkillContentKind;
  content_hash: string;
  file_count: number;
  total_bytes: number;
}

export interface SkillSourceResolution {
  operation_id: string;
  source: SkillSource;
  resolved_revision: string | null;
  candidates: SkillCandidateSummary[];
}

export interface PlannedSkill {
  manifest: {
    name: string;
    description: string;
    license: string | null;
    compatibility: string | null;
    metadata: Record<string, string>;
    allowed_tools: string | null;
  };
  existing_source: SkillSource | null;
  source: SkillSource;
  resolved_revision: string | null;
  files: SkillFileChange[];
  risk: SkillRiskSummary;
  existing_states: InventoryState[];
  replace_existing: boolean;
  content_hash: string;
}

export interface PlannedTarget {
  target_id: string;
  global_dir: string;
  expected: PlannedLinkState;
  primary_agent_ids: string[];
  affected_agent_ids: string[];
}

export interface OperationPlan {
  operation_id: string;
  kind: SkillOperationKind;
  skills: PlannedSkill[];
  targets: PlannedTarget[];
  settings_hash: string;
  candidate_hash: string;
  findings_hash: string;
  requires_risk_override: boolean;
  warnings: string[];
}

export interface PlanInstallRequest {
  resolution_id: string;
  skill_names: string[];
  agent_ids: string[];
  replace_conflicts: boolean;
}

export interface PlanImportRequest {
  identity: string;
  agent_ids: string[];
  replace_conflicts: boolean;
}

export interface PlanUpdateRequest {
  skill_name: string;
  replace_local_changes: boolean;
}

export interface PlanRemoveRequest {
  skill_name: string;
}

export interface PlanAssignmentRequest {
  skill_name: string;
  agent_ids: string[];
  enabled: boolean;
}

export interface PlanRepairRequest {
  skill_name: string;
  repair: { kind: "central" } | { kind: "target"; target_id: string };
}

export interface SkillCommitRequest {
  operation_id: string;
  candidate_hash: string;
  findings_confirmation: string | null;
}

export interface UpdateCheckOutcome {
  performed: boolean;
  checked: number;
  available: string[];
  skipped_pinned: string[];
  errors: Record<string, string>;
  checked_at: string | null;
}

export interface SkillCommandError {
  code: string;
  message: string;
  retry_at?: string;
  findings_hash?: string;
}
