import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import * as api from "../lib/api";
import { ModelsView } from "./ModelsView";
import { ToastProvider } from "./Toast";

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return {
    ...actual,
    listModelProfiles: vi.fn(),
    listModelProviders: vi.fn(),
  };
});

const source = await readFile(resolve(process.cwd(), "src/components/ModelsView.tsx"), "utf8");
const agentSource = await readFile(resolve(process.cwd(), "src/components/AgentView.tsx"), "utf8");
const css = await readFile(resolve(process.cwd(), "src/index.css"), "utf8");

beforeEach(() => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([]);
  vi.mocked(api.listModelProviders).mockResolvedValue([
    {
      id: "openrouter",
      name: "OpenRouter",
      default_base_url: "https://openrouter.ai/api/v1",
    },
    {
      id: "openai",
      name: "OpenAI",
      default_base_url: "https://api.openai.com/v1",
    },
    { id: "custom", name: "Custom Provider", default_base_url: null },
  ]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("maps Model cards to the shared resource interface", () => {
  const card = source.slice(source.indexOf("function ModelCard"), source.indexOf("function ModelInspector"));
  expect(card).toMatch(/<ResourceCard/);
  expect(card).toMatch(/identity=/);
  expect(card).toMatch(/configuration=/);
  expect(card).not.toMatch(/state=/);
  expect(card).toMatch(/<strong title=\{providerName\}>\{providerName\}<\/strong>/);
  expect(card).toMatch(/title=\{profile\.model\}>\{profile\.model\}<\/code>/);
  expect(card).toMatch(/title=\{protocolLabel\(profile\.protocol\)\}/);
  expect(card).not.toMatch(/title=\{profile\.name\}|profile\.reasoning|profile\.base_url|Keychain|无凭据/);
  expect(card).not.toMatch(/可用|生效中|已托管/);
  expect(card).not.toMatch(/<IconButton/);
});

it("uses neutral protocol classification without a card color rail", () => {
  expect(source).not.toMatch(/className="mux-model-protocol-dot" data-protocol=\{profile\.protocol\}/);
  expect(css).not.toMatch(/\.mux-model-card::before/);
  expect(css).not.toMatch(/\.mux-model-card\[data-protocol=/);
});

it("keeps Keychain presence-only rendering in the Inspector", () => {
  expect(source).toMatch(/profile\.credential_saved \? "已保存到 Keychain" : "未保存"/);
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

it("allows an auto-detected or custom Provider while keeping protocol explicit", () => {
  const providerField = source.slice(
    source.indexOf("<span>Provider</span>"),
    source.indexOf("<span>名称（可选）</span>"),
  );
  expect(providerField).toMatch(/<input\s+[\s\S]*?list="mux-model-provider-options"/);
  expect(providerField).toMatch(/<datalist id="mux-model-provider-options">/);
  expect(providerField).toMatch(/不在列表中可直接输入自定义 Provider ID/);
  expect(providerField).not.toMatch(/<select/);
  expect(source).toMatch(/provider: draft\.provider\.trim\(\)/);
  expect(source).toMatch(/此项不会根据 Base URL 自动识别/);
});

it("fills and switches a known Provider default until the Base URL is edited", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await waitFor(() => expect(screen.getByRole("button", { name: "新建模型" })).toBeEnabled());
  await user.click(screen.getByRole("button", { name: "新建模型" }));
  await waitFor(() => expect(screen.getByRole("heading", { name: "新建模型" })).toHaveFocus());
  const provider = screen.getByLabelText("Provider");
  const baseUrl = screen.getByLabelText("Base URL");

  await user.type(provider, "openrouter");
  expect(baseUrl).toHaveValue("https://openrouter.ai/api/v1");

  await user.clear(provider);
  await user.type(provider, "openai");
  expect(baseUrl).toHaveValue("https://api.openai.com/v1");

  await user.clear(provider);
  await user.type(provider, "custom");
  expect(baseUrl).toHaveValue("");

  await user.clear(provider);
  await user.type(provider, "openrouter");
  expect(baseUrl).toHaveValue("https://openrouter.ai/api/v1");

  await user.clear(baseUrl);
  await user.clear(provider);
  await user.type(provider, "openai");
  expect(baseUrl).toHaveValue("");

  await user.type(baseUrl, "https://gateway.example.test/v1");
  await user.clear(provider);
  await user.type(provider, "openrouter");
  expect(baseUrl).toHaveValue("https://gateway.example.test/v1");
});

it("does not overwrite a Base URL that was entered before the Provider", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await waitFor(() => expect(screen.getByRole("button", { name: "新建模型" })).toBeEnabled());
  await user.click(screen.getByRole("button", { name: "新建模型" }));
  await waitFor(() => expect(screen.getByRole("heading", { name: "新建模型" })).toHaveFocus());
  const baseUrl = screen.getByLabelText("Base URL");

  await user.type(baseUrl, "https://gateway.example.test/v1");
  await user.type(screen.getByLabelText("Provider"), "openrouter");

  expect(baseUrl).toHaveValue("https://gateway.example.test/v1");
});

it("does not overwrite the Base URL of an existing model", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "existing-model",
    name: "Existing Model",
    provider: "custom",
    protocol: "openai-responses",
    base_url: "https://gateway.example.test/v1",
    model: "existing-model-id",
    reasoning: false,
    catalog_key: "custom/existing-model-id",
    credential_saved: false,
  }]);
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await user.click(await screen.findByRole("button", { name: "打开模型 Existing Model 详情" }));
  await user.click(screen.getByRole("button", { name: "编辑" }));
  await waitFor(() => expect(screen.getByRole("heading", { name: "编辑模型" })).toHaveFocus());

  const provider = screen.getByLabelText("Provider");
  await user.clear(provider);
  await user.type(provider, "openrouter");

  expect(screen.getByLabelText("Base URL")).toHaveValue("https://gateway.example.test/v1");
});

it("submits an arbitrary Provider ID through the central asset plan", async () => {
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "model-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await waitFor(() => expect(screen.getByRole("button", { name: "新建模型" })).toBeEnabled());
  await user.click(screen.getByRole("button", { name: "新建模型" }));
  await waitFor(() => expect(screen.getByRole("heading", { name: "新建模型" })).toHaveFocus());
  await user.type(screen.getByLabelText("Provider"), "my-gateway");
  await user.type(screen.getByPlaceholderText("https://api.example.com/v1"), "https://models.example.test/v1/");
  await user.type(screen.getByPlaceholderText("model-name"), "custom-model");
  await user.click(screen.getByRole("button", { name: "审阅更改" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledTimes(1));
  expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "model",
    profile: expect.objectContaining({
      provider: "my-gateway",
      base_url: "https://models.example.test/v1",
      model: "custom-model",
    }),
  }));
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
