import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile(new URL("../index.css", import.meta.url), "utf8");
const component = await readFile(
  new URL("../components/AgentNavigation.tsx", import.meta.url),
  "utf8",
);

function declarations(source: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`));
  assert.ok(match, `expected ${selector} rule`);
  return match[1];
}

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

test("pinned Agent glyph is 30px normally and 28px in compact topbar", () => {
  assert.match(
    component,
    /className="mux-pinned-agent-glyph">\s*<AgentGlyph[^>]*size=\{30\}\s*\/>\s*<\/span>/,
  );

  const baseGlyph = declarations(css, ".mux-pinned-agent-glyph");
  assert.match(baseGlyph, /width:\s*30px/);
  assert.match(baseGlyph, /height:\s*30px/);
  const renderedGlyph = declarations(css, ".mux-pinned-agent-glyph > *");
  assert.match(renderedGlyph, /width:\s*100%\s*!important/);
  assert.match(renderedGlyph, /height:\s*100%\s*!important/);

  const compactStart = css.indexOf("@media (max-width: 1080px)");
  const compactEnd = css.indexOf("@media (prefers-reduced-motion", compactStart);
  assert.notEqual(compactStart, -1, "expected compact topbar media query");
  const compactCss = css.slice(compactStart, compactEnd);
  const compactGlyph = declarations(compactCss, ".mux-pinned-agent-glyph");
  assert.match(compactGlyph, /width:\s*28px/);
  assert.match(compactGlyph, /height:\s*28px/);
});
