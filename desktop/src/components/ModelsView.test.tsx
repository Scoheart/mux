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
    inferModelProvider: vi.fn(),
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
  vi.mocked(api.inferModelProvider).mockImplementation(async (baseUrl) => {
    const host = (() => {
      try {
        return new URL(baseUrl).hostname;
      } catch {
        return "";
      }
    })();
    if (host === "openrouter.ai") return "openrouter";
    if (host === "api.openai.com") return "openai";
    return "custom";
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

async function chooseFormSelect(
  user: ReturnType<typeof userEvent.setup>,
  label: string,
  option: string,
) {
  const combobox = screen.getByRole("combobox", { name: label });
  await user.click(combobox);
  await user.click(screen.getByRole("option", { name: option }));
  return combobox;
}

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

it("uses the unified Provider select with a conditional custom ID field", () => {
  const providerField = source.slice(
    source.indexOf("<span>Provider</span>"),
    source.indexOf("<span>名称（可选）</span>"),
  );
  expect(providerField).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel="Provider"/);
  expect(providerField).not.toMatch(/自动识别/);
  expect(providerField).toMatch(/placeholder="根据 Base URL 识别"/);
  expect(providerField).toMatch(/\{ value: CUSTOM_PROVIDER_OPTION, label: "Custom Provider…" \}/);
  expect(providerField).toMatch(/providerSelection === CUSTOM_PROVIDER_OPTION/);
  expect(providerField).toMatch(/aria-label="自定义 Provider ID"/);
  expect(providerField).not.toMatch(/<select|datalist/);
  expect(providerField).not.toMatch(/<small>/);
  const protocolField = source.slice(
    source.indexOf("<span>协议</span>"),
    source.indexOf("<span>Base URL</span>"),
  );
  const baseUrlField = source.slice(
    source.indexOf("<span>Base URL</span>"),
    source.indexOf("<span>模型 ID</span>"),
  );
  expect(protocolField).not.toMatch(/<small>/);
  expect(baseUrlField).not.toMatch(/<small>/);
  expect(source).toMatch(/provider: draft\.provider\.trim\(\)/);
});

it("uses one custom select surface for Provider and protocol", () => {
  expect(source).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel="Provider"/);
  expect(source).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel="协议"/);
  expect(source).not.toMatch(/<select/);
  expect(css).toMatch(/\.mux-form-select-menu/);
  expect(css).toMatch(/background: var\(--surface-popover\)/);
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
  const provider = screen.getByRole("combobox", { name: "Provider" });
  const baseUrl = screen.getByLabelText("Base URL");

  await chooseFormSelect(user, "Provider", "OpenRouter");
  expect(baseUrl).toHaveValue("https://openrouter.ai/api/v1");

  await chooseFormSelect(user, "Provider", "OpenAI");
  expect(baseUrl).toHaveValue("https://api.openai.com/v1");

  await chooseFormSelect(user, "Provider", "Custom Provider…");
  expect(baseUrl).toHaveValue("");
  await user.type(screen.getByLabelText("自定义 Provider ID"), "my-gateway");

  await chooseFormSelect(user, "Provider", "OpenRouter");
  expect(baseUrl).toHaveValue("https://openrouter.ai/api/v1");
  expect(screen.queryByLabelText("自定义 Provider ID")).not.toBeInTheDocument();

  await user.clear(baseUrl);
  await chooseFormSelect(user, "Provider", "OpenAI");
  expect(baseUrl).toHaveValue("");

  await user.type(baseUrl, "https://gateway.example.test/v1");
  await chooseFormSelect(user, "Provider", "OpenRouter");
  expect(baseUrl).toHaveValue("https://gateway.example.test/v1");
  expect(provider).toHaveTextContent("Custom Provider…");
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
  await chooseFormSelect(user, "Provider", "OpenRouter");

  expect(baseUrl).toHaveValue("https://gateway.example.test/v1");
  expect(screen.getByRole("combobox", { name: "Provider" })).toHaveTextContent("Custom Provider…");
  expect(screen.getByLabelText("自定义 Provider ID")).toHaveValue("custom");
});

it("infers a known Provider from Base URL and gives URL detection precedence", async () => {
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
  const provider = screen.getByRole("combobox", { name: "Provider" });
  const baseUrl = screen.getByLabelText("Base URL");

  expect(provider).toHaveTextContent("根据 Base URL 识别");
  await chooseFormSelect(user, "Provider", "OpenAI");
  await user.clear(baseUrl);
  await user.type(baseUrl, "https://openrouter.ai/api/v1");

  await waitFor(() => expect(provider).toHaveTextContent("OpenRouter"));
  expect(api.inferModelProvider).toHaveBeenLastCalledWith("https://openrouter.ai/api/v1");

  await chooseFormSelect(user, "Provider", "OpenAI");
  await waitFor(() => expect(provider).toHaveTextContent("OpenRouter"));
  expect(baseUrl).toHaveValue("https://openrouter.ai/api/v1");
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
  await waitFor(() => expect(screen.getByRole("combobox", { name: "Provider" })).toBeVisible());
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.getByRole("complementary", { name: "Existing Model 详情" })).toBeVisible();
  expect(screen.queryByRole("dialog", { name: "编辑模型" })).not.toBeInTheDocument();

  const provider = screen.getByRole("combobox", { name: "Provider" });
  expect(provider).toHaveTextContent("Custom Provider…");
  expect(screen.getByLabelText("自定义 Provider ID")).toHaveValue("custom");
  await chooseFormSelect(user, "Provider", "OpenRouter");

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
  await chooseFormSelect(user, "Provider", "Custom Provider…");
  await user.type(screen.getByLabelText("自定义 Provider ID"), "my-gateway");
  await user.type(screen.getByPlaceholderText("https://api.example.com/v1"), "https://models.example.test/v1/");
  await user.type(screen.getByPlaceholderText("model-name"), "custom-model");
  await user.click(screen.getByRole("button", { name: "保存" }));

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
