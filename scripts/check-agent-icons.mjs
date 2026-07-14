import { readFileSync, readdirSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const assetDir = resolve(root, "desktop/src/assets/agents");
const agents = JSON.parse(readFileSync(resolve(root, "data/agents.json"), "utf8"));
const aliases = JSON.parse(readFileSync(resolve(assetDir, "aliases.json"), "utf8"));
const extensions = new Set([".png", ".svg", ".webp"]);
const assets = new Set(
  readdirSync(assetDir)
    .filter((file) => extensions.has(extname(file)))
    .map((file) => file.slice(0, -extname(file).length))
);

const missingAliasTargets = Object.entries(aliases)
  .filter(([, target]) => !assets.has(target))
  .map(([id, target]) => `${id} -> ${target}`);
const configurable = Object.entries(agents).filter(([, agent]) => agent.enabled && agent.global);
const missingIcons = configurable
  .filter(([id]) => !assets.has(aliases[id] ?? id))
  .map(([id, agent]) => `${id} (${agent.name})`);

if (missingAliasTargets.length || missingIcons.length) {
  if (missingAliasTargets.length) {
    console.error(`Icon aliases reference missing assets:\n- ${missingAliasTargets.join("\n- ")}`);
  }
  if (missingIcons.length) {
    console.error(`Configurable agents without icons:\n- ${missingIcons.join("\n- ")}`);
  }
  process.exit(1);
}

console.log(`Verified icons for all ${configurable.length} configurable agents.`);
