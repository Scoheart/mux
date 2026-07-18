import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { assetOperationPlanFixture } from "../test/consumptionFixtures";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";

afterEach(cleanup);

it("requires explicit bound confirmation before replacing drift", async () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "update-asset";
  plan.requires_conflict_confirmation = true;
  plan.warnings = ["codex: model_owned_fields_drift"];
  const onCommit = vi.fn();
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      onCommit={onCommit}
      onCancel={vi.fn()}
    />,
  );
  const commit = screen.getByRole("button", { name: "确认并同步" });
  expect(commit).toBeDisabled();
  await userEvent.click(screen.getByRole("checkbox"));
  expect(commit).toBeEnabled();
  await userEvent.click(commit);
  expect(onCommit).toHaveBeenCalledWith(plan.candidate_hash);
});

it("shows central lifecycle impact independently from relationship changes", () => {
  const plan = assetOperationPlanFixture();
  plan.kind = "delete-asset";
  plan.central_changes = [{
    asset: { domain: "model", profile_id: "work" },
    action: "delete",
    summary: ["删除 Profile metadata", "级联解除 2 个 consumer"],
  }];
  render(
    <AssetOperationReviewDialog
      plan={plan}
      busy={false}
      onCommit={vi.fn()}
      onCancel={vi.fn()}
    />,
  );
  expect(screen.getByText("中央资产变化")).toBeVisible();
  expect(screen.getByText(/级联解除 2 个 consumer/)).toBeVisible();
  expect(screen.getByRole("button", { name: "确认删除并同步" })).toBeEnabled();
});
