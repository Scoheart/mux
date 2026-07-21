import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { skillsInventoryFixture } from "../test/skillsFixtures";
import { SkillCard } from "./SkillCard";

afterEach(cleanup);

describe("SkillCard", () => {
  it("opens with Enter or Space and exposes the selected state without nested buttons", () => {
    const item = skillsInventoryFixture().items[0];
    const onOpen = vi.fn();

    render(<SkillCard item={item} selected={false} onOpen={onOpen} />);

    const card = screen.getByRole("button", { name: /review-changes/ });
    expect(card).toHaveAttribute("aria-pressed", "false");
    expect(card.querySelector("button")).toBeNull();
    expect(screen.queryByText("Review repository changes")).not.toBeInTheDocument();
    expect(screen.queryByText("使用中")).not.toBeInTheDocument();
    expect(screen.queryByText("高风险")).not.toBeInTheDocument();
    expect(screen.getByText("GitHub · acme/skills / catalog/review-changes")).toBeVisible();
    expect(screen.getByText("rev 0123456789ab")).toBeVisible();
    expect(screen.getByText("有更新")).toBeVisible();

    fireEvent.keyDown(card, { key: "Enter" });
    fireEvent.keyDown(card, { key: " " });
    expect(onOpen).toHaveBeenCalledTimes(2);
  });

  it("keeps unknown provenance concise and leaves risk or update errors to the Inspector", () => {
    const item = {
      ...skillsInventoryFixture().items[0],
      source: null,
      resolved_revision: null,
      risk: null,
      update: {
        ...skillsInventoryFixture().items[0].update,
        available: false,
        error: "GitHub API rate limit",
        retry_at: "2026-07-17T01:02:03Z",
      },
    };

    render(<SkillCard item={item} selected onOpen={() => undefined} />);

    expect(screen.getByText("外部副本 · 来源未知")).toBeVisible();
    expect(screen.getByText("rev —")).toBeVisible();
    expect(screen.queryByText("未审阅")).not.toBeInTheDocument();
    expect(screen.queryByText(/更新检查失败：GitHub API rate limit/)).not.toBeInTheDocument();
    expect(screen.queryByText(/可重试：2026-07-17T01:02:03Z/)).not.toBeInTheDocument();
    expect(screen.queryByText("3 个 Agent")).not.toBeInTheDocument();
  });

  it("shows imported provenance without an extra status badge", () => {
    const item = {
      ...skillsInventoryFixture().items[1],
      source: {
        kind: "imported" as const,
        original_path: "~/.cursor/skills/local-copy",
        backup_path: "~/.mux/backups/skills/fixture/local-copy",
      },
    };

    render(<SkillCard item={item} selected={false} onOpen={() => undefined} />);

    expect(screen.getByText("导入副本 · ~/.cursor/skills/local-copy")).toBeVisible();
    expect(screen.queryByText("Imported")).not.toBeInTheDocument();
  });
});
