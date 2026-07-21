# MUX Agent 扩充审计（2026-07-22）

## 结论

截图中的 35 个唯一 Agent 不能直接全量照搬。MUX 当前已经覆盖其中 17 个名称或等价产品；剩余候选中，最值得按 MUX 的中央资产模型接入的是：

1. **CodeWhale**：MCP 与 Skills 有稳定的用户级路径；Models 有公开配置，但激活依赖 CLI profile / 环境变量，需单独设计消费契约。
2. **VT Code**：MCP 与 Skills 有公开、结构化配置；Models 的自定义 provider 与 Keyring 交互仍需独立验证后接入。
3. **Stakpak**：MCP 与 Skills 可管理；Models profile 没有稳定的持久化当前指针，不能直接套用 MUX 的单选消费关系。
4. **fast-agent**：能力完整，但存在多个 `FAST_AGENT_HOME`，必须先设计“当前用户环境”选择，不能假定只有一个全局配置。
5. **Dirac**：支持 Models 与 Skills，但官方明确不支持 MCP；可以作为部分能力 Agent 接入。
6. **TRAE CLI**：支持 Models 与 MCP，但默认配置是当前目录的 `trae_config.yaml`，不符合 MUX 当前只写用户级全局配置的规则，暂缓可写接入。

同时应把 **Auggie CLI** 合并到现有 `augment` Agent，把 **Devin CLI** 合并到现有 `devin` Agent，不应新增重复卡片。

## MUX 的接入门槛

MUX 不是 Agent 安装器，而是 MCP、Models、Skills 中央资产库。Agent 只有在存在可验证的用户级配置契约时，才适合成为“可写消费者”：

- MUX 仅管理全局 Agent 配置和用户级 Skills，不能把项目配置误当全局配置（[`AGENTS.md`](../AGENTS.md)）。
- 写入必须保留未知字段、注释和格式；凭据不能进入 Agent 配置或日志，只能使用 Keychain 或环境变量（[`AGENTS.md`](../AGENTS.md)）。
- 当前 Models adapter 只覆盖 14 个 Agent 配置族；新增 Models 消费者必须新增路径、codec、round-trip 和冲突测试（[`core/src/models.rs:421`](../core/src/models.rs#L421)）。
- 本轮审计开始前 Agent catalog 有 42 项；例如 Augment、Devin、Gemini、Grok Build、Kiro 已在 source of truth 中（[`data/agents.json:5`](../data/agents.json#L5)、[`data/agents.json:16`](../data/agents.json#L16)、[`data/agents.json:19`](../data/agents.json#L19)、[`data/agents.json:21`](../data/agents.json#L21)、[`data/agents.json:26`](../data/agents.json#L26)）。

ACP Registry 只能作为“这个 Agent 可以被 ACP Host 启动”的证据，不能作为“MUX 能安全写它的 MCP / Models / Skills 配置”的证据。官方 Registry 的 CI 主要验证 ACP 握手能返回有效的 `authMethods`，并不验证这些资产的本地配置文件。

## 截图与 MUX 现状对账

### 已存在或应合并现有 Agent（17）

| 截图名称 | MUX 处理方式 |
|---|---|
| Amp | 已存在 |
| Auggie CLI | 合并到现有 `augment`，补 `auggie` command probe 和展示别名 |
| Cline | 已存在 |
| Codebuddy Code | 已存在 |
| Cursor | 已存在 |
| Devin CLI | 合并到现有 `devin`，补 CLI probe；没有稳定全局写入契约前继续只读 |
| Factory Droid | 已存在 |
| Gemini CLI | 已存在 |
| goose | 已存在 |
| Grok | 已有 `grok-build` |
| Hermes | 已存在 |
| Junie | 已存在 |
| Kilo | 已有 `kilo-code` |
| Kiro CLI | 已有 `kiro`，且已探测 `kiro-cli` |
| Kimi Code CLI | 已存在 |
| Mistral Vibe | 已存在 |
| Qwen Code | 已存在 |

### 建议新增优先级

| 优先级 | Agent | MCP | Models | Skills | 结论 |
|---|---|---|---|---|---|
| P0 | CodeWhale | `~/.codewhale/mcp.json` | `~/.codewhale/config.toml` | `~/.codewhale/skills/` | 本轮接入 MCP + Skills；Models 待激活契约 |
| P0 | VT Code | `~/.vtcode/vtcode.toml` 的 `[[mcp.providers]]` | 同文件 provider/model/custom providers | `~/.agents/skills/` | 本轮接入 MCP + Skills；Models 待凭据契约 |
| P0 | Stakpak | `~/.stakpak/mcp.toml` / `.json` | `~/.stakpak/config.toml` profiles | `~/.config/stakpak/skills/` | 本轮接入 MCP + Skills；Models 待持久化当前指针 |
| P1 | fast-agent | `fast-agent.yaml` | 同一 YAML 的 providers/models | 支持 Skills 与目录覆盖 | 能力完整，但需先处理多 Home |
| P1 | Dirac | 官方明确不支持 | 可选多 Provider/Model | 自动读取 `.ai`、`.claude`、`.agents` | 只接 Models/Skills，不伪造 MCP |
| P1 | TRAE CLI | `trae_config.yaml` 内 MCP servers | 同文件多 Provider | 未发现稳定用户级 Skills 契约 | 先做探测/只读，等全局配置契约 |
| P2 | DeepAgents | ACP 可运行 | 框架式模型配置 | 更偏 SDK/框架 | 不是稳定的用户级产品配置，暂缓 |
| P2 | GLM Agent | ACP wrapper | 主要绑定 GLM Coding Plan | 未确认 | 可作为厂商 Agent 探测，不优先做中央资产写入 |
| P2 | Poolside | ACP 可运行 | 产品内模型 | 未确认 | 公共配置契约不足，先只读 |
| P2 | Cortex Code | ACP 可运行 | Snowflake 账户绑定 | 未确认 | 企业账户依赖强，先只读 |
| P3 | Agoragentic、Autohand、Corust、crow-cli、DimCode、Minion Code、Nova、siGit | 不完整 | 不完整或产品内固定 | 不完整 | 体量小或公开配置契约不足，收益低于维护成本 |

## 第一批候选详解

### 1. CodeWhale

CodeWhale 已具备最清晰的 MUX 映射：

- 主配置：`~/.codewhale/config.toml`，包含 provider、model、base URL 等。
- MCP：`~/.codewhale/mcp.json`，接受 `servers` 或兼容式 `mcpServers`，支持全局配置。
- Skills：`~/.codewhale/skills/`。
- 官方文档明确将三者列为用户级配置，并支持通过 `CODEWHALE_HOME` 覆盖根目录。

实现时必须检测 `CODEWHALE_HOME`。当它存在时，路径根就是变量值本身，不能再追加 `.codewhale`。凭据字段不得由 MUX 写回；MUX 只负责 provider/model/base URL 等非秘密字段，并使用自己的 credential contract。

来源：[`Hmbown/CodeWhale`](https://github.com/Hmbown/CodeWhale)、[`docs/MCP.md`](https://github.com/Hmbown/CodeWhale/blob/main/docs/MCP.md)、[`docs/CONFIGURATION.md`](https://github.com/Hmbown/CodeWhale/blob/main/docs/CONFIGURATION.md)。

### 2. VT Code

VT Code 的结构与 MUX 产品方向最一致：

- 用户层为 `~/.vtcode/vtcode.toml`，项目层与用户层有清晰优先级。
- MCP 使用 `[[mcp.providers]]`，可表达 stdio 和 HTTP。
- Models 支持 21+ provider 和 `[[custom_providers]]`，可在对话中 `/model` 切换。
- Skills 读取 `~/.agents/skills/`，可直接消费 MUX 的中央 Skill 链接目标。
- API Key 默认存 OS Keyring，配置只保留非秘密元数据。

这是最适合复用 MUX TOML 保真写入、Models adapter 与 Keychain 约束的候选。

来源：[`vinhnx/VTCode`](https://github.com/vinhnx/VTCode)、[`Configuration precedence`](https://github.com/vinhnx/VTCode/blob/main/docs/config/CONFIGURATION_PRECEDENCE.md)、[`Skills guide`](https://github.com/vinhnx/VTCode/tree/main/docs/skills)。

### 3. Stakpak

Stakpak 有稳定用户级文件：

- Models：`~/.stakpak/config.toml` 的 profiles 与 providers。
- MCP：`~/.stakpak/mcp.toml` 或 `~/.stakpak/mcp.json`，支持命令添加和 proxy。
- Skills：当前实现会发现用户级 `~/.config/stakpak/skills/`。

其难点不是能力不足，而是模型与 MCP 分属不同文件，且 MCP 同时接受 TOML/JSON。MUX 应以实际存在的文件决定 codec，两个格式同时存在时 fail closed，不猜优先级。

来源：[`stakpak/agent`](https://github.com/stakpak/agent)。

### 4. fast-agent

fast-agent 同时支持 Models、MCP、Skills，但配置不是唯一的固定用户文件。`fast-agent.yaml` 位于“active fast-agent home”，可通过 `--home` 或 `FAST_AGENT_HOME` 切换；workspace 还可以拥有自己的 `.fast-agent`。

因此它不适合直接硬编码为 `~/.fast-agent/fast-agent.yaml`。若要接入，core 需要先增加“发现所有 Home、选择一个用户级 Home、禁止修改 workspace Home”的契约，否则 MUX UI 中的一次开关可能写错环境。

来源：[`evalstate/fast-agent`](https://github.com/evalstate/fast-agent)。

### 5. Dirac

Dirac 的官方 README 明确写明 **MCP is not supported**，但会自动读取 `.ai`、`.claude`、`.agents` 中的 Skills，并支持多模型 Provider。它可以验证 MUX 是否真正支持“Agent 只消费部分资产”，不应为了卡片完整而显示无效 MCP 能力。

来源：[`dirac-run/dirac`](https://github.com/dirac-run/dirac)。

### 6. TRAE CLI

ByteDance 的开源 `trae-agent` 支持 OpenAI、Anthropic、Doubao、OpenRouter、Ollama、Gemini 等模型，也在 YAML 中支持 MCP servers；但 CLI 默认查找当前目录的 `trae_config.yaml`，没有确认稳定的用户级全局文件。它与 MUX 的“只管理全局配置”冲突。

短期可以加入安装探测和只读说明；等官方提供用户级 config home，或 MUX 未来明确引入项目资产作用域后，再启用写入。

来源：[`bytedance/trae-agent`](https://github.com/bytedance/trae-agent)。

## 为什么不按截图全加

1. **ACP 与资产管理不是一回事。** ACP 解决 Host 启动 Agent、会话与认证；MUX 还要知道 Agent 的用户级文件、数据结构、优先级与安全写入方式。
2. **重复身份会误导用户。** Auggie/Augment、Devin CLI/Devin、Kilo/Kilo Code 应是一个 Agent 的多个 surface/probe，而不是两张独立卡。
3. **项目配置不能偷换成全局配置。** TRAE、fast-agent 等工具允许当前项目配置；MUX 当前产品规则明确禁止重新开放项目级写入。
4. **只读比错误托管更诚实。** 没有官方稳定配置契约的产品可以先显示“已安装/可打开文档”，但不能声称能集中配置。

## 本轮已完成范围

- 已将 CodeWhale、Stakpak、VT Code 加入可配置 Agent，并补齐官方用户级 MCP 路径、产品专属格式适配、Skills 路径、安装探针和品牌图标。
- 已把 `auggie` 命令探针并入现有 Augment 身份，避免重复 Agent 卡片。
- 已增加 JSON、TOML map 与嵌套 `[[mcp.providers]]` 的无损 round-trip 测试，覆盖未知字段和用户策略保留。
- Models 暂不对这三个 Agent 开放：CodeWhale / Stakpak 的当前 profile 依赖运行参数或环境变量，VT Code 的 Keyring 与自定义 provider 凭据绑定尚未形成可验证的无密钥写入契约。后续应独立实现，不用 MCP 路径推断 Models 支持。

## 推荐实施顺序

### Batch A：低风险修正

- `augment` 增加 `auggie` command probe 与 “Auggie CLI” alias。
- `devin` 增加 Devin CLI probe、图标与版本识别；继续保持配置只读。
- UI 对 alias 不新增卡片，只在 Agent 详情显示“检测到的客户端”。

### Batch B：完整中央资产消费者

- 接入 CodeWhale。
- 接入 VT Code。
- 接入 Stakpak。
- 每个 Agent 同步完成 registry、codec、Models adapter、Skills target、probe、fixture、round-trip、unknown-field preservation、credential tests。

### Batch C：部分能力与复杂作用域

- Dirac：Models + Skills。
- fast-agent：先实现 Home 发现/选择，再接三类资产。
- TRAE CLI：暂时只读；用户级配置契约明确后再开启 Models/MCP 写入。

### Batch D：观察名单

- DeepAgents、GLM Agent、Poolside、Cortex Code。
- 其余小型候选只有在出现稳定用户级配置、显著用户需求或上游活跃度提升后再进入。

## 验收标准

- 同名/别名 Agent 不产生重复卡片。
- 每个可写 Agent 都有官方配置证据、用户级路径、冲突规则和安全凭据策略。
- MCP、Models、Skills 可独立出现；不支持的资产不展示伪状态。
- 所有 writer 保留未知字段，损坏或多配置冲突时拒绝写入。
- `HOME`、`CODEWHALE_HOME`、`VTCODE_HOME`、`FAST_AGENT_HOME` 等环境路径全部在隔离测试中覆盖。
- Agent 详情只分配中央资产，不在 Agent 页面创建资产。
