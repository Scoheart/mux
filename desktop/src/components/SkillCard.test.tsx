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
    expect(screen.getByText("已分配")).toBeVisible();
    expect(screen.getByText("高风险")).toMatchObject({
      className: "mux-skill-risk-badge",
    });
    expect(screen.getByText("高风险")).toHaveAttribute("data-level", "high");
    expect(screen.getByText("高风险")).not.toHaveAttribute("style");

    fireEvent.keyDown(card, { key: "Enter" });
    fireEvent.keyDown(card, { key: " " });
    expect(onOpen).toHaveBeenCalledTimes(2);
  });

  it("states unknown provenance and risk while retaining update errors and Agent impact", () => {
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
    expect(screen.getByText("未记录 revision")).toBeVisible();
    expect(screen.getByText("未审阅")).toBeVisible();
    expect(screen.getByText(/更新检查失败：GitHub API rate limit/)).toBeVisible();
    expect(screen.getByText(/可重试：2026-07-17T01:02:03Z/)).toBeVisible();
    expect(screen.getByText("3 个 Agent")).toBeVisible();
  });

  it("marks imported provenance explicitly", () => {
    const item = {
      ...skillsInventoryFixture().items[1],
      source: {
        kind: "imported" as const,
        original_path: "~/.cursor/skills/local-copy",
        backup_path: "~/.mux/backups/skills/fixture/local-copy",
      },
    };

    render(<SkillCard item={item} selected={false} onOpen={() => undefined} />);

    expect(screen.getByText("Imported")).toBeVisible();
    expect(screen.getByText("导入副本 · ~/.cursor/skills/local-copy")).toBeVisible();
  });
});
