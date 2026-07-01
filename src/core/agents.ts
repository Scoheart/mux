import type { AgentsConfig, AgentDefinition } from "../types.js";
import { DEFAULT_AGENTS } from "../constants.js";
import { loadSettings, mutateSettings } from "./settings.js";

/** Read the agent map from `settings.agents`; empty/missing ⇒ builtin defaults. */
export function readAgents(): AgentsConfig {
  const map = loadSettings().agents;
  if (!map || Object.keys(map).length === 0) {
    return { agents: { ...DEFAULT_AGENTS } };
  }
  return { agents: map };
}

/** Persist the agent map into `settings.agents` (other sections untouched). */
export function writeAgents(config: AgentsConfig): void {
  mutateSettings((s) => {
    s.agents = config.agents;
  });
}

export function getEnabledAgents(config: AgentsConfig): Record<string, AgentDefinition> {
  const result: Record<string, AgentDefinition> = {};
  for (const [name, def] of Object.entries(config.agents)) {
    if (def.enabled) result[name] = def;
  }
  return result;
}
