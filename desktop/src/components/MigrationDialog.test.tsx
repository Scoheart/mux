import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "../lib/api";
import type { MigrationCandidate } from "../lib/migration";
import { MigrationDialog } from "./MigrationDialog";

vi.mock("../lib/api", () => ({
  planOperation: vi.fn(),
  commitOperation: vi.fn(),
  cancelOperation: vi.fn(),
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
  it("imports each selected asset through the unified transaction wire", async () => {
    const modelPlan = {
      ...basePlan,
      operation_id: "model-operation",
      domain_plan: { domain: "model" as const, before: {}, after: {} },
      candidate_hash: "model-plan",
    };
    const skillPlan = {
      operation_id: "skill-operation",
      kind: "import" as const,
      skills: [],
      targets: [],
      settings_hash: "settings",
      candidate_hash: "skill-candidate",
      findings_hash: "findings",
      requires_risk_override: false,
      warnings: [],
    };
    vi.mocked(api.planOperation)
      .mockResolvedValueOnce({ domain: "asset", plan: modelPlan })
      .mockResolvedValueOnce({ domain: "asset", plan: basePlan })
      .mockResolvedValueOnce({ domain: "skill", plan: skillPlan });
    vi.mocked(api.commitOperation).mockImplementation(async (request) =>
      request.domain === "asset"
        ? { domain: "asset", inventory: { consumptions: [], external: [] } }
        : {
            domain: "skill",
            inventory: { items: [], agents: [], targets: [], recovery_error: null },
          });
    const onRefresh = vi.fn().mockResolvedValue(undefined);

    render(<MigrationDialog candidates={candidates} onClose={vi.fn()} onRefresh={onRefresh} />);
    await userEvent.click(screen.getByRole("button", { name: "导入 3 项" }));

    await waitFor(() => expect(onRefresh).toHaveBeenCalledOnce());
    expect(api.planOperation).toHaveBeenCalledWith({
      operation: "adopt_mcp",
      request: {
        asset_key: "github::stdio",
        agent_ids: ["claude-code"],
        candidate_fingerprints: { "claude-code": "candidate-fingerprint" },
      },
    });
    expect(api.planOperation).toHaveBeenCalledWith({
      operation: "adopt_model",
      request: {
        candidate_fingerprints: { "candidate-grok": "model-fingerprint" },
      },
    });
    expect(api.planOperation).toHaveBeenCalledWith({
      operation: "adopt_skill",
      request: {
        identity: "target:agents-user:review",
        agent_ids: ["codex"],
        replace_conflicts: false,
      },
    });
    expect(api.commitOperation).toHaveBeenCalledWith({
      domain: "skill",
      kind: "import",
      request: {
        operation_id: "skill-operation",
        candidate_hash: "skill-candidate",
        findings_confirmation: null,
      },
    });
    expect(screen.getByText("成功 3 项，失败 0 项")).toBeVisible();
    expect(screen.getByRole("button", { name: "导入 0 项" })).toBeDisabled();
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
    expect(screen.getByRole("button", { name: "导入 0 项" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "全选 Model" })).toBeDisabled();
  });

  it("supports selecting and clearing each safe domain as a group", async () => {
    render(<MigrationDialog candidates={candidates} onClose={vi.fn()} onRefresh={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "导入旧配置" })).toBeVisible();
    expect(screen.getByText("共 3 项 · 3 项可导入 · 0 项需先处理")).toBeVisible();
    expect(screen.getByText(
      "选择要整理到 MUX 的旧配置；原 Agent 的权限和登录状态不会改变。",
    )).toBeVisible();
    expect(screen.getByRole("button", { name: "关闭" })).toBeEnabled();

    await userEvent.click(screen.getByRole("button", { name: "取消全选 MCP" }));
    expect(screen.getByRole("button", { name: "导入 2 项" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "全选 MCP" })).toBeEnabled();

    await userEvent.click(screen.getByRole("button", { name: "全选 MCP" }));
    expect(screen.getByRole("button", { name: "导入 3 项" })).toBeEnabled();
  });

  it("stops and cancels when a Skill becomes high risk after review", async () => {
    vi.mocked(api.planOperation).mockResolvedValue({
      domain: "skill",
      plan: {
        operation_id: "risk-operation",
        kind: "import",
        skills: [],
        targets: [],
        settings_hash: "settings",
        candidate_hash: "risk-candidate",
        findings_hash: "risk-findings",
        requires_risk_override: true,
        warnings: [],
      },
    });
    vi.mocked(api.cancelOperation).mockResolvedValue(undefined);

    render(<MigrationDialog candidates={[candidates[2]]} onClose={vi.fn()} onRefresh={vi.fn().mockResolvedValue(undefined)} />);
    await userEvent.click(screen.getByRole("button", { name: "导入 1 项" }));

    await waitFor(() => expect(api.cancelOperation).toHaveBeenCalledWith({
      domain: "skill",
      operation_id: "risk-operation",
    }));
    expect(api.commitOperation).not.toHaveBeenCalled();
    expect(screen.getByText("Skill 风险状态已变化；请在 Skills 页面单独导入并确认风险。")).toBeVisible();
  });
});
