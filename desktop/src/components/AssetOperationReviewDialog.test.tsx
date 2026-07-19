import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { assetOperationPlanFixture } from "../test/consumptionFixtures";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";

afterEach(cleanup);

it("requires explicit bound confirmation before replacing drift", async () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-asset";
  plan.requires_conflict_confirmation = true;
  plan.warnings = ["codex: model_owned_fields_drift"];
  const onCommit = vi.fn();
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      onCommit={onCommit}
      onCancel={vi.fn()}
    />,
  );
  const commit = screen.getByRole("button", { name: "确认并同步" });
  expect(commit).toBeDisabled();
  await userEvent.click(screen.getByRole("checkbox"));
  expect(commit).toBeEnabled();
  await userEvent.click(commit);
  expect(onCommit).toHaveBeenCalledWith(plan.candidate_hash);
});

it("shows central lifecycle impact independently from relationship changes", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "delete-asset";
  plan.central_changes = [{
    asset: { domain: "model", profile_id: "work" },
    action: "delete",
    summary: ["删除 Profile metadata", "级联解除 2 个 consumer"],
  }];
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );
  expect(screen.getByText("中央资产变化")).toBeVisible();
  expect(screen.getByText(/级联解除 2 个 consumer/)).toBeVisible();
  expect(screen.getByRole("button", { name: "确认删除并同步" })).toBeEnabled();
});

it("presents Agent assignment as a direct add action", () => {
  const plan = assetOperationPlanFixture();
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentName="Claude Code"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("heading", { name: "确认添加 MCP" })).toBeVisible();
  expect(screen.getByText("Claude Code")).toBeVisible();
  expect(screen.getByRole("button", { name: "添加 MCP" })).toBeEnabled();
  expect(screen.getByText("Agent 变更")).toBeVisible();
  expect(screen.queryByText(/desired relationship/)).not.toBeInTheDocument();
});

it("shows configuration paths and shared Skill migration compactly", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-configuration";
  plan.domain_plan = {
    domain: "agent-configuration",
    agent_id: "codex",
    before: {
      mcp_path: "~/.codex/config.toml",
      model_paths: ["~/.codex/config.toml"],
      skills_global_dir: "~/.agents/skills",
    },
    after: {
      mcp_path: "~/.codex/config.toml",
      model_paths: ["~/.codex/config.toml"],
      skills_global_dir: "~/.private/skills",
    },
    skills_before: {},
    skills_after: {},
    affected_agent_ids: ["codex", "cursor"],
    migrated_skill_names: ["review-changes", "frontend-design"],
  };
  plan.affected_agent_ids = ["codex", "cursor"];
  plan.relationship_changes = [];
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentName="Codex"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("heading", { name: "确认修改配置" })).toBeVisible();
  expect(screen.getByText("~/.agents/skills")).toBeVisible();
  expect(screen.getByText("review-changes、frontend-design")).toBeVisible();
  expect(screen.getByText("Codex · 另影响 1 个 Agent")).toBeVisible();
  expect(screen.getByRole("button", { name: "保存配置" })).toBeEnabled();
});
