# Models（Beta）

MUX 桌面端把模型端点保存为中央 Model Profile，再由多个兼容 Agent 消费。创建 Profile 本身不会写 Agent 配置；支持多模型的 Agent 可以保留多个已添加 Profile，但同时只有一个 current profile。当前有 12 个自动写入目标；Qoder 与 MiniMax Code 保留安全引导。

## Profile、Provider 与状态

`provider` 表示真实 API/计费渠道（如 OpenRouter、Anthropic、OpenAI），`model_vendor` 表示模型开发商；例如通过 OpenRouter 使用 Claude 时，两者分别是 `openrouter` 与 `anthropic`。`provider/model` 只用于分类，同一组合可以有多个不同 endpoint 或 credential 的 Profile。

MUX 创建不可编辑的内部 `profile_id`，格式为可读的 `provider-model-random`。普通表单不展示它，详情页“技术详情”可以复制。显示名称在同一 Provider 内自动保持唯一。

Agent 中的一个 Profile 有四种消费状态：未添加、已添加但停用、已启用但非当前、已启用且当前。同步健康度（Synced / Drifted / Conflicted 等）与这四种状态分开；停用或移除当前 Profile 时，审阅页会明确展示确定性的 fallback。

## 导入历史配置

MUX 会只读扫描受支持 Agent 的历史自定义模型配置，在“历史配置”中预览后才接管。它会优先列出当前模型，并把相同 endpoint、protocol、model 与 credential identity 的条目合并为一份中央 Profile，同时保留每个 Agent 的 native provider identity。

预览和持久化计划不包含 credential 正文。Keychain-capable Agent 的明文 Key 可在显式导入时转存 Keychain；环境变量型 Agent 只接管环境变量引用。任意 credential command 不会被执行，明文 Key、共享 native provider 下的兄弟模型和无法解析的配置会保持“需处理”，不会静默覆盖。

## 首批支持

| Agent | 状态 | MUX 写入位置 | MUX 所有字段 |
|---|---|---|---|
| Claude Code | 自动配置 | `~/.claude/settings.json`（JSON/JSONC） | `model`、`apiKeyHelper`、`env.ANTHROPIC_BASE_URL` |
| Codex | 自动配置 | `~/.codex/config.toml` | `model`、`model_provider`、`model_providers.<mux-id>` |
| Grok Build | 自动配置 | `~/.grok/config.toml` | `model.<mux-id>`、`models.default` |
| Pi | 自动配置 | `~/.pi/agent/models.json` + `settings.json` | `providers.<mux-id>`、`defaultProvider`、`defaultModel` |
| OpenCode | 自动配置 | `~/.config/opencode/opencode.json`（JSONC） | `provider.<mux-id>`、`model` |
| Kilo Code CLI | 自动配置 | `~/.config/kilo/kilo.jsonc` | `provider.<mux-id>`、`model` |
| Qwen Code | 自动配置 | `~/.qwen/settings.json`（JSONC） | `modelProviders.<auth>[]`、`model.name`、`security.auth.selectedType` |
| Crush | 自动配置 | `~/.config/crush/crush.json`（JSONC） | `providers.<mux-id>`、`models.large` |
| Mistral Vibe | 自动配置 | `~/.vibe/config.toml` | `[[providers]]`、`[[models]]`、`active_model` |
| Hermes Agent | 自动配置 | `~/.hermes/config.yaml` | `providers.<mux-id>`、`model_aliases.<mux-id>`、`model.default/provider` |
| Factory Droid | 自动配置 | `~/.factory/settings.json`（JSONC） | `customModels[]`、`model` |
| Goose | 自动配置 | `~/Library/Application Support/Block/goose/config/config.yaml` + `custom_providers/<mux-id>.json` | `providers.<mux-id>`、`active_provider`、declarative provider 文件 |
| MiniMax Code | 安全引导 | `~/.mavis/config.yaml`（MUX 自动管理独立的 `~/.mavis/mcp.json`） | 不自动写 Model；当前 provider 流程会保存明文 `options.apiKey` |
| Qoder | 官方引导 | `~/.qoder/settings.json`（MUX 不写） | 在 Qoder 的 `/model` 中选择 |

Claude Code 只接收 Anthropic Messages，Codex 只接收 Responses；其余自动目标按各自能力接受一种或多种 Anthropic Messages、OpenAI Responses、OpenAI Chat Completions。Grok Build、OpenCode/Kilo、Qwen、Crush、Vibe、Hermes、Factory 与 Goose 使用环境变量引用，不导出 Keychain 密钥正文。Qwen 当前 stable 0.20.0 的发布包仍要求 `modelProviders.<auth>` 是数组；MUX 会把自己旧版写出的精确 `{ protocol, models }` wrapper 安全迁移为数组，带未知字段的 wrapper 则拒绝覆盖。Qoder 没有公开安全的非交互凭据写入接口；MiniMax Code 当前自定义 provider 会把 `options.apiKey` 作为字面量保存，因此两者仍由自身界面管理。

![MUX 模型接口与 Agent 分配](/img/model-endpoints.png)

## 新建中央 Profile 并选择消费者

1. 打开顶部 **Models**。
2. 新建模型接口，填写协议、Base URL 和模型 ID；Grok Build 需要认证时，在高级设置填写 API Key 环境变量名，本地无鉴权端点可留空。
3. Profile 保存到中央资产库后，进入对应 Agent 页的 Model 标签，查看当前状态并选择一个兼容 Profile。
4. 审阅关系变化、目标文件与异常状态后提交。MUX 备份、写入并重新读取验证；成功后重启对应 Agent 会话。

同一个 Profile 可以被多个 Agent 消费，但协议必须兼容；同一个 Agent 同时只能消费一个 Profile。两个入口修改的是同一份 desired relationship，MUX 会在修改磁盘前拒绝不兼容或多选组合。

模型卡片只用于选择，不常驻编辑或删除按钮；资产编辑和删除集中在右侧详情面板，Agent 关系只在 Agent 页修改。新建、编辑、关系变更和删除都先生成计划：编辑会保留关系并传播到全部消费者，删除会级联清理全部受管 Agent 配置和关系。Agent 页会区分已同步、配置缺失、漂移、冲突和外部配置；接管外部配置或重新同步 drift 必须单独审阅并明确确认。写入进行中时不能意外关闭，失败后可在原位置检查原因并重试。

Agent 配置中心会同时列出 Agent/模型配置文件和 MCP 配置文件。Codex 等产品可能把两类设置放在同一文件；Qoder Desktop、Pi 等产品则使用独立 MCP 文件。这里展示的是核验后的配置目标，不会读取或返回完整文件内容。

## 凭据与写入安全

- API Key 仅存于 macOS Keychain，`~/.mux/settings.json` 只有非敏感配置元数据。
- Claude Code、Codex 和 Pi 的配置只保存系统 Keychain 读取命令，不保存密钥正文。
- Grok Build 配置只保存 `env_key` 变量名，不写 `api_key`；启动 Grok Build 前需让该变量在其运行环境中可用。
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
- [OpenCode Provider](https://opencode.ai/docs/providers)与[Kilo 自定义模型](https://kilo.ai/docs/code-with-ai/agents/custom-models)
- [Qwen Model Providers](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/model-providers/)（stable 发布包结构以对应版本源码为准）
- [Crush 自定义 Provider](https://github.com/charmbracelet/crush#configuration)与[Mistral Vibe API profiles](https://docs.mistral.ai/vibe/code/cli/api-keys-profiles)
- [Hermes 模型配置](https://hermes-agent.nousresearch.com/docs/user-guide/configuring-models)与[Factory BYOK](https://docs.factory.ai/cli/byok/overview)
- [Goose Provider 配置](https://goose-docs.ai/docs/guides/config-files/)
- [Qoder 自定义模型](https://docs.qoder.com/user-guide/chat/custom-models)与[CLI `/model`](https://docs.qoder.com/en/cli/model)
- [Grok Build 自定义模型](https://docs.x.ai/build/overview#custom-models)与[设置参考](https://docs.x.ai/build/settings/reference#model-id)
- [MiniMax Code 官方下载与产品说明](https://agent.minimax.io/download)
