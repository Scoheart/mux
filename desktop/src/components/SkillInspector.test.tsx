import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  agentFixture,
  skillDetailFixture,
  skillsInventoryFixture,
} from "../test/skillsFixtures";
import { SkillInspector } from "./SkillInspector";

afterEach(cleanup);

describe("SkillInspector", () => {
  const consumers = ["codex", "cursor", "gemini"].map((agentId) => ({
    agent_id: agentId,
    asset: { domain: "skill" as const, name: "review-changes" },
    desired: true,
    observed: true,
    status: "synced" as const,
    reason: null,
    affected_agent_ids: ["codex", "cursor", "gemini"],
  }));

  it("renders provenance, retained risk evidence, Agent names, and hostile preview text inertly", () => {
    const item = {
      ...skillsInventoryFixture().items[0],
      risk: {
        ...skillsInventoryFixture().items[0].risk!,
        finding_count: 5,
        findings_truncated: true,
      },
    };
    const detail = {
      ...skillDetailFixture(),
      item,
      skill_md: '<script>alert("no")</script><a href="https://evil.invalid">open</a>',
      skill_md_truncated: true,
    };

    render(
      <SkillInspector
        item={item}
        detail={detail}
        agents={agentFixture()}
        consumers={consumers}
        targets={skillsInventoryFixture().targets}
        loading={false}
        error={null}
        onClose={() => undefined}
      />,
    );

    expect(screen.getByLabelText("review-changes 详情")).toBeVisible();
    expect(screen.getByText("GitHub · acme/skills / catalog/review-changes")).toBeVisible();
    expect(screen.getByText(item.resolved_revision!)).toBeVisible();
    expect(screen.getByText("scripts/install.sh:2")).toBeVisible();
    expect(screen.getByText("已显示 1 / 5 条证据")).toBeVisible();
    expect(screen.getByText("Codex、Cursor、Gemini CLI")).toBeVisible();
    expect(screen.getByText("SKILL.md 预览已截断")).toBeVisible();

    const preview = screen.getByLabelText("SKILL.md 纯文本预览");
    expect(preview.tagName).toBe("PRE");
    expect(preview).toHaveTextContent(detail.skill_md);
    expect(preview.querySelector("script")).toBeNull();
    expect(preview.querySelector("a")).toBeNull();
  });

  it("shows explicit loading and error states without inventing preview content", () => {
    const item = skillsInventoryFixture().items[0];
    const { rerender } = render(
      <SkillInspector
        item={item}
        detail={null}
        agents={agentFixture()}
        targets={skillsInventoryFixture().targets}
        loading
        error={null}
        onClose={() => undefined}
      />,
    );

    expect(screen.getByText("正在读取 Skill 详情…")).toBeVisible();
    expect(screen.queryByLabelText("SKILL.md 纯文本预览")).not.toBeInTheDocument();

    rerender(
      <SkillInspector
        item={item}
        detail={null}
        agents={agentFixture()}
        targets={skillsInventoryFixture().targets}
        loading={false}
        error={{
          code: "detail_unavailable",
          message: "detail unavailable",
          retry_at: "2026-07-17T01:02:03Z",
        }}
        onClose={() => undefined}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "读取详情失败：detail unavailable · 可重试：2026-07-17T01:02:03Z",
    );
    expect(screen.queryByLabelText("SKILL.md 纯文本预览")).not.toBeInTheDocument();
  });

  it("uses the consumption projection and delegates relationship changes", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const onManageConsumers = vi.fn();

    render(
      <SkillInspector
        item={inventory.items[0]}
        detail={null}
        agents={inventory.agents}
        consumers={consumers}
        loading={false}
        error={null}
        onClose={() => undefined}
        onPlan={vi.fn()}
        onManageConsumers={onManageConsumers}
      />,
    );

    expect(screen.getByText("Codex、Cursor、Gemini CLI")).toBeVisible();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "管理 Agent" }));
    expect(onManageConsumers).toHaveBeenCalledOnce();
  });

  it("offers an unchecked replacement choice and Update for locally modified content", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const item = {
      ...inventory.items[0],
      states: ["locally_modified" as const],
    };
    const onPlan = vi.fn();

    render(
      <SkillInspector
        item={item}
        detail={null}
        agents={inventory.agents}
        targets={inventory.targets}
        loading={false}
        error={null}
        onClose={() => undefined}
        onPlan={onPlan}
      />,
    );

    const replacement = screen.getByRole("checkbox", {
      name: "保留备份并替换本地更改",
    });
    expect(replacement).not.toBeChecked();
    expect(screen.getByRole("button", { name: "更新" })).toBeVisible();
    await user.click(replacement);
    await user.click(screen.getByRole("button", { name: "更新" }));
    expect(onPlan).toHaveBeenCalledWith({
      kind: "update",
      skillName: "review-changes",
      replaceLocalChanges: true,
    });
  });
});
