import type {
  OperationPlan,
  SkillContentKind,
  SkillInventoryItem,
  SkillSourceResolution,
} from "./types";

export type SkillStatusFilter =
  | "all"
  | "updates"
  | "needs_attention"
  | "external";
export type SkillSourceFilter = "all" | "github" | "local";
export type SkillContentFilter = "all" | SkillContentKind;

export interface SkillFilters {
  status: SkillStatusFilter;
  source: SkillSourceFilter;
  contentKind: SkillContentFilter;
  query: string;
}

const attentionStates = new Set([
  "locally_modified",
  "broken_link",
  "conflicting_link",
  "missing",
]);

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
      (filters.source === "local" && item.source?.kind === "imported");
    const contentMatches =
      filters.contentKind === "all" ||
      item.content_kind === filters.contentKind;
    const queryMatches =
      query.length === 0 ||
      `${item.name} ${item.description}`.toLowerCase().includes(query);
    return statusMatches && sourceMatches && contentMatches && queryMatches;
  });
}

export interface InstallWizardState {
  resolution: SkillSourceResolution | null;
  selectedSkillNames: string[];
  selectedAgentIds: string[];
  replaceConflicts: boolean;
  plan: OperationPlan | null;
}

export type InstallWizardAction =
  | { type: "resolution_loaded"; resolution: SkillSourceResolution }
  | { type: "toggle_skill"; skillName: string }
  | { type: "toggle_agent"; agentId: string }
  | { type: "set_replace_conflicts"; enabled: boolean }
  | { type: "plan_loaded"; plan: OperationPlan }
  | { type: "reset" };

const initialWizardState: InstallWizardState = {
  resolution: null,
  selectedSkillNames: [],
  selectedAgentIds: [],
  replaceConflicts: false,
  plan: null,
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
        selectedAgentIds: [],
        replaceConflicts: false,
        plan: null,
      };
    case "toggle_skill":
      return {
        ...state,
        selectedSkillNames: toggled(
          state.selectedSkillNames,
          action.skillName,
        ),
        plan: null,
      };
    case "toggle_agent":
      return {
        ...state,
        selectedAgentIds: toggled(state.selectedAgentIds, action.agentId),
        plan: null,
      };
    case "set_replace_conflicts":
      return {
        ...state,
        replaceConflicts: action.enabled,
        plan: null,
      };
    case "plan_loaded":
      return { ...state, plan: action.plan };
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
