import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join, basename } from "node:path";
import type { DiffEntry, AgentsConfig, RegistryEntry, McpConfig } from "../types.js";
import { pickAdapter } from "../adapters/index.js";
import { expandTilde, resolvePath } from "../utils/path.js";

function resolveConfigForMcp(entry: RegistryEntry): McpConfig {
  if (entry.config.stdio) return entry.config.stdio;
  if (entry.config.http) return entry.config.http;
  throw new Error(`No config found for MCP: ${entry.name}`);
}

function getFilePath(
  agentDef: { global: string | null; project: string | null },
  scope: "global" | "project",
  projectDir?: string
): string | null {
  if (scope === "global") {
    return agentDef.global ? expandTilde(agentDef.global) : null;
  }
  if (!agentDef.project || !projectDir) return null;
  return resolvePath(agentDef.project, "project", projectDir);
}

/** Copy a config file into the backups dir with a filename-safe timestamp. */
export function backupFile(filePath: string, backupsDir: string): void {
  if (!existsSync(filePath)) return;
  mkdirSync(backupsDir, { recursive: true });
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  const backupName = `${basename(filePath)}-${timestamp}`;
  copyFileSync(filePath, join(backupsDir, backupName));
}

export function applyDiffs(
  diffs: DiffEntry[],
  agentsConfig: AgentsConfig,
  registry: RegistryEntry[],
  backupsDir: string,
  projectDir?: string
): void {
  const backedUp = new Set<string>();
  const registryMap = new Map(registry.map((r) => [r.name, r]));

  for (const diff of diffs) {
    const agentDef = agentsConfig.agents[diff.agent];
    if (!agentDef) continue;

    const filePath = getFilePath(agentDef, diff.scope, projectDir);
    if (!filePath) continue;

    if (!backedUp.has(filePath) && existsSync(filePath)) {
      backupFile(filePath, backupsDir);
      backedUp.add(filePath);
    }

    const adapter = pickAdapter(agentDef.format, agentDef.key);

    if (diff.action === "add") {
      const entry = registryMap.get(diff.mcpName);
      if (!entry) continue;
      const config = resolveConfigForMcp(entry);
      const current = adapter.read(filePath);
      current[diff.mcpName] = config;
      adapter.write(filePath, current);
    } else if (diff.action === "remove") {
      adapter.remove(filePath, [diff.mcpName]);
    }
  }
}
