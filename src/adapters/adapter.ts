import type { McpConfig } from "../types.js";

export interface Adapter {
  read(filePath: string): Record<string, McpConfig>;
  write(filePath: string, mcps: Record<string, McpConfig>): void;
  remove(filePath: string, names: string[]): void;
}
