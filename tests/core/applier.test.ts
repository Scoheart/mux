import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { applyDiffs } from "../../src/core/applier.js";
import { mkdirSync, rmSync, writeFileSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { DiffEntry, AgentsConfig, RegistryEntry } from "../../src/types.js";

let testDir: string;
let backupsDir: string;

beforeEach(() => {
  testDir = join(tmpdir(), `mcp-hub-apply-test-${Date.now()}`);
  backupsDir = join(testDir, "backups");
  mkdirSync(backupsDir, { recursive: true });
});

afterEach(() => {
  rmSync(testDir, { recursive: true, force: true });
});

describe("applyDiffs", () => {
  it("adds an MCP to a JSON target", () => {
    const configFile = join(testDir, "claude.json");
    writeFileSync(configFile, JSON.stringify({ mcpServers: {} }));

    const agentsConf: AgentsConfig = {
      agents: {
        "claude-code": { global: configFile, project: null, format: "json", key: "mcpServers", enabled: true },
      },
    };

    const registry: RegistryEntry[] = [
      { name: "github", description: "GitHub", tags: [], config: { stdio: { command: "npx", args: ["-y", "mcp-github"] } } },
    ];

    const diffs: DiffEntry[] = [
      { action: "add", mcpName: "github", agent: "claude-code", scope: "global" },
    ];

    applyDiffs(diffs, agentsConf, registry, backupsDir);

    const content = JSON.parse(readFileSync(configFile, "utf-8"));
    expect(content.mcpServers.github).toEqual({ command: "npx", args: ["-y", "mcp-github"] });
  });

  it("removes an MCP from a JSON target", () => {
    const configFile = join(testDir, "claude.json");
    writeFileSync(configFile, JSON.stringify({
      mcpServers: { github: { command: "npx", args: [] }, chrome: { command: "npx", args: [] } },
    }));

    const agentsConf: AgentsConfig = {
      agents: {
        "claude-code": { global: configFile, project: null, format: "json", key: "mcpServers", enabled: true },
      },
    };

    const diffs: DiffEntry[] = [
      { action: "remove", mcpName: "github", agent: "claude-code", scope: "global" },
    ];

    applyDiffs(diffs, agentsConf, [], backupsDir);

    const content = JSON.parse(readFileSync(configFile, "utf-8"));
    expect(content.mcpServers.github).toBeUndefined();
    expect(content.mcpServers.chrome).toBeDefined();
  });

  it("creates backup before writing", () => {
    const configFile = join(testDir, "claude.json");
    writeFileSync(configFile, JSON.stringify({ mcpServers: { old: { command: "x" } } }));

    const agentsConf: AgentsConfig = {
      agents: {
        "claude-code": { global: configFile, project: null, format: "json", key: "mcpServers", enabled: true },
      },
    };

    const diffs: DiffEntry[] = [
      { action: "remove", mcpName: "old", agent: "claude-code", scope: "global" },
    ];

    applyDiffs(diffs, agentsConf, [], backupsDir);

    const backupFiles = readdirSync(backupsDir);
    expect(backupFiles.length).toBeGreaterThan(0);
  });
});
