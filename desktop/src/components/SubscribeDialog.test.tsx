import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/SubscribeDialog.tsx"), "utf8");

it("uses the shared editor shell and pending dismissal contract", () => {
  expect(source).toMatch(/<DialogShell/);
  expect(source).toMatch(/busy=\{busy\}/);
  expect(source).toMatch(/footerEnd=/);
  expect(source).not.toMatch(/<ModalHeader/);
});
