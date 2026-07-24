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
    summary: ["删除中央模型配置", "将从 2 个已关联 Agent 移除"],
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
  expect(screen.getByText("删除中央模型配置")).toBeVisible();
  expect(screen.getByText("将从 2 个已关联 Agent 移除")).toBeVisible();
  expect(screen.queryByText(/Profile metadata|consumer/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除" })).toBeEnabled();
});

it("explains a central-only Model update without presenting zero Agents as an error", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-asset";
  plan.central_changes = [{
    asset: { domain: "model", profile_id: "work" },
    action: "update",
    summary: ["仅更新中央模型配置", "当前没有已关联的 Agent，无需同步"],
  }];
  plan.affected_agent_ids = [];
  plan.target_files = [];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByText("0 个 Agent · 0 个目标")).toBeVisible();
  expect(screen.getByText("仅更新中央模型配置")).toBeVisible();
  expect(screen.getByText("当前没有已关联的 Agent，无需同步")).toBeVisible();
  expect(screen.queryByText(/desired consumer|credential presence/)).not.toBeInTheDocument();
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
  expect(screen.getAllByText("Claude Code")).toHaveLength(2);
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
  expect(screen.getByRole("heading", { name: "确认添加当前 Model" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: "确认切换 Model" })).not.toBeInTheDocument();
  expect(screen.getByText("未添加 → 已启用 · 当前")).toBeVisible();
});

it("labels a non-current Model addition as a backup without raw conflict codes", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "model",
    before: {
      pi: {
        profiles: { current: { profile_id: "current", enabled: true } },
        active_profile_id: "current",
      },
    },
    after: {
      pi: {
        profiles: {
          current: { profile_id: "current", enabled: true },
          backup: { profile_id: "backup", enabled: true },
        },
        active_profile_id: "current",
      },
    },
  };
  plan.relationship_changes = [{
    agent_id: "pi",
    asset: { domain: "model", profile_id: "backup" },
    action: "add",
  }];
  plan.model_state_changes = [{
    agent_id: "pi",
    profile_id: "backup",
    before: { added: false, enabled: false, active: false },
    after: { added: true, enabled: true, active: false },
    fallback_profile_id: null,
    reason: "model_added",
  }];
  plan.target_files = ["~/.pi/agent/models.json"];
  plan.warnings = [];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentId="pi"
      agentName="Pi"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("heading", { name: "确认添加备选 Model" })).toBeVisible();
  expect(screen.getByRole("button", { name: "添加备选 Model" })).toBeEnabled();
  expect(screen.getByText("未添加 → 已启用 · 非当前")).toBeVisible();
  expect(screen.getByText("~/.pi/agent/models.json")).toBeVisible();
  expect(screen.queryByText("~/.pi/agent/settings.json")).not.toBeInTheDocument();
  expect(screen.queryByText(/model_active_state_drift|model_external_current/)).not.toBeInTheDocument();
});

it("translates Model conflict warnings into user-facing copy", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = { domain: "model", before: {}, after: {} };
  plan.warnings = [
    "pi: model_active_state_drift",
    "pi: model_external_current",
  ];
  plan.can_commit = false;

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByText("Pi：当前 Model 与 MUX 记录不一致，请先刷新或重新选择当前 Model")).toBeVisible();
  expect(screen.getByText("Pi：当前 Model 由 Agent 外部配置管理，切换前需要先让 MUX 接管")).toBeVisible();
  expect(screen.queryByText(/pi: model_/)).not.toBeInTheDocument();
});

it("presents a Model removal as a readable danger summary", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "model",
    before: {
      "claude-code": {
        profiles: { openrouter: { profile_id: "openrouter", enabled: true } },
        active_profile_id: "openrouter",
      },
    },
    after: {
      "claude-code": {
        profiles: {},
        active_profile_id: null,
      },
    },
  };
  plan.relationship_changes = [{
    agent_id: "claude-code",
    asset: { domain: "model", profile_id: "openrouter" },
    action: "remove",
  }];
  plan.model_state_changes = [];
  plan.target_files = [];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentId="claude-code"
      agentName="Claude Code"
      assetDisplayNames={{ "model:openrouter": "OpenRouter 自用" }}
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("heading", { name: "确认移除 Model" })).toBeVisible();
  expect(screen.getByText(/将从/)).toHaveTextContent(
    "将从Claude Code移除 Model · OpenRouter 自用。",
  );
  expect(screen.getByText("移除")).toHaveAttribute("data-action", "remove");
  expect(screen.getByText("Model · OpenRouter 自用")).toBeVisible();
  expect(screen.getByRole("button", { name: "移除 Model" })).toHaveClass("btn-danger");
  expect(screen.getByRole("button", { name: "关闭" })).toBeEnabled();
});

it("maps a stale Model removal conflict to a concise retry state", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "model",
    before: {
      "claude-code": {
        profiles: { openrouter: { profile_id: "openrouter", enabled: true } },
        active_profile_id: "openrouter",
      },
    },
    after: {
      "claude-code": {
        profiles: {},
        active_profile_id: null,
      },
    },
  };
  plan.relationship_changes = [{
    agent_id: "claude-code",
    asset: { domain: "model", profile_id: "openrouter" },
    action: "remove",
  }];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      error={{
        code: "model_consumption_missing",
        message: "the requested model is not assigned to this Agent",
      }}
      agentId="claude-code"
      agentName="Claude Code"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("alert")).toHaveTextContent(
    "该 Model 未添加到此 Agent，无法移除。",
  );
  expect(screen.queryByText(/requested model/i)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重试移除 Model" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "关闭" })).toBeEnabled();
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

it("explains direct and shared-directory impact for a Skill removal", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "skill",
    before: { "claude-code": ["dws"], opencode: ["dws"] },
    after: { "claude-code": [], opencode: [] },
  };
  plan.relationship_changes = [
    {
      agent_id: "claude-code",
      asset: { domain: "skill", name: "dws" },
      action: "remove",
    },
    {
      agent_id: "opencode",
      asset: { domain: "skill", name: "dws" },
      action: "remove",
    },
  ];
  plan.target_files = ["~/.agents/skills/dws"];
  plan.affected_agent_ids = ["claude-code", "opencode"];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      agentId="claude-code"
      agentName="Claude Code"
      agentDisplayNames={{ "claude-code": "Claude Code", opencode: "OpenCode" }}
      assetDisplayNames={{ "skill:dws": "dws" }}
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  const summary = screen.getByRole("heading", { name: "影响摘要" }).closest("section");
  expect(summary).toHaveTextContent("将从Claude Code移除 Skill · dws。");
  expect(summary).toHaveTextContent("共用目录将同步影响OpenCode的 Skill 可见性。");
  expect(screen.getByText("直接移除")).toHaveAttribute("data-action", "remove");
  expect(screen.getByText("同步不可见")).toHaveAttribute("data-action", "remove");
  expect(screen.getByText("~/.agents/skills/dws")).toBeVisible();
  expect(screen.queryByText("opencode")).not.toBeInTheDocument();
});

it("maps an unsafe managed Skill link error and restores the known target path", () => {
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "skill",
    before: { "claude-code": ["dws"] },
    after: { "claude-code": [] },
  };
  plan.relationship_changes = [{
    agent_id: "claude-code",
    asset: { domain: "skill", name: "dws" },
    action: "remove",
  }];
  plan.target_files = ["~/.agents/skills/dws"];

  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      error={'Conflict { message: "only an exact managed Skill link can be disabled", path: "" }'}
      agentId="claude-code"
      agentName="Claude Code"
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("alert")).toHaveTextContent(
    "无法移除：这不是可安全移除的托管 Skill 链接（~/.agents/skills/dws）。",
  );
  expect(screen.queryByText(/only an exact managed/i)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重试移除 Skill" })).toBeEnabled();
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
