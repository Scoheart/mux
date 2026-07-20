import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const source = await readFile(resolve(process.cwd(), "src/components/ModelsView.tsx"), "utf8");
const agentSource = await readFile(resolve(process.cwd(), "src/components/AgentView.tsx"), "utf8");
const css = await readFile(resolve(process.cwd(), "src/index.css"), "utf8");

it("maps Model cards to the shared resource interface", () => {
  const card = source.slice(source.indexOf("function ModelCard"), source.indexOf("function ModelInspector"));
  expect(card).toMatch(/<ResourceCard/);
  expect(card).toMatch(/identity=/);
  expect(card).toMatch(/configuration=/);
  expect(card).toMatch(/state=/);
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

it("supports env-only Agent metadata without storing a secret value", () => {
  expect(source).toMatch(/API Key 环境变量/);
  expect(source).toMatch(/env_key: draft\.env_key\?\.trim\(\) \|\| undefined/);
  expect(source).toMatch(/变量值由启动环境提供，不从 Keychain 导出/);
  expect(agentSource).toMatch(/modelAgent\.credential_mode === "environment-reference"/);
  expect(agentSource).toMatch(/ENV · \$\{profile\.env_key\}/);
  expect(agentSource).toMatch(/需要 ENV/);
});

it("routes profile lifecycle through central asset plans", () => {
  expect(source).toMatch(/consumptionState\.planUpdate/);
  expect(source).toMatch(/consumptionState\.planDelete/);
  expect(source).not.toMatch(/saveModelProfile|deleteModelProfile|applyModelProfile/);
});

it("keeps the top-level Models workspace asset-only", () => {
  expect(source).toMatch(/searchPlaceholder="搜索模型资产"/);
  expect(source).toMatch(/label="模型资产"/);
  expect(source).not.toMatch(/listModelAgents|planForAgent|planForAsset/);
  expect(source).not.toMatch(/AssetConsumerDialog|管理 Agent|Agent 模型|使用中|未使用/);
  expect(css).not.toMatch(/\.mux-model-agent-grid/);
});

it("owns multi-Model display and switching inside the Agent panel", () => {
  expect(agentSource).toMatch(/title="Models"/);
  expect(agentSource).toMatch(/可保留多个并切换当前模型/);
  expect(agentSource).toMatch(/planModelEnabled/);
  expect(agentSource).toMatch(/planActiveModel/);
  expect(agentSource).toMatch(/设为当前/);
  expect(css).toMatch(/\.mux-consumption-activate/);
});
