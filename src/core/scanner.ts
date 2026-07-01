import type { AgentsConfig, ScannedMcp } from "../types.js";
import { JsonAdapter } from "../adapters/json-adapter.js";
import { TomlAdapter } from "../adapters/toml-adapter.js";
import type { Adapter } from "../adapters/adapter.js";
import { expandTilde } from "../utils/path.js";
import { resolve } from "node:path";

function getAdapter(format: string, key: string): Adapter {
  if (format === "toml") return new TomlAdapter();
  return new JsonAdapter(key);
}

export function scanAgents(
  config: AgentsConfig,
  projectDir?: string,
  scanAll = false
): ScannedMcp[] {
  const results: ScannedMcp[] = [];

  for (const [agentName, agentDef] of Object.entries(config.agents)) {
    if (!scanAll && !agentDef.enabled) continue;
    const adapter = getAdapter(agentDef.format, agentDef.key);

    if (agentDef.global) {
      const filePath = expandTilde(agentDef.global);
      const mcps = adapter.read(filePath);
      for (const [name, mcpConfig] of Object.entries(mcps)) {
        results.push({
          name,
          config: mcpConfig,
          source: { agent: agentName, scope: "global", filePath },
        });
      }
    }

    if (agentDef.project && projectDir) {
      const filePath = resolve(projectDir, agentDef.project);
      const mcps = adapter.read(filePath);
      for (const [name, mcpConfig] of Object.entries(mcps)) {
        results.push({
          name,
          config: mcpConfig,
          source: { agent: agentName, scope: "project", filePath },
        });
      }
    }
  }

  return results;
}
