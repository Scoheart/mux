import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  AgentCapabilityView,
  AgentInfo,
  ModelAdoptionCandidate,
  ModelAgentView,
  ModelProfileView,
} from "../lib/types";
import { assetOperationPlanFixture } from "../test/consumptionFixtures";
import { AgentView } from "./AgentView";
import { ToastProvider } from "./Toast";

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

const skillsOnlyProjection: AgentCapabilityView = {
  identity: {
    id: skillsOnlyAgent.id,
    name: skillsOnlyAgent.name,
    enabled: true,
    builtin: true,
    category: skillsOnlyAgent.category,
    evidence: skillsOnlyAgent.evidence,
    docs: skillsOnlyAgent.docs,
    note: skillsOnlyAgent.note,
    verified_at: skillsOnlyAgent.verified_at,
  },
  installed: true,
  capabilities: {
    skill: {
      installed: true,
      target_id: "cortex-user",
      global_dir: skillsOnlyAgent.skills_global_dir!,
      alias_dirs: [],
      affected_agent_ids: [skillsOnlyAgent.id],
    },
  },
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

it("opens a projection-only Skills Agent without a legacy MCP-shaped row", async () => {
  const projectionOnlyState = {
    ...state,
    agents: [],
    refreshAgents: vi.fn().mockResolvedValue([]),
  } as unknown as InstallState;
  const consumptionState = {
    agents: [skillsOnlyProjection],
    inventory: { consumptions: [], external: [] },
  } as unknown as ConsumptionState;

  render(
    <AgentView
      state={projectionOnlyState}
      skillsState={skillsState}
      consumptionState={consumptionState}
      agentId={skillsOnlyAgent.id}
    />,
  );

  const locations = screen.getByRole("region", { name: "配置位置" });
  expect(within(locations).getByText("配置位置")).toBeVisible();
  expect(within(locations).getByText(/读取或写入的实际位置/)).toBeVisible();
  expect(within(locations).getByText("MCPs")).toBeVisible();
  expect(within(locations).getByText("Models")).toBeVisible();
  expect(within(locations).getByText("Skills")).toBeVisible();
  expect(within(locations).getByText(skillsOnlyAgent.skills_global_dir!)).toBeVisible();
  expect(screen.getByRole("button", { name: "编辑 Agent 设置" })).toBeVisible();
  expect(screen.queryByText(/UNKNOWN/)).not.toBeInTheDocument();
  expect(screen.queryByText(/coding-agent/)).not.toBeInTheDocument();
  expect(screen.queryByText(/公开来源/)).not.toBeInTheDocument();
  expect(screen.queryByText(skillsOnlyAgent.verified_at!)).not.toBeInTheDocument();
  expect(screen.queryByText("已核验")).not.toBeInTheDocument();

  await waitFor(() => {
    expect(screen.getByRole("tab", { name: /Skills/ })).toHaveAttribute("aria-selected", "true");
  });

  await userEvent.click(screen.getByRole("tab", { name: /MCPs/ }));
  expect(screen.getByText("此 Agent 未接入 MCP。")).toBeVisible();
  expect(screen.queryByRole("button", { name: "添加 MCP" })).not.toBeInTheDocument();
});

it("keeps meaningful Agent badges while leaving builtin headers clean", () => {
  const communityAgent: AgentInfo = {
    ...skillsOnlyAgent,
    id: "pi",
    name: "Pi",
    evidence: "community-extension",
  };
  const customAgent: AgentInfo = {
    ...skillsOnlyAgent,
    id: "custom-agent",
    name: "Custom Agent",
    builtin: false,
  };
  const badgeState = {
    ...state,
    agents: [skillsOnlyAgent, communityAgent, customAgent],
  } as unknown as InstallState;
  const badgeSkillsState = {
    ...skillsState,
    inventory: { ...skillsState.inventory, agents: [] },
  } as unknown as SkillsState;

  const view = render(
    <AgentView
      state={badgeState}
      skillsState={badgeSkillsState}
      agentId={skillsOnlyAgent.id}
    />,
  );

  expect(screen.queryByText("已核验")).not.toBeInTheDocument();
  expect(screen.queryByText("自定义")).not.toBeInTheDocument();

  view.rerender(
    <AgentView
      state={badgeState}
      skillsState={badgeSkillsState}
      agentId={communityAgent.id}
    />,
  );
  expect(screen.getByText("社区扩展")).toBeVisible();

  view.rerender(
    <AgentView
      state={badgeState}
      skillsState={badgeSkillsState}
      agentId={customAgent.id}
    />,
  );
  expect(screen.getByText("自定义")).toBeVisible();
});

it("keeps a Model-only Agent in the full resource workspace", async () => {
  const modelOnlyAgent: AgentCapabilityView = {
    identity: {
      id: "model-only",
      name: "Model Only",
      enabled: true,
      builtin: true,
      category: "coding-agent",
      evidence: "official",
      docs: "https://example.invalid",
      note: "只管理 Model。",
      verified_at: "2026-07-23",
    },
    installed: true,
    capabilities: {
      model: {
        mode: "managed",
        installed: true,
        config_paths: ["~/.model-only/config.json"],
        assigned_profiles: [],
        active_profile: null,
        supports_multiple: false,
        credential_mode: "guided",
        supported_protocols: ["openai-responses"],
      },
    },
  };
  const modelOnlyState = {
    ...state,
    agents: [],
    refreshAgents: vi.fn().mockResolvedValue([]),
  } as unknown as InstallState;
  apiMocks.listModelAgents.mockResolvedValueOnce([{
    id: modelOnlyAgent.identity.id,
    name: modelOnlyAgent.identity.name,
    mode: "managed",
    installed: true,
    config_path: "~/.model-only/config.json",
    config_paths: ["~/.model-only/config.json"],
    docs: "https://example.invalid",
    assigned_profile: null,
    assigned_profiles: [],
    active_profile: null,
    supports_multiple: false,
    credential_mode: "guided",
    supported_protocols: ["openai-responses"],
    note: "",
  } satisfies ModelAgentView]);

  render(
    <AgentView
      state={modelOnlyState}
      skillsState={skillsState}
      consumptionState={{
        agents: [modelOnlyAgent],
        inventory: { consumptions: [], external: [] },
      } as unknown as ConsumptionState}
      agentId={modelOnlyAgent.identity.id}
    />,
  );

  await waitFor(() => {
    expect(screen.getByRole("button", { name: "编辑 Agent 设置" })).toBeVisible();
  });
  expect(screen.getByText("~/.model-only/config.json")).toBeVisible();
  expect(screen.queryByText(/未提供可写的用户级全局配置/)).not.toBeInTheDocument();
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
    skills_global_dir: "~/.agents/skills",
    skills_global_dirs: ["~/.agents/skills", "~/.claude/skills"],
  };
  const mcpState = {
    ...state,
    entries: [{
      name: "computer-use",
      description: "",
      tags: [],
      config: { stdio: { command: "computer-use" } },
    }],
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

  const locations = screen.getByRole("region", { name: "配置位置" });
  expect(within(locations).getByText(mcpAgent.global!)).toBeVisible();
  expect(within(locations).getByText("~/.agents/skills · ~/.claude/skills")).toBeVisible();
  expect(screen.queryByText(`${mcpAgent.id} · ${mcpAgent.category}`)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "编辑 Agent 设置" })).toBeVisible();
  await userEvent.click(within(locations).getByRole("button", { name: "编辑配置" }));
  expect(screen.getByRole("heading", { name: "编辑配置" })).toBeVisible();

  const card = screen.getByText("computer-use").closest<HTMLElement>("li");
  expect(card).not.toBeNull();
  expect(within(card!).queryByText("computer-use::stdio")).not.toBeInTheDocument();
  expect(card!.querySelector(".mux-consumption-copy small")).toBeNull();
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

it("uses one current-Model switch and activates a disabled backup atomically", async () => {
  const piAgent: AgentInfo = {
    ...skillsOnlyAgent,
    id: "pi",
    name: "Pi Coding Agent",
    format: "json",
    key: "mcpServers",
    has_global: true,
    global: "~/.pi/agent/mcp.json",
    supported_transports: ["stdio", "http"],
    skills_global_dir: "~/.pi/agent/skills",
  };
  const piModelAgent: ModelAgentView = {
    id: "pi",
    name: piAgent.name,
    mode: "managed",
    installed: true,
    config_path: "~/.pi/agent/models.json + ~/.pi/agent/settings.json",
    config_paths: ["~/.pi/agent/models.json", "~/.pi/agent/settings.json"],
    docs: "https://github.com/earendil-works/pi",
    assigned_profile: "idealab",
    assigned_profiles: ["idealab", "qwen"],
    active_profile: "idealab",
    supports_multiple: true,
    credential_mode: "keychain-command",
    supported_protocols: ["openai-responses"],
    note: "",
  };
  const profiles: ModelProfileView[] = [
    {
      id: "idealab",
      name: "idealab",
      provider: "idealab",
      protocol: "openai-responses",
      base_url: "https://idealab.example.test/v1",
      model: "Peach-07-17-DogFooding",
      reasoning: true,
      catalog_key: "idealab/Peach-07-17-DogFooding",
      credential_saved: true,
    },
    {
      id: "qwen",
      name: "Qwen3 7 Plus",
      provider: "max-ai",
      protocol: "openai-responses",
      base_url: "https://max-ai.example.test/v1",
      model: "qwen3.7-plus",
      reasoning: true,
      catalog_key: "max-ai/qwen3.7-plus",
      credential_saved: true,
    },
  ];
  apiMocks.listModelAgents.mockResolvedValue([piModelAgent]);
  apiMocks.listModelProfiles.mockResolvedValue(profiles);
  const inventory = {
    consumptions: [
      {
        agent_id: "pi",
        asset: { domain: "model" as const, profile_id: "idealab" },
        desired: true,
        observed: true,
        enabled: true,
        active: true,
        desired_active: true,
        status: "synced" as const,
        reason: null,
        affected_agent_ids: ["pi"],
      },
      {
        agent_id: "pi",
        asset: { domain: "model" as const, profile_id: "qwen" },
        desired: true,
        observed: false,
        enabled: false,
        active: false,
        desired_active: false,
        status: "synced" as const,
        reason: null,
        affected_agent_ids: ["pi"],
      },
    ],
    external: [],
  };
  const plan = assetOperationPlanFixture();
  plan.domain_plan = {
    domain: "model",
    before: {
      pi: {
        profiles: {
          idealab: { profile_id: "idealab", enabled: true },
          qwen: { profile_id: "qwen", enabled: false },
        },
        active_profile_id: "idealab",
      },
    },
    after: {
      pi: {
        profiles: {
          idealab: { profile_id: "idealab", enabled: true },
          qwen: { profile_id: "qwen", enabled: true },
        },
        active_profile_id: "qwen",
      },
    },
  };
  plan.relationship_changes = [];
  plan.model_state_changes = [
    {
      agent_id: "pi",
      profile_id: "idealab",
      before: { added: true, enabled: true, active: true },
      after: { added: true, enabled: true, active: false },
      reason: "active_model_changed",
    },
    {
      agent_id: "pi",
      profile_id: "qwen",
      before: { added: true, enabled: false, active: false },
      after: { added: true, enabled: true, active: true },
      reason: "active_model_changed",
    },
  ];
  plan.target_files = ["~/.pi/agent/models.json", "~/.pi/agent/settings.json"];
  plan.affected_agent_ids = ["pi"];
  const planActiveModel = vi.fn().mockResolvedValue(plan);
  const commit = vi.fn().mockResolvedValue(inventory);
  const taskSkillsState = {
    ...skillsState,
    refresh: vi.fn().mockResolvedValue(skillsState.inventory),
  } as unknown as SkillsState;
  const consumptionState = {
    agents: [],
    inventory,
    plan: null,
    planActiveModel,
    commit,
  } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <AgentView
        state={{ ...state, agents: [piAgent] } as unknown as InstallState}
        skillsState={taskSkillsState}
        consumptionState={consumptionState}
        agentId="pi"
      />
    </ToastProvider>,
  );

  await userEvent.click(await screen.findByRole("tab", { name: /Models/ }));
  expect(screen.getByText("2 个已添加 · 同一时间使用其中一个")).toBeVisible();
  expect(screen.queryByRole("button", { name: "设为当前" })).not.toBeInTheDocument();
  const current = screen.getByRole("switch", {
    name: "idealab 当前正在使用；请选择其他 Model 切换",
  });
  const backup = screen.getByRole("switch", { name: "使用 Qwen3 7 Plus" });
  expect(current).toBeChecked();
  expect(backup).not.toBeChecked();
  expect(
    screen
      .getAllByRole("switch")
      .filter((item) => item.getAttribute("aria-checked") === "true"),
  ).toHaveLength(1);

  await userEvent.click(current);
  expect(await screen.findByText("请先选择其他当前 Model。")).toBeVisible();
  expect(planActiveModel).not.toHaveBeenCalled();

  await userEvent.click(backup);
  await waitFor(() => {
    expect(planActiveModel).toHaveBeenCalledWith("pi", "qwen");
    expect(commit).toHaveBeenCalledOnce();
  });
});
