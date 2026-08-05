import type { AssetOperationPlan } from "./types";

/**
 * Keep confirmation dialogs for conflicts and surprising cross-Agent or Model
 * side effects. A direct add/remove click is already explicit intent, so a
 * routine reversible relationship change can proceed with progress feedback.
 */
export function requiresAgentReview(plan: AssetOperationPlan) {
  if (plan.kind === "clear-mcp") return true;
  if (
    !plan.can_commit
    || plan.warnings.length > 0
  ) {
    return true;
  }

  const hasAdd = plan.relationship_changes.some((change) => change.action === "add");
  const hasRemove = plan.relationship_changes.some((change) => change.action === "remove");
  const additiveOnly = hasAdd && !hasRemove;
  if (plan.affected_agent_ids.length > 1 && !additiveOnly) return true;

  return plan.model_state_changes.some((change) =>
    change.before.active
    && !change.after.active
    && (!change.after.enabled || !change.after.added)
  );
}
