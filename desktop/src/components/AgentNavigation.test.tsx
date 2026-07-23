import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { AgentCapabilityView, AgentInfo } from "../lib/types";
import { mergeAgentInfos } from "../lib/agentCapabilities";
import { AgentNavigation } from "./AgentNavigation";

const pinnedAgentMocks = vi.hoisted(() => ({
  commit: vi.fn(),
}));

vi.mock("../hooks/usePinnedAgents", () => ({
  usePinnedAgents: () => ({
    agentIds: ["claude-code", "codex", "qoder"],
    ready: true,
    saving: false,
    commit: pinnedAgentMocks.commit,
  }),
}));

function agent(id: string, name: string): AgentInfo {
  return {
    id,
    name,
    format: "json",
    key: "mcpServers",
    has_global: true,
    has_project: false,
    enabled: true,
    supported_transports: ["stdio", "http"],
    global: `~/.${id}/settings.json`,
    project: null,
    skills_global_dir: null,
    docs: null,
    note: null,
    category: "coding",
    evidence: "official",
    verified_at: null,
    builtin: true,
  };
}

const agents = [
  agent("claude-code", "Claude Code"),
  agent("codex", "Codex"),
  agent("qoder", "Qoder"),
  agent("amp", "Amp"),
];

beforeEach(() => {
  pinnedAgentMocks.commit.mockReset();
  pinnedAgentMocks.commit.mockResolvedValue(true);
});

afterEach(cleanup);

it("keeps the HTML drag source hit-testable and commits the dropped order", async () => {
  const { container } = render(
    <AgentNavigation
      agents={agents}
      selectedAgentId="codex"
      onSelectAgent={vi.fn()}
    />,
  );

  const trigger = container.querySelector<HTMLButtonElement>(
    ".mux-agent-picker-trigger",
  );
  expect(trigger).not.toBeNull();
  fireEvent.click(trigger!);

  const sourceHandle = screen.getByRole("button", {
    name: "调整 Qoder 的置顶顺序",
  });
  const sourceSlot = sourceHandle.closest(".mux-agent-picker-slot");
  const targetHandle = screen.getByRole("button", {
    name: "调整 Claude Code 的置顶顺序",
  });
  const targetSlot = targetHandle.closest(".mux-agent-picker-slot");
  expect(sourceSlot).not.toBeNull();
  expect(targetSlot).not.toBeNull();

  vi.spyOn(targetSlot!, "getBoundingClientRect").mockReturnValue({
    top: 0,
    bottom: 48,
    left: 0,
    right: 360,
    width: 360,
    height: 48,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  const dataTransfer = {
    effectAllowed: "none",
    dropEffect: "none",
    setData: vi.fn(),
    setDragImage: vi.fn(),
  } as unknown as DataTransfer;

  fireEvent.dragStart(sourceHandle, { dataTransfer });
  expect(sourceSlot).toHaveAttribute("data-drag-source", "true");
  expect(targetSlot).toHaveAttribute("data-sorting", "true");

  fireEvent.dragOver(targetSlot!, { clientY: 1, dataTransfer });
  fireEvent.drop(targetSlot!, { clientY: 1, dataTransfer });

  await waitFor(() => {
    expect(pinnedAgentMocks.commit).toHaveBeenCalledWith([
      "qoder",
      "claude-code",
      "codex",
    ]);
  });
});

it("uses a dedicated crossed pin for unpinning and keeps the action wired", async () => {
  const { container } = render(
    <AgentNavigation
      agents={agents}
      selectedAgentId="codex"
      onSelectAgent={vi.fn()}
    />,
  );

  fireEvent.click(container.querySelector<HTMLButtonElement>(".mux-agent-picker-trigger")!);

  const unpin = screen.getByRole("button", { name: "取消置顶 Claude Code" });
  const pin = screen.getByRole("button", { name: "置顶 Amp" });
  expect(screen.queryByText("JSON · claude-code")).not.toBeInTheDocument();
  expect(screen.queryByText("JSON · amp")).not.toBeInTheDocument();
  expect(unpin.querySelector('[data-icon="pin-off"]')).not.toBeNull();
  expect(unpin.querySelector('[data-icon="pin"]')).toBeNull();
  expect(pin.querySelector('[data-icon="pin"]')).not.toBeNull();
  expect(pin.querySelector('[data-icon="pin-off"]')).toBeNull();

  fireEvent.click(unpin);
  await waitFor(() => {
    expect(pinnedAgentMocks.commit).toHaveBeenCalledWith(["codex", "qoder"]);
  });
  expect(screen.getByText("Claude Code 已取消置顶")).toHaveAttribute("aria-live", "polite");
});

it("lists and opens a projection-only Model Agent", () => {
  const projection: AgentCapabilityView = {
    identity: {
      id: "model-only",
      name: "Projection Model",
      enabled: true,
      builtin: true,
      category: "coding-agent",
      evidence: "official",
    },
    installed: true,
    capabilities: {
      model: {
        mode: "managed",
        installed: true,
        config_paths: ["~/.model-only/config.json"],
        assigned_profiles: [],
        supports_multiple: false,
        credential_mode: "guided",
        supported_protocols: ["openai-responses"],
      },
    },
  };
  const onSelectAgent = vi.fn();
  const { container } = render(
    <AgentNavigation
      agents={mergeAgentInfos([], [projection])}
      selectedAgentId={null}
      onSelectAgent={onSelectAgent}
    />,
  );

  fireEvent.click(container.querySelector<HTMLButtonElement>(".mux-agent-picker-trigger")!);
  fireEvent.click(screen.getByText("Projection Model").closest("button")!);
  expect(onSelectAgent).toHaveBeenCalledWith("model-only");
});
