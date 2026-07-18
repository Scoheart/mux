import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/RegistryEditPage.tsx"), "utf8");

it("uses the shared editor and review shells", () => {
  expect(source).toMatch(/<DialogShell/);
  expect(source).toMatch(/kind="editor"/);
  expect(source).toMatch(/<ReviewDialog/);
  expect(source).not.toMatch(/window\.confirm/);
  expect(source).not.toMatch(/<ModalHeader/);
});
