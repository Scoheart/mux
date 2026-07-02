import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { existsSync, mkdirSync, rmSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { readRegistry, writeRegistryEntry, writeDiscoveredEntry } from "../../src/core/registry.js";
import { migrateRegistryToSources } from "../../src/core/sources.js";
import { saveSettings, loadSettings } from "../../src/core/settings.js";
import type { RegistryEntry } from "../../src/types.js";

let testHome: string;
let prevHome: string | undefined;

beforeEach(() => {
  testHome = join(tmpdir(), `mux-sources-test-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  mkdirSync(testHome, { recursive: true });
  prevHome = process.env.HOME;
  process.env.HOME = testHome;
});

afterEach(() => {
  if (prevHome === undefined) delete process.env.HOME;
  else process.env.HOME = prevHome;
  rmSync(testHome, { recursive: true, force: true });
});

const localFile = (name: string) => join(testHome, ".mux", "sources", "local", name);

describe("source-based storage (CLI/desktop parity)", () => {
  it("manual entries are stored in sources/local/manual.json, not settings.registry", () => {
    writeRegistryEntry({ name: "z", description: "", tags: [], config: { stdio: { command: "c" } } });

    expect(existsSync(localFile("manual.json"))).toBe(true);
    const arr = JSON.parse(readFileSync(localFile("manual.json"), "utf-8")) as RegistryEntry[];
    expect(arr.find((e) => e.name === "z")?.origin?.kind).toBe("manual");

    // Not in settings.registry.
    expect(loadSettings().registry).toBeUndefined();

    // And it shows in the catalog, tagged manual.
    const cat = readRegistry();
    expect(cat.find((e) => e.name === "z")?.origin?.kind).toBe("manual");
  });

  it("discovered entries land in sources/local/discovered.json with their origin", () => {
    writeDiscoveredEntry({
      name: "d",
      description: "",
      tags: [],
      config: { stdio: { command: "c" } },
      origin: { kind: "discovered", agent: "claude-code", scope: "global" },
    });
    expect(existsSync(localFile("discovered.json"))).toBe(true);
    const e = readRegistry().find((x) => x.name === "d");
    expect(e?.origin?.kind).toBe("discovered");
    expect(e?.origin?.agent).toBe("claude-code");
  });

  it("migrateRegistryToSources folds legacy settings.registry into source files", () => {
    saveSettings({
      registry: [
        { name: "m", description: "", tags: [], config: { stdio: { command: "c" } }, origin: { kind: "manual" } },
        { name: "d", description: "", tags: [], config: { stdio: { command: "c" } }, origin: { kind: "discovered", agent: "cursor", scope: "global" } },
      ],
    });
    migrateRegistryToSources();

    expect(loadSettings().registry).toBeUndefined();
    expect(existsSync(localFile("manual.json"))).toBe(true);
    expect(existsSync(localFile("discovered.json"))).toBe(true);
    const names = readRegistry().map((e) => e.name).sort();
    expect(names).toEqual(["d", "m"]);
  });
});
