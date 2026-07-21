import type { AgentInfo } from "./types";

export const MAX_PINNED_AGENTS = 6;

export interface AgentPickerSections {
  pinned: AgentInfo[];
  available: AgentInfo[];
  searchResults: AgentInfo[] | null;
}

function compareAgents(left: AgentInfo, right: AgentInfo): number {
  return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
}

export function buildAgentPickerSections(
  agents: AgentInfo[],
  pinnedIds: string[],
  query: string,
): AgentPickerSections {
  const configurable = agents.filter(
    (agent) => agent.has_global || Boolean(agent.skills_global_dir?.trim()),
  );
  const byId = new Map(configurable.map((agent) => [agent.id, agent]));
  const seen = new Set<string>();
  const pinned = pinnedIds.flatMap((id) => {
    const match = byId.get(id);
    if (!match || seen.has(id) || seen.size >= MAX_PINNED_AGENTS) return [];
    seen.add(id);
    return [match];
  });
  const available = configurable
    .filter((agent) => !seen.has(agent.id))
    .sort(compareAgents);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return { pinned, available, searchResults: null };
  const searchResults = configurable
    .filter((agent) =>
      [agent.name, agent.id, agent.category]
        .join(" ")
        .toLocaleLowerCase()
        .includes(normalizedQuery),
    )
    .sort(compareAgents);
  return { pinned, available, searchResults };
}

export function togglePinnedAgent(ids: string[], id: string): string[] {
  if (ids.includes(id)) return ids.filter((item) => item !== id);
  if (ids.length >= MAX_PINNED_AGENTS) return [...ids];
  return [...ids, id];
}

export function movePinnedAgentBy(ids: string[], id: string, offset: -1 | 1): string[] {
  const from = ids.indexOf(id);
  const to = from + offset;
  if (from < 0 || to < 0 || to >= ids.length) return [...ids];
  const next = [...ids];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

export function movePinnedAgentBefore(
  ids: string[],
  draggedId: string,
  targetId: string,
): string[] {
  if (draggedId === targetId || !ids.includes(draggedId) || !ids.includes(targetId)) {
    return [...ids];
  }
  const next = ids.filter((id) => id !== draggedId);
  next.splice(next.indexOf(targetId), 0, draggedId);
  return next;
}

export function movePinnedAgentAfter(
  ids: string[],
  draggedId: string,
  targetId: string,
): string[] {
  if (draggedId === targetId || !ids.includes(draggedId) || !ids.includes(targetId)) {
    return [...ids];
  }
  const next = ids.filter((id) => id !== draggedId);
  next.splice(next.indexOf(targetId) + 1, 0, draggedId);
  return next;
}

export type PinnedDropPlacement = "before" | "after";

export function previewPinnedAgentOrder(
  ids: string[],
  draggedId: string,
  targetId: string,
  placement: PinnedDropPlacement,
): string[] {
  return placement === "after"
    ? movePinnedAgentAfter(ids, draggedId, targetId)
    : movePinnedAgentBefore(ids, draggedId, targetId);
}

export function projectedPinnedAgentOffset(
  committedIds: string[],
  projectedIds: string[],
  id: string,
): number {
  const committedIndex = committedIds.indexOf(id);
  const projectedIndex = projectedIds.indexOf(id);
  if (committedIndex < 0 || projectedIndex < 0) return 0;
  return projectedIndex - committedIndex;
}
