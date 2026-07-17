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

test("drag preview drives both pinned surfaces and exposes target styling", () => {
  assert.match(component, /const \[previewIds, setPreviewIds\]/);
  assert.match(component, /const orderedPinnedAgents/);
  assert.equal(
    (component.match(/orderedPinnedAgents\.map/g) ?? []).length,
    2,
    "top shortcuts and pinned rows must share the preview order",
  );
  assert.match(component, /previewPinnedAgentOrder/);
  assert.match(component, /data-drop-position=/);

  const row = declarations(css, ".mux-agent-picker-row");
  assert.match(row, /position:\s*relative/);
  assert.match(css, /\.mux-agent-picker-row\[data-drop-position="before"\]::before/);
  assert.match(css, /\.mux-agent-picker-row\[data-drop-position="after"\]::after/);
});

test("reduced motion disables drag-preview transitions", () => {
  const reducedStart = css.indexOf("@media (prefers-reduced-motion: reduce)");
  assert.notEqual(reducedStart, -1);
  assert.match(css.slice(reducedStart), /\.mux-agent-picker-row\s*\{[^}]*transition:\s*none/);
});
