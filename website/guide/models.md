# Models（Beta）

MUX 桌面端可以保存一次模型端点，再应用到多个兼容 Agent。首批范围刻意限制为 Claude Code、Codex、Pi 和 Qoder，用于验证配置语义与安全边界。

## 首批支持

| Agent | 状态 | MUX 写入位置 | 协议 |
|---|---|---|---|
| Claude Code | 自动配置 | `~/.claude/settings.json` | Anthropic Messages |
| Codex | 自动配置 | `~/.codex/config.toml` | OpenAI Responses |
| Pi | 自动配置 | `~/.pi/agent/models.json` + `settings.json` | Anthropic Messages / OpenAI Responses / OpenAI Completions |
| Qoder | 官方引导 | `~/.qoder/settings.json`（MUX 不写） | 在 Qoder 的 `/model` 中选择 |

Qoder 的公开文档只提供界面和 `/model` 交互配置，没有公开安全的非交互凭据写入接口。MUX 不会把 API Key 明文塞进设置文件，也不会逆向写它的加密存储。

![MUX 模型接口与 Agent 分配](/img/model-endpoints.png)

## 新建并应用

1. 打开顶部 **Models**。
2. 新建模型接口，填写协议、Base URL 和模型 ID；API Key 可留空以支持本地无鉴权端点。
3. 可以点击接口卡片，在右侧详情面板中跨 Agent 分配；也可以从顶部选择目标 Agent，在同一页面确认配置路径、选择模型并点击 **应用模型**。
4. MUX 备份并写入后，重启对应 Agent 会话。

同一个接口可以分配给多个 Agent，但协议必须兼容。MUX 会在修改磁盘前拒绝不兼容组合。

Agent 配置中心会同时列出 Agent/模型配置文件和 MCP 配置文件。Codex 等产品可能把两类设置放在同一文件；Qoder Desktop、Pi 等产品则使用独立 MCP 文件。这里展示的是核验后的配置目标，不会读取或返回完整文件内容。

## 凭据与写入安全

- API Key 仅存于 macOS Keychain，`~/.mux/settings.json` 只有非敏感配置元数据。
- Claude Code、Codex 和 Pi 的配置只保存系统 Keychain 读取命令，不保存密钥正文。
- 修改已有 Agent 文件前创建 `~/.mux/backups/` 备份；备份失败则不写。
- JSONC 注释、TOML 表、MCP 配置和其他无关字段保持不变。
- 文件在准备写入后发生变化时，MUX 拒绝覆盖。
- Pi 两个配置文件按事务处理；第二个文件失败时回滚第一个文件。

Claude Code 已启用 Bedrock、Vertex 或 Foundry 路由时，MUX 会拒绝接管，而不是静默覆盖云提供商配置。

## 删除接口

删除会移除 MUX 中的接口元数据、Agent 分配记录和对应 Keychain 密钥，但不会回滚已经写入 Agent 的模型设置。需要切换时，直接应用另一个接口。

## 官方依据

- [Claude Code 模型配置](https://code.claude.com/docs/en/model-config)与[设置文件](https://code.claude.com/docs/en/settings)
- [Codex 高级配置](https://developers.openai.com/codex/config-advanced)与[配置字段参考](https://developers.openai.com/codex/config-reference)
- [Pi 自定义模型与 Provider](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md)
- [Qoder 自定义模型](https://docs.qoder.com/user-guide/chat/custom-models)与[CLI `/model`](https://docs.qoder.com/en/cli/model)
