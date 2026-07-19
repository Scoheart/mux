import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const relativeFile = (path: string) => new URL(path, import.meta.url);
const css = await readFile(relativeFile("../index.css"), "utf8");

function declarations(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return css.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`))?.[1] ?? "";
}

it("keeps picker content on one vertical scroll axis", () => {
  const pickerBody = declarations('.mux-dialog-shell[data-dialog-kind="picker"] .mux-dialog-shell-body');
  const list = declarations(".mux-picker-list");

  expect(pickerBody).toMatch(/overflow:\s*hidden/);
  expect(list).toMatch(/width:\s*100%/);
  expect(list).toMatch(/min-width:\s*0/);
  expect(list).toMatch(/overflow-x:\s*hidden/);
  expect(list).toMatch(/overflow-y:\s*auto/);
});

it("prevents picker rows from widening their scroll container", () => {
  const option = declarations(".mux-picker-option");

  expect(option).toMatch(/width:\s*100%/);
  expect(option).toMatch(/min-width:\s*0/);
  expect(option).toMatch(/max-width:\s*100%/);
});
