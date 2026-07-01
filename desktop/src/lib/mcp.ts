import type { RegistryEntry, InstalledMcp } from "./types";

export type Transport = "stdio" | "http";

/** Transport bucket of an entry: "stdio" (local process) or "http" (remote,
 *  covers http+sse). An entry carries exactly one transport; stdio wins if both. */
export function transportOf(entry: RegistryEntry): Transport {
  return entry.config.stdio ? "stdio" : "http";
}

/** Composite identity `name::transport`. Two entries with the same name but
 *  different transports (e.g. figma stdio vs http) are distinct catalog items. */
export function keyOf(entry: RegistryEntry): string {
  return `${entry.name}::${transportOf(entry)}`;
}

/** Composite key from a (name, transport) pair — for installed rows / requests. */
export function makeKey(name: string, transport: string): string {
  return `${name}::${transport}`;
}

/** Composite key of an installed (scanned) server. */
export function installedKey(item: InstalledMcp): string {
  return `${item.name}::${item.transport}`;
}

/** Precise transport label for display: stdio → "stdio"; http → the http type
 *  ("http" or "sse"). */
export function transportLabel(entry: RegistryEntry): string {
  if (entry.config.stdio) return "stdio";
  return entry.config.http?.type ?? "http";
}
