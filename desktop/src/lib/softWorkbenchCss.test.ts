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
  expect(root).toMatch(/--surface-frame:\s*#E3E9F0/);
  expect(root).toMatch(/--surface-navigation:\s*#EDF1F5/);
  expect(root).toMatch(/--surface-workspace:\s*#FAFBFD/);
  expect(root).toMatch(/--surface-section:\s*#E9EEF4/);
  expect(root).toMatch(/--surface-item:\s*#F1F4F8/);
  expect(root).toMatch(/--surface-control:\s*#FFFFFF/);
  expect(root).toMatch(/--surface-selected:\s*#DDEBFF/);
  expect(root).toMatch(/--surface-modal-scrim:\s*rgba\(27, 34, 48, \.34\)/);
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
  expect(row).toMatch(/background:\s*transparent/);
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

it("groups Agent identity and paths in one context region", () => {
  expect(agentView).toMatch(/className="mux-agent-context"/);
  expect(agentView).toMatch(/<section className="mux-agent-context"[^>]*>[\s\S]*?<AgentHeader agent=\{agent\} tone="reference" \/>/);
  expect(workspace).toMatch(/className="mux-workspace-intro"/);
  expect(declarations(".mux-agent-context")).toMatch(/border-radius:\s*var\(--radius-canvas\)/);
  expect(declarations(".mux-agent-context")).toMatch(/background:\s*var\(--surface-workspace\)/);
  expect(declarations(".mux-consumption-list > li")).toMatch(/border:\s*0/);
  expect(declarations(".mux-consumption-external")).toMatch(/background:\s*var\(--surface-attention\)/);
});

it("uses the same dense two-column asset region in central and Agent views", () => {
  const centralGrid = declarations(".mux-resource-grid");
  const agentGrid = declarations(".mux-consumption-list");
  const centralItem = declarations(".mux-resource-card");
  const agentItem = declarations(".mux-consumption-list > li");

  for (const grid of [centralGrid, agentGrid]) {
    expect(grid).toMatch(/grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\)/);
    expect(grid).toMatch(/gap:\s*6px/);
    expect(grid).toMatch(/padding:\s*6px/);
    expect(grid).toMatch(/border:\s*0/);
    expect(grid).toMatch(/border-radius:\s*var\(--radius-region\)/);
    expect(grid).toMatch(/background:\s*var\(--surface-section\)/);
  }

  for (const item of [centralItem, agentItem]) {
    expect(item).toMatch(/border:\s*0/);
    expect(item).toMatch(/border-radius:\s*var\(--radius-row\)/);
    expect(item).toMatch(/background:\s*transparent/);
  }
  expect(centralItem).toMatch(/grid-template-areas:\s*"identity"\s*"configuration"\s*"state"\s*"impact"/);
  expect(centralItem).toMatch(/box-shadow:\s*none/);

  const external = declarations(".mux-consumption-external");
  expect(external).toMatch(/width:\s*100%/);
  expect(external).toMatch(/border:\s*0/);
});
