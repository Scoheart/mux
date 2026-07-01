import type { AgentDefinition } from "./types.js";
import agentsData from "../data/agents.json" with { type: "json" };

export const MCP_HUB_DIR = "~/.mux";
export const REGISTRY_DIR = "registry";
export const AGENTS_FILE = "agents.json";
export const STATE_FILE = "state.json";
export const BACKUPS_DIR = "backups";
export const IMPORTED_MARKER = ".imported";

export const DEFAULT_AGENTS: Record<string, AgentDefinition> =
  agentsData as Record<string, AgentDefinition>;
