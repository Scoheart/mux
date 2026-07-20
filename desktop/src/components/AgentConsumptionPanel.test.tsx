import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConsumptionView } from "../lib/types";
import { AgentConsumptionPanel } from "./AgentConsumptionPanel";

afterEach(cleanup);

const syncedRow: ConsumptionView = {
  agent_id: "codex",
  asset: { domain: "mcp", key: "github::stdio" },
  desired: true,
  observed: true,
  enabled: true,
  status: "synced",
  reason: null,
  affected_agent_ids: ["codex"],
};

describe("AgentConsumptionPanel", () => {
  it("uses direct Agent actions and keeps healthy sync state quiet", async () => {
    const onManage = vi.fn();
    const onRemove = vi.fn();
    const onEnabledChange = vi.fn();
    render(
      <AgentConsumptionPanel
        title="MCP"
        description="1 个已添加到 Codex。"
        manageLabel="添加 MCP"
        rows={[syncedRow]}
        external={[]}
        present={() => ({ name: "GitHub", description: "GitHub tools" })}
        onManage={onManage}
        onEnabledChange={onEnabledChange}
        onRemove={onRemove}
        removeLabel={(name) => `从 Codex 移除 ${name}`}
      />,
    );

    expect(screen.getByRole("button", { name: "添加 MCP" })).toBeVisible();
    expect(screen.queryByText("已同步")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "停用 GitHub" }));
    expect(onEnabledChange).toHaveBeenCalledWith(syncedRow, false);
    await userEvent.click(screen.getByRole("button", { name: "从 Codex 移除 GitHub" }));
    expect(onRemove).toHaveBeenCalledWith(syncedRow.asset);
  });

  it("uses asset-specific empty copy", () => {
    render(
      <AgentConsumptionPanel
        title="Skills"
        description="0 个已添加到 Codex。"
        manageLabel="添加 Skill"
        rows={[]}
        external={[]}
        present={() => ({ name: "unused" })}
        onManage={vi.fn()}
        emptyTitle="还没有添加 Skill"
        emptyDescription="从 Skills 资产库选择并添加到 Codex。"
      />,
    );

    expect(screen.getByText("还没有添加 Skill")).toBeVisible();
    expect(screen.getByText("从 Skills 资产库选择并添加到 Codex。")).toBeVisible();
  });

  it("renders a domain-specific row action", async () => {
    const onActivate = vi.fn();
    render(
      <AgentConsumptionPanel
        title="Models"
        manageLabel="添加 Model"
        rows={[{ ...syncedRow, asset: { domain: "model", profile_id: "work" }, active: false }]}
        external={[]}
        present={() => ({ name: "Work" })}
        onManage={vi.fn()}
        renderAction={(item) => (
          <button type="button" onClick={() => onActivate(item)}>设为当前</button>
        )}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "设为当前" }));
    expect(onActivate).toHaveBeenCalledOnce();
  });
});
