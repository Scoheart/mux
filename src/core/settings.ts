import {
  readFileSync,
  writeFileSync,
  existsSync,
  mkdirSync,
  renameSync,
  readdirSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { homedir } from "node:os";
import type { RegistryEntry, AgentDefinition, StateConfig } from "../types.js";

/**
 * Single consolidated user-data file: `~/.mux/settings.json`.
 *
 * MUX's CLI and the desktop app share `~/.mux/`. Historically that meant a
 * sprawl of files (`registry/<name>__<transport>.json`, `agents.json`,
 * `state.json`, a `.imported` marker). This module collapses all of it into one
 * `settings.json`.
 *
 * Cross-tool rule: the CLI fully types the sections it owns
 * (`agents`/`registry`/`state`/`imported`) and carries the desktop-owned
 * `disabled` section — plus any unknown future keys — through opaquely, so a CLI
 * write never clobbers what the desktop wrote, and vice versa. Every mutation is
 * read-whole → modify one section → write-whole (atomically).
 */
export interface Settings {
  version?: number;
  /** Agent definitions map (matches the desktop's `agents` section shape). */
  agents?: Record<string, AgentDefinition>;
  /** User/custom/override registry entries (merged over builtins on read). */
  registry?: RegistryEntry[];
  /** Last applied state. */
  state?: StateConfig;
  /** First-scan import marker (ISO timestamp). */
  imported?: string;
  /** Desktop-owned disable snapshots — opaque to the CLI, carried through. */
  disabled?: unknown;
  /** Desktop-owned catalog sources (subscribed remote + local files) — opaque to
   *  the CLI (it doesn't browse subscribed servers yet), carried through. */
  sources?: unknown;
  /** Forward-compat: unknown top-level keys round-trip untouched. */
  [key: string]: unknown;
}

/** `~/.mux` — computed lazily so `$HOME` can be redirected (e.g. in tests). */
function muxDir(): string {
  return join(homedir(), ".mux");
}

export function settingsPath(): string {
  return join(muxDir(), "settings.json");
}

/** Read the whole settings document. Missing/corrupt ⇒ empty (defaults). */
export function loadSettings(): Settings {
  const path = settingsPath();
  if (!existsSync(path)) return {};
  try {
    return JSON.parse(readFileSync(path, "utf-8")) as Settings;
  } catch {
    return {};
  }
}

/** Atomic write (temp sibling + rename) so a crash can't leave a torn file. */
function writeAtomic(path: string, data: string): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp`;
  writeFileSync(tmp, data);
  renameSync(tmp, path);
}

/** Persist the whole settings document (pretty JSON, atomic). Stamps version. */
export function saveSettings(settings: Settings): void {
  if (settings.version == null) settings.version = 1;
  writeAtomic(settingsPath(), JSON.stringify(settings, null, 2) + "\n");
}

/** Load → apply `fn` to one section → save. */
export function mutateSettings(fn: (s: Settings) => void): void {
  const settings = loadSettings();
  fn(settings);
  saveSettings(settings);
}

/** Composite key `name::transport` (inlined to avoid a cycle with registry.ts). */
function keyOf(e: RegistryEntry): string {
  return `${e.name}::${e.config.stdio ? "stdio" : "http"}`;
}

/**
 * One-time migration: if `settings.json` is absent but legacy files exist, fold
 * them into a fresh `settings.json`, then move the old files aside into
 * `~/.mux/backups/legacy-<ts>/` (reversible). Idempotent once settings exists.
 */
export function migrateIfNeeded(): void {
  if (existsSync(settingsPath())) return;

  const dir = muxDir();
  const regDir = join(dir, "registry");
  const agentsPath = join(dir, "agents.json");
  const disabledPath = join(dir, "disabled.json");
  const statePath = join(dir, "state.json");
  const importedPath = join(dir, ".imported");

  const anyLegacy =
    existsSync(regDir) ||
    existsSync(agentsPath) ||
    existsSync(disabledPath) ||
    existsSync(statePath) ||
    existsSync(importedPath);
  if (!anyLegacy) return;

  const s: Settings = { version: 1 };

  // registry/*.json → registry[]  (dedup by composite key, last file wins)
  if (existsSync(regDir)) {
    const byKey = new Map<string, RegistryEntry>();
    for (const f of readdirSync(regDir).filter((f) => f.endsWith(".json"))) {
      try {
        const e = JSON.parse(readFileSync(join(regDir, f), "utf-8")) as RegistryEntry;
        byKey.set(keyOf(e), e);
      } catch {
        // ignore malformed files
      }
    }
    if (byKey.size) s.registry = [...byKey.values()];
  }
  // agents.json → agents  (tolerate {agents}/{targets} wrappers or a bare map)
  if (existsSync(agentsPath)) {
    try {
      const c = JSON.parse(readFileSync(agentsPath, "utf-8"));
      const map = c.agents ?? c.targets ?? c;
      if (map && typeof map === "object") s.agents = map;
    } catch {
      // ignore
    }
  }
  // disabled.json → disabled (opaque passthrough)
  if (existsSync(disabledPath)) {
    try {
      s.disabled = JSON.parse(readFileSync(disabledPath, "utf-8"));
    } catch {
      // ignore
    }
  }
  // state.json → state
  if (existsSync(statePath)) {
    try {
      s.state = JSON.parse(readFileSync(statePath, "utf-8")) as StateConfig;
    } catch {
      // ignore
    }
  }
  // .imported → imported
  if (existsSync(importedPath)) {
    try {
      const t = readFileSync(importedPath, "utf-8").trim();
      if (t) s.imported = t;
    } catch {
      // ignore
    }
  }

  saveSettings(s);

  // Archive the legacy files aside only after the new file is safely written.
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const legacyDir = join(muxDir(), "backups", `legacy-${stamp}`);
  mkdirSync(legacyDir, { recursive: true });
  const moves: Array<[string, string]> = [
    [regDir, "registry"],
    [agentsPath, "agents.json"],
    [disabledPath, "disabled.json"],
    [statePath, "state.json"],
    [importedPath, ".imported"],
  ];
  for (const [from, name] of moves) {
    if (existsSync(from)) {
      try {
        renameSync(from, join(legacyDir, name));
      } catch {
        // best-effort
      }
    }
  }
}
