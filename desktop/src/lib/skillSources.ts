import type { SkillInventoryItem, SkillsInventory } from "./types";

export type SkillSourceCategory = "github" | "local" | "archive" | "imported" | "unknown";
export interface SkillSourceGroup {
  id: string;
  category: SkillSourceCategory;
  label: string;
  path: string;
  count: number;
}

const cleanPath = (path: string) => path.replace(/\/+$/, "") || "/";
const basename = (path: string) => cleanPath(path).split("/").at(-1) || path;

export function skillSourceGroup(item: SkillInventoryItem): Omit<SkillSourceGroup, "count"> {
  const source = item.source;
  if (!source) return { id: "unknown", category: "unknown", label: "", path: "" };
  if (source.kind === "github") {
    const repo = `${source.owner}/${source.repo}`.toLowerCase();
    return { id: `github:${repo}`, category: "github", label: repo, path: repo };
  }
  const path = source.kind === "imported"
    ? cleanPath(source.original_path).replace(/\/[^/]+$/, "")
    : cleanPath(source.path);
  return {
    id: `${source.kind}:${path}`,
    category: source.kind,
    label: basename(path),
    path,
  };
}

export function groupSkillSources(items: SkillInventoryItem[]): SkillSourceGroup[] {
  const groups = new Map<string, SkillSourceGroup>();
  for (const item of items) {
    const source = skillSourceGroup(item);
    const existing = groups.get(source.id);
    if (existing) existing.count += 1;
    else groups.set(source.id, { ...source, count: 1 });
  }
  const values = [...groups.values()];
  // Identical folder names remain distinct and must be distinguishable in navigation.
  for (const group of values) {
    if (values.some((other) => other.id !== group.id
      && other.category === group.category && basename(other.path) === basename(group.path))) {
      group.label = group.path;
    }
  }
  return values.sort((a, b) => a.label.localeCompare(b.label));
}

/** Observed readable targets, not central desired assignments or missing links. */
export function skillConsumerAgents(inventory: SkillsInventory | null): Map<string, string[]> {
  const result = new Map<string, Set<string>>();
  const installed = new Set(inventory?.agents.map((agent) => agent.id));
  for (const item of inventory?.items ?? []) {
    if (item.location.kind !== "agent_target") continue;
    if (item.states.some((state) => ["missing", "broken_link", "conflicting_link"].includes(state))) continue;
    if (!item.states.includes("assigned")
      && !(item.states.includes("external") && item.content_hash !== null)) continue;
    const agents = result.get(item.name) ?? new Set<string>();
    for (const id of item.affected_agent_ids) if (installed.has(id)) agents.add(id);
    result.set(item.name, agents);
  }
  return new Map([...result].map(([name, ids]) => [name, [...ids].sort()]));
}
