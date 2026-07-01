import { join } from "node:path";
import pc from "picocolors";
import { expandTilde } from "../utils/path.js";
import { MCP_HUB_DIR, BACKUPS_DIR } from "../constants.js";
import { readAgents, getEnabledAgents } from "../core/agents.js";
import { readRegistry } from "../core/registry.js";
import { scanAgents } from "../core/scanner.js";
import { computeDiff } from "../core/differ.js";
import { applyDiffs } from "../core/applier.js";
import { writeState } from "../core/state.js";
import type { ActiveMcp, Scope } from "../types.js";

export function applyCommand(options: {
  names: string[];
  scope: Scope;
  agent: string;
  projectDir?: string;
}): void {
  const hubDir = expandTilde(MCP_HUB_DIR);
  const agentsConfig = readAgents();
  const registry = readRegistry();
  const backupsDir = join(hubDir, BACKUPS_DIR);

  let agentNames: string[];
  if (options.agent === "all") {
    agentNames = Object.keys(getEnabledAgents(agentsConfig));
  } else {
    agentNames = options.agent.split(",").map((t) => t.trim());
  }

  const desired: ActiveMcp[] = options.names.map((name) => ({
    name,
    scope: options.scope,
    agents: agentNames,
    projectPath: options.projectDir,
  }));

  const current = scanAgents(agentsConfig, options.projectDir);
  const diffs = computeDiff(desired, current);

  if (diffs.length === 0) {
    console.log(pc.dim("No changes needed."));
    return;
  }

  applyDiffs(diffs, agentsConfig, registry, backupsDir, options.projectDir);
  writeState({ active: desired });

  const adds = diffs.filter((d) => d.action === "add").length;
  const removes = diffs.filter((d) => d.action === "remove").length;
  console.log(pc.green(`✓ Applied: +${adds} -${removes}`));
}
