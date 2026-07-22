import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type { AgentInfo } from "../lib/types";
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

it("shows a Skills-only Agent without exposing empty MCP schema metadata", async () => {
  render(
    <AgentView
      state={state}
      skillsState={skillsState}
      agentId={skillsOnlyAgent.id}
    />,
  );

  const configurationLocations = screen.getByLabelText("配置位置");
  const mcpLocation = within(configurationLocations)
    .getByText("MCPs")
    .closest<HTMLElement>(".mux-agent-file-row");
  expect(mcpLocation).not.toBeNull();
  expect(within(mcpLocation!).getByText("此 Agent 未接入 MCP")).toBeVisible();
  expect(within(mcpLocation!).getByText("未接入")).toBeVisible();
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

  const card = screen.getByText("computer-use").closest<HTMLElement>("li");
  expect(card).not.toBeNull();
  expect(card).toHaveAttribute("data-enabled", "false");
  expect(within(card!).queryByRole("switch")).not.toBeInTheDocument();
  expect(within(card!).queryByRole("button", { name: /查看|移除/ })).not.toBeInTheDocument();

  await userEvent.click(within(card!).getByRole("button", { name: "让 MUX 管理" }));
  expect(onManageExternalMcp).toHaveBeenCalledWith("computer-use::stdio");
});
