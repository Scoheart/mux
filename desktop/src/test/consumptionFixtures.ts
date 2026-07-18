import type { AssetOperationPlan, ConsumptionInventory } from "../lib/types";

export const consumptionInventoryFixture = (): ConsumptionInventory => ({
  recovery_error: null,
  consumptions: [
    {
      agent_id: "claude-code",
      asset: { domain: "mcp", key: "github::stdio" },
      desired: true,
      observed: true,
      status: "synced",
      reason: null,
      affected_agent_ids: ["claude-code"],
    },
    {
      agent_id: "codex",
      asset: { domain: "skill", name: "review-changes" },
      desired: true,
      observed: false,
      status: "drifted",
      reason: "skill_target_missing",
      affected_agent_ids: ["codex", "cursor", "gemini"],
    },
  ],
  external: [
    {
      agent_id: "claude-code",
      asset: { domain: "mcp", key: "external::http" },
      desired: false,
      observed: true,
      status: "external",
      reason: "mcp_external_unmanaged",
      affected_agent_ids: ["claude-code"],
    },
  ],
});

export const assetOperationPlanFixture = (): AssetOperationPlan => ({
  operation_id: "00000000-0000-4000-8000-000000000001",
  kind: "set-consumption",
  domain_plan: {
    domain: "mcp",
    before: { "claude-code": ["github::stdio"] },
    after: { "claude-code": ["github::stdio", "filesystem::stdio"] },
  },
  central_changes: [],
  relationship_changes: [
    {
      agent_id: "claude-code",
      asset: { domain: "mcp", key: "filesystem::stdio" },
      action: "add",
    },
  ],
  target_files: ["~/.claude.json"],
  affected_agent_ids: ["claude-code"],
  warnings: [],
  can_commit: true,
  requires_conflict_confirmation: false,
  candidate_hash: "candidate",
});
