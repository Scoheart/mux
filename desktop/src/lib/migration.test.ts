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
      conflict: { kind: "mcp_connection_mismatch" },
    });
  });

  it("merges identical Skill directories and blocks different hashes", () => {
    expect(buildMigrationCandidates([], skills(["same", "same"]))[0]).toMatchObject({
      safe: true,
      agentIds: ["agent-0", "agent-1"],
    });
    expect(buildMigrationCandidates([], skills(["one", "two"]))[0]).toMatchObject({
      safe: false,
      conflict: { kind: "skill_content_mismatch" },
    });
    expect(buildMigrationCandidates([], skills(["same"], "high"))[0]).toMatchObject({
      safe: false,
      conflict: { kind: "skill_high_risk" },
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
      conflict: { kind: "model_source", reason: "需要安全 credential" },
    });
  });

  it.each([
    [
      "检测到多个不同的明文 credential，不能安全合并",
      "model_multiple_credentials",
    ],
    [
      "外部 credential command 不会被执行；请先改为明文一次性导入或安全环境变量",
      "model_external_credential_command",
    ],
    [
      "该 Agent 的 MUX writer 使用 Keychain，不能无损接管外部环境变量引用",
      "model_environment_to_keychain",
    ],
    [
      "该 Agent 仅支持环境变量引用；请先把明文 Key 改为环境变量",
      "model_literal_to_environment",
    ],
    [
      "Agent-native provider identity 含有不安全字符，MUX 不会把它用于配置键或文件名",
      "model_unsafe_native_identity",
    ],
    [
      "多个 Model 共用同一个 Agent-native provider；请先在 Agent 中拆分 provider identity，MUX 不会覆盖兄弟模型",
      "model_shared_native_provider",
    ],
  ] as const)("classifies the known Core Model conflict %s", (reason, kind) => {
    expect(buildMigrationCandidates([], null, [{
      ...model("grok-build", "needs-credential"),
      reason,
    }])[0]).toMatchObject({
      safe: false,
      conflict: { kind },
    });
  });
});
