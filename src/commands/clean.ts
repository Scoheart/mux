import { join } from "node:path";
import pc from "picocolors";
import { expandTilde } from "../utils/path.js";
import { MCP_HUB_DIR, BACKUPS_DIR } from "../constants.js";
import { readAgents } from "../core/agents.js";
import { JsonAdapter } from "../adapters/json-adapter.js";
import { TomlAdapter } from "../adapters/toml-adapter.js";
import { existsSync, copyFileSync, mkdirSync } from "node:fs";
import { basename } from "node:path";

export function cleanCommand(options: { agent?: string }): void {
  const hubDir = expandTilde(MCP_HUB_DIR);
  const agentsConfig = readAgents();
  const backupsDir = join(hubDir, BACKUPS_DIR);

  let cleaned = 0;
  for (const [name, def] of Object.entries(agentsConfig.agents)) {
    if (options.agent && name !== options.agent) continue;
    if (!def.enabled) continue;

    const adapter = def.format === "toml" ? new TomlAdapter() : new JsonAdapter(def.key);

    if (def.global) {
      const filePath = expandTilde(def.global);
      if (existsSync(filePath)) {
        mkdirSync(backupsDir, { recursive: true });
        const ts = new Date().toISOString().replace(/[:.]/g, "-");
        copyFileSync(filePath, join(backupsDir, `${basename(filePath)}-${ts}`));
        adapter.write(filePath, {});
        console.log(pc.green(`  ✓ ${name} [global] cleaned`));
        cleaned++;
      }
    }
  }

  if (cleaned === 0) {
    console.log(pc.dim("Nothing to clean."));
  } else {
    console.log(pc.bold(`\n${cleaned} agent(s) cleaned.`));
  }
}
