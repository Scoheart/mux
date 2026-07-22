import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { AgentInfo } from "../lib/types";
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
