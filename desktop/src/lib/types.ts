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
  skills_global_dir: string | null;
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
  /** Actual API/billing channel, such as openrouter or anthropic. */
  provider: string;
  /** Model creator; independent from the API provider. */
  model_vendor?: string;
  /** Agent-native identities retained by an adopted historical config. */
  native_ids?: Record<string, string>;
  protocol: ModelProtocol;
  base_url: string;
  model: string;
  /** Non-secret environment variable name for Agents such as Grok Build. */
  env_key?: string;
  context_window?: number;
  max_output_tokens?: number;
  reasoning: boolean;
}

export interface ModelProfileView extends ModelProfile {
  catalog_key: string;
  credential_saved: boolean;
}

export interface ModelProviderView {
  id: string;
  name: string;
  default_base_url: string | null;
}

export interface ModelAgentView {
  id: "claude-code" | "codex" | "pi" | "qoder" | string;
  name: string;
  mode: "managed" | "guided";
  installed: boolean;
  config_path: string;
  config_paths: string[];
  docs: string;
  assigned_profile: string | null;
  assigned_profiles: string[];
  active_profile: string | null;
  supports_multiple: boolean;
  credential_mode: "keychain-command" | "environment-reference" | "guided" | string;
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
export interface AgentConfigurationInput {
  mcp_path: string;
  model_paths: string[];
  skills_global_dir: string | null;
}
export interface InstalledMcp {
  name: string; agent: string; scope: string; file_path: string; transport: string;
  customized?: boolean;
  /** Whether the server is active in the agent's config (true) or merely
   *  remembered in MUX's disabled store (false). */
  enabled: boolean;
}
export type McpAdoptionStatus = "adoptable" | "drifted" | "external";
export interface McpAdoptionCandidate {
  agent_id: string;
  asset_key: string;
  enabled: boolean;
  status: McpAdoptionStatus;
  config_hash: string;
  fingerprint: string;
  settings_hash: string;
  target_hash: string;
  candidate_hash: string;
}
export interface PlanMcpAdoptionRequest {
  asset_key: string;
  agent_ids: string[];
  candidate_fingerprints: Record<string, string>;
}
export type ModelAdoptionStatus = "adoptable" | "needs-credential" | "unsupported" | "conflicted";
export type ModelCredentialKind = "none" | "environment-reference" | "literal" | "external-command";
export interface ModelAdoptionCandidate {
  candidate_id: string;
  agent_id: string;
  native_id: string;
  name: string;
  provider: string;
  model_vendor?: string | null;
  protocol: ModelProtocol;
  base_url: string;
  model: string;
  env_key?: string | null;
  active: boolean;
  credential_kind: ModelCredentialKind;
  status: ModelAdoptionStatus;
  reason?: string | null;
  fingerprint: string;
  settings_hash: string;
  target_hash: string;
  candidate_hash: string;
}
export interface PlanModelAdoptionRequest {
  candidate_fingerprints: Record<string, string>;
}
export interface PatchInput {
  args?: string[]; env?: Record<string, string>; url?: string; headers?: Record<string, string>;
}

export type ResourceNavigationRequest =
  | { domain: "mcp"; kind: "detail"; name: string; transport: string }
  | { domain: "mcp"; kind: "create" }
  | { domain: "model"; kind: "detail"; profileId: string }
  | { domain: "model"; kind: "create" }
  | { domain: "skill"; kind: "detail"; skillName: string };

export type ResourceNavigationIntent = ResourceNavigationRequest & { id: number };
export type SkillNavigationRequest = Extract<ResourceNavigationRequest, { domain: "skill" }>;
export type SkillNavigationIntent = Extract<ResourceNavigationIntent, { domain: "skill" }>;

/** Top-level GUI view. Resource editors are overlays and intentionally remain
 *  outside navigation state so the app chrome never disappears. */
export type View =
  | { kind: "registry"; intent?: Extract<ResourceNavigationIntent, { domain: "mcp" }> }
  | { kind: "models"; intent?: Extract<ResourceNavigationIntent, { domain: "model" }> }
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

export interface ProxySettings {
  proxy_url: string | null;
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
  | { kind: "archive"; path: string; subpath: string }
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

export type AssetRef =
  | { domain: "mcp"; key: string }
  | { domain: "model"; profile_id: string }
  | { domain: "skill"; name: string };

export type ConsumptionStatus =
  | "synced"
  | "pending"
  | "drifted"
  | "conflicted"
  | "unsupported"
  | "external";

export interface ConsumptionView {
  agent_id: string;
  asset: AssetRef;
  desired: boolean;
  observed: boolean;
  enabled?: boolean | null;
  active?: boolean | null;
  desired_active?: boolean | null;
  status: ConsumptionStatus;
  reason: string | null;
  affected_agent_ids: string[];
  target?: { target_id: string; global_dir: string } | null;
}

export interface ConsumptionInventory {
  consumptions: ConsumptionView[];
  external: ConsumptionView[];
  recovery_error?: string | null;
}

export type AgentConsumptionSelection =
  | { domain: "mcp"; asset_keys: string[] }
  | { domain: "model"; profile_ids: string[] }
  | { domain: "skill"; names: string[] };

export type RelationshipAction = "add" | "remove";

export type CentralAssetAction = "create" | "update" | "delete";

export interface CentralAssetChange {
  asset: AssetRef;
  action: CentralAssetAction;
  summary: string[];
}

export type CentralAssetDraft =
  | {
      domain: "mcp";
      existing_key?: string;
      entry: RegistryEntry;
    }
  | {
      domain: "model";
      existing_id?: string;
      profile: ModelProfile;
      /** undefined keeps, empty string clears, non-empty replaces. */
      credential?: string;
    };

export interface RelationshipChange {
  agent_id: string;
  asset: AssetRef;
  action: RelationshipAction;
}

export interface ModelStateSnapshot {
  added: boolean;
  enabled: boolean;
  active: boolean;
}

export interface ModelStateChange {
  agent_id: string;
  profile_id: string;
  before: ModelStateSnapshot;
  after: ModelStateSnapshot;
  fallback_profile_id?: string | null;
  reason: string;
}

export interface ModelConsumptionRecord {
  profile_id: string;
  enabled: boolean;
  last_selected_at?: string | null;
}

export interface ModelAgentSelection {
  profiles: Record<string, ModelConsumptionRecord>;
  active_profile_id?: string | null;
}

export type DomainPlan =
  | {
      domain: "mcp";
      before: Record<string, string[]>;
      after: Record<string, string[]>;
    }
  | {
      domain: "model";
      before: Record<string, ModelAgentSelection>;
      after: Record<string, ModelAgentSelection>;
    }
  | {
      domain: "skill";
      before: Record<string, string[]>;
      after: Record<string, string[]>;
    }
  | {
      domain: "agent-configuration";
      agent_id: string;
      before: AgentConfigurationInput;
      after: AgentConfigurationInput;
      skills_before: Record<string, string[]>;
      skills_after: Record<string, string[]>;
      affected_agent_ids: string[];
      migrated_skill_names: string[];
    };

export interface AssetOperationPlan {
  operation_id: string;
  kind: "set-consumption" | "update-asset" | "delete-asset" | "adopt" | "update-configuration";
  domain_plan: DomainPlan;
  central_changes: CentralAssetChange[];
  relationship_changes: RelationshipChange[];
  model_state_changes: ModelStateChange[];
  target_files: string[];
  affected_agent_ids: string[];
  warnings: string[];
  can_commit: boolean;
  requires_conflict_confirmation: boolean;
  candidate_hash: string;
}

export interface AssetCommandError {
  code: string;
  message: string;
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

export interface PlanSkillAssetInstallRequest {
  resolution_id: string;
  skill_names: string[];
  replace_conflicts: boolean;
}

export interface PlanImportRequest {
  identity: string;
  agent_ids: string[];
  replace_conflicts: boolean;
}

export interface PlanSkillAssetImportRequest {
  identity: string;
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
