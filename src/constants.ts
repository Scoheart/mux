import type { AgentDefinition } from "./types.js";
import agentsData from "../data/agents.json" with { type: "json" };

export const MCP_HUB_DIR = "~/.mux";
export const BACKUPS_DIR = "backups";

export const DEFAULT_AGENTS: Record<string, AgentDefinition> =
  agentsData as Record<string, AgentDefinition>;
