import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const relativeFile = (path: string) => new URL(path, import.meta.url);

const css = await readFile(relativeFile("../index.css"), "utf8");
const component = await readFile(
  relativeFile("../components/AgentNavigation.tsx"),
  "utf8",
);
const app = await readFile(relativeFile("../App.tsx"), "utf8");
const layout = await readFile(relativeFile("../components/Layout.tsx"), "utf8");
const icons = await readFile(relativeFile("../components/icons.tsx"), "utf8");
const types = await readFile(relativeFile("./types.ts"), "utf8");

function declarations(source: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`));
  expect(match, `expected ${selector} rule`).toBeTruthy();
  if (!match) throw new Error(`expected ${selector} rule`);
  return match[1];
}

function pixelValue(source: string, property: string): number {
  const match = source.match(new RegExp(`${property}:\\s*(\\d+)px`));
  expect(match, `expected ${property} in ${source}`).toBeTruthy();
  if (!match) throw new Error(`expected ${property}`);
  return Number(match[1]);
}

it("keeps Skills as the third resource segment before Agent navigation", () => {
  expect(types).toMatch(
    /\| \{ kind: "skills"; intent\?: SkillNavigationIntent \}/,
  );
  expect(icons).toMatch(/export function SparklesIcon\(/);
  expect(layout).toMatch(/onSelectSkills: \(\) => void/);

  const segmentStart = layout.indexOf('className="mux-seg mux-skill-seg');
  const agentNavigationStart = layout.indexOf("<AgentNavigation");
  expect(segmentStart, "expected the Skills-specific resource segment").not.toBe(-1);
  expect(agentNavigationStart, "expected AgentNavigation to remain separate").toBeGreaterThan(
    segmentStart,
  );

  const segment = layout.slice(segmentStart, agentNavigationStart);
  const labels = ["MCPs", "Models", "Skills"].map((label) => segment.indexOf(label));
  expect(labels.every((index) => index >= 0)).toBe(true);
  expect(labels).toEqual([...labels].sort((left, right) => left - right));
  expect(segment).toMatch(/data-active=\{view\.kind === "skills"/);
  expect(segment).toMatch(/onClick=\{onSelectSkills\}/);
  expect(segment).toMatch(/<SparklesIcon/);
});

it("routes the single app-owned Skills state before the MCP loading gate", () => {
  expect(app.match(/\buseSkillsState\(\)/g) ?? []).toHaveLength(1);
  expect(app).toMatch(/onSelectSkills=\{\(\) => setView\(\{ kind: "skills" \}\)\}/);
  expect(app).toMatch(/<SkillsView\s+state=\{skillsState\}/);
  expect(app).toMatch(/intent=\{view\.intent\}/);
  expect(app).toMatch(/onIntentConsumed=\{consumeSkillIntent\}/);

  const skillsRoute = app.indexOf('view.kind === "skills"');
  const loadingGate = app.indexOf("state.loading");
  const agentBranch = app.indexOf('view.kind === "agent"', loadingGate);
  expect(skillsRoute, "expected an explicit Skills route").not.toBe(-1);
  expect(skillsRoute).toBeLessThan(loadingGate);
  expect(agentBranch, "expected an explicit Agent route").toBeGreaterThan(loadingGate);
});

it("reserves compact 900px lanes for Skills and six pinned Agents", () => {
  const compactStart = css.indexOf("@media (max-width: 980px)");
  const compactEnd = css.indexOf("@media", compactStart + 1);
  expect(compactStart, "expected the 900px topbar media query").not.toBe(-1);
  const compactCss = css.slice(
    compactStart,
    compactEnd === -1 ? css.length : compactEnd,
  );

  const wordmark = declarations(compactCss, ".mux-topbar .mux-wordmark");
  expect(pixelValue(wordmark, "width")).toBeGreaterThanOrEqual(48);
  expect(pixelValue(wordmark, "flex-basis")).toBeGreaterThanOrEqual(48);

  const segment = declarations(compactCss, ".mux-topbar .mux-skill-seg");
  expect(pixelValue(segment, "width")).toBeLessThanOrEqual(240);
  expect(segment).toMatch(/flex:\s*0\s+0\s+\d+px/);

  const beta = declarations(compactCss, ".mux-topbar .mux-seg-beta");
  expect(beta).toMatch(/display:\s*none/);

  const picker = declarations(compactCss, ".mux-agent-picker-trigger");
  expect(pixelValue(picker, "width")).toBeLessThanOrEqual(132);

  const pickerName = declarations(compactCss, ".mux-agent-picker-trigger-name");
  expect(pixelValue(pickerName, "width")).toBe(1);
  expect(pickerName).toMatch(/clip-path:\s*inset\(50%\)/);
  expect(pickerName).not.toMatch(/display:\s*none/);

  const pinned = declarations(compactCss, ".mux-pinned-agent");
  expect(pixelValue(pinned, "width")).toBeLessThanOrEqual(26);
  expect(pixelValue(pinned, "flex-basis")).toBeLessThanOrEqual(26);
});

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

it("drag preview drives both pinned surfaces and exposes target styling", () => {
  expect(component).toMatch(/const \[previewIds, setPreviewIds\]/);
  expect(component).toMatch(/const orderedPinnedAgents/);
  expect(
    component.match(/orderedPinnedAgents\.map/g) ?? [],
    "top shortcuts and pinned rows must share the preview order",
  ).toHaveLength(2);
  expect(component).toMatch(/previewPinnedAgentOrder/);
  expect(component).toMatch(/data-drop-position=/);

  const row = declarations(css, ".mux-agent-picker-row");
  expect(row).toMatch(/position:\s*relative/);
  expect(css).toMatch(/\.mux-agent-picker-row\[data-drop-position="before"\]::before/);
  expect(css).toMatch(/\.mux-agent-picker-row\[data-drop-position="after"\]::after/);
});

it("reduced motion disables drag-preview transitions", () => {
  const reducedStart = css.indexOf("@media (prefers-reduced-motion: reduce)");
  expect(reducedStart).not.toBe(-1);
  expect(css.slice(reducedStart)).toMatch(
    /\.mux-agent-picker-row\s*\{[^}]*transition:\s*none/,
  );
});
