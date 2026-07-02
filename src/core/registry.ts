import type { RegistryEntry } from "../types.js";
import { loadSettings } from "./settings.js";
import {
  MANUAL_ID,
  DISCOVERED_ID,
  sourceEntries,
  writeManualEntry,
  writeDiscoveredEntry,
  removeManualEntry,
} from "./sources.js";

/** Transport bucket of an entry: "stdio" (local process) or "http" (remote,
 *  covers http+sse). An entry carries exactly one transport; stdio wins if both. */
export function transportOf(entry: RegistryEntry): "stdio" | "http" {
  return entry.config.stdio ? "stdio" : "http";
}

/** Composite identity `name::transport`. */
export function keyOf(entry: RegistryEntry): string {
  return `${entry.name}::${transportOf(entry)}`;
}

/** All catalog entries: assembled from every enabled source and deduped by
 *  composite key with precedence external < discovered < manual (the user's own
 *  edits win). No built-in base — the catalog is entirely source-driven, exactly
 *  like the desktop. */
export function readRegistry(): RegistryEntry[] {
  const defs = loadSettings().sources ?? [];
  const byKey = new Map<string, RegistryEntry>();
  for (const def of defs.filter((d) => d.enabled && d.id !== MANUAL_ID && d.id !== DISCOVERED_ID)) {
    for (const e of sourceEntries(def)) byKey.set(keyOf(e), e);
  }
  const disc = defs.find((d) => d.id === DISCOVERED_ID && d.enabled);
  if (disc) for (const e of sourceEntries(disc)) byKey.set(keyOf(e), e);
  const man = defs.find((d) => d.id === MANUAL_ID && d.enabled);
  if (man) for (const e of sourceEntries(man)) byKey.set(keyOf(e), e);
  return [...byKey.values()];
}

/** Create / edit an entry — stored in the managed "manual" local source. */
export function writeRegistryEntry(entry: RegistryEntry): void {
  writeManualEntry(entry);
}

export { writeDiscoveredEntry };

/** Remove a user override from the manual source. Returns true if one went. */
export function removeRegistryEntry(name: string, transport: "stdio" | "http"): boolean {
  return removeManualEntry(name, transport);
}

/** Names of all catalog entries. */
export function listRegistry(): string[] {
  return readRegistry().map((e) => e.name);
}
