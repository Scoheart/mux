import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { TomlAdapter } from "../../src/adapters/toml-adapter.js";
import { writeFileSync, mkdirSync, rmSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

let testDir: string;
let adapter: TomlAdapter;

beforeEach(() => {
  testDir = join(tmpdir(), `mcp-hub-toml-test-${Date.now()}`);
  mkdirSync(testDir, { recursive: true });
  adapter = new TomlAdapter();
});

afterEach(() => {
  rmSync(testDir, { recursive: true, force: true });
});

describe("TomlAdapter.read", () => {
  it("reads mcp_servers from TOML file", () => {
    const filePath = join(testDir, "config.toml");
    writeFileSync(filePath, `
model = "gpt-5.5"

[mcp_servers.chrome-devtools]
command = "npx"
args = ["-y", "@anthropic-ai/mcp-chrome-devtools@latest"]

[mcp_servers.chrome-devtools.env]
CDP_TARGET_URL = "http://localhost:9222"

[mcp_servers.github]
command = "npx"
args = ["-y", "mcp-github"]
`);

    const result = adapter.read(filePath);
    expect(Object.keys(result)).toEqual(["chrome-devtools", "github"]);
    expect(result["chrome-devtools"]).toEqual({
      command: "npx",
      args: ["-y", "@anthropic-ai/mcp-chrome-devtools@latest"],
      env: { CDP_TARGET_URL: "http://localhost:9222" },
    });
  });

  it("returns empty object if file does not exist", () => {
    const result = adapter.read(join(testDir, "nonexistent.toml"));
    expect(result).toEqual({});
  });

  it("returns empty object if mcp_servers section missing", () => {
    const filePath = join(testDir, "config.toml");
    writeFileSync(filePath, 'model = "gpt-5.5"\n');
    const result = adapter.read(filePath);
    expect(result).toEqual({});
  });
});

describe("TomlAdapter.write", () => {
  it("writes mcp_servers while preserving other config", () => {
    const filePath = join(testDir, "config.toml");
    writeFileSync(filePath, 'model = "gpt-5.5"\n');

    adapter.write(filePath, {
      "everything": { command: "npx", args: ["-y", "@modelcontextprotocol/server-everything"], env: { KEY: "abc" } },
    });

    const content = readFileSync(filePath, "utf-8");
    expect(content).toContain('model = "gpt-5.5"');
    expect(content).toContain("[mcp_servers.everything]");
    expect(content).toContain('command = "npx"');
  });
});

describe("TomlAdapter.remove", () => {
  it("removes specified servers from TOML", () => {
    const filePath = join(testDir, "config.toml");
    writeFileSync(filePath, `
model = "gpt-5.5"

[mcp_servers.chrome-devtools]
command = "npx"
args = ["-y", "mcp-chrome"]

[mcp_servers.github]
command = "npx"
args = ["-y", "mcp-github"]
`);

    adapter.remove(filePath, ["chrome-devtools"]);

    const content = readFileSync(filePath, "utf-8");
    expect(content).not.toContain("chrome-devtools");
    expect(content).toContain("[mcp_servers.github]");
  });
});
