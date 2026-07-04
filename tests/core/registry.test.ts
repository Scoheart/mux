import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { readRegistry, writeRegistryEntry } from "../../src/core/registry.js";
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { RegistryEntry } from "../../src/types.js";

// The registry now lives in ~/.mux/settings.json; redirect HOME so each test
// gets an isolated, empty store.
let testHome: string;
let prevHome: string | undefined;

beforeEach(() => {
  testHome = join(tmpdir(), `mcp-hub-reg-test-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  mkdirSync(testHome, { recursive: true });
  prevHome = process.env.HOME;
  process.env.HOME = testHome;
});

afterEach(() => {
  if (prevHome === undefined) delete process.env.HOME;
  else process.env.HOME = prevHome;
  rmSync(testHome, { recursive: true, force: true });
});

describe("writeRegistryEntry", () => {
  it("persists a registry entry", () => {
    const entry: RegistryEntry = {
      name: "chrome-devtools",
      description: "Chrome CDP",
      tags: ["debug"],
      config: { stdio: { command: "npx", args: ["-y", "mcp-chrome"] } },
    };
    writeRegistryEntry(entry);
    const chromeEntry = readRegistry().find((e) => e.name === "chrome-devtools");
    expect(chromeEntry).toBeDefined();
    expect(chromeEntry!.description).toBe("Chrome CDP");
  });
});

describe("readRegistry", () => {
  it("assembles the entries the user has added (no built-in base)", () => {
    const userEntries: RegistryEntry[] = [
      { name: "custom-a", description: "A", tags: [], config: { stdio: { command: "a" } } },
      { name: "custom-b", description: "B", tags: [], config: { http: { type: "http", url: "http://b" } } },
    ];
    for (const e of userEntries) writeRegistryEntry(e);
    const result = readRegistry();
    expect(result.length).toBe(2);
    expect(result.find((r) => r.name === "custom-a")).toBeDefined();
    expect(result.find((r) => r.name === "custom-b")).toBeDefined();
  });

  it("returns an empty catalog when there are no sources", () => {
    expect(readRegistry()).toEqual([]);
  });

  it("a later write with the same name replaces the entry, without duplicates", () => {
    writeRegistryEntry({
      name: "fetch",
      description: "My custom fetch",
      tags: ["custom"],
      config: { stdio: { command: "my-fetch" } },
    });
    const result = readRegistry();
    const fetchEntry = result.find((r) => r.name === "fetch");
    expect(fetchEntry!.description).toBe("My custom fetch");
    expect(result.filter((r) => r.name === "fetch")).toHaveLength(1);
  });

  it("re-writing the same key replaces rather than duplicates", () => {
    writeRegistryEntry({ name: "fetch", description: "v1", tags: [], config: { stdio: { command: "a" } } });
    writeRegistryEntry({ name: "fetch", description: "v2", tags: [], config: { stdio: { command: "b" } } });
    const hits = readRegistry().filter((r) => r.name === "fetch");
    expect(hits).toHaveLength(1);
    expect(hits[0].description).toBe("v2");
  });
});

describe("catalog names", () => {
  it("readRegistry surfaces all user-registered MCPs", () => {
    writeRegistryEntry({ name: "x", description: "", tags: [], config: { stdio: { command: "c" } } });
    writeRegistryEntry({ name: "y", description: "", tags: [], config: { stdio: { command: "c" } } });
    expect(readRegistry().map((e) => e.name).sort()).toEqual(["x", "y"]);
  });
});
