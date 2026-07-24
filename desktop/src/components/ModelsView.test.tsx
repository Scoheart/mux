import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import * as api from "../lib/api";
import * as modelsDev from "../lib/modelsDev";
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

vi.mock("../lib/modelsDev", async () => {
  const actual = await vi.importActual<typeof import("../lib/modelsDev")>("../lib/modelsDev");
  return {
    ...actual,
    getCachedModelsDevMetadata: vi.fn(() => ({})),
    loadModelsDevMetadata: vi.fn(async () => ({})),
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
  vi.mocked(modelsDev.getCachedModelsDevMetadata).mockReturnValue({});
  vi.mocked(modelsDev.loadModelsDevMetadata).mockResolvedValue({});
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
  expect(card).toMatch(/state=/);
  expect(card).toMatch(/<strong title=\{displayName\}>\{displayName\}<\/strong>/);
  expect(card).toMatch(/title=\{profile\.model\}>\{profile\.model\}<\/code>/);
  expect(card).toMatch(/mux-model-card-protocol/);
  expect(card).toMatch(/profile\.reasoning|metadata\?\.reasoning/);
  expect(card).not.toMatch(/profile\.base_url|Keychain|无凭据/);
  expect(card).not.toMatch(/可用|生效中|已托管/);
  expect(card).not.toMatch(/<IconButton/);
});

it("enriches an OpenRouter card without overriding user token limits", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "qwen-profile",
    name: "OpenRouter",
    provider: "openrouter",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "qwen/qwen3",
    context_window: 200_000,
    max_output_tokens: 16_000,
    reasoning: false,
    catalog_key: "openrouter/qwen/qwen3",
    credential_saved: true,
  }]);
  vi.mocked(modelsDev.loadModelsDevMetadata).mockResolvedValue({
    "qwen-profile": {
      name: "Qwen3",
      description: "A capable reasoning and tool-use model.",
      contextWindow: 262_144,
      maxOutputTokens: 32_768,
      reasoning: true,
      toolCall: true,
      inputCost: 0.2,
      outputCost: 0.8,
    },
  });

  render(
    <ToastProvider>
      <ModelsView />
    </ToastProvider>,
  );

  const card = await screen.findByRole("button", { name: "打开模型 OpenRouter 详情" });
  await waitFor(() => expect(within(card).getByText("Qwen3")).toBeVisible());
  expect(within(card).getByText("qwen/qwen3")).toBeVisible();
  expect(within(card).getByText("OpenRouter")).toBeVisible();
  expect(within(card).getByText("200K 上下文")).toBeVisible();
  expect(within(card).getByText("16K 输出")).toBeVisible();
  expect(within(card).queryByText("262.1K 上下文")).not.toBeInTheDocument();
  expect(within(card).getByText("$0.2/M 输入")).toBeVisible();
  expect(within(card).getByText("推理")).toBeVisible();
  expect(within(card).getByText("Tools")).toBeVisible();
  expect(within(card).getByText("参考 models.dev")).toBeVisible();
});

it("uses neutral protocol classification without a card color rail", () => {
  expect(source).not.toMatch(/className="mux-model-protocol-dot" data-protocol=\{profile\.protocol\}/);
  expect(css).not.toMatch(/\.mux-model-card::before/);
  expect(css).not.toMatch(/\.mux-model-card\[data-protocol=/);
});

it("uses protocol as the only sidebar classification and keeps its filtering behavior", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([
    {
      id: "anthropic-model",
      name: "Anthropic Model",
      provider: "custom",
      protocol: "anthropic-messages",
      base_url: "https://anthropic.example.test",
      model: "claude",
      reasoning: false,
      catalog_key: "custom/claude",
      credential_saved: false,
    },
    {
      id: "responses-model",
      name: "Responses Model",
      provider: "openai",
      protocol: "openai-responses",
      base_url: "https://api.openai.com/v1",
      model: "gpt-responses",
      reasoning: true,
      catalog_key: "openai/gpt-responses",
      credential_saved: true,
    },
    {
      id: "completions-model",
      name: "Completions Model",
      provider: "openrouter",
      protocol: "openai-completions",
      base_url: "https://openrouter.ai/api/v1",
      model: "openrouter/free",
      reasoning: false,
      catalog_key: "openrouter/free",
      credential_saved: false,
    },
  ]);
  const user = userEvent.setup();
  const view = render(
    <ToastProvider>
      <ModelsView />
    </ToastProvider>,
  );

  await screen.findByRole("button", { name: "打开模型 Completions Model 详情" });
  const sidebarElement = view.container.querySelector(".mux-workspace-sidebar");
  expect(sidebarElement).not.toBeNull();
  const sidebar = within(sidebarElement as HTMLElement);

  expect(sidebarElement?.querySelectorAll(".mux-sidebar-section")).toHaveLength(1);
  expect(sidebar.getByText("协议")).toBeVisible();
  expect(sidebar.queryByText("Provider")).not.toBeInTheDocument();
  expect(sidebar.queryByText("全部 Provider")).not.toBeInTheDocument();

  await user.click(sidebar.getByRole("button", { name: /OpenAI Chat Completions/ }));
  expect(screen.getByRole("button", { name: "打开模型 Completions Model 详情" })).toBeVisible();
  expect(screen.queryByRole("button", { name: "打开模型 Anthropic Model 详情" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "打开模型 Responses Model 详情" })).not.toBeInTheDocument();

  await user.click(sidebar.getByRole("button", { name: /全部协议/ }));
  expect(screen.getByRole("button", { name: "打开模型 Anthropic Model 详情" })).toBeVisible();
  expect(screen.getByRole("button", { name: "打开模型 Responses Model 详情" })).toBeVisible();
});

it("keeps Keychain presence-only rendering in the Inspector", () => {
  expect(source).toMatch(/profile\.credential_saved \? "已保存到 Keychain" : "未保存"/);
  expect(source).not.toMatch(/credential_saved\s*\}\s*<code/);
});

it("renders model details as one continuous field list without section cards", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "qwen3-7-plus",
    name: "Qwen3 7 Plus",
    provider: "max-ai",
    model_vendor: "Qwen",
    protocol: "openai-responses",
    base_url: "https://models.example.test/v1",
    model: "qwen3.7-plus",
    env_key: "MAX_AI_API_KEY",
    reasoning: true,
    catalog_key: "max-ai/qwen3.7-plus",
    credential_saved: true,
  }]);
  const user = userEvent.setup();

  render(
    <ToastProvider>
      <ModelsView />
    </ToastProvider>,
  );

  await user.click(await screen.findByRole("button", { name: "打开模型 Qwen3 7 Plus 详情" }));

  const inspector = screen.getByRole("complementary", { name: "Qwen3 7 Plus 详情" });
  const fields = within(inspector).getByRole("region", { name: "模型详情字段" });
  expect(fields).toHaveClass("mux-model-inspector-fields");
  expect(fields.querySelectorAll(".mux-inspector-field")).toHaveLength(10);
  expect(fields.querySelectorAll(".mux-inspector-section")).toHaveLength(0);
  for (const label of [
    "Provider",
    "模型开发商",
    "协议",
    "推理",
    "模型 ID",
    "Base URL",
    "环境变量",
    "API Key",
    "Profile ID",
    "Catalog Key",
  ]) {
    expect(within(fields).getByText(label)).toBeVisible();
  }
  expect(within(fields).getByText("已保存到 Keychain")).toBeVisible();
  expect(within(fields).getByRole("button", { name: "复制 Profile ID" })).toBeVisible();
  expect(within(inspector).queryByRole("heading", { name: "资产信息" })).not.toBeInTheDocument();
  expect(within(inspector).queryByRole("heading", { name: "接口" })).not.toBeInTheDocument();
  expect(within(inspector).queryByRole("heading", { name: "技术详情" })).not.toBeInTheDocument();
  expect(within(inspector).getByRole("button", { name: "删除" })).toBeVisible();
  expect(within(inspector).getByRole("button", { name: "编辑" })).toBeVisible();
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
  expect(provider).toHaveTextContent("OpenRouter");
  expect(screen.queryByLabelText("自定义 Provider ID")).not.toBeInTheDocument();
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
  expect(screen.getByRole("combobox", { name: "Provider" })).toHaveTextContent("OpenRouter");
  expect(screen.queryByLabelText("自定义 Provider ID")).not.toBeInTheDocument();
});

it("infers a known Provider from Base URL while keeping later manual selection authoritative", async () => {
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
  expect(provider).toHaveTextContent("OpenAI");
  expect(baseUrl).toHaveValue("https://openrouter.ai/api/v1");
});

it("infers OpenRouter when editing a historical profile with only its Base URL", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "historical-openrouter",
    name: "Historical OpenRouter",
    provider: "",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "openrouter/free",
    reasoning: false,
    catalog_key: "openrouter/free",
    credential_saved: false,
  }]);
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await user.click(await screen.findByRole("button", { name: "打开模型 Historical OpenRouter 详情" }));
  await user.click(screen.getByRole("button", { name: "编辑" }));

  const provider = await screen.findByRole("combobox", { name: "Provider" });
  await waitFor(() => expect(provider).toHaveTextContent("OpenRouter"));
  expect(api.inferModelProvider).toHaveBeenCalledWith("https://openrouter.ai/api/v1");
  expect(screen.queryByLabelText("自定义 Provider ID")).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(consumptionState.planUpdate).toHaveBeenCalledWith(
    expect.objectContaining({
      existing_id: "historical-openrouter",
      profile: expect.objectContaining({ provider: "openrouter" }),
    }),
  ));
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
  const editor = screen.getByRole("complementary", { name: "编辑模型 详情" });
  expect(editor).toBeVisible();
  expect(within(editor).getByRole("heading", { name: "编辑模型" })).toBeVisible();
  expect(within(editor).getByText("API Key 保存在 macOS Keychain。")).toBeVisible();
  expect(within(editor).queryByText("编辑 · OpenAI Responses")).not.toBeInTheDocument();
  expect(screen.queryByRole("dialog", { name: "编辑模型" })).not.toBeInTheDocument();

  const provider = screen.getByRole("combobox", { name: "Provider" });
  expect(provider).toHaveTextContent("Custom Provider…");
  expect(screen.getByLabelText("自定义 Provider ID")).toHaveValue("custom");
  await chooseFormSelect(user, "Provider", "OpenRouter");

  expect(screen.getByLabelText("Base URL")).toHaveValue("https://gateway.example.test/v1");
  expect(provider).toHaveTextContent("OpenRouter");
  expect(screen.queryByLabelText("自定义 Provider ID")).not.toBeInTheDocument();
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
  expect(agentSource).toMatch(/同一时间使用其中一个/);
  expect(agentSource).not.toMatch(/planModelEnabled/);
  expect(agentSource).toMatch(/planActiveModel/);
  expect(agentSource).toMatch(/toggleKind="current"/);
  expect(agentSource).not.toMatch(/设为当前/);
});
