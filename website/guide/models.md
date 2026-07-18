# Models（Beta）

MUX 桌面端把模型端点保存为中央 Model Profile，再由多个兼容 Agent 消费。创建 Profile 本身不会写 Agent 配置；每个 Agent 同时最多选择一个当前 Profile。当前范围包括 Claude Code、Codex、Pi，以及仅提供引导的 Qoder、Grok Build 和 MiniMax Code，用于验证配置语义与安全边界。

## 首批支持

| Agent | 状态 | MUX 写入位置 | 协议 |
|---|---|---|---|
| Claude Code | 自动配置 | `~/.claude/settings.json` | Anthropic Messages |
| Codex | 自动配置 | `~/.codex/config.toml` | OpenAI Responses |
| Grok Build | 官方引导 | `~/.grok/config.toml`（MUX 仅自动管理其中的 MCP） | Anthropic Messages / OpenAI Responses / OpenAI Completions |
| MiniMax Code | 安全引导 | `~/.mavis/config.yaml`（MUX 自动管理独立的 `~/.mavis/mcp.json`） | Anthropic Messages / OpenAI Responses / OpenAI Completions |
| Pi | 自动配置 | `~/.pi/agent/models.json` + `settings.json` | Anthropic Messages / OpenAI Responses / OpenAI Completions |
| Qoder | 官方引导 | `~/.qoder/settings.json`（MUX 不写） | 在 Qoder 的 `/model` 中选择 |

Qoder 的公开文档只提供界面和 `/model` 交互配置，没有公开安全的非交互凭据写入接口。Grok Build 支持 `[model.<name>]` 自定义端点，但每个模型只能直接保存 `api_key` 或引用 `env_key`，没有可按模型调用 Keychain 的凭据命令。MiniMax Code 的签名应用包确认自定义 provider 位于 `~/.mavis/config.yaml`，当前流程把 `options.apiKey` 作为字面量保存。MUX 不会把 API Key 明文塞进设置文件、逆向写加密存储，或为了单个模型接管 Agent 的全局认证。

![MUX 模型接口与 Agent 分配](/img/model-endpoints.png)

## 新建中央 Profile 并选择消费者

1. 打开顶部 **Models**。
2. 新建模型接口，填写协议、Base URL 和模型 ID；API Key 可留空以支持本地无鉴权端点。
3. Profile 保存到中央资产库后，点击卡片并在右侧详情面板选择“管理 Agent”；也可以从 Agent 页的 Model 标签选择一个中央 Profile。
4. 审阅关系变化、目标文件与异常状态后提交。MUX 备份、写入并重新读取验证；成功后重启对应 Agent 会话。

同一个 Profile 可以被多个 Agent 消费，但协议必须兼容；同一个 Agent 同时只能消费一个 Profile。两个入口修改的是同一份 desired relationship，MUX 会在修改磁盘前拒绝不兼容或多选组合。

模型卡片只用于选择，不常驻编辑或删除按钮；管理操作集中在右侧详情面板。新建、编辑、关系变更和删除都先生成计划：编辑会保留关系并传播到全部消费者，删除会级联清理全部受管 Agent 配置和关系。写入进行中时不能意外关闭，失败后可在原位置检查原因并重试。

Agent 配置中心会同时列出 Agent/模型配置文件和 MCP 配置文件。Codex 等产品可能把两类设置放在同一文件；Qoder Desktop、Pi 等产品则使用独立 MCP 文件。这里展示的是核验后的配置目标，不会读取或返回完整文件内容。

## 凭据与写入安全

- API Key 仅存于 macOS Keychain，`~/.mux/settings.json` 只有非敏感配置元数据。
- Claude Code、Codex 和 Pi 的配置只保存系统 Keychain 读取命令，不保存密钥正文。
- 修改已有 Agent 文件前创建 `~/.mux/backups/` 备份；备份失败则不写。
- JSONC 注释、TOML 表、MCP 配置和其他无关字段保持不变。
- 文件在准备写入后发生变化时，MUX 拒绝覆盖。
- Pi 两个配置文件按事务处理；第二个文件失败时回滚第一个文件。
- Profile metadata、全部消费者文件和 desired relationship 属于同一事务；Keychain 变更最后执行。App 重启后会验证完整提交，否则用持久化快照回滚。
- 消费目标存在漂移时，中央更新不会静默覆盖；必须审阅目标并用当前候选哈希显式确认。冲突或并发变化会阻止整个提交。

Claude Code 已启用 Bedrock、Vertex 或 Foundry 路由时，MUX 会拒绝接管，而不是静默覆盖云提供商配置。

## 删除接口

删除会先展示全部消费者和将修改的目标文件。确认后，MUX 在一个事务中清理所有受管 Agent 模型字段、desired relationship、中央 Profile metadata 和对应 Keychain credential；任一未解决的漂移、冲突或并发变化都会阻止删除，不会只删中央记录留下隐式副本。

## 官方依据

- [Claude Code 模型配置](https://code.claude.com/docs/en/model-config)与[设置文件](https://code.claude.com/docs/en/settings)
- [Codex 高级配置](https://developers.openai.com/codex/config-advanced)与[配置字段参考](https://developers.openai.com/codex/config-reference)
- [Pi 自定义模型与 Provider](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md)
- [Qoder 自定义模型](https://docs.qoder.com/user-guide/chat/custom-models)与[CLI `/model`](https://docs.qoder.com/en/cli/model)
- [Grok Build 自定义模型](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md)
- [MiniMax Code 官方下载与产品说明](https://agent.minimax.io/download)
