import { describe, it, expect } from "vitest";
import { computeDiff } from "../../src/core/differ.js";
import type { ActiveMcp, ScannedMcp } from "../../src/types.js";

describe("computeDiff", () => {
  it("detects additions", () => {
    const desired: ActiveMcp[] = [
      { name: "github", scope: "global", agents: ["claude-code"] },
    ];
    const current: ScannedMcp[] = [];

    const diff = computeDiff(desired, current);
    expect(diff).toHaveLength(1);
    expect(diff[0].action).toBe("add");
    expect(diff[0].mcpName).toBe("github");
    expect(diff[0].agent).toBe("claude-code");
    expect(diff[0].scope).toBe("global");
  });

  it("detects removals", () => {
    const desired: ActiveMcp[] = [];
    const current: ScannedMcp[] = [
      { name: "github", config: { command: "npx", args: [] }, source: { agent: "claude-code", scope: "global", filePath: "/x" } },
    ];

    const diff = computeDiff(desired, current);
    expect(diff).toHaveLength(1);
    expect(diff[0].action).toBe("remove");
    expect(diff[0].mcpName).toBe("github");
  });

  it("detects changes (target added)", () => {
    const desired: ActiveMcp[] = [
      { name: "github", scope: "global", agents: ["claude-code", "codex"] },
    ];
    const current: ScannedMcp[] = [
      { name: "github", config: { command: "npx", args: [] }, source: { agent: "claude-code", scope: "global", filePath: "/x" } },
    ];

    const diff = computeDiff(desired, current);
    const addDiff = diff.filter((d) => d.action === "add");
    expect(addDiff).toHaveLength(1);
    expect(addDiff[0].agent).toBe("codex");
  });

  it("handles 'both' scope by expanding to global and project", () => {
    const desired: ActiveMcp[] = [
      { name: "fetch", scope: "both", agents: ["claude-code"], projectPath: "/myproject" },
    ];
    const current: ScannedMcp[] = [];

    const diff = computeDiff(desired, current);
    expect(diff).toHaveLength(2);
    expect(diff.map((d) => d.scope).sort()).toEqual(["global", "project"]);
  });
});
