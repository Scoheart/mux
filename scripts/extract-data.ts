import { writeFileSync, mkdirSync } from "node:fs";
import { BUILTIN_REGISTRY } from "../src/builtin-registry.js";
import { DEFAULT_AGENTS } from "../src/constants.js";

mkdirSync("data", { recursive: true });
writeFileSync("data/registry.json", JSON.stringify(BUILTIN_REGISTRY, null, 2) + "\n");
writeFileSync("data/agents.json", JSON.stringify(DEFAULT_AGENTS, null, 2) + "\n");
console.log(`registry: ${BUILTIN_REGISTRY.length} servers, agents: ${Object.keys(DEFAULT_AGENTS).length}`);
