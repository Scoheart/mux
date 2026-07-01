import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { parse, stringify } from "smol-toml";
import type { McpConfig } from "../types.js";
import type { Adapter } from "./adapter.js";

const MULTILINE_ARRAY_THRESHOLD = 80;

function formatToml(raw: string): string {
  return raw.replace(/^(\w+)\s*=\s*\[(.+)\]$/gm, (match, key, inner) => {
    if (match.length <= MULTILINE_ARRAY_THRESHOLD) return match;
    const items = parseInlineArray(inner);
    if (items.length <= 1) return match;
    const formatted = items.map((item) => `  ${item.trim()}`).join(",\n");
    return `${key} = [\n${formatted},\n]`;
  });
}

function parseInlineArray(inner: string): string[] {
  const items: string[] = [];
  let current = "";
  let inString = false;
  let stringChar = "";

  for (let i = 0; i < inner.length; i++) {
    const char = inner[i];
    if (inString) {
      current += char;
      if (char === stringChar && inner[i - 1] !== "\\") {
        inString = false;
      }
    } else if (char === '"' || char === "'") {
      inString = true;
      stringChar = char;
      current += char;
    } else if (char === ",") {
      items.push(current.trim());
      current = "";
    } else {
      current += char;
    }
  }
  if (current.trim()) items.push(current.trim());
  return items;
}

export class TomlAdapter implements Adapter {
  read(filePath: string): Record<string, McpConfig> {
    if (!existsSync(filePath)) return {};
    try {
      const content = parse(readFileSync(filePath, "utf-8"));
      const servers = content["mcp_servers"] as Record<string, unknown> | undefined;
      if (!servers) return {};

      const result: Record<string, McpConfig> = {};
      for (const [name, value] of Object.entries(servers)) {
        result[name] = value as McpConfig;
      }
      return result;
    } catch {
      return {};
    }
  }

  write(filePath: string, mcps: Record<string, McpConfig>): void {
    let content: Record<string, unknown> = {};
    if (existsSync(filePath)) {
      try {
        content = parse(readFileSync(filePath, "utf-8")) as Record<string, unknown>;
      } catch {
        content = {};
      }
    } else {
      const dir = dirname(filePath);
      mkdirSync(dir, { recursive: true });
    }
    content["mcp_servers"] = mcps;
    const raw = stringify(content);
    writeFileSync(filePath, formatToml(raw) + "\n");
  }

  remove(filePath: string, names: string[]): void {
    if (!existsSync(filePath)) return;
    const content = parse(readFileSync(filePath, "utf-8")) as Record<string, unknown>;
    const servers = content["mcp_servers"] as Record<string, unknown> | undefined;
    if (!servers) return;
    for (const name of names) {
      delete servers[name];
    }
    content["mcp_servers"] = servers;
    const raw = stringify(content);
    writeFileSync(filePath, formatToml(raw) + "\n");
  }
}
