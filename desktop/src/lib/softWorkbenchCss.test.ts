import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const relativeFile = (path: string) => new URL(path, import.meta.url);

const css = await readFile(relativeFile("../index.css"), "utf8");
const layout = await readFile(relativeFile("../components/Layout.tsx"), "utf8");
const workspace = await readFile(relativeFile("../components/ResourceWorkspace.tsx"), "utf8");
const agentView = await readFile(relativeFile("../components/AgentView.tsx"), "utf8");
const ui = await readFile(relativeFile("../components/ui.tsx"), "utf8");

function declarations(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`));
  expect(match, `expected ${selector} rule`).toBeTruthy();
  return match?.[1] ?? "";
}

it("defines role-based Soft Workbench surfaces and radius hierarchy", () => {
  const root = declarations(":root");
  const dark = declarations(".dark");
  expect(root).toMatch(/--surface-frame:\s*#E3E9F0/);
  expect(root).toMatch(/--surface-navigation:\s*#EDF1F5/);
  expect(root).toMatch(/--surface-workspace:\s*#FAFBFD/);
  expect(root).toMatch(/--surface-section:\s*#E9EEF4/);
  expect(root).toMatch(/--surface-asset:\s*#FFFFFF/);
  expect(root).toMatch(/--surface-item:\s*#F1F4F8/);
  expect(root).toMatch(/--surface-control:\s*#FFFFFF/);
  expect(root).toMatch(/--surface-selected:\s*#DDEBFF/);
  expect(root).toMatch(/--surface-modal-scrim:\s*rgba\(27, 34, 48, \.34\)/);
  expect(dark).toMatch(/--surface-section:\s*#222A33/);
  expect(dark).toMatch(/--surface-asset:\s*#2C3641/);
  expect(root).toMatch(/--radius-canvas:\s*22px/);
  expect(root).toMatch(/--radius-region:\s*18px/);
  expect(root).toMatch(/--radius-row:\s*14px/);
  expect(root).toMatch(/--radius-control:\s*10px/);
});

it("uses regions and row islands instead of structural divider lines", () => {
  const shell = declarations(".mux-workspace");
  const sidebar = declarations(".mux-workspace-sidebar");
  const toolbar = declarations(".mux-workspace-toolbar");
  const filters = declarations(".mux-workspace-filters");
  const row = declarations(".mux-resource-card");

  expect(shell).toMatch(/gap:\s*14px/);
  expect(shell).toMatch(/padding:\s*14px/);
  for (const region of [sidebar, toolbar, filters, row]) {
    expect(region).toMatch(/border:\s*0/);
  }
  expect(row).toMatch(/border-radius:\s*var\(--radius-row\)/);
  expect(shell).toMatch(/background:\s*var\(--surface-frame\)/);
  expect(sidebar).toMatch(/background:\s*var\(--surface-navigation\)/);
  expect(toolbar).toMatch(/background:\s*var\(--surface-section\)/);
  expect(row).toMatch(/background:\s*var\(--surface-asset\)/);
  expect(declarations(".mux-resource-card:hover")).toMatch(/transform:\s*none/);
});

it("keeps the top bar and dialogs free of decorative separators", () => {
  expect(layout).not.toMatch(/borderBottom/);
  expect(layout).not.toMatch(/h-5 w-px/);
  expect(layout).toMatch(/background:\s*"var\(--surface-workspace\)"/);
  expect(declarations(".mux-dialog-shell-header")).toMatch(/border:\s*0/);
  expect(declarations(".mux-dialog-shell-footer")).toMatch(/border:\s*0/);
  expect(declarations(".mux-resource-inspector")).toMatch(/border:\s*0/);
  expect(ui).toMatch(/background:\s*"var\(--surface-modal-scrim\)"/);
});

it("separates interactive controls from nested preview content", () => {
  expect(declarations(".mux-search input")).toMatch(/background:\s*var\(--surface-control\)/);
  expect(declarations(".mux-config-preview")).toMatch(/background:\s*var\(--surface-item\)/);
  expect(declarations(".mux-skill-preview")).toMatch(/background:\s*var\(--surface-item\)/);
});

it("keeps passive metadata labels compact without shrinking interactive controls", () => {
  expect(ui).toMatch(/className="mux-badge"/);
  expect(ui).toMatch(/className="mux-transport-pill"/);

  const labelFrame = declarations(":where(.mux-badge, .mux-transport-pill)");
  const badge = declarations(".mux-badge");
  const transport = declarations(".mux-transport-pill");
  const risk = declarations(".mux-skill-risk-badge");
  for (const label of [labelFrame, risk]) {
    expect(label).toMatch(/height:\s*18px/);
    expect(label).toMatch(/padding:\s*0 6px/);
    expect(label).toMatch(/border-radius:\s*5px/);
  }
  expect(badge).toMatch(/font-size:\s*10px/);
  expect(transport).toMatch(/font:\s*650 9px\/1 var\(--font-mono\)/);
  expect(declarations(".mux-resource-tab")).toMatch(/height:\s*32px/);
  expect(declarations(".mux-icon-btn")).toMatch(/width:\s*30px; height:\s*30px/);
});

it("groups Agent identity and paths in one context region", () => {
  expect(agentView).toMatch(/className="mux-agent-context"/);
  expect(agentView).toMatch(/<section className="mux-agent-context"[^>]*>[\s\S]*?<AgentHeader agent=\{agent\} tone="reference" \/>/);
  expect(workspace).toMatch(/className="mux-workspace-intro"/);
  expect(declarations(".mux-agent-context")).toMatch(/border-radius:\s*var\(--radius-canvas\)/);
  expect(declarations(".mux-agent-context")).toMatch(/background:\s*var\(--surface-workspace\)/);
  expect(declarations(".mux-consumption-list > li")).toMatch(/border:\s*0/);
  expect(declarations(".mux-consumption-external")).toMatch(/background:\s*var\(--surface-attention\)/);
});

it("uses three columns for central assets and every Agent asset domain", () => {
  const centralGrid = declarations(".mux-resource-grid");
  const agentGrid = declarations(".mux-consumption-list");
  const agentSkillGrid = declarations(".mux-consumption-list[data-columns=\"3\"]");
  const centralItem = declarations(".mux-resource-card");
  const agentItem = declarations(".mux-consumption-list > li");

  expect(centralGrid).toMatch(/grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/);
  expect(centralGrid).toMatch(/gap:\s*8px/);
  expect(agentGrid).toMatch(/grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\)/);
  expect(agentSkillGrid).toMatch(/grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/);
  expect(agentView).toMatch(/title="MCP"[\s\S]{0,220}columns=\{3\}/);
  expect(agentView).toMatch(/title="Models"[\s\S]{0,300}columns=\{3\}/);
  expect(agentView).toMatch(/title="Skills"[\s\S]{0,900}columns=\{3\}/);
  expect(agentGrid).toMatch(/gap:\s*6px/);

  for (const grid of [centralGrid, agentGrid]) {
    expect(grid).toMatch(/padding:\s*6px/);
    expect(grid).toMatch(/border:\s*0/);
    expect(grid).toMatch(/border-radius:\s*var\(--radius-region\)/);
    expect(grid).toMatch(/background:\s*var\(--surface-section\)/);
    expect(grid).toMatch(/box-shadow:\s*none/);
  }

  for (const item of [centralItem, agentItem]) {
    expect(item).toMatch(/border:\s*0/);
    expect(item).toMatch(/border-radius:\s*var\(--radius-row\)/);
    expect(item).toMatch(/background:\s*var\(--surface-asset\)/);
    expect(item).toMatch(/box-shadow:\s*none/);
  }
  expect(declarations(".mux-consumption-list > li[data-enabled=\"false\"]")).toMatch(
    /background:\s*color-mix\(in srgb,\s*var\(--surface-asset\) 68%,\s*var\(--surface-section\)\)/,
  );
  expect(declarations(".mux-resource-card:hover")).toMatch(/var\(--surface-asset\)/);
  expect(declarations(".mux-consumption-list > li:hover")).toMatch(/var\(--surface-asset\)/);
  expect(centralItem).toMatch(/grid-template-areas:\s*"identity"\s*"configuration"\s*"state"\s*"impact"/);
  expect(centralItem).toMatch(/box-shadow:\s*none/);

  const external = declarations(".mux-consumption-external");
  expect(external).toMatch(/width:\s*100%/);
  expect(external).toMatch(/border:\s*0/);
  expect(agentView).toMatch(/externalMode="cards"/);
  expect(declarations(".mux-consumption-adopt")).toMatch(/color:\s*var\(--color-blue\)/);
});
