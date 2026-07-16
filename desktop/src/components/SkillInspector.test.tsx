import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import {
  agentFixture,
  skillDetailFixture,
  skillsInventoryFixture,
} from "../test/skillsFixtures";
import { SkillInspector } from "./SkillInspector";

afterEach(cleanup);

describe("SkillInspector", () => {
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
});
