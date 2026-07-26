import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
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
  it("reviews and commits MCP, Model, and Skill candidates one at a time", async () => {
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
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.queryByText(/导入 3 项|全选/)).not.toBeInTheDocument();

    await userEvent.click(within(
      screen.getByRole("region", { name: "Model 外部配置" }),
    ).getByRole("button", { name: "让 MUX 管理" }));
    expect(await screen.findByRole("heading", { name: "确认让 MUX 管理 HY3" })).toBeVisible();
    expect(api.commitOperation).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "确认让 MUX 管理" }));

    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(1));
    await userEvent.click(within(
      screen.getByRole("region", { name: "MCP 外部配置" }),
    ).getByRole("button", { name: "让 MUX 管理" }));
    expect(await screen.findByRole("heading", { name: "确认让 MUX 管理 github" })).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "确认让 MUX 管理" }));

    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(2));
    await userEvent.click(within(
      screen.getByRole("region", { name: "Skill 外部配置" }),
    ).getByRole("button", { name: "让 MUX 管理" }));
    expect(await screen.findByRole("heading", { name: "确认让 MUX 管理 review" })).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "确认让 MUX 管理" }));

    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(3));
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
    expect(screen.getByText("已管理 3 项，失败 0 项")).toBeVisible();
  });

  it("keeps conflicts visible but unavailable for management", () => {
    const conflict: MigrationCandidate = {
      ...candidates[0],
      id: "mcp:conflict::stdio",
      name: "conflict",
      safe: false,
      conflictReason: "同名 MCP 的连接配置不一致",
    };
    render(<MigrationDialog candidates={[conflict]} onClose={vi.fn()} onRefresh={vi.fn()} />);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "需先处理" })).toBeDisabled();
  });

  it("explains read-only detection and has no bulk selection affordance", () => {
    render(<MigrationDialog candidates={candidates} onClose={vi.fn()} onRefresh={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "已识别的外部配置" })).toBeVisible();
    expect(screen.getByText("共 3 项 · 3 项可逐项管理 · 0 项需先处理")).toBeVisible();
    expect(screen.getByText(
      "MUX 只识别这些 Agent 配置，不会自动导入。请检查每一项，并单独决定是否交给 MUX 管理。",
    )).toBeVisible();
    expect(screen.getAllByRole("button", { name: "关闭" })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "让 MUX 管理" })).toHaveLength(3);
    expect(screen.queryByText(/全选|导入 3 项/)).not.toBeInTheDocument();
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
    await userEvent.click(screen.getByRole("button", { name: "让 MUX 管理" }));

    await waitFor(() => expect(api.cancelOperation).toHaveBeenCalledWith({
      domain: "skill",
      operation_id: "risk-operation",
    }));
    expect(api.commitOperation).not.toHaveBeenCalled();
    expect(screen.getByText("Skill 风险状态已变化；请在 Skills 页面单独导入并确认风险。")).toBeVisible();
  });
});
