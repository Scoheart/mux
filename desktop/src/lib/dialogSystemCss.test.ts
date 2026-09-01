import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

const css = await readFile(new URL("../index.css", import.meta.url), "utf8");

it("keeps dialog geometry shared and inspectors content-driven", () => {
  expect(css).toMatch(/--mux-dialog-header-height:\s*56px/);
  expect(css).toMatch(/\.mux-workspace-inspector-surface\s*\{[^}]*max-height:\s*inherit/);
  expect(css).not.toMatch(/\.mux-workspace-inspector-surface\s*\{[^}]*height:\s*min\(680px/);
});

it("does not style dialog shells by descendant feature detection", () => {
  expect(css).not.toMatch(/\.mux-dialog-shell:has\(\.mux-provider-(catalog|form)\)/);
  expect(css).not.toMatch(/\.mux-dialog-shell:has\(\.mux-agent-create\)/);
});

it("keeps ordinary inspector groups and picker rows flat", () => {
  expect(css).toMatch(/\.mux-inspector-section\s*\{[^}]*background:\s*transparent/);
  expect(css).toMatch(/\.mux-picker-option\s*\{[^}]*border-bottom:/);
  expect(css).toMatch(/\.mux-picker-option\s*\{[^}]*background:\s*transparent/);
  expect(css).toMatch(/\.mux-model-form,[\s\S]*?\.mux-mcp-form\s*\{[^}]*padding:\s*0;[^}]*background:\s*transparent/);
  expect(css).toMatch(/\.mux-asset-review > section\s*\{[^}]*border-bottom:[^}]*background:\s*transparent/);
});

it("keeps destructive reviews on the shared compact frame", () => {
  expect(css).toMatch(/\.mux-review-dialog-danger \.mux-dialog-shell-header\s*\{[^}]*min-height:\s*var\(--mux-dialog-header-height\)/);
  expect(css).toMatch(/\.mux-review-dialog-danger \.mux-dialog-shell-footer\s*\{[^}]*min-height:\s*var\(--mux-dialog-footer-height\)/);
});
