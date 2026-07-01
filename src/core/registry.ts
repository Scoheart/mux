import type { RegistryEntry } from "../types.js";
import { BUILTIN_REGISTRY } from "../builtin-registry.js";
import { loadSettings, mutateSettings } from "./settings.js";

/** Transport bucket of an entry: "stdio" (local process) or "http" (remote,
 *  covers http+sse). An entry carries exactly one transport; stdio wins if both. */
export function transportOf(entry: RegistryEntry): "stdio" | "http" {
  return entry.config.stdio ? "stdio" : "http";
}

/** Composite identity `name::transport`. Two entries with the same name but
 *  different transports (e.g. figma stdio vs http) are distinct. */
export function keyOf(entry: RegistryEntry): string {
  return `${entry.name}::${transportOf(entry)}`;
}

/** All entries: builtins merged with the user's `settings.registry`. A user
 *  entry shadows the builtin with the same composite key; user entries dedup
 *  (last wins). */
export function readRegistry(): RegistryEntry[] {
  const userByKey = new Map<string, RegistryEntry>();
  for (const entry of loadSettings().registry ?? []) {
    userByKey.set(keyOf(entry), entry);
  }
  const userKeys = new Set(userByKey.keys());
  const builtinOnly = BUILTIN_REGISTRY.filter((b) => !userKeys.has(keyOf(b)));
  return [...builtinOnly, ...userByKey.values()];
}

/** Insert or replace a user registry entry in `settings.registry`. */
export function writeRegistryEntry(entry: RegistryEntry): void {
  const key = keyOf(entry);
  mutateSettings((s) => {
    const list = s.registry ?? [];
    s.registry = [...list.filter((e) => keyOf(e) !== key), entry];
  });
}

/** Remove the user entry matching `name`+`transport`. Returns true if one went. */
export function removeRegistryEntry(name: string, transport: "stdio" | "http"): boolean {
  const target = `${name}::${transport}`;
  let removed = false;
  mutateSettings((s) => {
    const list = s.registry ?? [];
    const next = list.filter((e) => keyOf(e) !== target);
    removed = next.length !== list.length;
    s.registry = next;
  });
  return removed;
}

/** Names of all user-registered MCP entries. */
export function listRegistry(): string[] {
  return (loadSettings().registry ?? []).map((e) => e.name);
}
