import pc from "picocolors";
import { readAgents } from "../core/agents.js";
import { scanAgents } from "../core/scanner.js";
import { readRegistry, writeDiscoveredEntry } from "../core/registry.js";
import { keyOf, configKey } from "../core/key.js";
import type { RegistryEntry, McpConfig, McpStdioConfig, McpHttpConfig } from "../types.js";

function configToRegistryEntry(
  name: string,
  config: McpConfig,
  source: { agent: string; scope: "global" | "project" }
): RegistryEntry {
  const entry: RegistryEntry = {
    name,
    description: "",
    tags: [],
    config: {},
    origin: { kind: "discovered", agent: source.agent, scope: source.scope },
  };
  if ("command" in config) {
    entry.config.stdio = config as McpStdioConfig;
  } else if ("url" in config) {
    entry.config.http = config as McpHttpConfig;
  }
  return entry;
}

export function importCommand(): void {
  const targetsConfig = readAgents();

  console.log(pc.bold("Scanning targets...\n"));
  const scanned = scanAgents(targetsConfig);

  // Dedup discovered servers by composite key (name + transport) so a tool
  // exposed over both stdio and http imports as two distinct entries.
  const configByKey = new Map<string, { name: string; config: McpConfig }>();
  const sourceMap = new Map<string, { agent: string; scope: "global" | "project" }>();

  for (const s of scanned) {
    const k = configKey(s.name, s.config);
    if (!configByKey.has(k)) {
      configByKey.set(k, { name: s.name, config: s.config });
      sourceMap.set(k, { agent: s.source.agent, scope: s.source.scope });
    }
  }

  const existing = new Set(readRegistry().map(keyOf));
  let imported = 0;

  for (const [k, { name, config }] of configByKey) {
    if (existing.has(k)) continue;
    const source = sourceMap.get(k)!;
    const entry = configToRegistryEntry(name, config, source);
    writeDiscoveredEntry(entry);
    existing.add(k);
    imported++;
    console.log(pc.green(`  + ${name}`) + pc.dim(` (from ${source.agent} [${source.scope}])`));
  }

  console.log(pc.bold(`\n${imported} new MCPs imported, ${existing.size} already registered.`));

}
