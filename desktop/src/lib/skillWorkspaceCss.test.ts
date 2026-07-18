import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const relativeFile = (path: string) => new URL(path, import.meta.url);
const css = await readFile(relativeFile("../index.css"), "utf8");
const skillsView = await readFile(
  relativeFile("../components/SkillsView.tsx"),
  "utf8",
);

function declarations(source: string, selector: string): string | null {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return source.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`))?.[1] ?? null;
}

function mediaBlocks(source: string, heading: string): string[] {
  const blocks: string[] = [];
  let searchFrom = 0;

  while (true) {
    const start = source.indexOf(heading, searchFrom);
    if (start === -1) return blocks;
    const openingBrace = source.indexOf("{", start + heading.length);
    if (openingBrace === -1) return blocks;

    let depth = 0;
    let end = openingBrace;
    for (; end < source.length; end += 1) {
      if (source[end] === "{") depth += 1;
      if (source[end] === "}") depth -= 1;
      if (depth === 0) break;
    }
    blocks.push(source.slice(start, end + 1));
    searchFrom = end + 1;
  }
}

it("scopes the Skills busy spinner and disables it for reduced motion", () => {
  expect(skillsView).toMatch(/className="mux-skill-check-icon/);
  expect(skillsView).toMatch(/data-busy=\{checking \? "true" : undefined\}/);
  expect(skillsView).not.toMatch(
    /style=\{checking \? \{ animation: "spin 0\.8s linear infinite" \}/,
  );

  const busy = declarations(
    css,
    '.mux-skill-check-icon[data-busy="true"]',
  );
  expect(busy).toMatch(/animation:\s*spin\s+0\.8s\s+linear\s+infinite/);

  const reducedMotionBusy = mediaBlocks(
    css,
    "@media (prefers-reduced-motion: reduce)",
  )
    .map((block) =>
      declarations(block, '.mux-skill-check-icon[data-busy="true"]'),
    )
    .find((rule) => rule !== null);
  expect(reducedMotionBusy).toMatch(/animation:\s*none/);
});

it("uses one shared workspace geometry for every resource domain", () => {
  expect(skillsView).toMatch(/className="mux-skill-workspace"/);

  const sharedSidebar = declarations(css, ".mux-workspace-sidebar");
  const sharedToolbar = declarations(css, ".mux-workspace-toolbar");
  const sharedFilters = declarations(css, ".mux-workspace-filters");
  expect(sharedSidebar).toMatch(/border-right:\s*1px\s+solid/);
  expect(sharedToolbar).toMatch(/border-bottom:\s*1px\s+solid/);
  expect(sharedFilters).toMatch(/border-bottom:\s*1px\s+solid/);

  expect(css).not.toMatch(/\.mux-skill-workspace\s+\.mux-workspace-(?:sidebar|toolbar|filters|scroll)/);
  expect(css).not.toMatch(/\.mux-skill-workspace\s+\.mux-resource-(?:grid|tabs|tab)/);
  expect(sharedToolbar).toMatch(/min-height:\s*56px/);
  expect(sharedFilters).toMatch(/height:\s*40px/);

  const grid = declarations(css, ".mux-resource-grid");
  expect(grid).toMatch(/minmax\(250px,\s*1fr\)/);
  expect(grid).toMatch(/gap:\s*12px/);
});
