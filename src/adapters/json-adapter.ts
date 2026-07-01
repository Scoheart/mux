import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import type { McpConfig } from "../types.js";
import type { Adapter } from "./adapter.js";

export class JsonAdapter implements Adapter {
  constructor(private key: string) {}

  read(filePath: string): Record<string, McpConfig> {
    if (!existsSync(filePath)) return {};
    try {
      const content = JSON.parse(readFileSync(filePath, "utf-8"));
      return (content[this.key] as Record<string, McpConfig>) ?? {};
    } catch {
      return {};
    }
  }

  write(filePath: string, mcps: Record<string, McpConfig>): void {
    let content: Record<string, unknown> = {};
    if (existsSync(filePath)) {
      try {
        content = JSON.parse(readFileSync(filePath, "utf-8"));
      } catch {
        content = {};
      }
    } else {
      const dir = dirname(filePath);
      mkdirSync(dir, { recursive: true });
    }
    content[this.key] = mcps;
    writeFileSync(filePath, JSON.stringify(content, null, 2) + "\n");
  }

  remove(filePath: string, names: string[]): void {
    if (!existsSync(filePath)) return;
    const content = JSON.parse(readFileSync(filePath, "utf-8"));
    const servers = content[this.key] as Record<string, McpConfig> | undefined;
    if (!servers) return;
    for (const name of names) {
      delete servers[name];
    }
    content[this.key] = servers;
    writeFileSync(filePath, JSON.stringify(content, null, 2) + "\n");
  }
}
