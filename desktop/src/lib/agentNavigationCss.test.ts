import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const relativeFile = (path: string) => new URL(path, import.meta.url);

const css = await readFile(relativeFile("../index.css"), "utf8");
const component = await readFile(
  relativeFile("../components/AgentNavigation.tsx"),
  "utf8",
);

function declarations(source: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`));
  expect(match, `expected ${selector} rule`).toBeTruthy();
  if (!match) throw new Error(`expected ${selector} rule`);
  return match[1];
}

it("popup action focus rule includes the search clear button", () => {
  const focusRule = Array.from(css.matchAll(/([^{}]+)\{([^{}]*)\}/g)).find(
    ([, selectors, declarations]) =>
      selectors.includes(".mux-agent-picker-select:focus-visible") &&
      declarations.includes("outline:"),
  );

  expect(focusRule, "expected the popup action focus-visible rule").toBeTruthy();
  if (!focusRule) throw new Error("expected the popup action focus-visible rule");
  const selectors = focusRule[1].split(",").map((selector) => selector.trim());
  expect(
    selectors,
    "search clear button must receive the popup action focus-visible treatment",
  ).toContain(".mux-agent-picker-search-clear:focus-visible");
});

it("pinned Agent glyph is 30px normally and 28px in compact topbar", () => {
  expect(component).toMatch(
    /className="mux-pinned-agent-glyph">\s*<AgentGlyph[^>]*size=\{30\}\s*\/>\s*<\/span>/,
  );

  const baseGlyph = declarations(css, ".mux-pinned-agent-glyph");
  expect(baseGlyph).toMatch(/width:\s*30px/);
  expect(baseGlyph).toMatch(/height:\s*30px/);
  const renderedGlyph = declarations(css, ".mux-pinned-agent-glyph > *");
  expect(renderedGlyph).toMatch(/width:\s*100%\s*!important/);
  expect(renderedGlyph).toMatch(/height:\s*100%\s*!important/);

  const compactStart = css.indexOf("@media (max-width: 1080px)");
  const compactEnd = css.indexOf("@media (prefers-reduced-motion", compactStart);
  expect(compactStart, "expected compact topbar media query").not.toBe(-1);
  const compactCss = css.slice(compactStart, compactEnd);
  const compactGlyph = declarations(compactCss, ".mux-pinned-agent-glyph");
  expect(compactGlyph).toMatch(/width:\s*28px/);
  expect(compactGlyph).toMatch(/height:\s*28px/);
});
