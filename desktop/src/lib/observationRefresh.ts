export type ObservationDomain = "mcp" | "model" | "skill" | "central";

export type ObservationTaskId =
  | "agents"
  | "agent-capabilities"
  | "relationships"
  | "skills"
  | "registry"
  | "sources"
  | "external-models";

export interface ObservationChange {
  domains?: ObservationDomain[];
}

export const ALL_OBSERVATION_TASK_IDS: readonly ObservationTaskId[] = [
  "agents",
  "agent-capabilities",
  "relationships",
  "skills",
  "registry",
  "sources",
  "external-models",
];

const DOMAIN_TASKS: Record<Exclude<ObservationDomain, "central">, readonly ObservationTaskId[]> = {
  mcp: ["agents", "agent-capabilities", "relationships"],
  model: ["agents", "agent-capabilities", "relationships", "external-models"],
  skill: ["agents", "agent-capabilities", "relationships", "skills"],
};

export function taskIdsForObservation(change: ObservationChange | null | undefined) {
  const domains = new Set(change?.domains ?? []);
  if (domains.size === 0 || domains.has("central")) {
    return [...ALL_OBSERVATION_TASK_IDS];
  }
  const selected = new Set<ObservationTaskId>();
  for (const domain of ["mcp", "model", "skill"] as const) {
    if (!domains.has(domain)) continue;
    for (const task of DOMAIN_TASKS[domain]) selected.add(task);
  }
  return selected.size > 0
    ? ALL_OBSERVATION_TASK_IDS.filter((task) => selected.has(task))
    : [...ALL_OBSERVATION_TASK_IDS];
}

export function focusRefreshDue(
  lastRefreshAt: number,
  now: number,
  minimumIntervalMs = 30_000,
) {
  return now >= lastRefreshAt + minimumIntervalMs;
}
