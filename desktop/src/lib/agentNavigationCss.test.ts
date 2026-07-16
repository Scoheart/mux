import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile(new URL("../index.css", import.meta.url), "utf8");

test("popup action focus rule includes the search clear button", () => {
  const focusRule = Array.from(css.matchAll(/([^{}]+)\{([^{}]*)\}/g)).find(
    ([, selectors, declarations]) =>
      selectors.includes(".mux-agent-picker-select:focus-visible") &&
      declarations.includes("outline:"),
  );

  assert.ok(focusRule, "expected the popup action focus-visible rule");
  const selectors = focusRule[1].split(",").map((selector) => selector.trim());
  assert.ok(
    selectors.includes(".mux-agent-picker-search-clear:focus-visible"),
    "search clear button must receive the popup action focus-visible treatment",
  );
});
