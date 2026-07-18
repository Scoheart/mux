import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/ModelsView.tsx"), "utf8");
const css = await readFile(resolve(process.cwd(), "src/index.css"), "utf8");

it("maps Model cards to the shared resource interface", () => {
  const card = source.slice(source.indexOf("function ModelCard"), source.indexOf("function ModelInspector"));
  expect(card).toMatch(/<ResourceCard/);
  expect(card).toMatch(/identity=/);
  expect(card).toMatch(/configuration=/);
  expect(card).toMatch(/state=/);
  expect(card).toMatch(/impact=/);
  expect(card).toMatch(/凭据已保存/);
  expect(card).not.toMatch(/<IconButton/);
});

it("uses neutral protocol classification without a card color rail", () => {
  expect(source).not.toMatch(/className="mux-model-protocol-dot" data-protocol=\{profile\.protocol\}/);
  expect(css).not.toMatch(/\.mux-model-card::before/);
  expect(css).not.toMatch(/\.mux-model-card\[data-protocol=/);
});

it("preserves Keychain presence-only rendering", () => {
  expect(source).toMatch(/profile\.credential_saved \? "凭据已保存" : "无已存凭据"/);
  expect(source).not.toMatch(/credential_saved\s*\}\s*<code/);
});
