import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { scanAgents } from "../../src/core/scanner.js";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { AgentsConfig } from "../../src/types.js";

let testDir: string;

beforeEach(() => {
  testDir = join(tmpdir(), `mcp-hub-scan-test-${Date.now()}`);
  mkdirSync(testDir, { recursive: true });
});

afterEach(() => {
  rmSync(testDir, { recursive: true, force: true });
});

describe("scanAgents", () => {
  it("scans JSON targets and returns found MCPs", () => {
    const claudeFile = join(testDir, "claude.json");
    writeFileSync(claudeFile, JSON.stringify({
      mcpServers: {
        "chrome-devtools": { command: "npx", args: ["-y", "mcp-chrome"] },
        "github": { command: "npx", args: ["-y", "mcp-github"] },
      },
    }));

    const agentsConf: AgentsConfig = {
      agents: {
        "claude-code": {
          global: claudeFile,
          project: null,
          format: "json",
          key: "mcpServers",
          enabled: true,
        },
      },
    };

    const result = scanAgents(agentsConf);
    expect(result).toHaveLength(2);
    expect(result[0].name).toBe("chrome-devtools");
    expect(result[0].source.agent).toBe("claude-code");
    expect(result[0].source.scope).toBe("global");
  });

  it("skips disabled targets", () => {
    const agentsConf: AgentsConfig = {
      agents: {
        "disabled-tool": {
          global: join(testDir, "x.json"),
          project: null,
          format: "json",
          key: "mcpServers",
          enabled: false,
        },
      },
    };

    const result = scanAgents(agentsConf);
    expect(result).toEqual([]);
  });

  it("scans TOML targets", () => {
    const codexFile = join(testDir, "config.toml");
    writeFileSync(codexFile, `
model = "gpt-5.5"

[mcp_servers.fetch]
command = "npx"
args = ["-y", "mcp-server-fetch"]
`);

    const agentsConf: AgentsConfig = {
      agents: {
        codex: {
          global: codexFile,
          project: null,
          format: "toml",
          key: "mcp_servers",
          enabled: true,
        },
      },
    };

    const result = scanAgents(agentsConf);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("fetch");
    expect(result[0].source.agent).toBe("codex");
  });
});
