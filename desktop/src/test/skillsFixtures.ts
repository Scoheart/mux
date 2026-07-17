import type { SkillsState } from "../hooks/useSkillsState";
import type {
  OperationPlan,
  SkillAgentView,
  SkillDetail,
  SkillInventoryItem,
  SkillSourceResolution,
  SkillsInventory,
} from "../lib/types";

const githubSource = {
  kind: "github" as const,
  owner: "acme",
  repo: "skills",
  subpath: "catalog/review-changes",
  requested_ref: "main",
  pinned: false,
};

const finding = {
  rule_id: "shell-pipe-download",
  rule_version: 1,
  level: "high" as const,
  path: "scripts/install.sh",
  line: 2,
  reason: "downloads content and pipes it to a shell",
};

const reviewItem = (): SkillInventoryItem => ({
  identity: "central:review-changes",
  name: "review-changes",
  description: "Review repository changes",
  content_kind: "automation",
  states: ["managed", "assigned", "update_available"],
  location: { kind: "central" },
  source: githubSource,
  resolved_revision: "0123456789abcdef0123456789abcdef01234567",
  content_hash: "content-review",
  risk: {
    level: "high",
    findings: [finding],
    finding_count: 1,
    findings_truncated: false,
  },
  update: {
    available: true,
    checked_at: "2026-07-16T08:00:00Z",
    resolved_revision: "2222222222222222222222222222222222222222",
    etag: "fixture-etag",
    error: null,
    retry_at: null,
  },
  assigned_target_ids: ["agents-user"],
  affected_agent_ids: ["codex", "cursor", "gemini"],
  installed_at: "2026-07-16T00:00:00Z",
  updated_at: "2026-07-16T00:00:00Z",
});

const unassignedItem = (): SkillInventoryItem => ({
  identity: "central:unassigned-skill",
  name: "unassigned-skill",
  description: "Unassigned safe reference",
  content_kind: "reference",
  states: ["managed"],
  location: { kind: "central" },
  source: { kind: "local", path: "~/fixtures", subpath: "unassigned-skill" },
  resolved_revision: null,
  content_hash: "content-safe",
  risk: {
    level: "low",
    findings: [],
    finding_count: 0,
    findings_truncated: false,
  },
  update: {
    available: false,
    checked_at: null,
    resolved_revision: null,
    etag: null,
    error: null,
    retry_at: null,
  },
  assigned_target_ids: [],
  affected_agent_ids: [],
  installed_at: "2026-07-16T00:00:00Z",
  updated_at: "2026-07-16T00:00:00Z",
});

export const agentFixture = (): SkillAgentView[] => [
  ["claude-code", "Claude Code", "claude-user", "~/.claude/skills"],
  ["codex", "Codex", "agents-user", "~/.agents/skills"],
  ["cursor", "Cursor", "cursor-user", "~/.cursor/skills"],
  ["gemini", "Gemini CLI", "gemini-user", "~/.gemini/skills"],
  ["opencode", "OpenCode", "opencode-user", "~/.config/opencode/skills"],
  ["copilot-cli", "GitHub Copilot CLI", "copilot-user", "~/.copilot/skills"],
].map(([id, name, target_id, global_dir]) => ({
  id,
  name,
  target_id,
  global_dir,
  affected_agent_ids: id === "codex" ? ["codex", "cursor", "gemini"] : [id],
  docs: "https://example.invalid/official-docs",
  evidence: "official",
  verified_at: "2026-07-16",
}));

export const skillsInventoryFixture = (): SkillsInventory => ({
  items: [reviewItem(), unassignedItem()],
  agents: agentFixture(),
  targets: [
    {
      target_id: "agents-user",
      global_dir: "~/.agents/skills",
      primary_agent_ids: ["codex"],
      affected_agent_ids: ["codex", "cursor", "gemini"],
      assignable: true,
    },
  ],
  recovery_error: null,
});

export const inventoryFixture = skillsInventoryFixture;

export const skillDetailFixture = (name = "review-changes"): SkillDetail => {
  const item =
    skillsInventoryFixture().items.find((row) => row.name === name) ?? reviewItem();
  return {
    item,
    files: [
      {
        path: "SKILL.md",
        kind: "file",
        size: 120,
        executable: false,
        link_target: null,
        sha256: "file-hash",
      },
    ],
    skill_md:
      "---\nname: review-changes\ndescription: Review repository changes\n---\n",
    skill_md_truncated: false,
  };
};

export const resolutionFixture = (): SkillSourceResolution => ({
  operation_id: "resolve-fixture",
  source: githubSource,
  resolved_revision: "0123456789abcdef0123456789abcdef01234567",
  candidates: [
    {
      name: "review-changes",
      description: "Review repository changes",
      relative_path: "review-changes",
      content_kind: "automation",
      content_hash: "content-review",
      file_count: 2,
      total_bytes: 240,
    },
  ],
});

export const sharedTargetPlanFixture = (): OperationPlan => ({
  operation_id: "resolve-fixture",
  kind: "install",
  skills: [
    {
      manifest: {
        name: "review-changes",
        description: "Review repository changes",
        license: null,
        compatibility: null,
        metadata: {},
        allowed_tools: null,
      },
      existing_source: null,
      source: githubSource,
      resolved_revision: "0123456789abcdef0123456789abcdef01234567",
      files: [
        {
          path: "SKILL.md",
          kind: "added",
          before_hash: null,
          after_hash: "file-hash",
          unified_diff: null,
          diff_truncated: false,
        },
      ],
      risk: {
        level: "low",
        findings: [],
        finding_count: 0,
        findings_truncated: false,
      },
      existing_states: [],
      replace_existing: false,
      content_hash: "content-review",
    },
  ],
  targets: [
    {
      target_id: "agents-user",
      global_dir: "~/.agents/skills",
      expected: "missing",
      primary_agent_ids: ["codex"],
      affected_agent_ids: ["codex", "cursor", "gemini"],
    },
  ],
  settings_hash: "settings-hash",
  candidate_hash: "candidate-hash",
  findings_hash: "findings-low",
  requires_risk_override: false,
  warnings: ["Gemini CLI also observes this shared directory"],
});

export const highRiskPlan = (findingsHash: string): OperationPlan => {
  const plan = sharedTargetPlanFixture();
  plan.skills[0].risk = {
    level: "high",
    findings: [finding],
    finding_count: 1,
    findings_truncated: false,
  };
  plan.findings_hash = findingsHash;
  plan.requires_risk_override = true;
  return plan;
};

const stateFrom = (inventory: SkillsInventory): SkillsState => ({
  inventory,
  loading: false,
  pendingOperation: null,
  error: null,
  refresh: async () => inventory,
  commit: async () => inventory,
  cancel: async () => undefined,
  checkUpdates: async () => ({
    performed: true,
    checked: 1,
    available: ["review-changes"],
    skipped_pinned: [],
    errors: {},
    checked_at: "2026-07-16T08:00:00Z",
  }),
});

export const skillsStateFixture = (): SkillsState =>
  stateFrom(skillsInventoryFixture());
export const sharedSkillsStateFixture = skillsStateFixture;
export const noop = () => undefined;
