import { cleanup, render, screen, within } from "@testing-library/react";
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

const externalRow: ConsumptionView = {
  ...syncedRow,
  asset: { domain: "mcp", key: "computer-use::stdio" },
  desired: false,
  status: "external",
  reason: "mcp_adoptable",
};

describe("AgentConsumptionPanel", () => {
  it("uses direct Agent actions and keeps healthy sync state quiet", async () => {
    const onManage = vi.fn();
    const onRemove = vi.fn();
    const onEnabledChange = vi.fn();
    render(
      <AgentConsumptionPanel
        domain="mcp"
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
        columns={3}
      />,
    );

    expect(screen.getByRole("list")).toHaveAttribute("data-columns", "3");
    expect(screen.getByRole("button", { name: "添加 MCP" })).toBeVisible();
    expect(screen.getByText("GitHub tools")).toBeVisible();
    expect(screen.queryByText("已同步")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "停用 GitHub" }));
    expect(onEnabledChange).toHaveBeenCalledWith(syncedRow, false);
    await userEvent.click(screen.getByRole("button", { name: "从 Codex 移除 GitHub" }));
    expect(onRemove).toHaveBeenCalledWith(syncedRow.asset);
  });

  it("does not expose an asset identity when the card has no description", () => {
    render(
      <AgentConsumptionPanel
        domain="mcp"
        title="MCP"
        manageLabel="添加 MCP"
        rows={[syncedRow]}
        external={[]}
        present={() => ({ name: "GitHub" })}
        onManage={vi.fn()}
      />,
    );

    const card = screen.getByText("GitHub").closest<HTMLElement>("li");
    expect(card).not.toBeNull();
    expect(within(card!).queryByText("github::stdio")).not.toBeInTheDocument();
    expect(card!.querySelector(".mux-consumption-copy small")).toBeNull();
  });

  it("uses asset-specific empty copy", () => {
    render(
      <AgentConsumptionPanel
        domain="skill"
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
        domain="model"
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

  it("renders every external asset as a disabled card with only its adoption action", async () => {
    const onAdopt = vi.fn();
    const onEnabledChange = vi.fn();
    const onOpenAsset = vi.fn();
    const onRemove = vi.fn();
    render(
      <AgentConsumptionPanel
        domain="mcp"
        title="MCP"
        manageLabel="添加 MCP"
        rows={[syncedRow]}
        external={[externalRow]}
        externalMode="cards"
        present={(asset) => ({ name: asset.domain === "mcp" ? asset.key : "asset" })}
        onManage={vi.fn()}
        onEnabledChange={onEnabledChange}
        onOpenAsset={onOpenAsset}
        onRemove={onRemove}
        renderExternalAction={(item) => (
          <button type="button" onClick={() => onAdopt(item)}>让 MUX 管理</button>
        )}
      />,
    );

    const cards = screen.getAllByRole("listitem");
    expect(cards).toHaveLength(2);
    expect(cards[1]).toHaveAttribute("data-status", "external");
    expect(cards[1]).toHaveAttribute("data-enabled", "false");
    expect(within(cards[1]).getByText("外部配置")).toBeVisible();
    expect(within(cards[1]).queryByRole("switch")).not.toBeInTheDocument();
    expect(within(cards[1]).queryByRole("button", { name: /查看|移除/ })).not.toBeInTheDocument();

    await userEvent.click(within(cards[1]).getByRole("button", { name: "让 MUX 管理" }));
    expect(onAdopt).toHaveBeenCalledWith(externalRow);
    expect(onEnabledChange).not.toHaveBeenCalled();
    expect(onOpenAsset).not.toHaveBeenCalled();
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("rejects cross-domain rows at the panel boundary", () => {
    const modelRow: ConsumptionView = {
      ...syncedRow,
      asset: { domain: "model", profile_id: "claude-opus-4-7" },
      active: true,
    };
    const skillRow: ConsumptionView = {
      ...syncedRow,
      asset: { domain: "skill", name: "dws" },
      enabled: null,
      affected_agent_ids: Array.from({ length: 9 }, (_, index) => `agent-${index}`),
    };

    render(
      <AgentConsumptionPanel
        domain="model"
        title="Models"
        manageLabel="添加 Model"
        rows={[modelRow, skillRow]}
        external={[]}
        present={(asset) => ({
          name: asset.domain === "model" ? asset.profile_id : asset.domain === "skill" ? asset.name : asset.key,
        })}
        onManage={vi.fn()}
      />,
    );

    expect(screen.getAllByText("claude-opus-4-7")[0]).toBeVisible();
    expect(screen.queryByText("dws")).not.toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
  });
});
