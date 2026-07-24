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
  const match = source.match(
    new RegExp(`(?:^|\\n)\\s*${escaped}\\s*\\{([^{}]*)\\}`),
  );
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

it("keeps Models, MCPs, Skills order before Agent navigation", () => {
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
  const labels = ["Models", "MCPs", "Skills"].map((label) => segment.indexOf(label));
  expect(labels.every((index) => index >= 0)).toBe(true);
  expect(labels).toEqual([...labels].sort((left, right) => left - right));
  expect(segment).toMatch(/data-active=\{view\.kind === "skills"/);
  expect(segment).toMatch(/onClick=\{onSelectSkills\}/);
  expect(segment).toMatch(/<SparklesIcon/);
  expect(segment).not.toMatch(/Beta|mux-seg-beta/);
  expect(css).not.toMatch(/mux-seg-beta/);
});

it("routes the single app-owned Skills state before the MCP loading gate", () => {
  expect(app.match(/\buseSkillsState\(\)/g) ?? []).toHaveLength(1);
  expect(app).toMatch(/onSelectSkills=\{\(\) => setView\(\{ kind: "skills" \}\)\}/);
  expect(app).toMatch(/<SkillsView\s+state=\{skillsState\}/);
  expect(app).toMatch(/intent=\{view\.intent\}/);
  expect(app).toMatch(/onIntentConsumed=\{consumeResourceIntent\}/);

  const skillsRoute = app.indexOf('view.kind === "skills"');
  const loadingGate = app.indexOf("state.loading");
  const agentBranch = app.indexOf('view.kind === "agent"', loadingGate);
  expect(skillsRoute, "expected an explicit Skills route").not.toBe(-1);
  expect(skillsRoute).toBeLessThan(loadingGate);
  expect(agentBranch, "expected an explicit Agent route").toBeGreaterThan(loadingGate);
});

it("keeps topbar controls at their wide-layout sizes without compact overrides", () => {
  const topbar = declarations(css, ".mux-topbar");
  expect(pixelValue(topbar, "gap")).toBe(8);
  expect(pixelValue(topbar, "padding-left")).toBe(16);
  expect(pixelValue(declarations(css, ".mux-icon-btn"), "width")).toBe(30);
  expect(pixelValue(declarations(css, ".mux-icon-btn"), "height")).toBe(30);
  const pickerAnchor = declarations(css, ".mux-agent-picker-anchor");
  expect(pixelValue(pickerAnchor, "--mux-agent-picker-width")).toBe(220);
  expect(pickerAnchor).toMatch(/width:\s*var\(--mux-agent-picker-width\)/);
  expect(declarations(css, ".mux-agent-picker-trigger")).toMatch(/width:\s*100%/);
  const picker = declarations(css, ".mux-agent-picker");
  expect(picker).toMatch(/right:\s*0/);
  expect(picker).toMatch(/width:\s*var\(--mux-agent-picker-width\)/);
  expect(pixelValue(declarations(css, ".mux-agent-picker-trigger"), "height")).toBe(40);
  expect(pixelValue(declarations(css, ".mux-pinned-agent"), "width")).toBe(34);
  expect(pixelValue(declarations(css, ".mux-pinned-agent"), "height")).toBe(34);
  expect(pixelValue(declarations(css, ".mux-update-check"), "height")).toBe(32);

  expect(css).not.toMatch(/\.mux-topbar \.mux-(?:wordmark|seg|skill-seg|seg-item|icon-btn|update-check)/);
  expect(css.match(/\.mux-agent-picker-trigger\s*\{/g) ?? []).toHaveLength(1);
  expect(css.match(/\.mux-pinned-agent-glyph\s*\{/g) ?? []).toHaveLength(1);
  expect(css).not.toMatch(/@media \(max-width: (?:980|840)px\)/);
  expect(layout).toMatch(/className="mux-update-check-label"/);
  expect(layout).toMatch(/aria-label=\{version \? `检查更新，当前版本/);
  expect(layout.match(/className="mux-resource-label"/g) ?? []).toHaveLength(3);
});

it("absorbs narrow widths in a horizontally scrollable pinned Agent lane", () => {
  expect(layout).toMatch(/className="mux-topbar-navigation-lane"/);
  const lane = declarations(css, ".mux-topbar-navigation-lane");
  expect(lane).toMatch(/min-width:\s*0/);
  expect(lane).toMatch(/flex:\s*1\s+1\s+auto/);

  const navigation = declarations(css, ".mux-agent-navigation");
  expect(navigation).toMatch(/width:\s*100%/);
  expect(navigation).toMatch(/min-width:\s*0/);
  const pinned = declarations(css, ".mux-pinned-agent-bar");
  expect(pinned).toMatch(/overflow-x:\s*auto/);
  expect(pinned).toMatch(/overscroll-behavior-inline:\s*contain/);
  expect(pinned).toMatch(/scroll-snap-type:\s*x\s+proximity/);
  expect(declarations(css, ".mux-agent-picker-anchor")).toMatch(/flex:\s*0\s+0\s+auto/);
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

it("pinned Agent glyph remains 30px at every viewport width", () => {
  expect(component).toMatch(
    /className="mux-pinned-agent-glyph">\s*<AgentGlyph[^>]*size=\{30\}\s*\/>\s*<\/span>/,
  );

  const baseGlyph = declarations(css, ".mux-pinned-agent-glyph");
  expect(baseGlyph).toMatch(/width:\s*30px/);
  expect(baseGlyph).toMatch(/height:\s*30px/);
  const renderedGlyph = declarations(css, ".mux-pinned-agent-glyph > *");
  expect(renderedGlyph).toMatch(/width:\s*100%\s*!important/);
  expect(renderedGlyph).toMatch(/height:\s*100%\s*!important/);

  expect(css.match(/\.mux-pinned-agent-glyph\s*\{/g) ?? []).toHaveLength(1);
});

it("drag preview keeps DOM order stable and projects only picker rows", () => {
  expect(component).toMatch(/const \[previewIds, setPreviewIds\]/);
  expect(component).toMatch(/const previewIdsRef = useRef/);
  expect(component).not.toMatch(/orderedPinnedAgents/);
  expect(
    component.match(/sections\.pinned\.map\(\(agent\) =>/g) ?? [],
    "top shortcuts and picker rows must keep the committed order during preview",
  ).toHaveLength(2);
  expect(component).toMatch(/previewPinnedAgentOrder/);
  expect(component).toMatch(
    /previewPinnedAgentOrder\(pinnedIds, draggedId, targetId, placement\)/,
  );
  expect(component).toMatch(/projectedPinnedAgentOffset/);
  expect(component).toMatch(/--mux-agent-order-offset/);
  expect(component).toMatch(/setDragImage\(\s*event\.currentTarget/);
  expect(component).toMatch(/className="mux-agent-picker-slot"/);
  expect(component).toMatch(/data-sorting=/);
  expect(component).toMatch(/data-drag-source=/);
  expect(component).toMatch(/data-drop-position=/);
  expect(component).toMatch(/data-settling=\{settling/);
  expect(component).toMatch(/requestAnimationFrame/);

  const slot = declarations(css, ".mux-agent-picker-slot");
  expect(slot).toMatch(/position:\s*relative/);
  const row = declarations(css, ".mux-agent-picker-row");
  expect(row).toMatch(/position:\s*relative/);
  expect(row).toMatch(/transform:\s*translateY\(var\(--mux-agent-order-offset/);
  expect(css).toMatch(
    /\.mux-agent-picker-slot\[data-sorting="true"\]:not\(\[data-drag-source="true"\]\) > \.mux-agent-picker-row\s*\{[^}]*pointer-events:\s*none/,
  );
  expect(css).toMatch(/\.mux-agent-picker-row\[data-drop-position="before"\]::before/);
  expect(css).toMatch(/\.mux-agent-picker-row\[data-drop-position="after"\]::after/);
  expect(css).toMatch(
    /\.mux-agent-picker-list\[data-settling="true"\] \.mux-agent-picker-row\s*\{[^}]*transition:\s*none/,
  );

  const handle = declarations(css, ".mux-agent-order-handle");
  expect(pixelValue(handle, "width")).toBeGreaterThanOrEqual(36);
  expect(pixelValue(handle, "height")).toBeGreaterThanOrEqual(40);
});

it("reduced motion disables drag-preview transitions", () => {
  const reducedStart = css.indexOf("@media (prefers-reduced-motion: reduce)");
  expect(reducedStart).not.toBe(-1);
  expect(css.slice(reducedStart)).toMatch(
    /\.mux-agent-picker-row\s*\{[^}]*transition:\s*none/,
  );
});
