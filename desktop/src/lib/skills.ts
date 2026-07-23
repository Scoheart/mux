import type { SkillInventoryItem, SkillSourceResolution } from "./types";

export type SkillStatusFilter =
  | "all"
  | "updates"
  | "needs_attention"
  | "external";
export type SkillSourceFilter = "all" | "github" | "local";

export interface SkillFilters {
  status: SkillStatusFilter;
  source: SkillSourceFilter;
  query: string;
}

const attentionStates = new Set([
  "locally_modified",
  "broken_link",
  "conflicting_link",
  "missing",
]);

/**
 * The inventory keeps target-specific identities for lifecycle operations, while
 * the library presents one user-facing Skill per manifest name.
 */
export function aggregateSkillsByName(
  items: SkillInventoryItem[],
): SkillInventoryItem[] {
  const groups = new Map<string, SkillInventoryItem[]>();
  for (const item of items) {
    const group = groups.get(item.name) ?? [];
    group.push(item);
    groups.set(item.name, group);
  }

  return [...groups.values()].map((group) => {
    const representative =
      group.find((item) => item.location.kind === "central") ??
      [...group].sort((left, right) => left.identity.localeCompare(right.identity))[0];
    const states = new Set(group.flatMap((item) => item.states));
    const contentHashes = new Set(
      group.flatMap((item) => item.content_hash ? [item.content_hash] : []),
    );
    if (contentHashes.size > 1) states.add("conflicting_link");

    return {
      ...representative,
      states: [...states],
      assigned_target_ids: [
        ...new Set(group.flatMap((item) => item.assigned_target_ids)),
      ].sort(),
      affected_agent_ids: [
        ...new Set(group.flatMap((item) => item.affected_agent_ids)),
      ].sort(),
    };
  });
}

export function filterSkills(
  items: SkillInventoryItem[],
  filters: SkillFilters,
): SkillInventoryItem[] {
  const query = filters.query.trim().toLowerCase();
  return items.filter((item) => {
    const statusMatches =
      filters.status === "all" ||
      (filters.status === "updates" && item.update.available) ||
      (filters.status === "external" && item.states.includes("external")) ||
      (filters.status === "needs_attention" &&
        (item.update.available ||
          item.risk?.level === "high" ||
          item.states.some((state) => attentionStates.has(state))));
    const sourceMatches =
      filters.source === "all" ||
      item.source?.kind === filters.source ||
      (filters.source === "local" &&
        (item.source?.kind === "archive" || item.source?.kind === "imported"));
    const queryMatches =
      query.length === 0 ||
      `${item.name} ${item.description}`.toLowerCase().includes(query);
    return statusMatches && sourceMatches && queryMatches;
  });
}

export interface InstallWizardState {
  resolution: SkillSourceResolution | null;
  selectedSkillNames: string[];
  replaceConflicts: boolean;
}

export type InstallWizardAction =
  | { type: "resolution_loaded"; resolution: SkillSourceResolution }
  | { type: "toggle_skill"; skillName: string }
  | { type: "set_replace_conflicts"; enabled: boolean }
  | { type: "reset" };

const initialWizardState: InstallWizardState = {
  resolution: null,
  selectedSkillNames: [],
  replaceConflicts: false,
};

const toggled = (values: string[], value: string) =>
  values.includes(value)
    ? values.filter((entry) => entry !== value)
    : [...values, value];

export function installWizardReducer(
  state: InstallWizardState = initialWizardState,
  action: InstallWizardAction,
): InstallWizardState {
  switch (action.type) {
    case "resolution_loaded":
      return {
        resolution: action.resolution,
        selectedSkillNames: action.resolution.candidates.map(
          (candidate) => candidate.name,
        ),
        replaceConflicts: false,
      };
    case "toggle_skill":
      return {
        ...state,
        selectedSkillNames: toggled(
          state.selectedSkillNames,
          action.skillName,
        ),
      };
    case "set_replace_conflicts":
      return {
        ...state,
        replaceConflicts: action.enabled,
      };
    case "reset":
      return initialWizardState;
  }
}

/**
 * Keeps only a staged result still owned by the current dialog request.
 * Install plans reuse their resolution's operation id, so ordinary selection
 * toggles only invalidate client state; call this for late/discarded results.
 */
export async function resolveStagedResult<T extends { operation_id: string }>(
  pending: Promise<T | null>,
  isCurrent: () => boolean,
  cancel: (operationId: string) => Promise<void>,
): Promise<T | null> {
  const result = await pending;
  if (result === null || isCurrent()) return result;
  try {
    await cancel(result.operation_id);
  } catch {
    // A discarded result must stay discarded even when best-effort cleanup fails.
  }
  return null;
}
