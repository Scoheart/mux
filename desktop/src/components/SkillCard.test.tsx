import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { skillsInventoryFixture } from "../test/skillsFixtures";
import { SkillCard } from "./SkillCard";

afterEach(cleanup);

describe("SkillCard", () => {
  it("opens one compact index row once per native keyboard or pointer activation", async () => {
    const item = skillsInventoryFixture().items[0];
    const onOpen = vi.fn();
    const user = userEvent.setup();

    render(<SkillCard item={item} selected={false} onOpen={onOpen} />);

    const row = screen.getByRole("button", { name: /review-changes/ });
    expect(row).toHaveAttribute("aria-pressed", "false");
    expect(row).toHaveClass("mux-asset-list-row", "mux-skill-list-row");
    expect(row.querySelector("button")).toBeNull();
    expect(screen.getByText("Review repository changes")).toHaveClass("mux-skill-list-description");
    expect(screen.getByText("GitHub · acme/skills / catalog/review-changes")).toBeVisible();
    expect(screen.getByText("rev 0123456789")).toBeVisible();
    expect(screen.getByText("高风险")).toBeVisible();
    expect(screen.getByText("有更新")).toBeVisible();
    expect(screen.getByText("需处理")).toBeVisible();

    row.focus();
    await user.keyboard("{Enter}");
    expect(onOpen).toHaveBeenCalledTimes(1);

    await user.keyboard(" ");
    expect(onOpen).toHaveBeenCalledTimes(2);

    await user.click(row);
    expect(onOpen).toHaveBeenCalledTimes(3);
  });

  it("keeps unknown provenance concise and leaves update error detail to the Inspector", () => {
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
    expect(screen.getByText("尚未检查")).toBeVisible();
    expect(screen.getByText("检查失败")).toBeVisible();
    expect(screen.queryByText(/GitHub API rate limit/)).not.toBeInTheDocument();
    expect(screen.queryByText(/可重试：2026-07-17T01:02:03Z/)).not.toBeInTheDocument();
    expect(screen.queryByText("3 个 Agent")).not.toBeInTheDocument();
  });

  it("shows imported provenance without changing the lifecycle controls", () => {
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
    expect(screen.getByText("正常")).toBeVisible();
    expect(screen.queryByText("Imported")).not.toBeInTheDocument();
  });
});
