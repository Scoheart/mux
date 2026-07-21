import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const relativeFile = (path: string) => new URL(path, import.meta.url);

const css = await readFile(relativeFile("../index.css"), "utf8");
const layout = await readFile(relativeFile("../components/Layout.tsx"), "utf8");
const workspace = await readFile(relativeFile("../components/ResourceWorkspace.tsx"), "utf8");
const agentView = await readFile(relativeFile("../components/AgentView.tsx"), "utf8");

function declarations(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^{}]*)\\}`));
  expect(match, `expected ${selector} rule`).toBeTruthy();
  return match?.[1] ?? "";
}

it("defines the four-level Soft Workbench surface and radius system", () => {
  const root = declarations(":root");
  expect(root).toMatch(/--surface-app:\s*#EEF1F5/);
  expect(root).toMatch(/--surface-canvas:\s*#FAFBFC/);
  expect(root).toMatch(/--surface-quiet:\s*#F3F5F8/);
  expect(root).toMatch(/--surface-selected:\s*#E7F1FF/);
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
  expect(row).toMatch(/background:\s*var\(--surface-quiet\)/);
  expect(declarations(".mux-resource-card:hover")).toMatch(/transform:\s*none/);
});

it("keeps the top bar and dialogs free of decorative separators", () => {
  expect(layout).not.toMatch(/borderBottom/);
  expect(layout).not.toMatch(/h-5 w-px/);
  expect(declarations(".mux-dialog-shell-header")).toMatch(/border:\s*0/);
  expect(declarations(".mux-dialog-shell-footer")).toMatch(/border:\s*0/);
  expect(declarations(".mux-resource-inspector")).toMatch(/border:\s*0/);
});

it("groups Agent identity and paths in one context region", () => {
  expect(agentView).toMatch(/className="mux-agent-context"/);
  expect(workspace).toMatch(/className="mux-workspace-intro"/);
  expect(declarations(".mux-agent-context")).toMatch(/border-radius:\s*var\(--radius-canvas\)/);
  expect(declarations(".mux-consumption-list > li")).toMatch(/border:\s*0/);
  expect(declarations(".mux-consumption-external")).toMatch(/background:\s*var\(--surface-attention\)/);
});
