import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import i18n from "../i18n";
import type { MigrationCandidate, MigrationDomain } from "../lib/migration";
import { MigrationBanner } from "./MigrationBanner";

afterEach(cleanup);

beforeEach(async () => {
  await i18n.changeLanguage("zh-CN");
});

function candidate(domain: MigrationDomain, index: number): MigrationCandidate {
  return {
    id: `${domain}:${index}`,
    domain,
    name: `${domain}-${index}`,
    detail: {
      kind: domain,
      ...(domain === "mcp"
        ? {
          transport: "STDIO",
          agentCount: 1,
          disabledCount: 0,
          centralExists: false,
        }
        : domain === "model"
          ? {
            provider: "openrouter",
            model: "example/model",
            agentCount: 1,
            activeCount: 0,
          }
          : { agentCount: 1, folderCount: 1 }),
    } as MigrationCandidate["detail"],
    agentIds: ["codex"],
    fingerprint: `${domain}:${index}:fingerprint`,
    safe: true,
    conflict: null,
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

  expect(screen.getByRole("status", { name: "外部配置识别提醒" })).toBeVisible();
  expect(screen.getByText("发现 4 项外部配置")).toBeVisible();
  expect(screen.getByText("MUX 只负责识别；请逐项确认是否交给 MUX 管理。")).toBeVisible();
  expect(screen.getByRole("list", { name: "外部配置分类" })).toHaveTextContent(
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

it("localizes the dynamic banner count and actions", async () => {
  await i18n.changeLanguage("en-US");
  render(
    <MigrationBanner
      candidates={[candidate("mcp", 1), candidate("skill", 1)]}
      onLater={vi.fn()}
      onOpen={vi.fn()}
    />,
  );

  expect(screen.getByRole("status", { name: "External configuration notice" })).toBeVisible();
  expect(screen.getByText("2 external configurations found")).toBeVisible();
  expect(screen.getByRole("button", { name: "Later" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Review" })).toBeVisible();
  expect(document.body).not.toHaveTextContent(/[\u3400-\u9fff]/);
});
