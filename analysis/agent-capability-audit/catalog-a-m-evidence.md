# MUX Catalog-only Agent A–M：MCP / Models / Skills 证据账本

> 审计日期：2026-07-22（Asia/Shanghai）
>
> 范围：审计开始时尚未进入 `data/agents.json`、位于同步后 catalog / ACP Registry 并集 A–M 范围的 114 个身份；本版提升项仍保留在本分片，便于追溯。
>
> 状态：本分片已完成。114 个身份均已归类；15 个身份具有至少一项 first-party 可写契约研究候选，其中本版实际提升 6 个身份（2 MCP、4 Skills）。

## 目标与判定规则

- 每个 Agent 最终记录：官方身份/别名、安装探针、用户级/项目级路径、MCP schema/传输、Models schema/切换/凭据、Skills 目录、MUX 是否适合可写、证据 URL 和核验状态。
- 证据优先级：官方稳定发布源码/发布包 > 官方技术文档 > 官方仓库 README > 社区资料。Glama 索引仅用于定位上游，不单独证明可写契约。
- **可提升**：已证明稳定用户级路径和局部可写 schema；凭据不需 MUX 明文落盘；有安装探针。
- **只读展示**：支持相关能力，但只有 Web/UI/云端状态或只有项目配置，没有稳定的用户级 writer 契约。
- **排除/合并**：不是独立本地 Agent，是 SDK/demo/server/plugin，或与已审计 Agent 共用同一产品身份与配置。
- 不从 MCP 路径推断 Models/Skills；“支持模型”不等于“存在 MUX 可安全管理的多 Model Profile”。

## 覆盖清单（114）

`5ire`, `agent-bridge`, `agent-cli`, `agent-one`, `agentai`, `agenticflow`, `agentkube`, `agoragentic-acp`, `aiaw`, `aiql-tuui`, `amazon-q-cli`, `apidog`, `apigene-mcp-client`, `archestra`, `argo-local-ai`, `askit-mcp`, `astr-bot`, `autohand`, `avatar-shell`, `beeai-framework`, `blackbox-cli`, `bob-shell`, `browse-wiz`, `call-chirp`, `call-my-bot`, `chainlit`, `chat-frame`, `chatbox`, `chatgpt`, `chatmcp`, `chatty`, `chatwise`, `cherry-studio`, `chorus`, `claude-ai`, `claude-mind`, `codegpt`, `cody`, `console-chat-gpt`, `copilot-mcp`, `copilot-xcode`, `cortex-code`, `corust-agent`, `crow-cli`, `daydreams`, `deepagents`, `deepchat`, `deepgram-saga`, `dimcode`, `dirac`, `docker-agent`, `docker-gordon`, `dolphin-mcp`, `eca-editor-code-assistant`, `emacs-mcp`, `fast-agent`, `flowdown`, `flujo`, `genaiscript`, `genkit`, `github-copilot`, `github-copilot-coding-agent`, `glm-acp-agent`, `glue`, `gptme`, `harn`, `heym-mcp-client`, `highlight-ai`, `hyper-chat`, `hyperagent`, `ibm-bob`, `inspector`, `jdbcx`, `jenova`, `jetbrains-ai-assistant`, `jetbrains-air`, `joey`, `kibitz`, `kiln-ai`, `klavis-ai-slack-discord-web`, `lang-bot`, `langdock`, `langflow`, `libre-chat`, `lm-kit-net`, `lovable`, `lutra`, `mcp-agent`, `mcp-assistant`, `mcp-bundler-for-macos`, `mcp-chatbot`, `mcp-cli-client`, `mcp-client-chatbot`, `mcp-client-go`, `mcp-partner`, `mcp-simple-slackbot`, `mcp-super-assistant`, `mcp-use`, `mcpbundles`, `mcpc`, `mcphub`, `mcpjam`, `mcpomni-connect`, `mcps-playground`, `memex`, `memgraph-lab`, `microsoft-365-copilot`, `microsoft-copilot-studio`, `mindpal`, `minion-code`, `mistral-ai-le-chat`, `modelcontextchat-com`, `moopoint`, `msty-studio`.

## 上游身份索引（首批）

下列仓库由 catalog 中的 Glama 条目反查，已记为后续 clone/源码检索的官方候选；只有源码或官方文档进一步证明配置契约后才会标记“已证实”。

| ID | 官方/上游候选 | 首轮状态 |
|---|---|---|
| agent-bridge | [ramblinghermit0403/agent_bridge](https://github.com/ramblinghermit0403/agent_bridge) | 待源码核验 |
| agent-cli | [belowthetree/agent-cli](https://github.com/belowthetree/agent-cli) | 待源码核验 |
| agent-one | [AgentOne-Dev/agent-one-public](https://github.com/AgentOne-Dev/agent-one-public) | 待源码核验 |
| agentai | [AdamStrojek/rust-agentai](https://github.com/AdamStrojek/rust-agentai) | 待源码核验 |
| aiaw | [NitroRCr/AIaW](https://github.com/NitroRCr/AIaW) | 待源码核验 |
| aiql-tuui | [AI-QL/tuui](https://github.com/AI-QL/tuui) | 待源码核验 |
| amazon-q-cli | [aws/amazon-q-developer-cli](https://github.com/aws/amazon-q-developer-cli) | 与 audited `amazon-q` 关系待核对 |
| argo-local-ai | [xark-argo/argo](https://github.com/xark-argo/argo) | 待源码核验 |
| askit-mcp | [johnrobinsn/askit](https://github.com/johnrobinsn/askit) | 待源码核验 |
| astr-bot | [AstrBotDevs/AstrBot](https://github.com/AstrBotDevs/AstrBot) | 待源码核验 |
| avatar-shell | [mfukushim/avatar-shell](https://github.com/mfukushim/avatar-shell) | 待源码核验 |
| beeai-framework | [i-am-bee/beeai-framework](https://github.com/i-am-bee/beeai-framework) | SDK/framework 可能性高 |
| chainlit | [Chainlit/chainlit](https://github.com/Chainlit/chainlit) | framework 可能性高 |
| chatbox | [chatboxai/chatbox](https://github.com/chatboxai/chatbox) | 待桌面端源码核验 |
| chatmcp | [daodao97/chatmcp](https://github.com/daodao97/chatmcp) | 待桌面端源码核验 |
| cherry-studio | [CherryHQ/cherry-studio](https://github.com/CherryHQ/cherry-studio) | 待桌面端源码核验 |
| console-chat-gpt | [amidabuddha/console-chat-gpt](https://github.com/amidabuddha/console-chat-gpt) | 待源码核验 |
| daydreams | [daydreamsai/daydreams](https://github.com/daydreamsai/daydreams) | SDK/framework 可能性高 |
| deepchat | [ThinkInAIXYZ/deepchat](https://github.com/ThinkInAIXYZ/deepchat) | 待桌面端源码核验 |
| docker-agent | [docker/docker-agent](https://github.com/docker/docker-agent) | 待官方 CLI 配置核验 |
| dolphin-mcp | [QuixiAI/dolphin-mcp](https://github.com/QuixiAI/dolphin-mcp) | 待源码核验 |
| eca-editor-code-assistant | [editor-code-assistant/eca](https://github.com/editor-code-assistant/eca) | 待源码核验 |
| emacs-mcp | [lizqwerscott/mcp.el](https://github.com/lizqwerscott/mcp.el) | Emacs library 可能性高 |
| fast-agent | [evalstate/fast-agent](https://github.com/evalstate/fast-agent) | framework，但可能有用户级 YAML |
| flowdown | [Lakr233/FlowDown](https://github.com/Lakr233/FlowDown) | 待 Apple 应用源码核验 |
| flujo | [mario-andreschak/FLUJO](https://github.com/mario-andreschak/FLUJO) | 待源码核验 |
| genaiscript | [microsoft/genaiscript](https://github.com/microsoft/genaiscript) | framework/CLI 待分类 |
| genkit | [firebase/genkit](https://github.com/firebase/genkit) | SDK/framework 可能性高 |
| gptme | [gptme/gptme](https://github.com/gptme/gptme) | 待 CLI 源码核验 |
| heym-mcp-client | [heymrun/heym](https://github.com/heymrun/heym) | 待源码核验 |
| hyper-chat | [BigSweetPotatoStudio/HyperChat](https://github.com/BigSweetPotatoStudio/HyperChat) | 待桌面端源码核验 |
| hyperagent | [hyperbrowserai/HyperAgent](https://github.com/hyperbrowserai/HyperAgent) | SDK/framework 可能性高 |
| jdbcx | [jdbcx/jdbcx](https://github.com/jdbcx/jdbcx) | 待源码核验 |
| joey | [benkaiser/joey-mcp-client](https://github.com/benkaiser/joey-mcp-client) | 待源码核验 |
| kibitz | [nick1udwig/kibitz](https://github.com/nick1udwig/kibitz) | 待 CLI 源码核验 |
| kiln-ai | [Kiln-AI/Kiln](https://github.com/Kiln-AI/Kiln) | 待桌面端/项目配置核验 |
| klavis-ai-slack-discord-web | [Klavis-AI/klavis](https://github.com/Klavis-AI/klavis) | hosted SDK/service 可能性高 |
| lang-bot | [RockChinQ/LangBot](https://github.com/RockChinQ/LangBot) | bot framework 可能性高 |
| langflow | [langflow-ai/langflow](https://github.com/langflow-ai/langflow) | server/framework 可能性高 |
| libre-chat | [danny-avila/LibreChat](https://github.com/danny-avila/LibreChat) | self-hosted server 可能性高 |
| mcp-agent | [lastmile-ai/mcp-agent](https://github.com/lastmile-ai/mcp-agent) | SDK/framework 可能性高 |
| mcp-assistant | [zonlabs/mcp-assistant](https://github.com/zonlabs/mcp-assistant) | 待源码核验 |
| mcp-chatbot | [3choff/mcp-chatbot](https://github.com/3choff/mcp-chatbot) | demo 可能性高 |
| mcp-cli-client | [adhikasp/mcp-client-cli](https://github.com/adhikasp/mcp-client-cli) | 待 CLI 源码核验 |
| mcp-client-chatbot | [cgoinglove/mcp-client-chatbot](https://github.com/cgoinglove/mcp-client-chatbot) | demo 可能性高 |
| mcp-client-go | [yincongcyincong/mcp-client-go](https://github.com/yincongcyincong/mcp-client-go) | 待 CLI 源码核验 |
| mcp-simple-slackbot | [sooperset/mcp-client-slackbot](https://github.com/sooperset/mcp-client-slackbot) | bot/demo 可能性高 |
| mcp-super-assistant | [srbhptl39/MCP-SuperAssistant](https://github.com/srbhptl39/MCP-SuperAssistant) | browser extension 待核验 |
| mcp-use | [mcp-use/mcp-use](https://github.com/mcp-use/mcp-use) | SDK/framework 可能性高 |
| mcpc | [apify/mcp-cli](https://github.com/apify/mcp-cli) | 待 CLI 源码核验 |
| mcphub | [ravitemer/mcphub.nvim](https://github.com/ravitemer/mcphub.nvim) | Neovim plugin 待核验 |
| mcpjam | [MCPJam/inspector](https://github.com/MCPJam/inspector) | inspector/devtool 可能性高 |
| mcpomni-connect | [omnirexflora-labs/omnicoreagent](https://github.com/omnirexflora-labs/omnicoreagent) | framework 可能性高 |

## 逐项证据

> 低证据条目不做可写结论；本分片已按字典序完成核验与分类。

### A–B（20 项，已核验）

#### `agent-bridge` — Agent Bridge

- **身份/安装探针**：开源的 client + Python backend 自托管应用，并非独立系统 CLI；可用 repo checkout / backend service 作弱探针，无稳定用户级命令。
- **MCP**：仓库 `server/servers.json` / backend bootstrap 导入配置，运行后核心状态位于 backend DB 的 `mcp_server_settings`；不是 `HOME` 下的稳定文件契约。
- **Models**：`server/.env` 用 `GOOGLE_API_KEY` / `OPENAI_API_KEY`，provider 由 backend 配置；没有 MUX 可局部管理的用户级 profile/current pointer。
- **Skills**：未找到 Agent Skills / `SKILL.md` 目录契约。
- **MUX 结论**：**排除 writer**（自托管应用/DB 状态）；保留 catalog 只读。
- **证据**：[official repo](https://github.com/ramblinghermit0403/agent_bridge)、[README MCP setup](https://github.com/ramblinghermit0403/agent_bridge/tree/6a27ec62bb76171cac62735f13a6d38c46fe360e#readme)、[server model](https://github.com/ramblinghermit0403/agent_bridge/blob/6a27ec62bb76171cac62735f13a6d38c46fe360e/backend/app/models/settings.py)。**状态：已证实（official source）**。

#### `agent-cli` — belowthetree Agent CLI

- **身份/安装探针**：Rust 终端 Agent，Cargo package/binary 均为 `agent-cli`；可以 command `agent-cli` 与用户配置目录联合探测。
- **MCP**：macOS 用户级 `~/Library/Application Support/agent-cli/config.json`；非 ACP 模式会先读 cwd `config.json`，ACP 模式只读用户级。根键 `mcp.server` 是 map；stdio `{command,args,envs}`，Streamable HTTP `{url}`，旧 SSE `{sse}`。
- **Models**：同文件单组 `api_key`, `url`, `model`；`api_key` 是必填明文，无 env/keychain 引用，因此不满足 MUX Model writer 安全契约。
- **Skills**：未找到 `SKILL.md` 或 skills 目录扫描。
- **MUX 结论**：**MCP 可提升**，需新 codec（`mcp.server` + `envs` + `sse`），只管理用户级路径；Models/Skills 不开放。
- **证据**：[official repo](https://github.com/belowthetree/agent-cli)、[`src/config.rs`](https://github.com/belowthetree/agent-cli/blob/392ac0a8b77f822014d1df7cb71444bfdeccd6c8/src/config.rs)、[README](https://github.com/belowthetree/agent-cli/tree/392ac0a8b77f822014d1df7cb71444bfdeccd6c8#readme)。**状态：已证实（official source）**。

#### `agent-one` — AgentOne

- **身份/探针**：闭源桌面应用；官方 public repo 只是入口，Snap package `agent-one` 可作 Linux 探针。
- **MCP / Models / Skills**：公开仓库与官方入口未提供稳定用户级路径或 schema；无法核验凭据保存和局部更新语义。
- **MUX 结论**：**只读展示**，不从闭源 app 存储推断 writer。
- **证据**：[official public repo](https://github.com/AgentOne-Dev/agent-one-public)、[official site](https://www.agent-one.dev/)。**状态：已检索，契约未找到**。

#### `agentai` — rust-agentai

- **身份/探针**：Rust **library/SDK**，不是具有用户配置生命周期的本地 Agent。
- **MCP**：通过 `McpToolBox::new(command,args,env)` 程序化创建 stdio client，没有默认配置文件。
- **Models**：应用代码传入 base URL/key，示例读 `AGENTAI_BASE_URL`, `AGENTAI_API_KEY`, `AGENTAI_MODEL`；不是 profile store。
- **Skills**：无 Agent Skills 扫描契约。
- **MUX 结论**：**排除**（SDK 误分类）。
- **证据**：[official repo](https://github.com/AdamStrojek/rust-agentai)、[MCP implementation](https://github.com/AdamStrojek/rust-agentai/blob/aba9207adae9c7325dc2afd01f816dd709fb0c61/crates/agentai/src/tool/mcp.rs)。**状态：已证实（official source）**。

#### `agenticflow` — AgenticFlow

- **身份**：官方当前文档将 AgenticFlow 暴露为远程 MCP **server** (`https://mcp.agenticflow.ai/mcp`)，指导 ChatGPT/其他 AI tool 连接它；未证明其是独立本地 MCP client。
- **MCP / Models / Skills**：资产位于 SaaS workspace/in-app directory，没有本地 user-level writer 契约。
- **MUX 结论**：**排除/重分类为 MCP server 索引**。
- **证据**：[official connection guide](https://docs.agenticflow.ai/integrations/agenticflow-mcp/connecting-to-agenticflow-mcp)。**状态：已证实（official docs）**。

#### `agentkube` — Agentkube Desktop

- **身份/探针**：Kubernetes 桌面 IDE；探针建议 `/Applications/Agentkube.app` + `~/.agentkube` 而不猜 CLI。
- **MCP**：官方路径 `~/.agentkube/mcp.json`，根键 `mcpServers` map；stdio `{command,args,transport:"stdio",env,enabled}`，远程官方文档只证明旧 SSE；UI 的 Refresh 会重读文件。
- **Models**：官方 changelog 证明 BYOK/BYO-LLM、OpenAI/Anthropic/Google/Azure/Bedrock 与 fallback model，但未公布可安全局部修改的文件 schema/凭据契约。
- **Skills**：官方文档索引未找到 Agent Skills / `SKILL.md` 目录。
- **MUX 结论**：**MCP 可提升**，暂只开 stdio + SSE，保留 `enabled`/未知字段；Models/Skills 只读说明。
- **证据**：[official MCP docs](https://agentkube.mintlify.app/agents/mcp)、[official changelog](https://docs.agentkube.com/changelog)。**状态：MCP 已证实；Models 只证明产品能力**。

#### `aiaw` — AIaW

- **身份/探针**：开源 Web/Tauri 聊天客户端；桌面 app 或 browser origin 可探测，无独立配置文件探针。
- **MCP**：MCP 被建模为 plugin manifest，stdio/http/sse schema 在官方 docs 中完整声明；安装后存入 Dexie IndexedDB `data` 数据库的 `installedPluginsV2` / reactive state，不是 JSON/YAML/TOML 文件。
- **Models**：支持 custom providers/模型映射，同样位于 Dexie `providers`/用户数据，无 MUX 文件 writer 契约。
- **Skills**：未找到 `SKILL.md` 目录扫描。
- **MUX 结论**：**只读展示**（IndexedDB/云同步状态）。
- **证据**：[official repo](https://github.com/NitroRCr/AIaW)、[MCP docs](https://github.com/NitroRCr/AIaW/blob/e9e1c55d0b8e8683cf1239a10939b243d6a429d9/docs/usage/mcp.md)、[Dexie schema](https://github.com/NitroRCr/AIaW/blob/e9e1c55d0b8e8683cf1239a10939b243d6a429d9/src/utils/db.ts)、[custom provider docs](https://github.com/NitroRCr/AIaW/blob/e9e1c55d0b8e8683cf1239a10939b243d6a429d9/docs/usage/custom-provider.md)。**状态：已证实（official source）**。

#### `aiql-tuui` — AIQL TUUI

- **身份/探针**：Electron 聊天客户端；可以 app 路径探测。
- **MCP**：内置 release resource `resources/assets/config/mcp.json`，根键 `mcpServers`；但官方 README 明确用户导入/修改后默认存到 browser `localStorage`，资源文件是默认值而非用户状态。
- **Models**：`llm.json` 也是 release default，用户状态进 `localStorage`。
- **Skills**：无 `SKILL.md` 扫描契约。
- **MUX 结论**：**只读展示**；不覆写 app bundle 内默认资源。
- **证据**：[official repo](https://github.com/AI-QL/tuui)、[README configuration](https://github.com/AI-QL/tuui/blob/0be513448dc674c76581e84bb1ff0a3a0be104c7/README.md#additional-configuration)。**状态：已证实（official source）**。

#### `amazon-q-cli` — Amazon Q Developer CLI（已停止主动维护）

- **身份/别名/探针**：官方 repo 明确已迁移为闭源 Kiro CLI；历史 command `q`、app `Amazon Q.app`、`~/.aws/amazonq` 可探测。不应与 audited `amazon-q` IDE 盲目合并，两者使用不同 MCP 文件。
- **MCP**：历史 user `~/.aws/amazonq/mcp.json`，project `.amazonq/mcp.json`，`mcpServers` map；终版 custom-agent 文件也能在 `~/.aws/amazonq/cli-agents/*.json` / `.amazonq/cli-agents/*.json` 内嵌 `mcpServers`。legacy schema 为 stdio `{command,args,env,timeout}`；agent v1 schema 也包含 HTTP `type,url,headers`。
- **Models**：custom agent 有单一 `model` 字段，只能选 Q service `/model` 返回的 hosted model ID，无任意 provider/base URL/key 契约。
- **Skills**：未找到 Agent Skills 目录；`.amazonq/rules` 是 context rules，不是 Skills。
- **MUX 结论**：技术上 legacy MCP 可写，但产品 **EOL**；建议保留 catalog 并标注 superseded by Kiro，不新增 writer。
- **证据**：[official repo/EOL notice](https://github.com/aws/amazon-q-developer-cli)、[agent format](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/docs/agent-format.md)、[file locations](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/docs/agent-file-locations.md)。**状态：已证实（official source）**。

#### `apidog` — Apidog

- **身份/探针**：商业 Web/桌面 API 工具；`/Applications/Apidog.app` 可探测。
- **MCP**：官方 UI 支持粘贴 `mcpServers` JSON 或单 server，stdio + Streamable HTTP，但配置“保存到 project 用于团队协作”；官方没有公布稳定的本地 user-level 文件。
- **Models / Skills**：MCP Client 是调试器，不是可选多 LLM profile 的 coding agent；未找到 Agent Skills 目录。
- **MUX 结论**：**只读展示**，不写其项目/云状态。
- **证据**：[official MCP Client docs](https://docs.apidog.com/mcp-client-1930835m0)。**状态：已证实 UI 能力，未找到 user file**。

#### `apigene-mcp-client` — Apigene

- **身份**：SaaS/API 平台中的 tenant-scoped Agent/MCP client；非本地用户配置 Agent。
- **MCP**：配置对象经认证 API `/api/mcp-server/list` 保存，对象包含 `name`, `config.url`, `config.headers`, `enabled`, `server_type`；无 HOME 文件。
- **Models / Skills**：云端 agent/tenant 资产，无本地 profile 或 Skills 目录契约。
- **MUX 结论**：**排除 writer**（需官方 API connector，非文件 writer）。
- **证据**：[official API docs](https://docs.apigene.ai/api-reference/mcp-server/mcp-servers-list)、[official agent docs](https://docs.apigene.ai/user-guide/agents)。**状态：已证实（official docs）**。

#### `archestra` — Archestra

- **身份**：开源/自托管 enterprise MCP/Agent 平台，资产受 RBAC、environment 和 backend DB 管理；不是一个独立的 local config consumer。
- **MCP**：Archestra 管理 upstream servers/gateways 于 backend，还会用 setup script 反向修改 Claude/Codex/Cursor/Copilot 等已审计 Agent；不存在通用 `~/.archestra/mcp.json`。
- **Models**：LLM proxy/providers/virtual keys 属平台资产，不是本地 file profile。
- **Skills**：原生支持 SKILL.md manifest 与 bundled files，但保存在组织/backend，经 `list_skills`/`load_skill`/CRUD tools 访问，非用户目录。
- **MUX 结论**：**排除 file writer**；未来若整合，应做 API/MCP connector，不应冒充本地 Agent。
- **证据**：[official repo](https://github.com/archestra-ai/archestra)、[agents](https://archestra.ai/docs/platform-agents)、[connect clients](https://archestra.ai/docs/platform-connection)、[Skills/MCP server](https://archestra.ai/docs/platform-archestra-mcp-server)。**状态：已证实（official docs/source）**。

#### `argo-local-ai` — Argo

- **身份/探针**：开源 local-first Web/桌面 AI 应用，通常以 backend service/app 运行；无稳定用户 CLI 探针。
- **MCP**：MCP server config 保存在 SQLAlchemy table `mcp_server_config`，经 backend service/API 增删改；不是 user file。
- **Models**：provider/model/API key 保存在 `ModelProviderSetting`/DB，源码中 `api_key` 字段为应用状态；无 Keychain/env-ref 文件 writer 契约。
- **Skills**：未找到 Agent Skills / `SKILL.md` 目录扫描。
- **MUX 结论**：**排除 file writer**（app DB）。
- **证据**：[official repo](https://github.com/xark-argo/argo)、[MCP model](https://github.com/xark-argo/argo/blob/615e92152915ad9ad24005e643507780369bec55/backend/models/mcp_server.py)、[provider model](https://github.com/xark-argo/argo/blob/615e92152915ad9ad24005e643507780369bec55/backend/models/provider.py)。**状态：已证实（official source）**。

#### `askit-mcp` — AskIt

- **身份/探针**：Python library + `python -m askit` CLI；安装 `pip install git+https://github.com/johnrobinsn/askit.git`，无明确 console-script 用户级探针。
- **MCP**：只自动读 cwd `mcp_config.json`，根键 `mcpServers`，stdio `{command,cwd,args,env}`，Streamable HTTP `{transport:"http",url,disabled}`；API 可显式传任意路径。
- **Models**：provider/model 为 CLI 参数，key 通过 provider-specific env 安全传入；无持久化 profile/current pointer。
- **Skills**：无 Agent Skills 目录契约。
- **MUX 结论**：**保留只读**；MUX 不管理项目级写入。
- **证据**：[official repo/README](https://github.com/johnrobinsn/askit)、[example config](https://github.com/johnrobinsn/askit/blob/fb1e0e675f4210832852fc4f78fa8d183adebc4a/mcp_config.json.example)。**状态：已证实（official source）**。

#### `astr-bot` — AstrBot

- **身份/探针**：开源 bot/agent 平台；Python script entry 与 packaged desktop 并存。建议 command `astrbot`/官方 entry + path `~/.astrbot` 联合探测。
- **路径语义**：`ASTRBOT_ROOT` 可覆盖 root；packaged desktop 默认 root `~/.astrbot`，源码运行默认 cwd。因此 packaged user MCP 为 `~/.astrbot/data/mcp_server.json`，Skills 为 `~/.astrbot/data/skills/`；自定义 root 必须可配置目标路径。
- **MCP**：`mcpServers` map，stdio `{command,args,env?}`，remote `{url,headers?}`，个体开关 `active` 默认 true；应用动态重载并原生保存该文件。
- **Models**：`data/cmd_config.json` 中 `provider` / `provider_settings`，provider 对象包含大量明文 `api_key`；属复杂 app config，不满足 MUX Model 安全写入条件。
- **Skills**：原生 Agent Skills，用户级 `<root>/data/skills/<name>/SKILL.md`，项目 workspace 还可读 `<workspace>/skills/`；MUX 只应管 packaged user 目录。
- **MUX 结论**：**MCP + Skills 可提升**；注册 packaged 默认路径并在 note 说明 `ASTRBOT_ROOT`；Models 不开放。
- **证据**：[official repo](https://github.com/AstrBotDevs/AstrBot)、[path source](https://github.com/AstrBotDevs/AstrBot/blob/a325819048ff10753c5963527b0d21869152500c/astrbot/core/utils/astrbot_path.py)、[MCP loader](https://github.com/AstrBotDevs/AstrBot/blob/a325819048ff10753c5963527b0d21869152500c/astrbot/core/provider/func_tool_manager.py)、[Skills manager](https://github.com/AstrBotDevs/AstrBot/blob/a325819048ff10753c5963527b0d21869152500c/astrbot/core/skills/skill_manager.py)。**状态：已证实（official source）**。

#### `avatar-shell` — Avatar Shell

- **身份/探针**：Electron 桌面 Agent，app path 可探测。
- **MCP**：`sysConfig.mcpServers` map；stdio server `{enable,def:{command,args,env}}`，Streamable HTTP `{enable,def:{type,url,note?}}`。
- **Models**：`sysConfig.generators` 中分 provider 存 `apiKey`, `model`, `baseUrl`，不是 MUX 可直接引用 Keychain 的契约。
- **存储风险**：用户状态由 `electron-store` 存于 app userData 的 `config.json`，且启用固定 `encryptionKey` 混淆；直接 JSON writer 不可行，也不应在 MUX 复制该解密逻辑。
- **Skills**：未找到 `SKILL.md` 目录扫描。
- **MUX 结论**：**只读展示**。
- **证据**：[official repo](https://github.com/mfukushim/avatar-shell)、[`ConfigService.ts`](https://github.com/mfukushim/avatar-shell/blob/d6bdafc039480c79cda24bb6ea4fda5c630fdb29/packages/main/src/ConfigService.ts)、[MCP schema](https://github.com/mfukushim/avatar-shell/blob/d6bdafc039480c79cda24bb6ea4fda5c630fdb29/packages/common/Def.ts)。**状态：已证实（official source）**。

#### `beeai-framework` — BeeAI Framework

- **身份**：TypeScript/Python Agent SDK/framework，并非用户安装后具有单一配置根的本地 Agent。
- **MCP / Models**：由应用代码构建 MCP client/provider，官方 docs 是 integration API 和 examples，无稳定 user file/current pointer。
- **Skills**：未找到面向终端用户的 `SKILL.md` 目录扫描契约。
- **MUX 结论**：**排除**（SDK/framework 误分类）。
- **证据**：[official repo](https://github.com/i-am-bee/beeai-framework)、[MCP integration](https://github.com/i-am-bee/beeai-framework/tree/158247fdf9921813f7fdc0c894ee98c28fce735f/docs-old/integrations)。**状态：已证实（official source）**。

#### `blackbox-cli` — BLACKBOX CLI

- **身份/探针**：闭源 coding CLI，官方 installer 后 command `blackbox`。catalog 中旧 MCP URL 已 404。
- **MCP**：当前命令 `blackbox mcp` 是“运行内置 MCP servers”；官方现行文档未给出将外部 MCP servers 持久化到 user file 的 schema/path。
- **Models**：`blackbox configure` 和 `/agent model` 支持模型/代理 Agent 切换，但官方未公开本地文件与凭据存储 schema，不可推断。
- **Skills**：官方只证实 cwd/repo `.blackbox/skills/<name>/SKILL.md`，未证实用户级目录；MUX 当前不管项目级 Skills。
- **MUX 结论**：**只读展示**，等官方公布 user-level 契约。
- **证据**：[official commands](https://docs.blackbox.ai/features/blackbox-cli/commands-reference)、[official setup/models](https://docs.blackbox.ai/features/blackbox-cli/introduction)、[official Skills](https://docs.blackbox.ai/features/blackbox-cli/skills)。**状态：产品能力已证实，user file 未找到**。

#### `bob-shell` — IBM Bob Shell

- **身份/探针**：IBM 官方 CLI，installer `curl -fsSL https://bob.ibm.com/download/bobshell.sh | bash`，command `bob`，path `~/.bob` 可联合探测。
- **MCP**：用户 `~/.bob/mcp_settings.json`，project `.bob/mcp.json`，`mcpServers` map；stdio `command,args,env,cwd`，SSE `url,headers`，Streamable HTTP `httpURL` **（官方拼写）**，附加 `timeout,alwaysAllow,disabled`。官方另一个总 settings 文档使用 `httpUrl`，两处大小写冲突；实现前需发布包/运行验证解决。
- **Models**：Bob hosted model 由 service/会话选择；认证可以 browser SSO 或 env `BOBSHELL_API_KEY`，未找到任意 provider/base URL 的本地 Model Profile writer 契约。
- **Skills**：IBM Bob 官方 Skills 文档证明 project `.bob/skills/` 与 global `~/.bob/skills/`，`SKILL.md` + supporting files；但页面归类于 Bob IDE，Bob Shell 是否在当前 stable 共用该扫描器还需安装包验证。
- **MUX 结论**：**MCP 高优先级候选，Skills 有条件候选**；先解决 `httpURL`/`httpUrl` 冲突和 Shell Skills 实机证据，Models 不开放。
- **证据**：[official MCP docs](https://bob.ibm.com/docs/shell/configuration/mcp/mcp-bobshell)、[official layered config](https://bob.ibm.com/docs/shell/configuration/configuring)、[official install/auth](https://bob.ibm.com/docs/shell/getting-started/install-and-setup)、[official Bob Skills](https://bob.ibm.com/docs/ide/features/skills)。**状态：MCP 已证实但有字段冲突；Skills 待 Shell stable 核验**。

#### `browse-wiz` — BrowseWiz

- **身份/探针**：闭源 Chromium 侧边栏扩展，extension ID `ioohfnlbpolaalcbppaggpgcgpldohfg`；可以 browser extension install 作只读探针。
- **MCP**：官方 UI 接收远程 URL，先尝试 Streamable HTTP、回退 SSE；不支持 stdio，状态在 extension storage 而非公开文件。
- **Models**：BYOK + OpenAI-compatible model 在扩展设置中管理，无 user file/Keychain reference 契约。
- **Skills**：无 Agent Skills 目录。
- **MUX 结论**：**只读展示**（browser extension storage）。
- **证据**：[official Tools docs](https://browsewiz.com/docs/settings-page/tools)、[Chrome Web Store](https://chromewebstore.google.com/detail/browsewiz-ai-assistant-th/ioohfnlbpolaalcbppaggpgcgpldohfg)。**状态：已证实（official docs/store）**。

### C–M：高价值配置契约

#### `chatbox` — Chatbox（有能力，但本轮不提升）

- **身份/探针**：Electron 桌面客户端；macOS app `Chatbox.app`，官方包 `appId: xyz.chatboxapp.app`。
- **MCP**：源码将用户设置写入 `<Electron userData>/config.json` 的 `settings.mcp.servers[]`；元素为 `{id,name,enabled,transport}`，stdio transport `{type:"stdio",command,args,env?}`，HTTP `{type:"http",url,headers?}`。历史官方 issue 能证明 macOS 发布版曾使用 `~/Library/Application Support/xyz.chatboxapp.app/config.json`，但当前 Community Edition package 的 `productName` 是 `xyz.chatboxapp.ce`，公开源码没有消除这两个发布身份的路径歧义。
- **Skills**：原生目录 `<Electron userData>/skills/<name>/SKILL.md`，并自动发现 `~/.claude/skills` 与 `~/.agents/skills`。同样受 userData 身份歧义影响；共享目录可只读展示，但不应把它误写成 Chatbox 专属资产。
- **Models**：providers/API keys 与 MCP 共处 `config.json`；直接纳管会接触明文凭据与整份应用状态。
- **MUX 结论**：**只读/有条件候选**。先用正式安装包确认当前 macOS/Windows/Linux userData，再注册 MCP 与 native Skills；Models 不开放。
- **证据**：[official repo](https://github.com/chatboxai/chatbox)、[`store-node.ts`](https://github.com/chatboxai/chatbox/blob/8639c946c0baedfdd12bbc88ac10f5aa87431647/src/main/store-node.ts)、[`settings.ts`](https://github.com/chatboxai/chatbox/blob/8639c946c0baedfdd12bbc88ac10f5aa87431647/src/shared/types/settings.ts)、[official Work Mode guide](https://chatboxai.app/en/guide/work-mode/configuration)、[official issue path evidence](https://github.com/chatboxai/chatbox/issues/2490)。**状态：schema 已证实；正式包路径仍冲突**。

#### `chatmcp` — ChatMCP

- **身份/探针**：Flutter desktop MCP chat client；macOS 可用 `/Applications/ChatMCP.app`，其他平台用 app/data path 联合探测。
- **MCP 路径**：macOS `~/Library/Application Support/ChatMcp/mcp_server.json`；Windows `%APPDATA%\ChatMcp\mcp_server.json`；Linux `${XDG_DATA_HOME:-~/.local/share}/ChatMcp/mcp_server.json`。
- **MCP schema**：根键 `mcpServers` map。stdio `{command,args,env,type?}`；remote 使用 `{command:<url>,type:"sse"|"streamable"}`，不是标准 `url` codec。应用还会写 `installed`，OAuth 场景可能带附加对象/令牌。无凭据文件可局部 merge 并保留未知字段；只要任一条目含 `oauth`、`token` 或 `client_secret`，敏感条目不进入 inventory，整个文件的更新、停用、快照与备份均在写前拒绝。
- **Models / Skills**：模型与 key 在 SharedPreferences，未找到 Agent Skills 扫描契约。
- **MUX 结论**：**MCP 可直接提升**；需 `chatmcp` custom codec，支持 stdio/SSE/Streamable HTTP 与未知字段 round-trip。
- **证据**：[official repo](https://github.com/daodao97/chatmcp)、[`storage_manager.dart`](https://github.com/daodao97/chatmcp/blob/707ad3700c0cd4b889f2be1c4c583452dddcd01a/lib/utils/storage_manager.dart)、[`mcp_server_provider.dart`](https://github.com/daodao97/chatmcp/blob/707ad3700c0cd4b889f2be1c4c583452dddcd01a/lib/provider/mcp_server_provider.dart)。**状态：已证实（official source）**。

#### `chatty` — Chatty（catalog false positive）

- **身份**：`x00real/chatty` / `mipsel64/chatty` 是同一终端 LLM 客户端代码线。
- **配置**：只证明 `$XDG_CONFIG_HOME/chatty/config.toml`、`~/.config/chatty/config.toml`、`~/.chatty.toml` 中 `[[backend.connections]]` 模型连接；当前官方源码没有 MCP client 或 Agent Skills。
- **MUX 结论**：**只读**；移除 catalog 中“支持 MCP”的暗示，Models 因明文 `api_key` 不开放。
- **证据**：[upstream repo](https://github.com/x00real/chatty)、[fork/current source](https://github.com/mipsel64/chatty)。**状态：已证实无 MCP writer**。

#### `cherry-studio` — Cherry Studio

- **MCP / Models**：当前正式源码把 MCP/provider state 放在 `cherrystudio.sqlite`，不是早期 JSON 文件。
- **Skills**：app canonical 目录为 `{userData}/Data/Skills`，再镜像到 `{userData}/.claude/skills`；userData 可被 `~/.cherrystudio/config/config.json` 重定位，因此不能只登记一个静态 HOME 路径。
- **MUX 结论**：**只读**。后续若纳管，应使用官方 app API/IPC 或先实现 path resolver，不直写 SQLite，也不把镜像目录当 source of truth。
- **证据**：[official repo](https://github.com/CherryHQ/cherry-studio)、[database source](https://github.com/CherryHQ/cherry-studio/tree/ab7b418abe8e28ce6706f307b9a1112225ebdd24/src/main/services/database)、[Skills source](https://github.com/CherryHQ/cherry-studio/tree/ab7b418abe8e28ce6706f307b9a1112225ebdd24/src/main/services/skills)。**状态：已证实（official source）**。

#### `deepchat` — DeepChat

- **MCP / Models**：当前状态位于 `~/Library/Application Support/DeepChat/app_db/agent.db`（其他平台对应 app userData）；旧 `mcp-settings.json` / `model-config.json` 已不是现行契约。
- **Skills**：默认 `~/.deepchat/skills/<name>/SKILL.md`，但 `skillsPath` 是可配置 override。只登记默认目录会在用户改路径后“安装成功但不生效”。
- **MUX 结论**：**只读/有条件 Skills 候选**；先实现读取 `skillsPath` 的 resolver，再开放 Skills。MCP/Models 不直写 SQLite。
- **证据**：[official repo](https://github.com/ThinkInAIXYZ/deepchat)、[`skill/settings.ts`](https://github.com/ThinkInAIXYZ/deepchat/blob/2f6852b388e36e568859ee4845916b1d2f8d81f7/src/main/skill/settings.ts)、[`settingsStore.ts`](https://github.com/ThinkInAIXYZ/deepchat/blob/2f6852b388e36e568859ee4845916b1d2f8d81f7/src/main/config/settingsStore.ts)。**状态：已证实（official source）**。

#### `docker-agent` — Docker Agent（原 cagent）

- **身份/探针**：官方 CLI；commands `docker-agent` 与 `docker agent`，用户配置仍兼容 `~/.config/cagent/`。
- **Skills**：明确扫描 `~/.agents/skills/`（递归）和 `~/.claude/skills/`（只看直接子目录），另外扫描项目目录；标准 `SKILL.md`。MUX 应把它登记为共享 Skills consumer，首选 canonical `~/.agents/skills`，不复制到第二份目录。
- **MCP**：server 定义属于每个 agent YAML 或 remote catalog，不存在独立 user-level MCP asset file。
- **Models**：`~/.config/cagent/config.yaml` 可定义 `providers` / `settings.default_model`，秘密通常来自 `~/.config/cagent/.env` 或 credential helper；中央 Model asset 当前不能保证 env/credential-helper 注入，暂不开放。
- **MUX 结论**：**Skills 可直接提升**；MCP/Models 只读。
- **证据**：[official repo](https://github.com/docker/docker-agent)、[official Skills docs](https://github.com/docker/docker-agent/blob/1d46f74048cc4ad19b795f8ec05a3ff3ec66ed52/docs/features/skills/index.md)、[config source/docs](https://github.com/docker/docker-agent/tree/1d46f74048cc4ad19b795f8ec05a3ff3ec66ed52/docs)。**状态：已证实（official source）**。

#### `eca-editor-code-assistant` — ECA

- **身份/探针**：官方 editor/CLI agent，command `eca`；global config 为 `${XDG_CONFIG_HOME:-~/.config}/eca/config.json`。
- **MCP**：根键 `mcpServers` map；stdio `{command,args,env}`，Streamable HTTP/SSE 自动协商 `{url}`，可选 `headers,clientId,clientSecret,clientName,oauthPort,authScope,disabled`。动态值支持 `${env:...}` 等插值；MUX 只管理常规 transport/开关，保留 OAuth 扩展字段。
- **Skills**：global `${XDG_CONFIG_HOME:-~/.config}/eca/skills/**/SKILL.md`，递归；另读项目 `.eca/skills` / `.agents/skills` 和 config `skills[].path`。MUX 管 global native 目录即可。
- **Models**：同文件 `providers.<id>{api,url,key,models}` 与 `defaultModel`，schema 很合适，但 ECA 不理解 MUX Keychain reference；除非中央资产能提供 `${env:VAR}` 对应的进程环境，否则激活后可能无凭据，故本轮不开放。
- **MUX 结论**：**MCP + Skills 可直接提升**；Models 等 credential projection。
- **证据**：[official repo](https://github.com/editor-code-assistant/eca)、[official MCP config](https://eca.dev/config/tools/)、[official Models config](https://eca.dev/config/models/)、[`skills.clj`](https://github.com/editor-code-assistant/eca/blob/74132651df0c10c9602830324a55442f18da318c/src/eca/features/skills.clj)。**状态：已证实（official docs/source）**。

#### `fast-agent` — fast-agent

- **配置作用域**：当前设计围绕 active home：`--home` > `--workspace/.fast-agent` > `FAST_AGENT_HOME` > cwd `./.fast-agent`；MCP/Models 在 `<active-home>/fast-agent.yaml`，不是固定用户资产。
- **Skills**：默认 `.fast-agent/skills`, `.agents/skills`, `.claude/skills` 都按 active workspace/home 解析；官方没有证明它把 `~/.agents/skills` 当固定 global root。`~/.fast-agent/fast-agent.yaml` 只作为 global plugin layer，不能反推所有能力都来自那里。
- **MUX 结论**：**只读**。未来需“可配置 Agent 实例/home”数据模型，不应把 cwd 配置伪装成全局配置。
- **证据**：[official repo](https://github.com/evalstate/fast-agent)、[core concepts](https://fast-agent.ai/guides/core-concepts/)、[Skills guide](https://fast-agent.ai/guides/skills/)、[config reference](https://fast-agent.ai/ref/config_file/)。**状态：已证实（official docs/source）**。

#### `flowdown` 与 `flujo`

- **FlowDown**：native Apple app；MCP 与 models 由 `Storage.db()` / ConfigurableKit/app container 管理，`.fdmodel` / MCP import file 是交换格式而非 live user config；repo 自带 `Skills/flowdown-agent/SKILL.md` 是给其他 agent 使用的产品说明 Skill，不是 FlowDown 扫描目录。**只读**。
- **FLUJO**：local-first Next.js automation platform，数据默认 `~/.flujo`，但 models、encrypted API keys、MCP servers 是应用 DB/API 资产；没有稳定 JSON/YAML 局部 writer。**只读**。
- **证据**：[FlowDown official repo](https://github.com/Lakr233/FlowDown)、[FLUJO official repo](https://github.com/mario-andreschak/FLUJO)。**状态：均已证实（official source）**。

#### `gptme` — gptme

- **身份/探针**：official Python CLI，command `gptme` / `gptme-util`；global config `${XDG_CONFIG_HOME:-~/.config}/gptme/config.toml`。
- **MCP**：`[[mcp.servers]]` array-of-table；每项 `{name,enabled,command,args,env}`（stdio）或 `{name,enabled,url,headers}`（HTTP）。`[mcp] enabled/auto_start` 是全局开关；`config.local.toml` 可按 name 覆盖/扩展，因此 writer 必须按 `name` merge 并保留 local overlay 语义。
- **Skills**：native `${XDG_CONFIG_HOME:-~/.config}/gptme/skills/<name>/SKILL.md`，并读 `~/.claude/skills`、`~/.agents/skills`；MUX 首选 native target，或把 shared source 显示为“共享”。
- **Models**：`[models] default/favorites` 与 custom provider 都可配置，但 credentials 另存 `credentials.toml` / env / `config.local.toml`；中央 Model writer 尚不能保证安全投影与可用性。
- **MUX 结论**：**MCP + Skills 可直接提升**；MCP 需 TOML array codec，Models 暂缓。
- **证据**：[official repo](https://github.com/gptme/gptme)、[official MCP docs](https://gptme.org/docs/mcp.html)、[official Skills docs](https://gptme.org/docs/skills.html)、[official config docs](https://gptme.org/docs/config.html)。**状态：已证实（official docs/source）**。

#### `hyper-chat` — HyperChat

- **身份/探针**：official package `@dadigua/hyperchat`，commands `hyperchat` / `hc`，Electron app 同源。
- **MCP 路径**：默认全局 workspace `~/Documents/HyperChat/.hyperchat/mcp.json`；`HyperChat_AppDataDir` 可覆盖 `~/Documents/HyperChat`。根键 `mcpServers` map。
- **MCP schema**：`{command,args,env,headers,url,type,disabled}`；`type` 为 `stdio|sse|streamableHttp|inMemory`，MUX 不得创建 `inMemory`，enabled 与 `disabled` 反向映射。default writer 仅在未设置 `HyperChat_AppDataDir` 时使用；若 override 存在应展示目标需手工绑定。
- **Models**：`<appDataDir>/app-settings.jsonc` 中含 models/customProviders/builtinApiKeys，整份设置带明文 key，暂不开放。
- **Skills**：未找到 Agent Skills scanner。
- **MUX 结论**：**MCP 可直接提升**，需 `streamableHttp` type codec 与 override 保护。
- **证据**：[official repo](https://github.com/BigSweetPotatoStudio/HyperChat)、[`const.mts`](https://github.com/BigSweetPotatoStudio/HyperChat/blob/3f9cf0341c44621a7282c0ca6203ae5849313701/packages/core/src/const.mts)、[`types.mts`](https://github.com/BigSweetPotatoStudio/HyperChat/blob/3f9cf0341c44621a7282c0ca6203ae5849313701/packages/shared/src/types.mts)、[`manager.mts`](https://github.com/BigSweetPotatoStudio/HyperChat/blob/3f9cf0341c44621a7282c0ca6203ae5849313701/packages/core/src/agent/mcp/manager.mts)。**状态：已证实（official source）**。

#### `jetbrains-air` — JetBrains Air

- **MCP**：官方支持 Global / Local / Workspace；只公开 local `.air/mcp.json` 与 workspace `.mcp.json`，没有公开 Global 的物理路径，而且文档明确 bearer token 直接写入 JSON、不支持 env substitution。
- **Skills**：`.agents/skills` 及 agent-specific `.claude/.codex/.gemini/.junie/skills` 都是 project 目录。
- **Models**：provider/account 通过 Settings 登录，任务内切 model；无公开 profile file。
- **MUX 结论**：**只读**，不猜 JetBrains private settings path。
- **证据**：[official MCP docs](https://www.jetbrains.com/help/air/mcp-servers.html)、[official Skills docs](https://www.jetbrains.com/help/air/skills.html)、[official agents/models docs](https://www.jetbrains.com/help/air/select-agents-and-models.html)。**状态：已证实（official docs）**。

#### `joey`, `kibitz`, `mcpc`

- **Joey**：MCP servers 在 app SQLite table `mcp_servers`，OpenRouter key 在 Flutter SharedPreferences；**只读**。[official repo](https://github.com/benkaiser/joey-mcp-client)。
- **Kibitz**：MCP state 在 IndexedDB/browser/app service store，非 stable file；**只读**。[official repo](https://github.com/nick1udwig/kibitz)。
- **mcpc**：官方 `@apify/mcpc` 是通用 MCP CLI；`~/.mcpc` 只保存 sessions/OAuth profiles/wallet/logs，server 定义来自用户显式传入或自动发现的 Claude/Cursor/VS Code/Kiro 等**其他 Agent 文件**。把这些反向登记给 mcpc 会造成重复 ownership；**只读**。[official repo](https://github.com/apify/mcpc)。

### ACP Registry / catalog 并集补入（13 项，已核验）

#### `5ire` — 5ire Desktop

- **身份/探针**：开源 Electron 桌面 MCP 客户端；macOS `/Applications/5ire.app`，bundle id `app.5ire.desktop`。
- **MCP / Models**：当前源码把两类资产存入 `<Electron userData>/Database` 的 PGlite 数据库。MCP 记录支持 `stdio`、`http-streamable`、`endpoint`、`config.env` / `config.headers`、`active` 与 `approvalPolicy`；`<userData>/mcp.json`、`config.json` 只是旧版迁移入口，不是 live source of truth。
- **Skills**：未找到 Agent Skills / `SKILL.md` 扫描契约。
- **MUX 结论**：**只读**。不能把旧迁移文件登记成 writer，也不直写应用内部数据库。
- **证据**：[official repo](https://github.com/nanbingxyz/5ire)、[official site](https://5ireai.com/)。**状态：已证实（official source）**。

#### `agoragentic-acp` — Agoragentic ACP

- **身份/探针**：ACP Registry 启动身份为 `npx agoragentic-mcp@1.3.0 --acp`；这是 Agoragentic 托管 marketplace 的 MCP/ACP 适配器。
- **MCP / Models**：只接受 `AGORAGENTIC_API_KEY`、`AGORAGENTIC_MCP_URL`、`AGORAGENTIC_BASE_URL` 等服务连接环境变量；没有“配置其他 MCP Server”的用户文件，也没有 BYO model profile store。
- **Skills**：仓库中的 `SKILL.md` 是产品使用资料，源码没有证明运行时扫描用户 Skills。
- **MUX 结论**：**只读**。这是有效 ACP Agent，不是误分类，但没有本地资产 writer 契约。
- **证据**：[official integrations repo](https://github.com/rhein1/agoragentic-integrations)、[official ACP Registry manifest](https://github.com/agentclientprotocol/registry/blob/cc4eb37906eb477a0b0a5e46a7312cbf25366aef/agoragentic-acp/agent.json)。**状态：已证实（official source）**。

#### `autohand` — Autohand Code

- **身份/探针**：commands `autohand` / `autohand-acp`；默认 home `~/.autohand`，支持 `AUTOHAND_HOME` 和显式 `AUTOHAND_CONFIG`。
- **MCP**：按现存格式读取 `config.toml|config.yaml|config.yml|config.json`；多个格式同时存在会报错，因此 writer 必须更新唯一现存文件，不能固定新建 JSON。schema 为 `mcp.enabled` + `mcp.servers[]`，元素 `{name,transport:"stdio"|"sse"|"http",command?,args?,url?,env?,headers?,autoConnect?}`；项目覆盖 `.autohand/settings.local.json`。
- **Skills**：native user `~/.autohand/skills`、project `.autohand/skills`；也读取 `~/.codex/skills`、`~/.claude/skills`、`~/.agent/skills`、`~/.agents/skills` 及项目共享目录；标准 `SKILL.md`。
- **Models**：同一多格式配置含 provider/custom provider、base URL/model(s)/`apiKey`；涉及秘密与多格式原子更新，本轮不开放。
- **MUX 结论**：**MCP + Skills 可提升**。MCP codec 必须先 resolve format/home override，并局部 merge；Skills 首选 native user directory。
- **证据**：[official ACP repo](https://github.com/autohandai/autohand-acp)、[official CLI repo](https://github.com/autohandai/code-cli)。**状态：已证实（official source）**。

#### `cortex-code` — Snowflake Cortex Code

- **身份/探针**：official CLI/desktop coding agent；ACP command `cortex acp serve`。
- **MCP**：user `~/.snowflake/cortex/mcp.json`，根键 `mcpServers`。stdio `{type:"stdio",command,args,cwd?,env?}`；remote `{type:"http"|"sse",url,headers?}`；通用 `timeout`，HTTP 可含 OAuth 字段。支持 `${VAR}`、`${VAR:-default}`、`$VAR`。禁用集合另存 `~/.snowflake/cortex/mcp-disabled.json`；管理员策略可能禁止 user MCP，应用后必须验证实际生效。产品会把明文凭据迁移到 Keychain，MUX 应优先保留/写 env 引用。
- **Skills**：native user `~/.snowflake/cortex/skills/<name>/SKILL.md`，另读 `~/.claude/skills`；project `.snowflake/cortex/skills` 与新版 `.agents/skills`。`~/.snowflake/cortex/skills.json` 是注册源/收藏/cache 状态，不作为普通目录安装 writer。
- **Models**：Snowflake 托管 allowlist/session selection，无外部 provider profile writer。
- **MUX 结论**：**MCP + Skills 可提升**；MCP 的 enabled 必须与 disabled sidecar 协调，普通 Skill 写 native directory。
- **证据**：[official MCP docs](https://docs.snowflake.com/en/user-guide/cortex-code/cortex-code-mcp)、[official extensibility docs](https://docs.snowflake.com/en/user-guide/cortex-code/extensibility)、[official Skills docs](https://docs.snowflake.com/en/user-guide/cortex-code/cortex-code-desktop/skills)、[official changelog](https://docs.snowflake.com/en/user-guide/cortex-code/changelog)。**状态：已证实（official docs）**。

#### `corust-agent` — Corust Agent

- **身份/探针**：ACP binary `corust-agent-acp`；official release repository 只含发布清单、图标和许可证。
- **MCP / Models / Skills**：公开 release repo 没有实现源码、稳定用户路径或 schema；官网的 `corust` CLI 与 ACP binary 不能在无官方契约时合并为同一配置目标。
- **MUX 结论**：**只读**。它是有效 ACP Agent，不是误分类；等待官方配置文档或 API。
- **证据**：[official release repo](https://github.com/Corust-ai/corust-agent-release)、[official site](https://corust.ai/)。**状态：官方身份已证实，配置契约未公开**。

#### `crow-cli` — Crow CLI

- **身份/探针**：commands `crow-cli` / `uvx crow-cli`。
- **MCP**：user `~/.crow/config.yaml`，根键 `mcpServers` map；元素 `{transport:"stdio"|"http"|"sse",command?,args?,env?,url?,headers?}`。ACP session 传入的 servers 与用户配置合并；未证明持久化 `enabled` 字段，因此移除条目是唯一可靠禁用语义。
- **Models**：同 YAML 的 `providers.<id>{base_url,api_key}` 与 `models.<id>{provider,model}`，秘密由 `~/.crow/.env` 注入并支持 `${VAR}`；需要 YAML + `.env` 双文件 secret-safe transaction，本轮不开放。
- **Skills**：官方明确尚未实现 Agent Skills。
- **MUX 结论**：**MCP 可提升**；Models 等安全双文件 credential projection。
- **证据**：[official repo](https://github.com/crow-cli/crow-cli)、[official site](https://crow-ai.dev/)。**状态：已证实（official source）**。

#### `deepagents` — DeepAgents ACP

- **身份/探针**：official `deepagents-acp` package/CLI，ACP Registry 以 `npx` 分发。
- **MCP**：server 从 ACP `session/new.mcpServers` 接收客户端提供的 servers，不持久化为 DeepAgents user file。
- **Models**：`--model` + provider API-key environment variables，未提供 user-level profile/current pointer。
- **Skills**：CLI 默认只扫描 active workspace 的 `.deepagents/skills` 与 `skills`；`--skills` 可显式指定其他路径。文档示例中的 `~/.deepagents/skills` 只是显式参数，不是默认 global store。
- **MUX 结论**：**只读**。需要 Agent invocation/instance 模型才能管理 CLI flags，不能把 cwd 目录伪装成 global asset。
- **证据**：[official repo](https://github.com/langchain-ai/deepagentsjs)、[ACP package README](https://github.com/langchain-ai/deepagentsjs/tree/b215e70d880011e26d4d9225ac9b25c9ed0a1d80/libs/acp)、[`cli.ts`](https://github.com/langchain-ai/deepagentsjs/blob/b215e70d880011e26d4d9225ac9b25c9ed0a1d80/libs/acp/src/cli.ts)。**状态：已证实（official source）**。

#### `dimcode` — DimAgent / DimCode

- **身份/探针**：command `dim`，ACP mode `dim acp`; default data home `~/.dimcode/v2`, override `DIMCODE_HOME`。
- **MCP**：user `<home>/mcp.json`（default `~/.dimcode/v2/mcp.json`），project `<cwd>/.mcp.json`，根键 `mcpServers`。stdio `{command,args,env,enabled,name?,cwd?}`；HTTP `{type:"http",url,auth?,headers,enabled}`；`streamable-http` 是读入 alias 并写回为 `http`。支持 `${VAR}` / `${VAR:-default}`，缺 required env 时跳过该 server。
- **Models**：provider/model state 位于 `<home>/dimcode.sqlite`，凭据在 `credentials.json` / `credentials/<providerId>.json`；官方 `dim provider` CLI 不等于稳定文件 writer，MUX 不直写 DB 或把 key 放命令行。
- **Skills**：产品已证明支持 SKILL.md discovery/remote install，但公开配置/data-location 文档未给出稳定 native user directory/schema，本轮不从 marketing 文案猜路径。
- **MUX 结论**：**MCP 可提升**；仅管理 user `mcp.json`，支持 home override resolution 和 unknown-field round-trip。
- **证据**：[official config docs](https://dimcode.dev/en/docs/config/)、[official CLI docs](https://dimcode.dev/en/docs/cli/)、[official ACP docs](https://dimcode.dev/en/docs/acp/)。**状态：MCP 已证实；Models storage 已证实；Skills path 未公开**。

#### `dirac` — Dirac

- **身份/探针**：open-source CLI/ACP coding agent，command `dirac`。
- **MCP**：official README 明确 “no MCP” / MCP unsupported。
- **Skills**：global scan includes `~/.dirac/skills`, `~/.agents/skills`, `~/.claude/skills`, `~/.ai/skills`; project scans `.diracrules/skills`, `.dirac/skills`, `.claude/skills`, `.ai/skills`, `.agents/skills`。内置“新建全局 Skill”也写 `~/.agents/skills`，因此 MUX canonical target 选共享 `~/.agents/skills`；标准 `SKILL.md`。
- **Models**：provider settings/current state 和 secrets 分别位于 `~/.dirac/data/globalState.json` / `secrets.json`（且 `DIRAC_DIR` 可重定位），是 Cline-fork app state + secret store，本轮不开放。
- **MUX 结论**：**Skills 可提升**；MCP 明确不支持，Models 只读。
- **证据**：[official repo/README](https://github.com/dirac-run/dirac)、[`skillsStorage.ts`](https://github.com/dirac-run/dirac/blob/542fa0b53e78d0845ac43765e4f8b33ff59a4909/src/core/storage/skillsStorage.ts)、[`createSkillFile.ts`](https://github.com/dirac-run/dirac/blob/542fa0b53e78d0845ac43765e4f8b33ff59a4909/src/core/controller/file/createSkillFile.ts)。**状态：已证实（official source）**。

#### `github-copilot` — GitHub Copilot Language Server ACP

- **身份/探针**：official language server ACP preview，command `npx -y @github/copilot-language-server --acp`。它与已审计的 `copilot-cli` 是不同 runtime identity，不合并配置 ownership。
- **MCP / Models / Skills**：公开 release repo 只证明客户端经 `workspace/didChangeConfiguration` / ACP session 输入提供配置；`~/.jetbrains/acp.json` 是 JetBrains 注册这个 Agent 的 launcher 文件，不是 Copilot 资产 store。模型、认证与能力由 GitHub service/session 管理；未公开该 language-server identity 的 user-level Skills store。
- **MUX 结论**：**只读**。不把 host/client 的配置反向登记为 GitHub Copilot 自有 writer。
- **证据**：[official release repo](https://github.com/github/copilot-language-server-release)、[official README ACP/config sections](https://github.com/github/copilot-language-server-release/blob/c29acc4f71efa45fa5ba6dd59baaa059dac42c1f/README.md#agent-client-protocol-acp-preview)。**状态：已证实（official source）**。

#### `glm-acp-agent` — GLM ACP Agent

- **身份/探针**：official package/command `glm-acp-agent`，native ACP coding agent。
- **MCP**：接受 ACP `session/new.mcpServers` 并在 session 内连接，不存在持久 user MCP file；内置 Vision/Web MCP 由实现固定管理。
- **Models**：default/available/base URL 通过 `ACP_GLM_MODEL`, `ACP_GLM_AVAILABLE_MODELS`, `ACP_GLM_BASE_URL` 等环境变量；per-session `session/set_model` 会写 session state，但没有 user-level model profile file。API key 来自 `Z_AI_API_KEY` 或 `$XDG_CONFIG_HOME/glm-acp-agent/credentials.json`（0600），不应纳入 Model writer。
- **Skills**：只读取 cwd 的 `AGENTS.md` / `CLAUDE.md` project context，未实现 Agent Skills scanner。
- **MUX 结论**：**只读**。未来若 MUX 管理 ACP launcher env，可另建 invocation binding，而不是伪造资产文件。
- **证据**：[official repo/README](https://github.com/stefandevo/glm-acp-agent)、[`credentials.ts`](https://github.com/stefandevo/glm-acp-agent/blob/b9e7288f4ca4575266af0b996a70eded8ff815b2/src/llm/credentials.ts)、[`agent.ts`](https://github.com/stefandevo/glm-acp-agent/blob/b9e7288f4ca4575266af0b996a70eded8ff815b2/src/protocol/agent.ts)。**状态：已证实（official source）**。

#### `harn` — Harn

- **身份/探针**：official binary `harn`; `harn serve acp <agent.harn>` 将 `.harn` agent pipeline 作为 native ACP Agent 运行。
- **MCP**：`[[mcp]]` servers 位于最近项目的 `harn.toml`，支持 stdio/HTTP、lazy/OAuth 等丰富 schema，但没有固定 user-level MCP asset file。
- **Models**：`[llm]` providers/models 同属 project/package manifest，credentials 来自 environment/secret facilities；不是 user profile store。
- **Skills**：稳定 user layer `~/.harn/skills/<name>/SKILL.md`；另有 CLI、env、project、manifest、package、system、host layers。格式兼容 Agent Skills，但 Harn 额外要求 compact `short:` frontmatter card，writer 必须验证/补齐。
- **MUX 结论**：**Skills 可提升**；只管理 user layer。MCP/Models 需 instance/project model 后再考虑。
- **证据**：[official repo](https://github.com/burin-labs/harn)、[official Skills docs](https://harnlang.com/skills.html)、[official MCP/ACP docs](https://harnlang.com/mcp-and-acp.html)。**状态：已证实（official docs/source）**。

#### `minion-code` — Minion Code

- **身份/探针**：commands `mcode` / `minion-code`，ACP `mcode acp`。
- **MCP**：只通过 `--config <path>` 显式载入 JSON（根键 `mcpServers`，stdio/HTTP/SSE）；没有 stable default user MCP file。
- **Models**：`~/.minion/minion-code.json` 只保存单一 default `model` pointer；OpenRouter OAuth/API key 在 `~/.minion/credentials.json`，完整 Minion profile 另有混合/遗留 state。单 pointer 不满足中央 Model profile 的添加/开关/删除契约。
- **Skills**：native user `~/.minion/skills`, shared `~/.claude/skills`; project `.minion/skills`, `.claude/skills`。扫描直接子目录及一层 nested `SKILL.md`；project overrides user。
- **MUX 结论**：**Skills 可提升**，首选 native `~/.minion/skills`；MCP/Models 只读。
- **证据**：[official repo/README](https://github.com/femto/minion-code)、[`skill_loader.py`](https://github.com/femto/minion-code/blob/37e8fa6947665bd7546606370268a51854f2f471/minion_code/skills/skill_loader.py)、[`acp_server/main.py`](https://github.com/femto/minion-code/blob/37e8fa6947665bd7546606370268a51854f2f471/minion_code/acp_server/main.py)。**状态：已证实（official source）**。

### 其余条目的分类覆盖

以下分类解决“是否现在进入 writer”问题；低证据条目仍保留 catalog 入口，不用 Glama 索引推导路径。

| 分类 | ID | 统一结论 |
|---|---|---|
| Web/SaaS/云端或闭源 UI 状态 | `agent-one`, `apidog`, `apigene-mcp-client`, `archestra`, `browse-wiz`, `call-chirp`, `call-my-bot`, `chat-frame`, `chatgpt`, `chatwise`, `chorus`, `claude-ai`, `claude-mind`, `codegpt`, `cody`, `github-copilot-coding-agent`, `glue`, `heym-mcp-client`, `highlight-ai`, `ibm-bob`, `jenova`, `jetbrains-ai-assistant`, `klavis-ai-slack-discord-web`, `langdock`, `lovable`, `lutra`, `microsoft-365-copilot`, `microsoft-copilot-studio`, `mindpal`, `mistral-ai-le-chat`, `modelcontextchat-com`, `msty-studio` | 只读；需要官方 API/extension connector 或公开 user file，不能猜内部存储。 |
| 本地 app/server，但状态在 DB/app store | `agent-bridge`, `aiaw`, `argo-local-ai`, `avatar-shell`, `cherry-studio`, `deepchat`, `flowdown`, `flujo`, `joey`, `kibitz`, `kiln-ai`, `lang-bot`, `langflow`, `libre-chat`, `moopoint` | 只读；不直写 SQLite/IndexedDB/electron-store/服务端数据库。 |
| 仅项目/cwd/显式路径，没有固定 user writer | `aiql-tuui`, `amazon-q-cli`, `askit-mcp`, `blackbox-cli`, `bob-shell`, `console-chat-gpt`, `copilot-xcode`, `deepgram-saga`, `docker-gordon`, `dolphin-mcp`, `emacs-mcp`, `fast-agent`, `genaiscript`, `jdbcx`, `mcp-assistant`, `mcp-cli-client`, `mcp-client-go`, `mcp-partner`, `mcp-super-assistant`, `mcpbundles`, `mcpc`, `memex` | 保留只读；需要 instance/workspace model 或 stable global contract 后再提升。 |
| SDK/framework/server/plugin/demo/devtool 误分类 | `agentai`, `agenticflow`, `beeai-framework`, `chainlit`, `copilot-mcp`, `daydreams`, `genkit`, `hyperagent`, `inspector`, `lm-kit-net`, `mcp-agent`, `mcp-bundler-for-macos`, `mcp-chatbot`, `mcp-client-chatbot`, `mcp-simple-slackbot`, `mcp-use`, `mcphub`, `mcpjam`, `mcpomni-connect`, `mcps-playground`, `memgraph-lab` | 从“可配置 Agent”移出；可在 marketplace/tooling 索引保留。 |

## 官方可写契约清单（A–M）

> 此表中的 ✅ 表示官方证据已证明存在可写契约，**不等于当前 MUX 的静态 registry + 现有 codec 已能安全实现**。动态 home、多格式择一、sidecar 开关、root-level enable 与非标准 enabled 字段必须先补 resolver/codec，不能直接套 `standard` decoder。

| Agent | MCP | Models | Skills | 关键实现约束 |
|---|---:|---:|---:|---|
| `agent-cli` | ✅ | — | — | macOS userData；`mcp.server`、`envs`、`sse` custom codec |
| `agentkube` | ✅ | — | — | `~/.agentkube/mcp.json`；保留 `enabled` 和未知字段 |
| `astr-bot` | ✅ | — | ✅ | packaged root `~/.astrbot`；支持 `ASTRBOT_ROOT` note |
| `autohand` | ✅ | — | ✅ | resolve `AUTOHAND_HOME` / `AUTOHAND_CONFIG` 与唯一现存 JSON/YAML/TOML；native `~/.autohand/skills` |
| `chatmcp` | ✅ | — | — | remote URL 存在 `command`；`type=streamable|sse` custom codec |
| `cortex-code` | ✅ | — | ✅ | `mcp.json` + `mcp-disabled.json` 协调；管理员策略生效验证；native `~/.snowflake/cortex/skills` |
| `crow-cli` | ✅ | — | — | `~/.crow/config.yaml`；无持久 `enabled`，移除条目才是可靠禁用 |
| `dimcode` | ✅ | — | — | resolve `DIMCODE_HOME`；`streamable-http` 读入 alias、写回 `http` |
| `dirac` | — | — | ✅ | shared consumer；canonical target `~/.agents/skills`；官方明确不支持 MCP |
| `docker-agent` | — | — | ✅ | shared consumer；canonical target `~/.agents/skills` |
| `eca-editor-code-assistant` | ✅ | — | ✅ | XDG paths；JSON-with-comments safe merge；递归 Skills |
| `gptme` | ✅ | — | ✅ | TOML `[[mcp.servers]]` 按 `name` merge；尊重 `config.local.toml` |
| `harn` | — | — | ✅ | native `~/.harn/skills`；写入前确保 `short:` frontmatter |
| `hyper-chat` | ✅ | — | — | default appData only；`type=streamableHttp`；反向 `disabled` |
| `minion-code` | — | — | ✅ | native `~/.minion/skills`；支持一层 nested discovery |

**本分片 Models 增量为 0。** 多个产品确实支持多模型，但它们要么把 key 明文和 app state 混存，要么依赖 env/credential helper/login DB；在 MUX 能把 Keychain asset 安全投影为目标 Agent 可读的 credential reference 之前，不能以“写入了 provider/model 名”冒充“配置可用”。

### 新增 13 项深度核验结果与当前实现就绪度

| Agent | 官方契约 | 当前 MUX 静态 registry | 原因 |
|---|---|---|---|
| `autohand` | MCP + Skills | **候选，暂不直接登记** | `AUTOHAND_HOME` / `AUTOHAND_CONFIG` + JSON/YAML/TOML 唯一现存格式；Skills 也跟随动态 home |
| `cortex-code` | MCP + Skills | **Skills 已登记；MCP 候选** | Skills 有固定 native user dir；MCP enabled 在 `mcp-disabled.json` sidecar，且需管理员策略生效验证 |
| `crow-cli` | MCP | **候选，暂不直接登记** | 无持久 `enabled`，当前 decoder 无法表达“关闭但保留”；Models 还需 YAML + `.env` 双文件事务 |
| `dimcode` | MCP | **候选，暂不直接登记** | `DIMCODE_HOME` 动态路径，且 `streamable-http` alias / auth unknown fields 需 codec |
| `dirac` | Skills | **已登记** | canonical shared target `~/.agents/skills`，标准 `SKILL.md` |
| `harn` | Skills | **候选，暂不直接登记** | 固定目录，但 Harn 额外要求 `short:`；当前原样复制不能保证安装后可加载 |
| `minion-code` | Skills | **已登记** | fixed native `~/.minion/skills`，标准 `SKILL.md`，一层 nested discovery |
| 其余 6 项 | 无 user writer | **只读** | DB/app state、session/launcher 输入、env-only 或未公开 schema |

同理，既有候选也不能因“路径固定”就套 `standard`：`agentkube.enabled`、`astr-bot.active`、`eca-editor-code-assistant.disabled`，以及 `gptme` 的 root `mcp.enabled` + entry `enabled` 都需要对应 decoder/encoder 语义；`chatmcp`、`hyper-chat`、`agent-cli` 本就要求 custom codec。Skills-only 的固定目录消费者可独立于 MCP codec 先登记。

### 实现处置（2026-07-22）

- 本版实际进入 MUX registry：`agentkube`（MCP）、`chatmcp`（MCP）、`docker-agent`（Skills）、`cortex-code`（Skills）、`dirac`（Skills）、`minion-code`（Skills）。
- `agentkube` 对 `enabled=false` 的现存条目采用 fail-closed：扫描不把它当作已启用，更新也不会偷偷重新绑定。
- `chatmcp` 对凭据采用整文件 fail-closed：含 OAuth、token 或 client secret 的条目不进入 MUX inventory；只要文件中存在这类条目，更新、停用、快照与备份整个文件都会在写前拒绝，凭据继续由 ChatMCP 管理。
- `agent-cli`、`astr-bot`、`eca-editor-code-assistant`、`gptme` 与 `hyper-chat` 均未接入 writer。它们的 cwd/XDG/环境变量/overlay 可改变真实配置源；当前 Agent definition 不能解析 effective layer，写默认路径会产生 shadow file 或“写入成功但不生效”。
- Autohand、Cortex MCP、Crow CLI、DimCode 与 Harn Skills 仍是官方契约候选；分别等待多格式/动态 home resolver、disabled sidecar、持久 enable 语义、专用 codec 或 `short:` frontmatter materialization。

## 机器可校验覆盖摘要

```json
{
  "schema_version": 1,
  "shard": "catalog-only-a-m",
  "captured_at": "2026-07-22",
  "identity_count": 114,
  "classification_covered_count": 114,
  "verified_count": 49,
  "official_source_checked_count": 49,
  "configuration_or_storage_contract_verified_count": 45,
  "research_candidate_count": 15,
  "read_only_count": 78,
  "misclassified_count": 21,
  "duplicate_count": 0,
  "status_partition_total": 114,
  "research_candidates": [
    "agent-cli",
    "agentkube",
    "astr-bot",
    "autohand",
    "chatmcp",
    "cortex-code",
    "crow-cli",
    "dimcode",
    "dirac",
    "docker-agent",
    "eca-editor-code-assistant",
    "gptme",
    "harn",
    "hyper-chat",
    "minion-code"
  ],
  "misclassified": [
    "agentai",
    "agenticflow",
    "beeai-framework",
    "chainlit",
    "copilot-mcp",
    "daydreams",
    "genkit",
    "hyperagent",
    "inspector",
    "lm-kit-net",
    "mcp-agent",
    "mcp-bundler-for-macos",
    "mcp-chatbot",
    "mcp-client-chatbot",
    "mcp-simple-slackbot",
    "mcp-use",
    "mcphub",
    "mcpjam",
    "mcpomni-connect",
    "mcps-playground",
    "memgraph-lab"
  ],
  "read_only": [
    "5ire",
    "agent-bridge",
    "agent-one",
    "agoragentic-acp",
    "aiaw",
    "aiql-tuui",
    "amazon-q-cli",
    "apidog",
    "apigene-mcp-client",
    "archestra",
    "argo-local-ai",
    "askit-mcp",
    "avatar-shell",
    "blackbox-cli",
    "bob-shell",
    "browse-wiz",
    "call-chirp",
    "call-my-bot",
    "chat-frame",
    "chatbox",
    "chatgpt",
    "chatty",
    "chatwise",
    "cherry-studio",
    "chorus",
    "claude-ai",
    "claude-mind",
    "codegpt",
    "cody",
    "console-chat-gpt",
    "copilot-xcode",
    "corust-agent",
    "deepagents",
    "deepchat",
    "deepgram-saga",
    "docker-gordon",
    "dolphin-mcp",
    "emacs-mcp",
    "fast-agent",
    "flowdown",
    "flujo",
    "genaiscript",
    "github-copilot",
    "github-copilot-coding-agent",
    "glm-acp-agent",
    "glue",
    "heym-mcp-client",
    "highlight-ai",
    "ibm-bob",
    "jdbcx",
    "jenova",
    "jetbrains-ai-assistant",
    "jetbrains-air",
    "joey",
    "kibitz",
    "kiln-ai",
    "klavis-ai-slack-discord-web",
    "lang-bot",
    "langdock",
    "langflow",
    "libre-chat",
    "lovable",
    "lutra",
    "mcp-assistant",
    "mcp-cli-client",
    "mcp-client-go",
    "mcp-partner",
    "mcp-super-assistant",
    "mcpbundles",
    "mcpc",
    "memex",
    "microsoft-365-copilot",
    "microsoft-copilot-studio",
    "mindpal",
    "mistral-ai-le-chat",
    "modelcontextchat-com",
    "moopoint",
    "msty-studio"
  ],
  "duplicates": [],
  "research_candidate_capabilities": {
    "agent-cli": ["mcp"],
    "agentkube": ["mcp"],
    "astr-bot": ["mcp", "skills"],
    "autohand": ["mcp", "skills"],
    "chatmcp": ["mcp"],
    "cortex-code": ["mcp", "skills"],
    "crow-cli": ["mcp"],
    "dimcode": ["mcp"],
    "dirac": ["skills"],
    "docker-agent": ["skills"],
    "eca-editor-code-assistant": ["mcp", "skills"],
    "gptme": ["mcp", "skills"],
    "harn": ["skills"],
    "hyper-chat": ["mcp"],
    "minion-code": ["skills"]
  },
  "implemented_in_this_release": {
    "agentkube": ["mcp"],
    "chatmcp": ["mcp"],
    "docker-agent": ["skills"],
    "cortex-code": ["skills"],
    "dirac": ["skills"],
    "minion-code": ["skills"]
  },
  "new_13_requires_resolver_or_custom_codec": {
    "autohand": ["mcp", "skills"],
    "cortex-code": ["mcp"],
    "crow-cli": ["mcp"],
    "dimcode": ["mcp"],
    "harn": ["skills"]
  }
}
```

`verified_count=49` 只统计本轮实际打开官方源码/文档完成身份与能力核验的条目；其中 45 个进一步核对了 path/schema 或明确的存储机制。其余 65 项虽已完成 writer 分类，但低证据项维持 catalog-only，不伪装成“官方契约已验证”。机器字段 `research_candidates` / `research_candidate_capabilities` 表示“官方可写契约候选”，不代表当前版本已接线；真实交付以 `implemented_in_this_release` 与最终 capability baseline 为准。`read_only_count + misclassified_count + research_candidate_count + duplicate_count = 114`。
