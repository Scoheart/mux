import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "../lib/api";
import type { MigrationCandidate } from "../lib/migration";
import { MigrationDialog } from "./MigrationDialog";

vi.mock("../lib/api", () => ({
  planMcpAdoption: vi.fn(),
  planModelAdoption: vi.fn(),
  commitAssetOperation: vi.fn(),
  cancelAssetOperation: vi.fn(),
  planSkillImport: vi.fn(),
  commitSkillImport: vi.fn(),
  cancelSkillOperation: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const basePlan = {
  operation_id: "mcp-operation",
  kind: "adopt" as const,
  domain_plan: { domain: "mcp" as const, before: {}, after: {} },
  central_changes: [],
  relationship_changes: [],
  model_state_changes: [],
  target_files: [],
  affected_agent_ids: [],
  warnings: [],
  can_commit: true,
  requires_conflict_confirmation: false,
  candidate_hash: "mcp-candidate",
};

const candidates: MigrationCandidate[] = [
  {
    id: "model:same",
    domain: "model",
    name: "HY3",
    detail: "openrouter · tencent/hy3:free · 1 个 Agent",
    agentIds: ["grok-build"],
    fingerprint: "model-fingerprint",
    safe: true,
    conflictReason: null,
    model: {
      candidateFingerprints: { "candidate-grok": "model-fingerprint" },
      provider: "openrouter",
      model: "tencent/hy3:free",
      active: true,
    },
  },
  {
    id: "mcp:github::stdio",
    domain: "mcp",
    name: "github",
    detail: "STDIO · 1 个 Agent",
    agentIds: ["claude-code"],
    fingerprint: "mcp-fingerprint",
    safe: true,
    conflictReason: null,
    mcp: {
      assetKey: "github::stdio",
      candidateFingerprints: { "claude-code": "candidate-fingerprint" },
    },
  },
  {
    id: "skill:review",
    domain: "skill",
    name: "review",
    detail: "1 个 Agent · 1 个目录",
    agentIds: ["codex"],
    fingerprint: "skill-fingerprint",
    safe: true,
    conflictReason: null,
    skill: { identity: "target:agents-user:review" },
  },
];

describe("MigrationDialog", () => {
  it("imports each selected asset through its existing verified transaction", async () => {
    vi.mocked(api.planMcpAdoption).mockResolvedValue(basePlan);
    vi.mocked(api.commitAssetOperation).mockResolvedValue({ consumptions: [], external: [] });
    vi.mocked(api.planSkillImport).mockResolvedValue({
      operation_id: "skill-operation",
      kind: "import",
      skills: [],
      targets: [],
      settings_hash: "settings",
      candidate_hash: "skill-candidate",
      findings_hash: "findings",
      requires_risk_override: false,
      warnings: [],
    });
    vi.mocked(api.commitSkillImport).mockResolvedValue({ items: [], agents: [], targets: [], recovery_error: null });
    const onRefresh = vi.fn().mockResolvedValue(undefined);

    vi.mocked(api.planModelAdoption).mockResolvedValue({
      ...basePlan,
      operation_id: "model-operation",
      domain_plan: { domain: "model", before: {}, after: {} },
      candidate_hash: "model-plan",
    });
    render(<MigrationDialog candidates={candidates} onClose={vi.fn()} onRefresh={onRefresh} />);
    await userEvent.click(screen.getByRole("button", { name: "导入并统一管理 (3)" }));

    await waitFor(() => expect(onRefresh).toHaveBeenCalledOnce());
    expect(api.planMcpAdoption).toHaveBeenCalledWith({
      asset_key: "github::stdio",
      agent_ids: ["claude-code"],
      candidate_fingerprints: { "claude-code": "candidate-fingerprint" },
    });
    expect(api.commitAssetOperation).toHaveBeenCalledWith(basePlan);
    expect(api.planModelAdoption).toHaveBeenCalledWith({
      candidate_fingerprints: { "candidate-grok": "model-fingerprint" },
    });
    expect(api.planSkillImport).toHaveBeenCalledWith({
      identity: "target:agents-user:review",
      agent_ids: ["codex"],
      replace_conflicts: false,
    });
    expect(api.commitSkillImport).toHaveBeenCalledWith({
      operation_id: "skill-operation",
      candidate_hash: "skill-candidate",
      findings_confirmation: null,
    });
    expect(screen.getByText("成功 3 项，失败 0 项")).toBeVisible();
  });

  it("keeps conflicts disabled and unselected", () => {
    const conflict: MigrationCandidate = {
      ...candidates[0],
      id: "mcp:conflict::stdio",
      name: "conflict",
      safe: false,
      conflictReason: "同名 MCP 的连接配置不一致",
    };
    render(<MigrationDialog candidates={[conflict]} onClose={vi.fn()} onRefresh={vi.fn()} />);
    expect(screen.getByRole("checkbox")).toBeDisabled();
    expect(screen.getByRole("button", { name: "导入并统一管理 (0)" })).toBeDisabled();
  });

  it("stops and cancels when a Skill becomes high risk after review", async () => {
    vi.mocked(api.planSkillImport).mockResolvedValue({
      operation_id: "risk-operation",
      kind: "import",
      skills: [],
      targets: [],
      settings_hash: "settings",
      candidate_hash: "risk-candidate",
      findings_hash: "risk-findings",
      requires_risk_override: true,
      warnings: [],
    });
    vi.mocked(api.cancelSkillOperation).mockResolvedValue(undefined);

    render(<MigrationDialog candidates={[candidates[2]]} onClose={vi.fn()} onRefresh={vi.fn().mockResolvedValue(undefined)} />);
    await userEvent.click(screen.getByRole("button", { name: "导入并统一管理 (1)" }));

    await waitFor(() => expect(api.cancelSkillOperation).toHaveBeenCalledWith("risk-operation"));
    expect(api.commitSkillImport).not.toHaveBeenCalled();
    expect(screen.getByText("Skill 风险状态已变化；请在 Skills 页面单独导入并确认风险。")).toBeVisible();
  });
});
