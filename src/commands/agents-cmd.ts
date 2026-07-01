import pc from "picocolors";
import { readAgents, writeAgents } from "../core/agents.js";

export function agentsListCommand(): void {
  const config = readAgents();

  console.log(pc.bold("Configured agents:\n"));
  for (const [name, def] of Object.entries(config.agents)) {
    const status = def.enabled ? pc.green("enabled") : pc.dim("disabled");
    console.log(`  ${name} [${status}]`);
    if (def.global) console.log(`    global:  ${pc.dim(def.global)}`);
    if (def.project) console.log(`    project: ${pc.dim(def.project)}`);
  }
}

export function agentsEnableCommand(name: string): void {
  const config = readAgents();

  if (!config.agents[name]) {
    console.log(pc.red(`Agent "${name}" not found.`));
    return;
  }
  config.agents[name].enabled = true;
  writeAgents(config);
  console.log(pc.green(`✓ ${name} enabled`));
}

export function agentsDisableCommand(name: string): void {
  const config = readAgents();

  if (!config.agents[name]) {
    console.log(pc.red(`Agent "${name}" not found.`));
    return;
  }
  config.agents[name].enabled = false;
  writeAgents(config);
  console.log(pc.green(`✓ ${name} disabled`));
}
