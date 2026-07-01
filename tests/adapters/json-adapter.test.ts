import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { JsonAdapter } from "../../src/adapters/json-adapter.js";
import { writeFileSync, mkdirSync, rmSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

let testDir: string;
let adapter: JsonAdapter;

beforeEach(() => {
  testDir = join(tmpdir(), `mcp-hub-test-${Date.now()}`);
  mkdirSync(testDir, { recursive: true });
  adapter = new JsonAdapter("mcpServers");
});

afterEach(() => {
  rmSync(testDir, { recursive: true, force: true });
});

describe("JsonAdapter.read", () => {
  it("reads mcpServers from existing file", () => {
    const filePath = join(testDir, "settings.json");
    writeFileSync(filePath, JSON.stringify({
      mcpServers: {
        "chrome-devtools": { command: "npx", args: ["-y", "mcp-chrome"] },
        "github": { command: "npx", args: ["-y", "mcp-github"] },
      },
      hooks: { PreToolUse: [] },
    }));

    const result = adapter.read(filePath);
    expect(Object.keys(result)).toEqual(["chrome-devtools", "github"]);
    expect(result["chrome-devtools"]).toEqual({ command: "npx", args: ["-y", "mcp-chrome"] });
  });

  it("returns empty object if file does not exist", () => {
    const result = adapter.read(join(testDir, "nonexistent.json"));
    expect(result).toEqual({});
  });

  it("returns empty object if key is missing", () => {
    const filePath = join(testDir, "empty.json");
    writeFileSync(filePath, JSON.stringify({ hooks: {} }));
    const result = adapter.read(filePath);
    expect(result).toEqual({});
  });
});

describe("JsonAdapter.write", () => {
  it("writes mcpServers while preserving other keys", () => {
    const filePath = join(testDir, "settings.json");
    writeFileSync(filePath, JSON.stringify({ hooks: { PreToolUse: [] }, mcpServers: {} }, null, 2));

    adapter.write(filePath, {
      "deepwiki": { type: "http", url: "https://mcp.deepwiki.com/sse" } as any,
    });

    const content = JSON.parse(readFileSync(filePath, "utf-8"));
    expect(content.hooks).toEqual({ PreToolUse: [] });
    expect(content.mcpServers["deepwiki"]).toEqual({ type: "http", url: "https://mcp.deepwiki.com/sse" });
  });

  it("creates file with minimal structure if it does not exist", () => {
    const filePath = join(testDir, "new.json");
    adapter.write(filePath, { "github": { command: "npx", args: [] } });

    const content = JSON.parse(readFileSync(filePath, "utf-8"));
    expect(content.mcpServers.github).toEqual({ command: "npx", args: [] });
  });
});

describe("JsonAdapter.remove", () => {
  it("removes specified entries from mcpServers", () => {
    const filePath = join(testDir, "settings.json");
    writeFileSync(filePath, JSON.stringify({
      mcpServers: {
        "chrome-devtools": { command: "npx", args: [] },
        "github": { command: "npx", args: [] },
        "fetch": { type: "http", url: "https://example.com" },
      },
    }));

    adapter.remove(filePath, ["chrome-devtools", "fetch"]);

    const content = JSON.parse(readFileSync(filePath, "utf-8"));
    expect(Object.keys(content.mcpServers)).toEqual(["github"]);
  });
});

describe("JsonAdapter with different keys", () => {
  it("works with 'servers' key for VS Code", () => {
    const vsAdapter = new JsonAdapter("servers");
    const filePath = join(testDir, "mcp.json");
    writeFileSync(filePath, JSON.stringify({ servers: { "test": { command: "node" } } }));

    const result = vsAdapter.read(filePath);
    expect(result["test"]).toEqual({ command: "node" });
  });

  it("works with 'context_servers' key for Zed", () => {
    const zedAdapter = new JsonAdapter("context_servers");
    const filePath = join(testDir, "settings.json");

    zedAdapter.write(filePath, { "test": { command: "node", args: [] } });

    const content = JSON.parse(readFileSync(filePath, "utf-8"));
    expect(content.context_servers.test).toEqual({ command: "node", args: [] });
  });
});
