import pc from "picocolors";
import { readRegistry } from "../core/registry.js";

export function listCommand(): void {
  const entries = readRegistry();

  if (entries.length === 0) {
    console.log(pc.dim("No MCPs registered. Run 'mcp-hub import' to scan existing configs."));
    return;
  }

  console.log(pc.bold(`${entries.length} MCPs in registry:\n`));
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    const tags = entry.tags.length ? pc.dim(` [${entry.tags.join(", ")}]`) : "";
    console.log(`  ${pc.green(entry.name)}${tags}`);
    if (entry.description) console.log(`    ${pc.dim(entry.description)}`);
  }
}
