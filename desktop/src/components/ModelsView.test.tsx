import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
    listModelProviderInstances: vi.fn(),
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
      base_url: "https://openrouter.ai/api/v1",
      protocols: {
        "openai-responses": { endpoint_path: "/responses" },
      },
      default_base_url: "https://openrouter.ai/api/v1",
      default_protocol: "openai-responses",
      additional_endpoints: [],
      category: "gateway",
    },
    {
      id: "openai",
      name: "OpenAI",
      base_url: "https://api.openai.com/v1",
      protocols: {
        "openai-responses": { endpoint_path: "/responses" },
      },
      default_base_url: "https://api.openai.com/v1",
      default_protocol: "openai-responses",
      additional_endpoints: [],
      category: "official",
    },
    {
      id: "alibaba-coding-plan",
      name: "Alibaba Coding Plan (Global)",
      base_url: "https://coding-intl.dashscope.aliyuncs.com",
      protocols: {
        "anthropic-messages": { endpoint_path: "/apps/anthropic/v1/messages" },
        "openai-completions": { endpoint_path: "/v1/chat/completions" },
      },
      default_base_url: "https://coding-intl.dashscope.aliyuncs.com/v1",
      default_protocol: "openai-completions",
      additional_endpoints: [{
        protocol: "anthropic-messages",
        base_url: "https://coding-intl.dashscope.aliyuncs.com/apps/anthropic",
      }],
      category: "official",
    },
    {
      id: "ollama",
      name: "Ollama",
      base_url: "http://localhost:11434/v1",
      protocols: {
        "openai-completions": { endpoint_path: "/chat/completions" },
      },
      default_base_url: "http://localhost:11434/v1",
      default_protocol: "openai-completions",
      additional_endpoints: [],
      category: "local",
    },
    {
      id: "custom",
      name: "Custom Provider",
      base_url: "",
      protocols: {
        "openai-responses": { endpoint_path: "/responses" },
      },
      default_base_url: null,
      default_protocol: "openai-responses",
      additional_endpoints: [],
      category: "custom",
    },
  ]);
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([]);
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

async function openProviderTemplate(
  user: ReturnType<typeof userEvent.setup>,
  provider: string,
) {
  await user.click(screen.getByRole("button", { name: "添加 Provider" }));
  const catalog = screen.getByRole("dialog", { name: "添加 Provider" });
  expect(catalog).toBeVisible();
  await user.click(within(catalog).getByRole("radio", { name: new RegExp(provider) }));
  await user.click(within(catalog).getByRole("button", { name: "使用此模板" }));
  await waitFor(() =>
    expect(screen.getByRole("heading", { name: "新建 Provider" })).toHaveFocus()
  );
}

it("maps Models to one compact, scannable list", () => {
  const list = source.slice(source.indexOf("function ModelList"), source.indexOf("function ModelInspector"));
  expect(list).toMatch(/className="mux-asset-list mux-model-list" role="list"/);
  expect(list).toMatch(/role="listitem"/);
  expect(list).toMatch(/className="mux-asset-list-row mux-model-list-row"/);
  expect(list).toMatch(/<strong title=\{displayName\}>\{displayName\}<\/strong>/);
  expect(list).toMatch(/title=\{profile\.model\}>\{profile\.model\}<\/code>/);
  expect(list).toMatch(/protocolLabel\(profile\.protocol\)/);
  expect(list).toMatch(/profile\.credential_saved/);
  expect(list).not.toMatch(/<ResourceCard/);
  expect(source).toMatch(/className="mux-models-workspace"/);
  expect(source).not.toMatch(/<ResourceTabs/);
});

it("keeps the Models workspace visible through loading, error recovery, and empty states", async () => {
  vi.mocked(api.listModelProfiles).mockReturnValueOnce(new Promise<never>(() => undefined));
  const loadingView = render(
    <ToastProvider>
      <ModelsView />
    </ToastProvider>,
  );

  expect(screen.getByRole("status", { name: "正在读取模型资产" })).toBeVisible();
  expect(screen.getByRole("button", { name: "添加 Provider" })).toBeVisible();
  loadingView.unmount();

  vi.mocked(api.listModelProfiles)
    .mockRejectedValueOnce(new Error("model registry unavailable"))
    .mockResolvedValueOnce([]);
  const user = userEvent.setup();
  const errorView = render(
    <ToastProvider>
      <ModelsView />
    </ToastProvider>,
  );

  expect(await screen.findByText("读取模型资产失败")).toBeVisible();
  expect(screen.getByRole("alert")).toHaveTextContent("model registry unavailable");
  await user.click(screen.getByRole("button", { name: "重试" }));
  expect(await screen.findByText("暂无模型资产")).toBeVisible();
  expect(screen.getByText("新建一个可复用的模型配置。")).toBeVisible();
  errorView.unmount();
});

it("recovers a no-match Models search without losing the compact index", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "qwen-profile",
    name: "Qwen Profile",
    provider: "openrouter",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "qwen/qwen3",
    catalog_key: "openrouter/qwen/qwen3",
    credential_saved: true,
  }]);
  const user = userEvent.setup();

  render(
    <ToastProvider>
      <ModelsView />
    </ToastProvider>,
  );

  expect(await screen.findByRole("list", { name: "模型资产" })).toBeVisible();
  await user.type(screen.getByPlaceholderText("搜索名称、Model ID 或 Provider"), "not-present");
  expect(screen.getByText("没有匹配项")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "清除筛选" }));
  expect(screen.getByRole("button", { name: "打开模型 Qwen Profile 详情" })).toBeVisible();
});

it("enriches an OpenRouter list row without overriding user token limits", async () => {
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "qwen-profile",
    name: "OpenRouter",
    provider: "openrouter",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "qwen/qwen3",
    context_window: 200_000,
    max_output_tokens: 16_000,
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
  expect(within(card).getByText("200K")).toBeVisible();
  expect(within(card).queryByText("262.1K")).not.toBeInTheDocument();
  expect(within(card).queryByText("$0.2/M 输入")).not.toBeInTheDocument();
  expect(within(card).queryByText("推理")).not.toBeInTheDocument();
  expect(within(card).queryByText("Tools")).not.toBeInTheDocument();
  expect(within(card).queryByText(/models\.dev/)).not.toBeInTheDocument();
});

it("uses neutral protocol classification without a card color rail", () => {
  expect(source).not.toMatch(/className="mux-model-protocol-dot" data-protocol=\{profile\.protocol\}/);
  expect(css).not.toMatch(/\.mux-model-card::before/);
  expect(css).not.toMatch(/\.mux-model-card\[data-protocol=/);
});

it("keeps the sidebar limited to the model library and configured Providers", async () => {
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([
    {
      id: "openai-personal",
      name: "OpenAI Personal",
      provider: "openai",
      base_url: "https://api.openai.com",
      protocols: { "openai-responses": { endpoint_path: "/v1/responses" } },
      credential_saved: true,
      model_count: 1,
    },
    {
      id: "openrouter-personal",
      name: "OpenRouter Personal",
      provider: "openrouter",
      base_url: "https://openrouter.ai",
      protocols: { "openai-completions": { endpoint_path: "/api/v1/chat/completions" } },
      credential_saved: false,
      model_count: 1,
    },
  ]);
  vi.mocked(api.listModelProfiles).mockResolvedValue([
    {
      id: "responses-model",
      name: "Responses Model",
      provider_id: "openai-personal",
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
      provider_id: "openrouter-personal",
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

  expect(sidebarElement?.querySelectorAll(".mux-sidebar-section")).toHaveLength(2);
  expect(sidebar.getByText("模型库")).toBeVisible();
  expect(sidebar.getByText("My Providers")).toBeVisible();
  expect(sidebar.getByText("全部模型")).toBeVisible();
  expect(sidebar.queryByText("协议")).not.toBeInTheDocument();
  expect(sidebar.queryByText("全部协议")).not.toBeInTheDocument();

  const openAiProvider = sidebar.getByRole("button", { name: /OpenAI Personal/ });
  expect(openAiProvider.querySelector('[data-provider-icon="openai"]')).toBeInTheDocument();
  await user.click(openAiProvider);
  expect(screen.getByRole("button", { name: "打开模型 Responses Model 详情" })).toBeVisible();
  expect(screen.queryByRole("button", { name: "打开模型 Completions Model 详情" })).not.toBeInTheDocument();
  expect(view.container.querySelector(
    '.mux-model-provider-banner [data-provider-icon="openai"]',
  )).toBeInTheDocument();

  await user.click(sidebar.getByRole("button", { name: /全部模型/ }));
  expect(screen.getByRole("button", { name: "打开模型 Responses Model 详情" })).toBeVisible();
  expect(screen.getByRole("button", { name: "打开模型 Completions Model 详情" })).toBeVisible();
});

it("keeps the built-in Provider Catalog unfiltered, searchable, and keyboard-selectable", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await user.click(await screen.findByRole("button", { name: "添加 Provider" }));
  const catalog = screen.getByRole("dialog", { name: "添加 Provider" });
  expect(catalog).toBeVisible();
  expect(within(catalog).getByRole("radio", { name: /OpenRouter/ })).toHaveAttribute("aria-checked", "true");
  expect(within(catalog).getByRole("radio", { name: /OpenAI/ })).toBeVisible();
  expect(within(catalog).getByRole("radio", { name: /Ollama/ })).toBeVisible();
  expect(within(catalog).getByRole("radio", { name: /Custom Provider/ })).toBeVisible();
  expect(within(catalog).getAllByRole("radio")[0]).toHaveAccessibleName(/Custom Provider/);
  expect(within(catalog).getByRole("radio", { name: /OpenAI/ })
    .querySelector('[data-provider-icon="openai"]')).toBeInTheDocument();
  expect(within(catalog).getByRole("radio", { name: /Custom Provider/ })
    .querySelector('[data-provider-icon="fallback"]')).toBeInTheDocument();
  expect(within(catalog).queryByText("Local endpoint")).not.toBeInTheDocument();
  for (const label of ["Official", "Gateway", "Local", "Custom"]) {
    expect(within(catalog).queryByText(label)).not.toBeInTheDocument();
  }
  expect(catalog.querySelector(".mux-provider-catalog-categories")).not.toBeInTheDocument();
  expect(within(catalog).queryByRole("button", { name: "全部" })).not.toBeInTheDocument();

  const search = within(catalog).getByRole("searchbox", { name: "搜索 Provider" });
  await user.type(search, "apps/anthropic");
  expect(within(catalog).getAllByRole("radio")).toHaveLength(1);
  expect(within(catalog).getByRole("radio", { name: /Alibaba Coding Plan/ })).toBeVisible();

  await user.clear(search);
  await user.type(search, "api.openai.com");
  expect(within(catalog).getAllByRole("radio")).toHaveLength(1);
  expect(within(catalog).getByRole("radio", { name: /OpenAI/ })).toBeVisible();
  expect(within(catalog).getByRole("button", { name: "使用此模板" })).toBeDisabled();

  await user.clear(search);
  expect(within(catalog).getAllByRole("radio")).toHaveLength(5);
  const openAi = within(catalog).getByRole("radio", { name: /OpenAI/ });
  openAi.focus();
  await user.keyboard("{Enter}");
  expect(openAi).toHaveAttribute("aria-checked", "true");
  expect(within(catalog).getByText("已选择 OpenAI")).toBeVisible();

  await user.click(within(catalog).getByRole("button", { name: "使用此模板" }));
  await waitFor(() => expect(screen.getByRole("heading", { name: "新建 Provider" })).toHaveFocus());
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.openai.com/v1");
  expect(screen.getByText("API Key")).toBeVisible();
  expect(source).toMatch(/<DialogShell\s+[\s\S]*?kind="picker"/);
  expect(source).not.toMatch(/ProviderCatalogDrawer|provider-catalog-drawer/);
});

it("centers compact Provider cards without sacrificing a dedicated selection column", () => {
  expect(css).toMatch(/\.mux-provider-catalog-item\s*\{[\s\S]*?min-height: 56px/);
  expect(css).toMatch(/grid-template-columns: 26px minmax\(0, 1fr\) 15px; align-items: center/);
  expect(css).toMatch(/\.mux-provider-catalog-copy\s*\{[\s\S]*?align-content: center; gap: 3px/);
  expect(css).toMatch(/\.mux-provider-catalog-grid\s*\{[\s\S]*?row-gap: 6px/);
  expect(css).toMatch(/\.mux-provider-catalog-copy code\s*\{[\s\S]*?text-overflow: ellipsis; white-space: nowrap/);
  expect(css).not.toMatch(/\.mux-provider-catalog-categories/);
});

it("keeps Keychain presence-only rendering in the Inspector", () => {
  expect(source).toMatch(/profile\.credential_saved \? t\("models\.keychainSaved"\) : t\("models\.keychainNotSaved"\)/);
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
  expect(fields.querySelectorAll(".mux-inspector-field")).toHaveLength(7);
  expect(fields.querySelectorAll(".mux-inspector-section")).toHaveLength(0);
  for (const label of [
    "模型提供商",
    "协议",
    "推理",
    "模型 ID",
    "完整请求 URL",
    "环境变量",
    "API Key",
  ]) {
    expect(within(fields).getByText(label)).toBeVisible();
  }
  expect(within(fields).getByText("已保存到 Keychain")).toBeVisible();
  for (const removed of ["模型开发商", "目录来源", "Profile ID", "Catalog Key"]) {
    expect(within(fields).queryByText(removed)).not.toBeInTheDocument();
  }
  expect(within(fields).queryByText(/models\.dev/)).not.toBeInTheDocument();
  expect(within(inspector).queryByRole("heading", { name: "资产信息" })).not.toBeInTheDocument();
  expect(within(inspector).queryByRole("heading", { name: "接口" })).not.toBeInTheDocument();
  expect(within(inspector).queryByRole("heading", { name: "技术详情" })).not.toBeInTheDocument();
  expect(within(inspector).getByRole("button", { name: "删除" })).toBeVisible();
  expect(within(inspector).getByRole("button", { name: "编辑" })).toBeVisible();
});

it("keeps environment and credential metadata on Provider rather than Model forms", () => {
  const modelDialog = source.slice(
    source.indexOf("function ModelProfileDialog"),
    source.indexOf("function ModelProviderDialog"),
  );
  const providerDialog = source.slice(source.indexOf("function ModelProviderDialog"));
  expect(modelDialog).not.toMatch(/models\.apiKeyEnv|models\.apiKey"\)/);
  expect(providerDialog).toMatch(/t\("models\.apiKeyEnv"\)/);
  expect(providerDialog).toMatch(/env_key: draft\.env_key\?\.trim\(\) \|\| undefined/);
  expect(agentSource).toMatch(/modelAgent\.credential_mode === "environment-reference"/);
  expect(agentSource).toMatch(/ENV · \$\{profile\.env_key\}/);
  expect(agentSource).toMatch(/需要 ENV/);
});

it("requires Model forms to select a persisted Provider relationship", () => {
  const profileDialog = source.indexOf("function ModelProfileDialog");
  const providerField = source.slice(
    source.indexOf('<span>{t("models.provider")}</span>', profileDialog),
    source.indexOf('<span>{t("models.protocol")}</span>', profileDialog),
  );
  expect(providerField).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel=\{t\("models\.provider"\)\}/);
  expect(providerField).not.toMatch(/自动识别/);
  expect(providerField).toMatch(/placeholder=\{t\("models\.providerPlaceholder"\)\}/);
  expect(providerField).toMatch(/options=\{providerInstances\.map/);
  expect(providerField).not.toMatch(/CUSTOM_PROVIDER_OPTION|customProviderId/);
  expect(providerField).not.toMatch(/<select|datalist/);
  expect(providerField).toMatch(/models\.providerRequired/);
  expect(source).toMatch(/provider_id: provider\.id/);
});

it("uses one custom select surface for model provider, protocol, and reasoning", () => {
  expect(source).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel=\{t\("models\.provider"\)\}/);
  expect(source).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel=\{t\("models\.protocol"\)\}/);
  expect(source).toMatch(/<FormSelect\s+[\s\S]*?ariaLabel=\{t\("models\.reasoningMode"\)\}/);
  expect(source).not.toMatch(/<select/);
  expect(css).toMatch(/\.mux-form-select-menu/);
  expect(css).toMatch(/\.mux-form-select\[data-open="true"\]\s*\{\s*z-index: 621/);
  expect(css).toMatch(/background: var\(--surface-popover\)/);
});

it("fills a standalone Provider form from the selected catalog template", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "OpenRouter");
  expect(screen.getByRole("heading", { name: "新建 Provider" })).toBeVisible();
  expect(screen.getByRole("combobox", { name: "Provider 类型" })).toHaveTextContent("OpenRouter");
  expect(screen.queryByLabelText("自定义模型提供商 ID")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://openrouter.ai/api/v1");
  expect(screen.getAllByLabelText("Base URL")).toHaveLength(1);
  expect(screen.getByRole("switch", { name: /OpenAI Responses/ })).toBeChecked();
  expect(screen.getByRole("switch", { name: /Anthropic Messages/ })).not.toBeChecked();
  expect(screen.getByLabelText("OpenAI Responses Endpoint Path")).toHaveValue(
    "/responses",
  );
  const requestUrl = screen.getByLabelText("完整请求 URL");
  expect(requestUrl.tagName).toBe("OUTPUT");
  expect(requestUrl).toHaveTextContent("https://openrouter.ai/api/v1/responses");
  expect(requestUrl).not.toHaveClass("mux-model-field");
  const protocolList = screen.getByRole("region", { name: "支持的协议" })
    .querySelector(".mux-provider-protocol-list");
  expect(protocolList).not.toBeNull();
  expect(within(protocolList as HTMLElement).getByText("/v1/messages")).toBeVisible();
  expect(within(protocolList as HTMLElement).getByText("/responses")).toBeVisible();
  expect(within(protocolList as HTMLElement).getByText("/chat/completions")).toBeVisible();
  const responseSwitch = screen.getByRole("switch", { name: "OpenAI Responses" });
  const responseRow = responseSwitch.closest(".mux-provider-protocol-toggle");
  const responsePath = within(responseRow as HTMLElement).getByText("/responses");
  const visualSwitch = responseRow?.querySelector(".mux-provider-protocol-switch");
  expect(visualSwitch).not.toBeNull();
  expect(responsePath.compareDocumentPosition(visualSwitch as Node) & Node.DOCUMENT_POSITION_FOLLOWING)
    .toBeTruthy();
  expect(screen.getByText("已启用 1 个")).toBeVisible();
  expect(screen.getByText("API Key")).toBeVisible();
  expect(screen.getByText("API Key 环境变量")).toBeVisible();
});

it("keeps the empty protocol requirement inside the compact section header", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "OpenRouter");
  await user.click(screen.getByRole("switch", { name: /OpenAI Responses/ }));

  const protocols = screen.getByRole("region", { name: "支持的协议" });
  const header = protocols.querySelector(".mux-provider-protocols-head");
  expect(header).not.toBeNull();
  expect(within(header as HTMLElement).getByRole("status")).toHaveTextContent("至少启用一种协议。");
  expect(within(header as HTMLElement).getByText("已启用 0 个")).toBeVisible();
  expect(protocols.querySelector(".mux-provider-protocol-list + .mux-provider-protocol-error"))
    .not.toBeInTheDocument();
});

it("keeps plan-specific protocols under one Base URL", async () => {
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "provider-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "Alibaba Coding Plan");
  expect(screen.getByLabelText("Base URL")).toHaveValue(
    "https://coding-intl.dashscope.aliyuncs.com",
  );
  expect(screen.getByRole("switch", { name: /Anthropic Messages/ })).toBeChecked();
  expect(screen.getByRole("switch", { name: /OpenAI Chat Completions/ })).toBeChecked();
  expect(screen.getByLabelText("Anthropic Messages Endpoint Path")).toHaveValue(
    "/apps/anthropic/v1/messages",
  );
  expect(screen.getByLabelText("OpenAI Chat Completions Endpoint Path")).toHaveValue(
    "/v1/chat/completions",
  );
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "model-provider",
    provider: expect.objectContaining({
      provider: "alibaba-coding-plan",
      base_url: "https://coding-intl.dashscope.aliyuncs.com",
      protocols: {
        "anthropic-messages": { endpoint_path: "/apps/anthropic/v1/messages" },
        "openai-completions": { endpoint_path: "/v1/chat/completions" },
      },
    }),
  })));
});

it("keeps an explicitly entered Provider endpoint while changing its type", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "Custom Provider");
  const endpoint = screen.getByLabelText("Base URL");

  expect(screen.queryByLabelText("自定义模型提供商 ID")).not.toBeInTheDocument();
  await user.type(endpoint, "https://gateway.example.test/v1");
  await chooseFormSelect(user, "Provider 类型", "OpenRouter");

  expect(endpoint).toHaveValue("https://gateway.example.test/v1");
  expect(screen.getByRole("combobox", { name: "Provider 类型" })).toHaveTextContent("OpenRouter");
  expect(screen.queryByLabelText("自定义模型提供商 ID")).not.toBeInTheDocument();
});

it("replaces an untouched template connection when the Provider type changes", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "Ollama");
  expect(screen.getByRole("switch", { name: /OpenAI Chat Completions/ })).toBeChecked();
  expect(screen.getByLabelText("Base URL")).toHaveValue("http://localhost:11434/v1");

  await chooseFormSelect(user, "Provider 类型", "OpenRouter");
  expect(screen.getByRole("switch", { name: /OpenAI Responses/ })).toBeChecked();
  expect(screen.getByRole("switch", { name: /OpenAI Chat Completions/ })).not.toBeChecked();
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://openrouter.ai/api/v1");
});

it("enables, previews, resets, and submits protocol Endpoint Paths", async () => {
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "provider-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "OpenRouter");
  await user.click(screen.getByRole("switch", { name: /Anthropic Messages/ }));
  expect(screen.getAllByLabelText("Base URL")).toHaveLength(1);
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://openrouter.ai/api/v1");
  const path = screen.getByLabelText("Anthropic Messages Endpoint Path");
  expect(path).toHaveValue("/v1/messages");
  await user.clear(path);
  await user.type(path, "custom/messages");
  expect(path).toHaveValue("custom/messages");
  const protocolCard = path.closest(".mux-provider-protocol");
  expect(protocolCard).not.toBeNull();
  expect(within(protocolCard as HTMLElement).getByLabelText("完整请求 URL"))
    .toHaveTextContent("https://openrouter.ai/api/v1/custom/messages");
  await user.click(within(protocolCard as HTMLElement).getByRole("button", { name: "恢复默认" }));
  expect(path).toHaveValue("/v1/messages");
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "model-provider",
    provider: expect.objectContaining({
      base_url: "https://openrouter.ai/api/v1",
      protocols: expect.objectContaining({
        "anthropic-messages": { endpoint_path: "/v1/messages" },
        "openai-responses": { endpoint_path: "/responses" },
      }),
    }),
  })));
});

it("rejects absolute, fragmented, and traversal Endpoint Paths in the Provider form", async () => {
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "Custom Provider");
  await user.type(screen.getByLabelText("Base URL"), "https://gateway.example.test");
  const path = screen.getByLabelText("OpenAI Responses Endpoint Path");
  const protocolCard = path.closest(".mux-provider-protocol");
  expect(protocolCard).not.toBeNull();
  const save = screen.getByRole("button", { name: "保存" });

  for (const invalid of [
    "https://other.example/responses",
    "/v1/responses#fragment",
    "/v1/responses?mode=fast",
    "/v1/%2e%2e/responses",
  ]) {
    await user.clear(path);
    await user.type(path, invalid);
    expect(screen.getByText(/请输入相对路径/)).toBeVisible();
    expect(within(protocolCard as HTMLElement).getByText("—")).toBeVisible();
    expect(within(protocolCard as HTMLElement).queryByText(invalid)).not.toBeInTheDocument();
    expect(save).toBeDisabled();
  }
});

it("filters Model protocols by Provider and previews the selected request URL", async () => {
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([
    {
      id: "multi-provider",
      name: "Multi Provider",
      provider: "custom",
      base_url: "https://multi.example.test/api",
      protocols: {
        "anthropic-messages": { endpoint_path: "/anthropic/v1/messages" },
        "openai-responses": { endpoint_path: "/openai/responses" },
      },
      credential_saved: false,
      model_count: 0,
    },
    {
      id: "chat-provider",
      name: "Chat Provider",
      provider: "custom",
      base_url: "https://chat.example.test",
      protocols: {
        "openai-completions": { endpoint_path: "/v2/chat/completions" },
      },
      credential_saved: false,
      model_count: 0,
    },
  ]);
  const user = userEvent.setup();
  const consumptionState = { plan: null, planUpdate: vi.fn() } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await user.click(await screen.findByRole("button", { name: "添加模型" }));
  const protocol = screen.getByRole("combobox", { name: "协议" });
  expect(protocol).toHaveTextContent("Anthropic Messages");
  expect(screen.getByLabelText("完整请求 URL")).toHaveValue(
    "https://multi.example.test/api/anthropic/v1/messages",
  );

  await user.click(protocol);
  expect(screen.getByRole("option", { name: "Anthropic Messages" })).toBeVisible();
  expect(screen.getByRole("option", { name: "OpenAI Responses" })).toBeVisible();
  expect(screen.queryByRole("option", { name: "OpenAI Chat Completions" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("option", { name: "OpenAI Responses" }));
  expect(screen.getByLabelText("完整请求 URL")).toHaveValue(
    "https://multi.example.test/api/openai/responses",
  );

  await chooseFormSelect(user, "模型提供商", "Chat Provider");
  expect(protocol).toHaveTextContent("OpenAI Chat Completions");
  expect(screen.getByLabelText("完整请求 URL")).toHaveValue(
    "https://chat.example.test/v2/chat/completions",
  );
});

it("switches an existing Model to another persisted Provider relationship", async () => {
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([
    {
      id: "custom-provider",
      name: "Custom Team",
      provider: "custom",
      base_url: "https://gateway.example.test",
      protocols: { "openai-responses": { endpoint_path: "/v1/responses" } },
      credential_saved: false,
      model_count: 1,
    },
    {
      id: "openrouter-provider",
      name: "OpenRouter Team",
      provider: "openrouter",
      base_url: "https://openrouter.ai",
      protocols: { "openai-responses": { endpoint_path: "/api/v1/responses" } },
      credential_saved: true,
      model_count: 0,
    },
  ]);
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "existing-model",
    name: "Existing Model",
    provider_id: "custom-provider",
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
  await waitFor(() => expect(screen.getByRole("combobox", { name: "模型提供商" })).toBeVisible());
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  const editor = screen.getByRole("complementary", { name: "编辑模型 详情" });
  expect(editor).toBeVisible();
  expect(within(editor).getByRole("heading", { name: "编辑模型" })).toBeVisible();
  expect(within(editor).getByText("模型必须引用一个 Provider；连接与凭据由 Provider 统一管理。")).toBeVisible();
  expect(within(editor).queryByText("编辑 · OpenAI Responses")).not.toBeInTheDocument();
  expect(screen.queryByRole("dialog", { name: "编辑模型" })).not.toBeInTheDocument();

  const provider = screen.getByRole("combobox", { name: "模型提供商" });
  expect(provider).toHaveTextContent("Custom Team");
  await chooseFormSelect(user, "模型提供商", "OpenRouter Team");
  expect(provider).toHaveTextContent("OpenRouter Team");
  expect(screen.getByLabelText("完整请求 URL")).toHaveValue(
    "https://openrouter.ai/api/v1/responses",
  );
  await user.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(consumptionState.planUpdate).toHaveBeenCalledWith(
    expect.objectContaining({
      existing_id: "existing-model",
      profile: expect.objectContaining({ provider_id: "openrouter-provider" }),
    }),
  ));
});

it("submits an independent custom Provider through the central asset plan", async () => {
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "model-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "Custom Provider");
  await user.type(
    screen.getByLabelText("Base URL"),
    "https://models.example.test/v1/",
  );
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledTimes(1));
  expect(planUpdate).toHaveBeenCalledWith({
    domain: "model-provider",
    existing_id: undefined,
    provider: expect.objectContaining({
      id: "",
      provider: "custom",
      base_url: "https://models.example.test/v1",
      protocols: expect.objectContaining({
        "openai-responses": { endpoint_path: "/responses" },
      }),
    }),
    credential: undefined,
  });
});

it("configures a Gemini native GenerateContent endpoint on a custom Provider", async () => {
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "gemini-provider-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await openProviderTemplate(user, "Custom Provider");
  fireEvent.change(screen.getByLabelText("Base URL"), {
    target: { value: "http://127.0.0.1:18080/v1beta" },
  });
  const geminiSwitch = screen.getByRole("switch", { name: "Gemini GenerateContent" });
  await user.click(geminiSwitch);
  const protocolRow = geminiSwitch.closest("article") as HTMLElement;
  expect(within(protocolRow).getByLabelText("完整请求 URL")).toHaveTextContent(
    "http://127.0.0.1:18080/v1beta/models/{model}:generateContent",
  );
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "model-provider",
    provider: expect.objectContaining({
      provider: "custom",
      base_url: "http://127.0.0.1:18080/v1beta",
      protocols: expect.objectContaining({
        "gemini-generate-content": {
          endpoint_path: "/models/{model}:generateContent",
        },
      }),
    }),
  })));
});

it("keeps Model fields local while writing an explicit Provider reference", async () => {
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([{
    id: "openrouter-team-a",
    name: "OpenRouter Team",
    provider: "openrouter",
    base_url: "https://openrouter.ai",
    protocols: { "openai-responses": { endpoint_path: "/api/v1/responses" } },
    credential_saved: true,
    model_count: 0,
  }]);
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "model-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  await user.click(await screen.findByRole("button", { name: "添加模型" }));
  expect(screen.queryByText("高级设置")).not.toBeInTheDocument();
  expect(screen.getByLabelText("上下文窗口")).toHaveValue(null);
  expect(screen.getByLabelText("最大输出")).toHaveValue(null);
  expect(screen.queryByPlaceholderText("MY_API_KEY")).not.toBeInTheDocument();
  expect(screen.queryByText("API Key")).not.toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "推理" })).toHaveTextContent("自动");

  await chooseFormSelect(user, "推理", "关闭");
  await user.type(screen.getByPlaceholderText("model-name"), "explicit-reasoning-off");
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledTimes(1));
  const submitted = planUpdate.mock.calls[0][0].profile;
  expect(submitted).toEqual(expect.objectContaining({
    provider_id: "openrouter-team-a",
    reasoning: false,
  }));
  expect(submitted).not.toHaveProperty("context_window");
  expect(submitted).not.toHaveProperty("max_output_tokens");
});

it("adds another Model to an existing Provider without repeating shared connection fields", async () => {
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([{
    id: "openrouter-team-a",
    name: "OpenRouter Team",
    provider: "openrouter",
    base_url: "https://openrouter.ai",
    protocols: {
      "openai-completions": { endpoint_path: "/api/v1/chat/completions" },
    },
    credential_saved: true,
    model_count: 1,
  }]);
  vi.mocked(api.listModelProfiles).mockResolvedValue([{
    id: "existing-openrouter-model",
    name: "Existing OpenRouter Model",
    provider_id: "openrouter-team-a",
    provider: "openrouter",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "openrouter/free",
    catalog_key: "openrouter/openrouter/free",
    credential_saved: true,
  }]);
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "model-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  const view = render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );

  const sidebar = within(view.container.querySelector(".mux-workspace-sidebar") as HTMLElement);
  await user.click(await sidebar.findByTitle("OpenRouter Team"));
  await user.click(screen.getByRole("button", { name: "添加模型" }));

  expect(screen.getByRole("heading", { name: "新建模型" })).toBeVisible();
  expect(screen.getByRole("combobox", { name: "模型提供商" })).toHaveTextContent(
    "OpenRouter Team",
  );
  expect(screen.queryByLabelText("Base URL")).not.toBeInTheDocument();
  expect(screen.queryByText("API Key")).not.toBeInTheDocument();
  expect(screen.queryByText("API Key 环境变量")).not.toBeInTheDocument();

  fireEvent.change(screen.getByPlaceholderText("model-name"), {
    target: { value: "anthropic/claude-sonnet-4" },
  });
  await user.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(planUpdate).toHaveBeenCalledWith(expect.objectContaining({
    domain: "model",
    profile: expect.objectContaining({
      provider_id: "openrouter-team-a",
      provider: "openrouter",
      protocol: "openai-completions",
      base_url: "https://openrouter.ai",
      model: "anthropic/claude-sonnet-4",
    }),
  })));
});

it("edits one Provider configuration through a single central asset plan", async () => {
  vi.mocked(api.listModelProviderInstances).mockResolvedValue([{
    id: "openrouter-team-a",
    name: "OpenRouter Team",
    provider: "openrouter",
    base_url: "https://openrouter.ai",
    protocols: {
      "openai-completions": { endpoint_path: "/api/v1/chat/completions" },
      "anthropic-messages": { endpoint_path: "/api/v1/messages" },
    },
    env_key: "OPENROUTER_API_KEY",
    credential_saved: true,
    model_count: 3,
  }]);
  const user = userEvent.setup();
  const planUpdate = vi.fn().mockResolvedValue({ operation_id: "provider-plan" });
  const consumptionState = { plan: null, planUpdate } as unknown as ConsumptionState;

  const view = render(
    <ToastProvider>
      <ModelsView consumptionState={consumptionState} />
    </ToastProvider>,
  );
  const sidebar = within(view.container.querySelector(".mux-workspace-sidebar") as HTMLElement);
  await user.click(await sidebar.findByTitle("OpenRouter Team"));
  await user.click(screen.getByRole("button", { name: "编辑 Provider" }));

  expect(screen.getByRole("switch", { name: /Anthropic Messages/ })).toBeChecked();
  expect(screen.getByRole("switch", { name: /OpenAI Chat Completions/ })).toBeChecked();
  const baseUrl = screen.getByLabelText("Base URL");
  expect(baseUrl).toHaveValue("https://openrouter.ai");
  fireEvent.change(baseUrl, {
    target: { value: "https://gateway.example.test/v2/" },
  });
  fireEvent.change(screen.getByLabelText("Anthropic Messages Endpoint Path"), {
    target: { value: "/anthropic/v1/messages" },
  });
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(planUpdate).toHaveBeenCalledWith({
    domain: "model-provider",
    existing_id: "openrouter-team-a",
    provider: expect.objectContaining({
      id: "openrouter-team-a",
      name: "OpenRouter Team",
      provider: "openrouter",
      base_url: "https://gateway.example.test/v2",
      protocols: expect.objectContaining({
        "openai-completions": { endpoint_path: "/api/v1/chat/completions" },
        "anthropic-messages": { endpoint_path: "/anthropic/v1/messages" },
      }),
      env_key: "OPENROUTER_API_KEY",
    }),
    credential: undefined,
  }));
});

it("routes profile lifecycle through central asset plans", () => {
  expect(source).toMatch(/consumptionState\.planUpdate/);
  expect(source).toMatch(/consumptionState\.planDelete/);
  expect(source).not.toMatch(/saveModelProfile|deleteModelProfile|applyModelProfile/);
});

it("keeps the top-level Models workspace asset-only", () => {
  expect(source).toMatch(/searchPlaceholder=\{t\("models\.search"\)\}/);
  expect(source).toMatch(/label=\{t\("models\.asset"\)\}/);
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
