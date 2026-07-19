import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AssetConsumerDialog } from "./AssetConsumerDialog";

afterEach(cleanup);

describe("AssetConsumerDialog", () => {
  const consumers = [{
    agent_id: "codex",
    asset: { domain: "skill" as const, name: "review-changes" },
    desired: true,
    observed: true,
    status: "synced" as const,
    reason: null,
    affected_agent_ids: ["codex", "cursor"],
  }];
  const options = [
    { id: "codex", name: "Codex", description: "~/.agents/skills" },
    { id: "claude-code", name: "Claude Code", description: "~/.claude/skills" },
    { id: "guided", name: "Guided", disabled: true, reason: "只支持官方引导" },
  ];

  it("submits a complete desired Agent set without mutating the source rows", async () => {
    const onReview = vi.fn().mockResolvedValue(undefined);
    render(
      <AssetConsumerDialog
        asset={{ domain: "skill", name: "review-changes" }}
        assetName="review-changes"
        options={options}
        consumers={consumers}
        onReview={onReview}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: /Codex/ })).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(screen.getByRole("button", { name: /Claude Code/ }));
    await userEvent.click(screen.getByRole("button", { name: "审阅变更" }));
    expect(onReview).toHaveBeenCalledWith(["claude-code", "codex"]);
    expect(consumers).toHaveLength(1);
  });

  it("shows incompatibility as a disabled option", () => {
    render(
      <AssetConsumerDialog
        asset={{ domain: "model", profile_id: "gateway" }}
        assetName="Gateway"
        options={options}
        consumers={[]}
        onReview={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: /Guided/ })).toBeDisabled();
    expect(screen.getByText("只支持官方引导")).toBeVisible();
  });

  it("selects every Agent sharing one physical Skill target", async () => {
    const onReview = vi.fn().mockResolvedValue(undefined);
    render(
      <AssetConsumerDialog
        asset={{ domain: "skill", name: "review-changes" }}
        assetName="review-changes"
        options={[{
          id: "codex",
          name: "Codex",
          affectedAgentIds: ["codex", "cursor", "gemini"],
        }]}
        consumers={[]}
        onReview={onReview}
        onClose={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: /Codex/ }));
    expect(screen.getByText("共用 · 3")).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "审阅变更" }));
    expect(onReview).toHaveBeenCalledWith(["codex", "cursor", "gemini"]);
  });

  it("renders one option for Agents using the same physical target", () => {
    render(
      <AssetConsumerDialog
        asset={{ domain: "skill", name: "review-changes" }}
        assetName="review-changes"
        options={[
          {
            id: "codex",
            name: "Codex",
            targetId: "agents-user",
            affectedAgentIds: ["codex", "cursor"],
          },
          {
            id: "cursor",
            name: "Cursor",
            targetId: "agents-user",
            affectedAgentIds: ["codex", "cursor"],
          },
        ]}
        consumers={[]}
        onReview={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getAllByRole("button", { name: /Codex、Cursor/ })).toHaveLength(1);
  });
});
