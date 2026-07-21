import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/RegistryView.tsx"), "utf8");

it("maps MCP cards to the shared resource interface", () => {
  const card = source.slice(source.indexOf("function RegistryCard"), source.indexOf("function RegistryDetail"));
  expect(card).toMatch(/<ResourceCard/);
  expect(card).toMatch(/identity=/);
  expect(card).toMatch(/configuration=/);
  expect(card).not.toMatch(/state=/);
  expect(card).not.toMatch(/impact=/);
  expect(card).not.toMatch(/被覆盖|生效中/);
  expect(card).toMatch(/transportOf\(entry\)\.toUpperCase\(\)/);
  expect(card).not.toMatch(/<IconButton/);
});

it("keeps mutations and redacted configuration in the Inspector", () => {
  const inspector = source.slice(source.indexOf("function RegistryDetail"));
  expect(inspector).toMatch(/redactSensitiveConfig\(entry\.config\)/);
  expect(inspector).toMatch(/onCopy/);
  expect(inspector).toMatch(/onEdit/);
  expect(inspector).toMatch(/onDelete/);
});

it("routes deletion through the central lifecycle planner", () => {
  expect(source).toMatch(/consumptionState\.planDelete/);
  expect(source).not.toMatch(/forgetEntry|deleteMcp|uninstall/);
});
