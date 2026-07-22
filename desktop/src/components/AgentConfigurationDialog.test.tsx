import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { AgentInfo } from "../lib/types";
import { assetOperationPlanFixture } from "../test/consumptionFixtures";
import { AgentConfigurationDialog } from "./AgentConfigurationDialog";

const apiMocks = vi.hoisted(() => ({
  planUpdateAgentConfiguration: vi.fn(),
}));

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return {
    ...actual,
    planUpdateAgentConfiguration: apiMocks.planUpdateAgentConfiguration,
  };
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const amp: AgentInfo = {
  id: "amp",
  name: "Amp",
  format: "json",
  key: "amp.mcpServers",
  has_global: true,
  has_project: false,
  enabled: true,
  supported_transports: ["stdio", "http"],
  global: "~/.config/amp/settings.json",
  project: null,
  skills_global_dir: null,
  docs: null,
  note: null,
  category: "coding",
  evidence: "official",
  verified_at: null,
  builtin: true,
};

it("edits and submits the MCP path and configuration key together", async () => {
  apiMocks.planUpdateAgentConfiguration.mockResolvedValue(assetOperationPlanFixture());
  render(
    <AgentConfigurationDialog
      agent={amp}
      modelAgent={null}
      onClose={vi.fn()}
      onSaved={vi.fn()}
    />,
  );

  expect(screen.getByLabelText("MCP 文件路径")).toHaveValue("~/.config/amp/settings.json");
  const keyInput = screen.getByLabelText("MCP 配置键");
  expect(keyInput).toHaveValue("amp.mcpServers");

  await userEvent.clear(keyInput);
  expect(screen.getByRole("button", { name: "继续" })).toBeDisabled();
  await userEvent.type(keyInput, "  custom.mcpServers  ");
  await userEvent.click(screen.getByRole("button", { name: "继续" }));

  await waitFor(() => {
    expect(apiMocks.planUpdateAgentConfiguration).toHaveBeenCalledWith("amp", {
      mcp_path: "~/.config/amp/settings.json",
      mcp_key: "custom.mcpServers",
      model_paths: [],
      skills_global_dir: null,
      skills_alias_dirs: [],
    });
  });
});

it("adds and removes multiple Skills compatibility directories", async () => {
  apiMocks.planUpdateAgentConfiguration.mockResolvedValue(assetOperationPlanFixture());
  const codex: AgentInfo = {
    ...amp,
    id: "codex",
    name: "Codex",
    global: "~/.codex/config.toml",
    skills_global_dir: "~/.agents/skills",
    skills_global_dirs: ["~/.agents/skills", "~/.claude/skills"],
  };
  render(
    <AgentConfigurationDialog
      agent={codex}
      modelAgent={null}
      onClose={vi.fn()}
      onSaved={vi.fn()}
    />,
  );

  expect(screen.getByLabelText("Skills 2")).toHaveValue("~/.claude/skills");
  await userEvent.click(screen.getByRole("button", { name: "移除 Skills 目录 2" }));
  expect(screen.queryByLabelText("Skills 2")).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "添加 Skills 目录" }));
  const second = screen.getByLabelText("Skills 2");
  expect(screen.getByRole("button", { name: "继续" })).toBeDisabled();
  await userEvent.type(second, "~/.codex/skills");
  await userEvent.click(screen.getByRole("button", { name: "继续" }));

  await waitFor(() => {
    expect(apiMocks.planUpdateAgentConfiguration).toHaveBeenCalledWith("codex", {
      mcp_path: "~/.codex/config.toml",
      mcp_key: "amp.mcpServers",
      model_paths: [],
      skills_global_dir: "~/.agents/skills",
      skills_alias_dirs: ["~/.codex/skills"],
    });
  });
});
