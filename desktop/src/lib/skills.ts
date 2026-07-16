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
  const query = filters.query.trim().toLocaleLowerCase();
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
      `${item.name} ${item.description}`.toLocaleLowerCase().includes(query);
    return statusMatches && sourceMatches && contentMatches && queryMatches;
  });
}

export interface InstallWizardState {
  resolution: SkillSourceResolution | null;
  selectedSkillNames: string[];
  selectedAgentIds: string[];
  plan: OperationPlan | null;
}

export type InstallWizardAction =
  | { type: "resolution_loaded"; resolution: SkillSourceResolution }
  | { type: "toggle_skill"; skillName: string }
  | { type: "toggle_agent"; agentId: string }
  | { type: "plan_loaded"; plan: OperationPlan }
  | { type: "reset" };

const initialWizardState: InstallWizardState = {
  resolution: null,
  selectedSkillNames: [],
  selectedAgentIds: [],
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
    case "plan_loaded":
      return { ...state, plan: action.plan };
    case "reset":
      return initialWizardState;
  }
}

/**
 * Keeps only a source resolution still owned by the current dialog request.
 * Install plans reuse this resolution's operation id, so selection changes and
 * install replans must invalidate client state without calling this cleanup.
 */
export async function resolveStagedResult(
  pending: Promise<SkillSourceResolution | null>,
  isCurrent: () => boolean,
  cancel: (operationId: string) => Promise<void>,
): Promise<SkillSourceResolution | null> {
  const result = await pending;
  if (result === null || isCurrent()) return result;
  try {
    await cancel(result.operation_id);
  } catch {
    // A discarded result must stay discarded even when best-effort cleanup fails.
  }
  return null;
}
