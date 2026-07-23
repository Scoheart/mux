import { describe, expect, it } from "vitest";
import {
  buildMigrationCandidates,
  mcpMigrationCandidateId,
  modelMigrationCandidateId,
  skillMigrationCandidateId,
} from "./migration";
import type { McpAdoptionCandidate, ModelAdoptionCandidate, SkillInventoryItem } from "./types";

const mcp = (
  agent: string,
  configHash: string,
  status: McpAdoptionCandidate["status"] = "external",
): McpAdoptionCandidate => ({
  agent_id: agent,
  asset_key: "github::stdio",
  enabled: true,
  status,
  config_hash: configHash,
  fingerprint: `${agent}-${configHash}`,
  settings_hash: "settings",
  target_hash: "target",
  candidate_hash: "candidate",
});

const skills = (hashes: string[], level: "low" | "high" = "low"): SkillInventoryItem[] =>
  hashes.map((hash, index) => ({
    identity: `target:target-${index}:review`,
    name: "review",
    description: "Review changes",
    content_kind: "instructions",
    states: ["external"],
    location: { kind: "agent_target", target_id: `target-${index}`, global_dir: `~/skills-${index}` },
    source: null,
    resolved_revision: null,
    content_hash: hash,
    risk: { level, findings: [], finding_count: 0, findings_truncated: false },
    update: { available: false, checked_at: null, resolved_revision: null, etag: null, error: null, retry_at: null },
    assigned_target_ids: [],
    affected_agent_ids: [`agent-${index}`],
    installed_at: null,
    updated_at: null,
  }));

const model = (agent: string, status: ModelAdoptionCandidate["status"] = "adoptable"): ModelAdoptionCandidate => ({
  candidate_id: `candidate-${agent}`,
  agent_id: agent,
  native_id: `native-${agent}`,
  name: "HY3",
  provider: "openrouter",
  model_vendor: "tencent",
  protocol: "openai-completions",
  base_url: "https://openrouter.ai/api/v1",
  model: "tencent/hy3:free",
  env_key: "OPENROUTER_API_KEY",
  active: agent === "grok-build",
  credential_kind: "environment-reference",
  status,
  reason: status === "adoptable" ? null : "需要安全 credential",
  fingerprint: "same-model",
  settings_hash: "settings",
  target_hash: `target-${agent}`,
  candidate_hash: `hash-${agent}`,
});

describe("migration candidates", () => {
  it("uses a stable MCP candidate id for targeted adoption", () => {
    expect(mcpMigrationCandidateId("github::stdio")).toBe("mcp:github::stdio");
    expect(buildMigrationCandidates([mcp("a", "same")], null)[0].id).toBe(
      mcpMigrationCandidateId("github::stdio"),
    );
  });

  it("uses stable Model and Skill candidate ids for targeted adoption", () => {
    expect(modelMigrationCandidateId("same-model")).toBe("model:same-model");
    expect(skillMigrationCandidateId("review")).toBe("skill:review");
    expect(buildMigrationCandidates([], null, [model("grok-build")])[0].id).toBe(
      modelMigrationCandidateId("same-model"),
    );
    expect(buildMigrationCandidates([], skills(["same"]))[0].id).toBe(
      skillMigrationCandidateId("review"),
    );
  });

  it("merges identical MCP copies and blocks divergent copies", () => {
    expect(buildMigrationCandidates([mcp("a", "same"), mcp("b", "same")], null)[0]).toMatchObject({
      safe: true,
      agentIds: ["a", "b"],
    });
    expect(buildMigrationCandidates([mcp("a", "one"), mcp("b", "two")], null)[0]).toMatchObject({
      safe: false,
      conflictReason: "同名 MCP 的连接配置不一致；请先在原 Agent 中统一或重命名后重新扫描",
    });
  });

  it("merges identical Skill directories and blocks different hashes", () => {
    expect(buildMigrationCandidates([], skills(["same", "same"]))[0]).toMatchObject({
      safe: true,
      agentIds: ["agent-0", "agent-1"],
    });
    expect(buildMigrationCandidates([], skills(["one", "two"]))[0]).toMatchObject({
      safe: false,
      conflictReason: "同名 Skill 的内容不一致；请先统一内容或重命名来源目录后重新扫描",
    });
    expect(buildMigrationCandidates([], skills(["same"], "high"))[0]).toMatchObject({
      safe: false,
      conflictReason: "Skill 包含高风险内容；请在 Skills 页面单独导入并确认风险",
    });
  });

  it("groups identical Model connections and keeps unsafe credentials blocked", () => {
    expect(buildMigrationCandidates([], null, [model("grok-build"), model("opencode")])[0]).toMatchObject({
      domain: "model",
      safe: true,
      agentIds: ["grok-build", "opencode"],
      model: { active: true },
    });
    expect(buildMigrationCandidates([], null, [model("grok-build", "needs-credential")])[0]).toMatchObject({
      safe: false,
      conflictReason: "需要安全 credential",
    });
  });
});
