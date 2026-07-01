import type { RegistryEntry } from "./types.js";
import registryData from "../data/registry.json" with { type: "json" };

/**
 * Built-in MCP Server registry — loaded from the shared data/registry.json,
 * which is also consumed by the Tauri Rust core.
 */
export const BUILTIN_REGISTRY: RegistryEntry[] = registryData as RegistryEntry[];
