import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/RegistryEditPage.tsx"), "utf8");

it("routes central MCP changes through the shared asset plan", () => {
  expect(source).toMatch(/<DialogShell/);
  expect(source).toMatch(/kind="editor"/);
  expect(source).toMatch(/consumptionState\.planUpdate/);
  expect(source).toMatch(/consumptionState\.planDelete/);
  expect(source).not.toMatch(/upsertRegistry|deleteRegistry|resyncEntry/);
  expect(source).not.toMatch(/window\.confirm/);
  expect(source).not.toMatch(/<ModalHeader/);
});
