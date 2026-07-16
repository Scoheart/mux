import assert from "node:assert/strict";
import test from "node:test";
import type { AgentInfo } from "./types.ts";
import {
  MAX_PINNED_AGENTS,
  buildAgentPickerSections,
  movePinnedAgentBefore,
  movePinnedAgentBy,
  togglePinnedAgent,
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

test("sections preserve pinned order and exclude read-only or duplicate rows", () => {
  const sections = buildAgentPickerSections(
    agents,
    ["qoder", "missing", "codex", "qoder"],
    "",
  );
  assert.deepEqual(sections.pinned.map(({ id }) => id), ["qoder", "codex"]);
  assert.deepEqual(sections.available.map(({ id }) => id), ["claude-code"]);
  assert.equal(sections.searchResults, null);
});

test("search merges pinned and available matches without duplicates", () => {
  const sections = buildAgentPickerSections(agents, ["qoder"], "code");
  assert.deepEqual(
    sections.searchResults?.map(({ id }) => id),
    ["claude-code", "codex"],
  );
});

test("toggle removes existing pins, appends new pins, and enforces the limit", () => {
  const removed = ["codex", "qoder"];
  const appended = ["codex"];
  assert.deepEqual(togglePinnedAgent(removed, "codex"), ["qoder"]);
  assert.deepEqual(togglePinnedAgent(appended, "qoder"), ["codex", "qoder"]);
  assert.deepEqual(removed, ["codex", "qoder"]);
  assert.deepEqual(appended, ["codex"]);
  const full = Array.from({ length: MAX_PINNED_AGENTS }, (_, index) => `agent-${index}`);
  assert.deepEqual(togglePinnedAgent(full, "overflow"), full);
});

test("keyboard and drag ordering are stable at boundaries", () => {
  const ids = ["claude-code", "codex", "qoder"];
  assert.deepEqual(movePinnedAgentBy(ids, "codex", -1), ["codex", "claude-code", "qoder"]);
  assert.deepEqual(movePinnedAgentBy(ids, "claude-code", -1), ids);
  assert.deepEqual(movePinnedAgentBefore(ids, "qoder", "claude-code"), ["qoder", "claude-code", "codex"]);
  assert.deepEqual(movePinnedAgentBefore(ids, "codex", "codex"), ids);
});

test("keyboard ordering moves down and preserves the lower boundary", () => {
  const ids = ["claude-code", "codex", "qoder"];
  const snapshot = [...ids];

  assert.deepEqual(movePinnedAgentBy(ids, "codex", 1), ["claude-code", "qoder", "codex"]);
  assert.deepEqual(movePinnedAgentBy(ids, "qoder", 1), ids);
  assert.deepEqual(movePinnedAgentBy(ids, "missing", 1), ids);
  assert.deepEqual(ids, snapshot);
});

test("drag ordering is stable for unknown source and target IDs", () => {
  const ids = ["claude-code", "codex", "qoder"];
  const snapshot = [...ids];

  assert.deepEqual(movePinnedAgentBefore(ids, "missing", "codex"), ids);
  assert.deepEqual(movePinnedAgentBefore(ids, "codex", "missing"), ids);
  assert.deepEqual(ids, snapshot);
});

test("reorder operations preserve original input arrays", () => {
  const keyboardIds = ["claude-code", "codex", "qoder"];
  const dragIds = ["claude-code", "codex", "qoder"];
  const keyboardSnapshot = [...keyboardIds];
  const dragSnapshot = [...dragIds];

  assert.deepEqual(movePinnedAgentBy(keyboardIds, "codex", 1), ["claude-code", "qoder", "codex"]);
  assert.deepEqual(movePinnedAgentBefore(dragIds, "qoder", "claude-code"), ["qoder", "claude-code", "codex"]);
  assert.deepEqual(keyboardIds, keyboardSnapshot);
  assert.deepEqual(dragIds, dragSnapshot);
});
