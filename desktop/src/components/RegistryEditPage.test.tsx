import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterEach, expect, it, vi } from "vitest";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { RegistryEditPage } from "./RegistryEditPage";
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

  await user.click(screen.getByRole("button", { name: "审阅更改" }));
  await waitFor(() => expect(planUpdate).toHaveBeenCalledTimes(1));
  expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "mcp",
    entry: expect.objectContaining({ tags: ["official", "catalog"] }),
  }));
});
