import type { McpConfig, RegistryEntry } from "../types.js";

/** Transport bucket of an entry: "stdio" (local process) or "http" (remote,
 *  covers http+sse). An entry carries exactly one transport; stdio wins if both. */
export function transportOf(entry: RegistryEntry): "stdio" | "http" {
  return entry.config.stdio ? "stdio" : "http";
}

/** Composite identity `name::transport` — same-named stdio and http servers are
 *  distinct catalog items. */
export function keyOf(entry: RegistryEntry): string {
  return `${entry.name}::${transportOf(entry)}`;
}

/** The same key shape for a raw scanned config (`command` ⇒ stdio). */
export function configKey(name: string, config: McpConfig): string {
  return `${name}::${"command" in config ? "stdio" : "http"}`;
}
