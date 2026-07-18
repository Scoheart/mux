import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { skillsStateFixture } from "../test/skillsFixtures";
import { AgentSkillsSection } from "./AgentSkillsSection";

describe("AgentSkillsSection compatibility shell", () => {
  it("exposes no Agent-scoped install or assignment mutation", () => {
    render(<AgentSkillsSection agentId="codex" state={skillsStateFixture()} />);
    expect(screen.getByText("请使用中央 Skills 选择器")).toBeVisible();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.queryByText(/安装 Skill/)).not.toBeInTheDocument();
  });
});
