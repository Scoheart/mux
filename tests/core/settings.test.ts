import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { existsSync, mkdirSync, rmSync, writeFileSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadSettings, saveSettings, mutateSettings, migrateIfNeeded } from "../../src/core/settings.js";
import { writeRegistryEntry } from "../../src/core/registry.js";

let testHome: string;
let prevHome: string | undefined;

beforeEach(() => {
  testHome = join(tmpdir(), `mux-settings-test-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  mkdirSync(testHome, { recursive: true });
  prevHome = process.env.HOME;
  process.env.HOME = testHome;
});

afterEach(() => {
  if (prevHome === undefined) delete process.env.HOME;
  else process.env.HOME = prevHome;
  rmSync(testHome, { recursive: true, force: true });
});

const muxDir = () => join(testHome, ".mux");
const readSettingsFile = () => JSON.parse(readFileSync(join(muxDir(), "settings.json"), "utf-8"));

describe("settings round-trip", () => {
  it("save then load preserves data and stamps version", () => {
    saveSettings({ registry: [], imported: "2026-01-02T03:04:05" });
    const back = loadSettings();
    expect(back.imported).toBe("2026-01-02T03:04:05");
    expect(back.version).toBe(1);
  });

  it("missing file loads as empty defaults", () => {
    expect(loadSettings()).toEqual({});
  });
});

describe("cross-tool passthrough", () => {
  it("a desktop-owned `disabled` section survives a CLI registry write", () => {
    // Simulate the desktop having written a disabled section.
    saveSettings({ disabled: { "claude-code": [{ name: "figma", transport: "http", scope: "global", config: { type: "http", url: "x" } }] } } as never);
    // CLI adds a manual entry (now stored in a managed local source, whose
    // registration lives in settings.sources — not settings.registry).
    writeRegistryEntry({ name: "z", description: "", tags: [], config: { stdio: { command: "c" } } });
    const raw = readSettingsFile();
    expect(raw.disabled["claude-code"]).toHaveLength(1);
    expect(raw.sources.find((s: { id: string }) => s.id === "manual")).toBeDefined();
  });

  it("unknown future keys round-trip untouched", () => {
    mutateSettings((s) => {
      (s as Record<string, unknown>).futureThing = { a: 1 };
    });
    mutateSettings((s) => {
      s.imported = "now";
    });
    const raw = readSettingsFile();
    expect(raw.futureThing).toEqual({ a: 1 });
    expect(raw.imported).toBe("now");
  });
});

describe("migrateIfNeeded", () => {
  it("folds legacy files into settings.json and archives them", () => {
    const dir = muxDir();
    mkdirSync(join(dir, "registry"), { recursive: true });
    writeFileSync(
      join(dir, "registry", "custom__stdio.json"),
      JSON.stringify({ name: "custom", description: "", tags: [], config: { stdio: { command: "c" } } })
    );
    writeFileSync(join(dir, "agents.json"), JSON.stringify({ "my-agent": { global: "~/x", project: null, format: "json", key: "mcpServers", enabled: true } }));
    writeFileSync(join(dir, "state.json"), JSON.stringify({ active: [{ name: "git", scope: "global", agents: ["claude-code"] }] }));
    writeFileSync(join(dir, ".imported"), "2026-01-01T00:00:00");

    migrateIfNeeded();

    const s = loadSettings();
    expect(s.registry?.find((e) => e.name === "custom")).toBeDefined();
    expect(s.agents?.["my-agent"]).toBeDefined();
    expect((s.state as { active: unknown[] }).active).toHaveLength(1);
    expect(s.imported).toBe("2026-01-01T00:00:00");

    // Legacy files moved aside into backups/legacy-*, not left at top level.
    expect(existsSync(join(dir, "agents.json"))).toBe(false);
    expect(existsSync(join(dir, "registry"))).toBe(false);
    const legacy = readdirSync(join(dir, "backups")).filter((d) => d.startsWith("legacy-"));
    expect(legacy.length).toBe(1);
  });

  it("is a no-op when settings.json already exists", () => {
    saveSettings({ imported: "keep" });
    const dir = muxDir();
    writeFileSync(join(dir, "agents.json"), JSON.stringify({ "x": {} }));
    migrateIfNeeded();
    // agents.json untouched (not archived), settings unchanged.
    expect(existsSync(join(dir, "agents.json"))).toBe(true);
    expect(loadSettings().imported).toBe("keep");
  });
});
