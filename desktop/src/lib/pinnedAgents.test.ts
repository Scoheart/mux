import { expect, it } from "vitest";
import type { AgentInfo } from "./types.ts";
import {
  MAX_PINNED_AGENTS,
  buildAgentPickerSections,
  movePinnedAgentAfter,
  movePinnedAgentBefore,
  movePinnedAgentBy,
  previewPinnedAgentOrder,
  togglePinnedAgent,
  type PinnedDropPlacement,
} from "./pinnedAgents.ts";

function agent(id: string, name: string, hasGlobal = true): AgentInfo {
  return {
    id,
    name,
    format: "json",
    key: "mcpServers",
    has_global: hasGlobal,
    has_project: false,
    enabled: true,
    supported_transports: ["stdio", "http"],
    global: hasGlobal ? `~/.${id}/settings.json` : null,
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
  agent("codex", "Codex"),
  agent("claude-code", "Claude Code"),
  agent("qoder", "Qoder CLI"),
  agent("catalog-only", "Catalog Only", false),
];

it("sections preserve pinned order and exclude read-only or duplicate rows", () => {
  const sections = buildAgentPickerSections(
    agents,
    ["qoder", "missing", "codex", "qoder"],
    "",
  );
  expect(sections.pinned.map(({ id }) => id)).toEqual(["qoder", "codex"]);
  expect(sections.available.map(({ id }) => id)).toEqual(["claude-code"]);
  expect(sections.searchResults).toBeNull();
});

it("search merges pinned and available matches without duplicates", () => {
  const sections = buildAgentPickerSections(agents, ["qoder"], "code");
  expect(sections.searchResults?.map(({ id }) => id)).toEqual(["claude-code", "codex"]);
});

it("toggle removes existing pins, appends new pins, and enforces the limit", () => {
  const removed = ["codex", "qoder"];
  const appended = ["codex"];
  expect(togglePinnedAgent(removed, "codex")).toEqual(["qoder"]);
  expect(togglePinnedAgent(appended, "qoder")).toEqual(["codex", "qoder"]);
  expect(removed).toEqual(["codex", "qoder"]);
  expect(appended).toEqual(["codex"]);
  const full = Array.from({ length: MAX_PINNED_AGENTS }, (_, index) => `agent-${index}`);
  expect(togglePinnedAgent(full, "overflow")).toEqual(full);
});

it("keyboard and drag ordering are stable at boundaries", () => {
  const ids = ["claude-code", "codex", "qoder"];
  expect(movePinnedAgentBy(ids, "codex", -1)).toEqual(["codex", "claude-code", "qoder"]);
  expect(movePinnedAgentBy(ids, "claude-code", -1)).toEqual(ids);
  expect(movePinnedAgentBefore(ids, "qoder", "claude-code")).toEqual(["qoder", "claude-code", "codex"]);
  expect(movePinnedAgentBefore(ids, "codex", "codex")).toEqual(ids);
});

it("keyboard ordering moves down and preserves the lower boundary", () => {
  const ids = ["claude-code", "codex", "qoder"];
  const snapshot = [...ids];

  expect(movePinnedAgentBy(ids, "codex", 1)).toEqual(["claude-code", "qoder", "codex"]);
  expect(movePinnedAgentBy(ids, "qoder", 1)).toEqual(ids);
  expect(movePinnedAgentBy(ids, "missing", 1)).toEqual(ids);
  expect(ids).toEqual(snapshot);
});

it("drag ordering is stable for unknown source and target IDs", () => {
  const ids = ["claude-code", "codex", "qoder"];
  const snapshot = [...ids];

  expect(movePinnedAgentBefore(ids, "missing", "codex")).toEqual(ids);
  expect(movePinnedAgentBefore(ids, "codex", "missing")).toEqual(ids);
  expect(ids).toEqual(snapshot);
});

it("move-after places the first pinned Agent after the final row", () => {
  const ids = ["claude-code", "codex", "qoder"];

  expect(movePinnedAgentAfter(ids, "claude-code", "qoder")).toEqual([
    "codex",
    "qoder",
    "claude-code",
  ]);
  expect(ids).toEqual(["claude-code", "codex", "qoder"]);
});

it("move-after preserves order for invalid source, target, and self moves", () => {
  const ids = ["claude-code", "codex", "qoder"];

  expect(movePinnedAgentAfter(ids, "missing", "codex")).toEqual(ids);
  expect(movePinnedAgentAfter(ids, "codex", "missing")).toEqual(ids);
  expect(movePinnedAgentAfter(ids, "codex", "codex")).toEqual(ids);
  expect(ids).toEqual(["claude-code", "codex", "qoder"]);
});

it("reorder operations preserve original input arrays", () => {
  const keyboardIds = ["claude-code", "codex", "qoder"];
  const dragIds = ["claude-code", "codex", "qoder"];
  const keyboardSnapshot = [...keyboardIds];
  const dragSnapshot = [...dragIds];

  expect(movePinnedAgentBy(keyboardIds, "codex", 1)).toEqual(["claude-code", "qoder", "codex"]);
  expect(movePinnedAgentBefore(dragIds, "qoder", "claude-code")).toEqual(["qoder", "claude-code", "codex"]);
  expect(keyboardIds).toEqual(keyboardSnapshot);
  expect(dragIds).toEqual(dragSnapshot);
});

it("drag preview follows before and after targets across multiple rows", () => {
  const ids = ["claude-code", "codex", "qoder", "pi"];
  const first = previewPinnedAgentOrder(ids, "qoder", "claude-code", "before");
  const second = previewPinnedAgentOrder(first, "qoder", "codex", "after");

  expect(first).toEqual(["qoder", "claude-code", "codex", "pi"]);
  expect(second).toEqual(["claude-code", "codex", "qoder", "pi"]);
  expect(ids).toEqual(["claude-code", "codex", "qoder", "pi"]);
});

it("drag preview preserves order for invalid and self targets", () => {
  const ids = ["claude-code", "codex", "qoder"];
  const placements: PinnedDropPlacement[] = ["before", "after"];

  for (const placement of placements) {
    expect(previewPinnedAgentOrder(ids, "codex", "codex", placement)).toEqual(ids);
    expect(previewPinnedAgentOrder(ids, "missing", "codex", placement)).toEqual(ids);
    expect(previewPinnedAgentOrder(ids, "codex", "missing", placement)).toEqual(ids);
  }
});
