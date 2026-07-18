import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/AddAgentDialog.tsx"), "utf8");

it("uses the shared editor shell without changing schema ownership", () => {
  expect(source).toMatch(/<DialogShell/);
  expect(source).toMatch(/busy=\{busy\}/);
  expect(source).toMatch(/schemaLocked/);
  expect(source).not.toMatch(/<ModalHeader/);
});
