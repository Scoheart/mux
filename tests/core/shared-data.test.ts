import { describe, it, expect } from "vitest";
import { BUILTIN_REGISTRY } from "../../src/builtin-registry.js";
import { DEFAULT_AGENTS } from "../../src/constants.js";

describe("shared data", () => {
  it("registry loads from JSON with expected shape", () => {
    expect(Array.isArray(BUILTIN_REGISTRY)).toBe(true);
    expect(BUILTIN_REGISTRY.length).toBeGreaterThanOrEqual(40);
    const first = BUILTIN_REGISTRY.find((e) => e.name === "filesystem");
    expect(first).toBeDefined();
    expect(first!.config.stdio?.command).toBe("npx");
  });

  it("agents load from JSON with expected shape", () => {
    expect(DEFAULT_AGENTS["claude-code"].format).toBe("json");
    expect(DEFAULT_AGENTS["claude-code"].key).toBe("mcpServers");
    expect(DEFAULT_AGENTS["codex"].format).toBe("toml");
    expect(Object.keys(DEFAULT_AGENTS).length).toBeGreaterThanOrEqual(18);
  });
});
