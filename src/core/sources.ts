import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { homedir } from "node:os";
import type {
  RegistryEntry,
  RegistryOrigin,
  McpConfig,
  McpHttpConfig,
  SourceDef,
} from "../types.js";
import { JsonAdapter } from "../adapters/json-adapter.js";
import { TomlAdapter } from "../adapters/toml-adapter.js";
import { loadSettings, mutateSettings, type Settings } from "./settings.js";

/**
 * Catalog **sources**, mirroring the desktop's Rust `core/sources.rs`. The
 * catalog is assembled from user-added sources (subscribed remote + local files)
 * plus two *managed* local sources — "手动添加" (manual) and "自动探索"
 * (discovered) — whose servers are stored as files under
 * `~/.mux/sources/local/<id>.json`, NOT in `settings.registry`. Both tools share
 * these files so they never diverge.
 */

export const MANUAL_ID = "manual";
export const DISCOVERED_ID = "discovered";

function muxDir(): string {
  return join(homedir(), ".mux");
}
export function sourcesDir(): string {
  return join(muxDir(), "sources");
}

/** Cached-file path backing a source (`remote/<id>.<ext>` or `local/<id>.<ext>`). */
export function cachedPath(def: SourceDef): string | null {
  const ext = def.format === "toml" ? "toml" : "json";
  const file = `${def.id}.${ext}`;
  if (def.kind === "remote") return join(sourcesDir(), "remote", file);
  if (def.kind === "local") return join(sourcesDir(), "local", file);
  return null;
}

function keyOf(e: RegistryEntry): string {
  return `${e.name}::${e.config.stdio ? "stdio" : "http"}`;
}

function originFor(def: SourceDef): RegistryOrigin {
  return { kind: def.kind, source: def.id };
}

function readSection(format: string, key: string, path: string): Record<string, McpConfig> {
  const adapter = format === "toml" ? new TomlAdapter() : new JsonAdapter(key);
  return adapter.read(path);
}

function entryFrom(name: string, cfg: McpConfig, origin: RegistryOrigin): RegistryEntry {
  const config: RegistryEntry["config"] = {};
  if ("command" in cfg) config.stdio = cfg;
  else config.http = cfg as McpHttpConfig;
  return { name, description: "", tags: [], config, origin };
}

/** Parse a source file into entries. Tries the rich MUX array first (a JSON
 *  `[RegistryEntry, …]`, preserving each entry's own origin — that's how the
 *  managed manual/discovered files keep their provenance), else the standard
 *  `mcpServers` map via the adapter. */
export function parseFile(path: string, format: string, key: string, origin: RegistryOrigin): RegistryEntry[] {
  if (format !== "toml" && existsSync(path)) {
    try {
      const parsed = JSON.parse(readFileSync(path, "utf-8"));
      if (Array.isArray(parsed)) {
        return (parsed as RegistryEntry[]).map((e) => ({ ...e, origin: e.origin ?? origin }));
      }
    } catch {
      // fall through to the mcpServers-map path
    }
  }
  const map = readSection(format, key, path);
  return Object.entries(map).map(([name, cfg]) => entryFrom(name, cfg, origin));
}

/** Entries a source contributes to the catalog (from its cached file). */
export function sourceEntries(def: SourceDef): RegistryEntry[] {
  const path = cachedPath(def);
  if (!path || !existsSync(path)) return [];
  return parseFile(path, def.format, def.key, originFor(def));
}

function nowIso(): string {
  return new Date().toISOString().slice(0, 19);
}

function managedDef(id: string, name: string): SourceDef {
  return { id, kind: "local", name, format: "json", key: "mcpServers", enabled: true, added_at: nowIso() };
}

function ensureManaged(s: Settings, id: string, name: string): void {
  const list = s.sources ?? (s.sources = []);
  if (!list.some((d) => d.id === id)) list.push(managedDef(id, name));
}

function readArray(path: string): RegistryEntry[] {
  if (!existsSync(path)) return [];
  try {
    const a = JSON.parse(readFileSync(path, "utf-8"));
    return Array.isArray(a) ? (a as RegistryEntry[]) : [];
  } catch {
    return [];
  }
}

function writeArray(path: string, list: RegistryEntry[]): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(list, null, 2) + "\n");
}

function writeManaged(id: string, name: string, entry: RegistryEntry): void {
  mutateSettings((s) => ensureManaged(s, id, name));
  const path = cachedPath(managedDef(id, name))!;
  const key = keyOf(entry);
  writeArray(path, [...readArray(path).filter((e) => keyOf(e) !== key), entry]);
}

function removeManaged(id: string, name: string, targetKey: string): boolean {
  const path = cachedPath(managedDef(id, name))!;
  const list = readArray(path);
  const next = list.filter((e) => keyOf(e) !== targetKey);
  const removed = next.length !== list.length;
  if (removed) writeArray(path, next);
  return removed;
}

/** Store a user-created / edited entry into the managed "manual" local source. */
export function writeManualEntry(entry: RegistryEntry): void {
  writeManaged(MANUAL_ID, "手动添加", { ...entry, origin: { kind: "manual", source: MANUAL_ID } });
}

/** Store an auto-discovered entry into the managed "discovered" local source. */
export function writeDiscoveredEntry(entry: RegistryEntry): void {
  writeManaged(DISCOVERED_ID, "自动探索", entry);
}

/** The entries currently in a managed source (origins preserved). */
export function managedEntries(id: string): RegistryEntry[] {
  return sourceEntries(managedDef(id, id));
}

/** Remove a user override (`name`+`transport`) from the manual source. */
export function removeManualEntry(name: string, transport: "stdio" | "http"): boolean {
  return removeManaged(MANUAL_ID, "手动添加", `${name}::${transport}`);
}

/** One-time: fold any legacy `settings.registry` entries into the managed
 *  source files (discovered→discovered, else→manual), then clear the section.
 *  Idempotent — a no-op once `settings.registry` is empty. Mirrors the desktop. */
export function migrateRegistryToSources(): void {
  const reg = loadSettings().registry;
  if (!reg || reg.length === 0) return;
  for (const e of reg) {
    if (e.origin?.kind === "discovered") writeDiscoveredEntry(e);
    else writeManualEntry(e);
  }
  mutateSettings((s) => {
    delete s.registry;
  });
}
