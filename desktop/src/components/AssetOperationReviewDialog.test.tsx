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
  const commit = screen.getByRole("button", { name: "应用更改" });
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
  expect(screen.getByText("资源变化")).toBeVisible();
  expect(screen.getByText(/级联解除 2 个 consumer/)).toBeVisible();
  expect(screen.getByRole("button", { name: "删除" })).toBeEnabled();
});

it("presents Agent assignment as a direct add action", () => {
  const plan = assetOperationPlanFixture();
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentId="claude-code"
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

it("distinguishes adding a Model from switching the current Model", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "model",
    before: { codex: { profiles: {}, active_profile_id: null } },
    after: {
      codex: {
        profiles: { work: { profile_id: "work", enabled: true } },
        active_profile_id: "work",
      },
    },
  };
  plan.relationship_changes = [{
    agent_id: "codex",
    asset: { domain: "model", profile_id: "work" },
    action: "add",
  }];
  plan.model_state_changes = [{
    agent_id: "codex",
    profile_id: "work",
    before: { added: false, enabled: false, active: false },
    after: { added: true, enabled: true, active: true },
    fallback_profile_id: null,
    reason: "model_added",
  }];
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentId="codex"
      agentName="Codex"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );
  expect(screen.getByRole("heading", { name: "确认添加 Model" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: "确认切换 Model" })).not.toBeInTheDocument();
  expect(screen.getByText("未添加 → 已启用 · 当前")).toBeVisible();
});

it("separates direct Skill assignment from compatible visibility", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "skill",
    before: { "claude-code": [], opencode: [] },
    after: { "claude-code": ["frontend-design"], opencode: ["frontend-design"] },
  };
  plan.relationship_changes = [
    {
      agent_id: "claude-code",
      asset: { domain: "skill", name: "frontend-design" },
      action: "add",
    },
    {
      agent_id: "opencode",
      asset: { domain: "skill", name: "frontend-design" },
      action: "add",
    },
  ];
  plan.target_files = ["~/.claude/skills/frontend-design"];
  plan.affected_agent_ids = ["claude-code", "opencode"];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentId="claude-code"
      agentName="Claude Code"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByText("Claude Code · 同一目录也被 1 个 Agent 读取")).toBeVisible();
  expect(screen.getByText("只写入一个目录；兼容 Agent 会读取同一份 Skill，不会重复安装。")).toBeVisible();
  expect(screen.getByRole("heading", { name: "生效范围" })).toBeVisible();
  expect(screen.getByText("直接添加")).toBeVisible();
  expect(screen.getByText("兼容可见")).toBeVisible();
  expect(screen.getByRole("heading", { name: "实际写入位置" })).toBeVisible();
  expect(screen.getByText("~/.claude/skills/frontend-design")).toBeVisible();
  expect(screen.queryByText("~/.config/opencode/skills/frontend-design")).not.toBeInTheDocument();
});

it("shows configuration paths and shared Skill migration compactly", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-configuration";
  plan.domain_plan = {
    domain: "agent-configuration",
    agent_id: "codex",
    before: {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "mcp_servers",
      model_paths: ["~/.codex/config.toml"],
      skills_global_dir: "~/.agents/skills",
    },
    after: {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "mcp_servers",
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
  expect(screen.getByText("Codex · Skills 目录变更涉及 1 个其他 Agent")).toBeVisible();
  expect(screen.queryByText(/另影响/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存配置" })).toBeEnabled();
});

it("describes shared Skills readers even when the changed directory has no Skills to migrate", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-configuration";
  plan.domain_plan = {
    domain: "agent-configuration",
    agent_id: "codex",
    before: {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "mcp_servers",
      model_paths: ["~/.codex/config.toml"],
      skills_global_dir: "~/.agents/skills",
    },
    after: {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "mcp_servers",
      model_paths: ["~/.codex/config.toml"],
      skills_global_dir: "~/.private/skills",
    },
    skills_before: {},
    skills_after: {},
    affected_agent_ids: ["codex", "cursor"],
    migrated_skill_names: [],
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

  expect(screen.getByText("Codex · Skills 目录变更涉及 1 个其他 Agent")).toBeVisible();
  expect(screen.queryByText(/另影响/)).not.toBeInTheDocument();
});

it("reviews an MCP key-only location change without implying migration", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-configuration";
  plan.domain_plan = {
    domain: "agent-configuration",
    agent_id: "amp",
    before: {
      mcp_path: "~/.config/amp/settings.json",
      mcp_key: "mcpServers",
      model_paths: [],
      skills_global_dir: null,
    },
    after: {
      mcp_path: "~/.config/amp/settings.json",
      mcp_key: "amp.mcpServers",
      model_paths: [],
      skills_global_dir: null,
    },
    skills_before: {},
    skills_after: {},
    affected_agent_ids: ["amp"],
    migrated_skill_names: [],
  };
  plan.target_files = [];
  plan.affected_agent_ids = ["amp"];
  plan.relationship_changes = [];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentName="Amp"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByText("MCP 配置键")).toBeVisible();
  expect(screen.queryByText("MCP 文件路径")).not.toBeInTheDocument();
  expect(screen.getByText("mcpServers")).toBeVisible();
  expect(screen.getByText("amp.mcpServers")).toBeVisible();
  expect(screen.getByText(
    "只更新 MUX 后续使用的 MCP 配置位置；旧文件不会删除，现有 MCP 配置不会复制到新位置。",
  )).toBeVisible();
});

it("explains that changing Model paths does not move existing configuration", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-configuration";
  plan.domain_plan = {
    domain: "agent-configuration",
    agent_id: "codex",
    before: {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "mcp_servers",
      model_paths: ["~/.codex/config.toml"],
      skills_global_dir: "~/.agents/skills",
    },
    after: {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "mcp_servers",
      model_paths: ["~/.codex/models.toml"],
      skills_global_dir: "~/.agents/skills",
    },
    skills_before: {},
    skills_after: {},
    affected_agent_ids: ["codex"],
    migrated_skill_names: [],
  };
  plan.target_files = [];
  plan.affected_agent_ids = ["codex"];
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

  expect(screen.getByText(
    "只更新 MUX 后续使用的 Model 配置位置；旧文件不会删除，现有 Model 配置不会复制到新位置。",
  )).toBeVisible();
  expect(screen.queryByText(/后续使用的 MCP/)).not.toBeInTheDocument();
});
