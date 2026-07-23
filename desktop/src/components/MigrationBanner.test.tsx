import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { MigrationCandidate, MigrationDomain } from "../lib/migration";
import { MigrationBanner } from "./MigrationBanner";

afterEach(cleanup);

function candidate(domain: MigrationDomain, index: number): MigrationCandidate {
  return {
    id: `${domain}:${index}`,
    domain,
    name: `${domain}-${index}`,
    detail: "旧配置",
    agentIds: ["codex"],
    fingerprint: `${domain}:${index}:fingerprint`,
    safe: true,
    conflictReason: null,
  };
}

it("shows readable copy, domain counts, and clear primary and secondary actions", async () => {
  const onLater = vi.fn();
  const onOpen = vi.fn();
  render(
    <MigrationBanner
      candidates={[
        candidate("mcp", 1),
        candidate("mcp", 2),
        candidate("model", 1),
        candidate("skill", 1),
      ]}
      onLater={onLater}
      onOpen={onOpen}
    />,
  );

  expect(screen.getByRole("status", { name: "旧配置导入提醒" })).toBeVisible();
  expect(screen.getByText("发现 4 项可导入的旧配置")).toBeVisible();
  expect(screen.getByText("整理到 MUX，后续可以统一查看和管理。")).toBeVisible();
  expect(screen.getByRole("list", { name: "待导入配置分类" })).toHaveTextContent(
    "MCP 2Model 1Skill 1",
  );

  await userEvent.click(screen.getByRole("button", { name: "稍后" }));
  await userEvent.click(screen.getByRole("button", { name: "去处理" }));
  expect(onLater).toHaveBeenCalledOnce();
  expect(onOpen).toHaveBeenCalledOnce();
});

it("omits empty domain counters", () => {
  render(
    <MigrationBanner
      candidates={[candidate("skill", 1)]}
      onLater={vi.fn()}
      onOpen={vi.fn()}
    />,
  );

  expect(screen.getByText("Skill 1")).toBeVisible();
  expect(screen.queryByText(/MCP 0|Model 0/)).not.toBeInTheDocument();
});
