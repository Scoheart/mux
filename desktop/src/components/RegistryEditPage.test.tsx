import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterEach, expect, it, vi } from "vitest";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { RegistryEditPage } from "./RegistryEditPage";
import { ResourceWorkspace } from "./ResourceWorkspace";
import { ToastProvider } from "./Toast";

const source = await readFile(resolve(process.cwd(), "src/components/RegistryEditPage.tsx"), "utf8");

afterEach(cleanup);

it("routes central MCP changes through the shared asset plan", () => {
  expect(source).toMatch(/<DialogShell/);
  expect(source).toMatch(/kind="editor"/);
  expect(source).toMatch(/consumptionState\.planUpdate/);
  expect(source).toMatch(/consumptionState\.planDelete/);
  expect(source).not.toMatch(/upsertRegistry|deleteRegistry|resyncEntry/);
  expect(source).not.toMatch(/window\.confirm/);
  expect(source).not.toMatch(/<ModalHeader/);
});

it("hides manual tag editing while preserving existing asset tags", () => {
  expect(source).not.toMatch(/tagsText|标签（逗号分隔）|official, builtin/);
  expect(source).toMatch(/tags: existing\?\.tags \?\? \[\]/);
});

it("keeps imported tags when an existing MCP edit is planned", async () => {
  const user = userEvent.setup();
  const existing: RegistryEntry = {
    name: "source-backed-mcp",
    description: "Imported MCP",
    tags: ["official", "catalog"],
    origin: { kind: "remote", source: "mux-curated" },
    config: { stdio: { command: "npx", args: ["-y", "source-backed-mcp"] } },
  };
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "mcp-plan" });
  const state = {
    entries: [existing],
    customKeys: new Set<string>(),
  } as unknown as InstallState;
  const consumptionState = { planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <RegistryEditPage
        state={state}
        consumptionState={consumptionState}
        name={existing.name}
        transport="stdio"
        onBack={() => undefined}
      />
    </ToastProvider>,
  );

  await user.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(planUpdate).toHaveBeenCalledTimes(1));
  expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "mcp",
    entry: expect.objectContaining({ tags: ["official", "catalog"] }),
  }));
});

it("renders an existing MCP editor inside one resource dialog", () => {
  const existing: RegistryEntry = {
    name: "single-shell-mcp",
    description: "Single shell",
    tags: [],
    origin: { kind: "manual" },
    config: { stdio: { command: "npx", args: ["single-shell-mcp"] } },
  };
  const state = {
    entries: [existing],
    customKeys: new Set<string>(),
  } as unknown as InstallState;
  const consumptionState = {} as ConsumptionState;

  render(
    <ToastProvider>
      <ResourceWorkspace
        sidebar={<div />}
        query=""
        onQueryChange={() => undefined}
        searchPlaceholder="搜索 MCP"
        toolbarActions={null}
        inspector={
          <RegistryEditPage
            state={state}
            consumptionState={consumptionState}
            name={existing.name}
            entry={existing}
            transport="stdio"
            presentation="inspector"
            onBack={() => undefined}
          />
        }
      >
        <div />
      </ResourceWorkspace>
    </ToastProvider>,
  );

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.getByRole("complementary", { name: `${existing.name} 详情` })).toBeVisible();
  expect(screen.queryByRole("dialog", { name: "编辑 MCP" })).not.toBeInTheDocument();
});
