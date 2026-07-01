import pc from "picocolors";
import { readAgents } from "../core/agents.js";
import { scanAgents } from "../core/scanner.js";

export function statusCommand(): void {
  const targetsConfig = readAgents();
  const scanned = scanAgents(targetsConfig);

  if (scanned.length === 0) {
    console.log(pc.dim("No MCPs currently active in any target."));
    return;
  }

  const byTarget = new Map<string, typeof scanned>();
  for (const s of scanned) {
    const key = `${s.source.agent} [${s.source.scope}]`;
    if (!byTarget.has(key)) byTarget.set(key, []);
    byTarget.get(key)!.push(s);
  }

  for (const [target, mcps] of byTarget) {
    console.log(pc.bold(`  ${target}:`));
    for (const m of mcps) {
      console.log(`    ${pc.green(m.name)}`);
    }
    console.log("");
  }
}
