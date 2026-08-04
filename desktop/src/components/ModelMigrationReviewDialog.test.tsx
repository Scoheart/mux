import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AssetOperationPlan, MigrationReview } from "../lib/types";
import { ModelMigrationReviewDialog } from "./ModelMigrationReviewDialog";

afterEach(cleanup);

function plan(candidateHash: string): AssetOperationPlan {
  return {
    operation_id: `${candidateHash}-operation`,
    kind: "update-asset",
    domain_plan: { domain: "model", before: {}, after: {} },
    central_changes: [],
    relationship_changes: [],
    model_state_changes: [],
    consumption_state_changes: [],
    target_files: ["~/.claude/settings.json"],
    affected_agent_ids: ["claude-code"],
    warnings: ["claude-code / model:legacy: model_owned_fields_drift"],
    can_commit: true,
    requires_conflict_confirmation: true,
    candidate_hash: candidateHash,
  };
}

function review(): MigrationReview {
  return {
    stage: "model_profile_migration",
    source_schema_version: 1,
    target_schema_version: 2,
    review_hash: "review-hash",
    can_commit: true,
    requires_conflict_confirmation: true,
    blockers: [{
      agent_id: "claude-code",
      agent_name: "Claude Code",
      target_files: ["~/.claude/settings.json"],
      profile_id: "legacy",
      reason: "model_owned_fields_drift",
      message: "safe summary",
      before: { profile_id: "legacy", enabled: true, active: true },
      after: { profile_id: "anthropic-claude", enabled: true, active: true },
      keep_agent_fallback_profile_id: null,
      keep_agent_released_profile_ids: ["legacy"],
      migrates_keychain_reference: true,
      agent_restart_recommended: true,
      mux_owned_field_categories: ["Model identity", "credential reference"],
    }],
    actions: [
      {
        strategy: "use_mux",
        title: "use",
        consequence: "replace",
        modifies_agent_targets: true,
        preserves_agent_targets: false,
        plan: plan("use-hash"),
      },
      {
        strategy: "keep_agent",
        title: "keep",
        consequence: "preserve",
        modifies_agent_targets: false,
        preserves_agent_targets: true,
        plan: plan("keep-hash"),
      },
    ],
    supported_actions: ["use_mux", "keep_agent", "recheck", "later"],
  };
}

describe("ModelMigrationReviewDialog", () => {
  it("shows a sanitized conflict and requires an explicit strategy confirmation", () => {
    const onResolve = vi.fn();
    render(
      <ModelMigrationReviewDialog
        review={review()}
        busy={false}
        error={null}
        onResolve={onResolve}
        onLater={() => undefined}
      />,
    );

    expect(screen.getByText("Claude Code")).toBeVisible();
    expect(screen.getByText("~/.claude/settings.json")).toBeVisible();
    expect(screen.getByText("Agent 中由 MUX 管理的模型字段已与中央配置不同")).toBeVisible();
    const commit = screen.getByRole("button", { name: "确认并继续" });
    expect(commit).toBeDisabled();

    fireEvent.click(screen.getByRole("radio", { name: /以 MUX 配置为准并继续/ }));
    expect(commit).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(commit);
    expect(onResolve).toHaveBeenCalledWith("use_mux", "use-hash");
  });

  it("exposes recheck and later without exposing backend error text", () => {
    const onResolve = vi.fn();
    const onLater = vi.fn();
    render(
      <ModelMigrationReviewDialog
        review={review()}
        busy={false}
        error={{ code: "migration_review_stale", message: "SECRET_INTERNAL_CHAIN" }}
        onResolve={onResolve}
        onLater={onLater}
      />,
    );

    expect(screen.queryByText("SECRET_INTERNAL_CHAIN")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重新检查" }));
    expect(onResolve).toHaveBeenCalledWith("recheck");
    fireEvent.click(screen.getAllByRole("button", { name: "稍后处理" })[1]);
    expect(onLater).toHaveBeenCalled();
  });
});
