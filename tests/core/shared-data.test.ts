import { describe, it, expect } from "vitest";
import registryData from "../../data/registry.json" with { type: "json" };
import { DEFAULT_AGENTS } from "../../src/constants.js";
import type { RegistryEntry } from "../../src/types.js";

const CURATED = registryData as RegistryEntry[];

describe("shared data", () => {
  it("curated collection loads from JSON with expected shape", () => {
    expect(Array.isArray(CURATED)).toBe(true);
    expect(CURATED.length).toBeGreaterThanOrEqual(40);
    const first = CURATED.find((e) => e.name === "filesystem");
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
