import type { Adapter } from "./adapter.js";
import { JsonAdapter } from "./json-adapter.js";
import { TomlAdapter } from "./toml-adapter.js";

/** The adapter for an agent's config format. TOML (Codex) hardcodes its
 *  `mcp_servers` section, so `key` only applies to JSON. */
export function pickAdapter(format: string, key: string): Adapter {
  if (format === "toml") return new TomlAdapter();
  return new JsonAdapter(key);
}
