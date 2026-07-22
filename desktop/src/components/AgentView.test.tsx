import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type { AgentInfo, ModelAdoptionCandidate, ModelAgentView } from "../lib/types";
import { AgentView } from "./AgentView";

const apiMocks = vi.hoisted(() => ({
  listModelAgents: vi.fn().mockResolvedValue([]),
  listModelProfiles: vi.fn().mockResolvedValue([]),
}));

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return { ...actual, ...apiMocks };
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const skillsOnlyAgent: AgentInfo = {
  id: "cortex-code",
  name: "Cortex Code",
  format: "unknown",
  key: "",
  has_global: false,
  has_project: false,
  enabled: true,
  supported_transports: [],
  global: null,
  project: null,
  skills_global_dir: "~/.snowflake/cortex/skills",
  docs: "https://docs.snowflake.com/",
  note: "只管理 Skills。",
  category: "coding-agent",
  evidence: "official-source",
  verified_at: "2026-07-22",
  builtin: true,
};

const state = {
  entries: [],
  agents: [skillsOnlyAgent],
  installed: [],
  refreshAgents: vi.fn().mockResolvedValue([skillsOnlyAgent]),
  rescan: vi.fn().mockResolvedValue([]),
} as unknown as InstallState;

const skillsState = {
  inventory: {
    items: [],
    agents: [{
      id: skillsOnlyAgent.id,
      name: skillsOnlyAgent.name,
      target_id: "cortex-user",
      global_dir: skillsOnlyAgent.skills_global_dir,
      affected_agent_ids: [skillsOnlyAgent.id],
      docs: skillsOnlyAgent.docs,
      evidence: skillsOnlyAgent.evidence,
      verified_at: skillsOnlyAgent.verified_at,
    }],
    targets: [],
    recovery_error: null,
  },
  loading: false,
  error: null,
  refresh: vi.fn(),
} as unknown as SkillsState;

it("shows a Skills-only Agent without a configuration-path map or empty MCP schema metadata", async () => {
  render(
    <AgentView
      state={state}
      skillsState={skillsState}
      agentId={skillsOnlyAgent.id}
    />,
  );

  expect(screen.queryByText("配置位置")).not.toBeInTheDocument();
  expect(screen.queryByText(/读取或写入的实际位置/)).not.toBeInTheDocument();
  expect(screen.queryByText(skillsOnlyAgent.skills_global_dir!)).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "编辑 Agent 设置" })).not.toBeInTheDocument();
  expect(screen.queryByText(/UNKNOWN/)).not.toBeInTheDocument();

  await waitFor(() => {
    expect(screen.getByRole("tab", { name: /Skills/ })).toHaveAttribute("aria-selected", "true");
  });

  await userEvent.click(screen.getByRole("tab", { name: /MCPs/ }));
  expect(screen.getByText("此 Agent 未接入 MCP。")).toBeVisible();
  expect(screen.queryByRole("button", { name: "添加 MCP" })).not.toBeInTheDocument();
});

it("offers only targeted MUX adoption for an external MCP card", async () => {
  const mcpAgent: AgentInfo = {
    ...skillsOnlyAgent,
    id: "codex",
    name: "Codex",
    format: "toml",
    key: "mcp_servers",
    has_global: true,
    global: "~/.codex/config.toml",
    supported_transports: ["stdio", "http"],
    skills_global_dir: null,
  };
  const mcpState = {
    ...state,
    agents: [mcpAgent],
    refreshAgents: vi.fn().mockResolvedValue([mcpAgent]),
  } as unknown as InstallState;
  const consumptionState = {
    inventory: {
      consumptions: [],
      external: [{
        agent_id: "codex",
        asset: { domain: "mcp", key: "computer-use::stdio" },
        desired: false,
        observed: true,
        enabled: true,
        status: "external",
        reason: "mcp_adoptable",
        affected_agent_ids: ["codex"],
      }],
    },
  } as unknown as ConsumptionState;
  const onManageExternalMcp = vi.fn();

  render(
    <AgentView
      state={mcpState}
      skillsState={skillsState}
      consumptionState={consumptionState}
      agentId="codex"
      onManageExternalMcp={onManageExternalMcp}
    />,
  );

  expect(screen.queryByText("配置位置")).not.toBeInTheDocument();
  expect(screen.queryByText(mcpAgent.global!)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "编辑 Agent 设置" })).toBeVisible();

  const card = screen.getByText("computer-use").closest<HTMLElement>("li");
  expect(card).not.toBeNull();
  expect(card).toHaveAttribute("data-enabled", "false");
  expect(within(card!).queryByRole("switch")).not.toBeInTheDocument();
  expect(within(card!).queryByRole("button", { name: /查看|移除/ })).not.toBeInTheDocument();

  await userEvent.click(within(card!).getByRole("button", { name: "让 MUX 管理" }));
  expect(onManageExternalMcp).toHaveBeenCalledWith("computer-use::stdio");
});

it("renders every external Model as its own disabled adoption card", async () => {
  const modelAgentInfo: AgentInfo = {
    ...skillsOnlyAgent,
    id: "opencode",
    name: "OpenCode",
    format: "json",
    key: "mcp",
    has_global: true,
    global: "~/.config/opencode/opencode.json",
    supported_transports: ["stdio", "http"],
    skills_global_dir: null,
  };
  const modelAgent: ModelAgentView = {
    id: "opencode",
    name: "OpenCode",
    mode: "managed",
    installed: true,
    config_path: "~/.config/opencode/opencode.json",
    config_paths: ["~/.config/opencode/opencode.json"],
    docs: "https://opencode.ai/docs/models/",
    assigned_profile: null,
    assigned_profiles: [],
    active_profile: null,
    supports_multiple: true,
    credential_mode: "environment-reference",
    supported_protocols: ["openai-completions"],
    note: "",
  };
  const candidate: ModelAdoptionCandidate = {
    candidate_id: "opencode:openrouter",
    agent_id: "opencode",
    native_id: "openrouter",
    name: "HY3",
    provider: "openrouter",
    model_vendor: "tencent",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "tencent/hy3:free",
    env_key: "OPENROUTER_API_KEY",
    active: true,
    credential_kind: "environment-reference",
    status: "adoptable",
    reason: null,
    fingerprint: "same-model",
    settings_hash: "settings",
    target_hash: "target",
    candidate_hash: "candidate",
  };
  apiMocks.listModelAgents.mockResolvedValueOnce([modelAgent]);
  const onOpenMigration = vi.fn();

  render(
    <AgentView
      state={{ ...state, agents: [modelAgentInfo] } as unknown as InstallState}
      skillsState={skillsState}
      consumptionState={{ inventory: { consumptions: [], external: [] } } as unknown as ConsumptionState}
      agentId="opencode"
      modelMigrationCandidates={[candidate]}
      onOpenMigration={onOpenMigration}
    />,
  );

  await userEvent.click(screen.getByRole("tab", { name: /Models/ }));
  const card = await screen.findByText("HY3").then((node) => node.closest<HTMLElement>("li"));
  expect(card).not.toBeNull();
  expect(card).toHaveAttribute("data-enabled", "false");
  expect(within(card!).getByText("Agent 当前")).toBeVisible();
  expect(within(card!).queryByRole("switch")).not.toBeInTheDocument();
  expect(within(card!).queryByRole("button", { name: /查看|移除/ })).not.toBeInTheDocument();

  await userEvent.click(within(card!).getByRole("button", { name: "让 MUX 管理" }));
  expect(onOpenMigration).toHaveBeenCalledWith("model:same-model");
});

it("renders external Skills as disabled cards with targeted adoption", async () => {
  const externalSkill = {
    identity: "target:cortex-user:review",
    name: "review",
    description: "Review changes",
    content_kind: "instructions",
    states: ["external"],
    location: { kind: "agent_target", target_id: "cortex-user", global_dir: "~/.snowflake/cortex/skills" },
    source: null,
    resolved_revision: null,
    content_hash: "hash",
    risk: { level: "low", findings: [], finding_count: 0, findings_truncated: false },
    update: { available: false, checked_at: null, resolved_revision: null, etag: null, error: null, retry_at: null },
    assigned_target_ids: [],
    affected_agent_ids: [skillsOnlyAgent.id],
    installed_at: null,
    updated_at: null,
  };
  const externalSkillsState = {
    ...skillsState,
    inventory: { ...skillsState.inventory!, items: [externalSkill] },
  } as unknown as SkillsState;
  const consumptionState = {
    inventory: {
      consumptions: [],
      external: [{
        agent_id: skillsOnlyAgent.id,
        asset: { domain: "skill", name: "review" },
        desired: false,
        observed: true,
        status: "external",
        reason: "skill_external",
        affected_agent_ids: [skillsOnlyAgent.id],
      }],
    },
  } as unknown as ConsumptionState;
  const onOpenMigration = vi.fn();

  render(
    <AgentView
      state={state}
      skillsState={externalSkillsState}
      consumptionState={consumptionState}
      agentId={skillsOnlyAgent.id}
      onOpenMigration={onOpenMigration}
    />,
  );

  const card = await screen.findByText("review").then((node) => node.closest<HTMLElement>("li"));
  expect(card).not.toBeNull();
  expect(card).toHaveAttribute("data-enabled", "false");
  expect(within(card!).getByText("Review changes")).toBeVisible();
  expect(within(card!).queryByRole("button", { name: /查看|移除/ })).not.toBeInTheDocument();

  await userEvent.click(within(card!).getByRole("button", { name: "让 MUX 管理" }));
  expect(onOpenMigration).toHaveBeenCalledWith("skill:review");
});
