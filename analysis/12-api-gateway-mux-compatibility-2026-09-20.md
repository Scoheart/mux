# Routeway、APInex、Cavoti、Infron 与 MUX 的接入兼容性分析

## Overview

结论：Routeway、Infron、APInex 已按一等内置 Provider 加入 MUX 的可选集合；Cavoti 仍暂缓。Routeway 的公开文档同时给出了 OpenAI Chat Completions、Responses 和 Anthropic Messages，并明确提供 Codex、Claude Code、OpenCode 的接入说明；Infron 也公开了三种协议的 API Reference。APInex 已确认 Chat Completions 和 Anthropic Messages，Cavoti 只确认了 OpenAI-compatible 宣传和 Key 创建流程，尚未从公开页面确认具体 Base URL、模型 ID 和完整协议合同。

“能在 MUX 中创建 Provider”与“能被每个 Agent 使用”是两件事：MUX 按 Profile 的协议和 Agent 的协议能力做兼容性判断，OpenAI-compatible 的 Chat Completions 不能自动等同于 Responses。

## Relevant files

- `core/src/domain/types.rs:6-84` — MUX 支持的模型协议、Provider Base URL、Endpoint Path 和凭据来源。
- `core/src/resources/model/mod.rs:337-827` — 内置 Provider 模板，以及 `custom` Provider。
- `core/src/resources/model/mod.rs:970-1029` — 根据 Base URL 推断 Provider；未知域名落到 `custom`。
- `core/src/resources/model/discovery.rs:53-64, 85-122, 936-985` — 模型目录发现和自定义 Provider 的同源 `/models` 推导。
- `core/src/resources/model/mod.rs:3070-3478` — Agent 的协议、管理模式和配置能力。
- `core/src/resources/model/credential.rs:56-127` — 各 Agent 的 API Key 交付能力。
- `core/src/assets/compatibility.rs:165-206` — MUX 最终按 Agent 模式、协议、Endpoint Path 和凭据判断兼容性。

## Core analysis

### 1. MUX 的接入模型

MUX 的 Provider 是一个共享连接：一个 Base URL、多个协议 Endpoint Path、一个凭据来源；Model Profile 只引用 Provider 并保存精确 Model ID。协议枚举包括 Anthropic Messages、OpenAI Responses、OpenAI Chat Completions 和 Gemini Generate Content（`core/src/domain/types.rs:6-24`）。

未知域名不会被错误识别成官方厂商：MUX 只为已审核域名返回内置 Provider，其他域名返回 `custom`（`core/src/resources/model/mod.rs:970-1029`）。本次已把三个网关域名加入审核映射，因此它们会显示为独立的 `Routeway`、`Infron`、`APInex` 模板，而不是错误归入内置的 `DeepSeek` Provider；内置 `DeepSeek` 仍指向 `https://api.deepseek.com`。

自定义 Provider 可以按已配置的协议端点推导同源 `/models`；如果平台没有标准模型目录，也可以手动填写 Model ID（`core/src/resources/model/discovery.rs:936-985`）。MUX 不会因为一个平台声称兼容 OpenAI 就替它猜测非标准路径。

### 2. 四个平台的证据与结论

| 平台 | 已核实的公开接口 | MUX 中央 Provider | 当前推荐的 MUX 协议 | 结论 |
|---|---|---|---|---|
| Routeway | `https://api.routeway.ai/v1`；Chat Completions、Responses、Anthropic Messages；有 Codex、Claude Code、OpenCode 指南 | `routeway` 内置模板 | 三种都可配置；具体模型仍以 `/v1/models` 或模型页返回的 ID 为准 | **已加入，优先使用** |
| Infron | `https://llm.onerouter.pro/v1`；Chat Completions、Responses、Anthropic Messages；有 `/v1/models` | `infron` 内置模板 | 三种都可配置；`deepseek/deepseek-v4-flash:free` 已在文档示例出现 | **已加入，协议证据强** |
| APInex | `https://api.apinex.bond/v1`；Chat Completions；另有 `/v1/messages` Anthropic-compatible 页面；模型 ID 来自 `/v1/models` 或控制台 | `apinex` 内置模板 | Chat Completions 和 Anthropic Messages；Responses 尚未找到独立的公开接口合同 | **已加入，Codex 暂不判定** |
| Cavoti | 官方页面声称 OpenAI-compatible，并说明注册后创建 Key；公开页面未给出可复核的具体 Base URL、模型 ID 或 Responses/Messages 路径 | `custom`（理论上） | 至少需要 Chat Completions 的实际 Base URL；其他协议未知 | **暂不建议直接录入，先从账户文档取得接口信息** |

外部证据：Routeway 文档给出 Base URL 和 Chat Completions 示例（[Introduction](https://docs.routeway.ai/getting-started/introduction)），并单独给出 Codex、Claude Code、Responses 和 Messages 文档；Infron 的 [API Reference](https://infron.ai/models/deepseek/deepseek-v4-flash:free/api-reference) 给出完整 URL、Key Header 和三种客户端示例；APInex 的 [Chat Completions 文档](https://apinex.bond/developers/models/chat) 和 [Messages 页面](https://apinex.bond/developers/models/messages) 给出对应路径；Cavoti 当前公开的 [API 平台说明](https://cavoti.com/docs) 只足以确认产品定位和 OpenAI-compatible 声明。

### 3. 建议的 Provider 连接形态

为了让同一个 Provider 同时适配不同客户端，Base URL 可以取域名根，Endpoint Path 写完整的版本路径：

| Provider | Base URL | OpenAI Responses | OpenAI Completions | Anthropic Messages |
|---|---|---|---|---|
| Routeway | `https://api.routeway.ai` | `/v1/responses` | `/v1/chat/completions` | `/v1/messages` |
| Infron | `https://llm.onerouter.pro` | `/v1/responses` | `/v1/chat/completions` | `/v1/messages` |
| APInex | `https://api.apinex.bond` | 暂无独立公开证据 | `/v1/chat/completions` | `/v1/messages` |
| Cavoti | 以账户/API 文档为准 | 未知 | 未知 | 未知 |

用域名根作为 Base URL 的好处是同一个 Provider 可以同时保存 `/v1/responses`、`/v1/chat/completions` 和 `/v1/messages`；MUX 在投影给具体 Agent 时会生成该 Agent 所需的客户端 Base URL。也可以按平台文档使用带 `/v1` 的 Base URL，但不要把 `/v1` 重复拼进 Endpoint Path。

模型 ID 必须使用平台当前目录返回的精确字符串，不能把不同平台的名字混用：Routeway 当前示例使用 `deepseek-v4-flash-0731`，APInex 示例使用带命名空间的 `deepseek/deepseek-v4-flash-0731`，Infron 使用 `deepseek/deepseek-v4-flash:free`。优先让 MUX 读取 `/models`，失败时再手填。

### 4. Agent 侧的实际可用范围

MUX 的 Model Profile 兼容性会先检查 Agent 是否为 `managed`，再检查协议、Endpoint Path 和凭据交付（`core/src/assets/compatibility.rs:165-206`）。按当前代码：

- **Anthropic Messages**：Claude Code、Claude Desktop 原生只接受该协议；OpenCode、Pi、Kilo Code 等也可用。Routeway、Infron 有明确证据；APInex 有 Messages 页面；Cavoti 未确认。
- **OpenAI Responses**：Codex 原生只接受该协议；OpenCode、Pi、Grok Build、Kilo Code 等也可用。Routeway 和 Infron 有明确证据；APInex、Cavoti 暂不应假设可用。
- **OpenAI Chat Completions**：OpenCode、Pi、Grok Build、Kilo Code、Qwen Code、Crush、Hermes、Goose、Mistral Vibe、Factory Droid、ZCode 等可用。Routeway、Infron、APInex 已有足够证据；Cavoti 需要补齐真实接口资料。
- **MiniMax Code、Qoder IDE、Kimi Code CLI/Desktop**：当前是 MUX 的 guided 目标，不是完整自动写入路径；即使平台协议兼容，也要在 Agent 自己的设置中配置，不能把“中央 Provider 可建”理解为 MUX 会自动完成全部接入。

凭据方面，Claude Code/Codex 可走 MUX Keychain helper，OpenCode 可走环境变量、Agent store 或其他已核验路线，Qwen Code/Grok Build/Crush/Hermes/Factory Droid/Goose 等主要走环境变量引用；MUX 不把 API Key 写入普通中央配置（`core/src/resources/model/credential.rs:56-127`）。

### 5. 对这批免费模型的特殊判断

模型的“免费”状态不改变协议判断，但会改变可用性和安全风险：

- Routeway 的模型目录把 DeepSeek V4 Flash Free 标为支持 Function Calling，但具体模型是否被该平台声明为 Responses endpoint，仍应从模型目录或一次最小 Responses 请求确认；Chat Completions 路径更有直接证据。
- Infron 的免费端点页面明确要求余额超过 5 美元、每天最多 10 次，并警告 prompts/outputs 会被记录，仅供试用；不适合把私有仓库、凭据或敏感业务输入送入测试。
- APInex 的模型 ID、价格和限额来自动态目录，MUX 应保存平台返回的精确 ID，不应固定 X 帖子里的旧名称。
- Cavoti 的公开资料强调“比较并连接独立供应商”，但平台、上游 Provider 和最终数据处理责任需要按其账户页面逐项确认；在 Base URL 和隐私合同未落地前，不应接入真实项目。

## Recommended order

1. **Routeway**：已作为内置模板提供三种协议，优先验证 `/v1/chat/completions` 和 `/v1/responses`；如果要给 Claude Code，再验证 `/v1/messages`。
2. **Infron**：已作为内置模板提供三种协议，但免费端点只用于无敏感数据的短测试，并接受其余额和日请求限制。
3. **APInex**：已作为 Chat/Messages 内置网关提供，适合 OpenCode 或 Claude Code；在没有独立 Responses 文档和实际请求成功前，不将它分配给 Codex。
4. **Cavoti 暂缓**：取得登录后显示的 Base URL、模型目录 URL、精确 Model ID、Key Header 和数据保留政策后，再考虑加入内置集合。

## Minimal verification checklist

每个平台创建 Provider 后，依次验证：

1. MUX `/models` 发现是否成功，或者手动 Model ID 是否能保存。
2. 一条短文本的非流式请求是否返回标准响应。
3. 流式结束事件、工具调用、usage、401、402/403、429、5xx 和超时是否符合目标 Agent 预期。
4. 对 Codex 单独验证 `/v1/responses`；对 Claude Code/Desktop 单独验证 `/v1/messages`，不能用 Chat Completions 成功替代。
5. 只用无敏感内容的测试目录，确认重启后 Agent 仍能读取 Key；不要把 API Key 写入仓库、截图或日志。

## Related source references

- MUX README 的“Reusable model connections”与 Model Provider 说明：`README.md`。
- MUX Provider 来源与官方端点审计：`core/src/resources/model/PROVIDER_SOURCES.md`。
- MUX Agent 能力矩阵：`core/src/resources/model/mod.rs:3070-3478`。
