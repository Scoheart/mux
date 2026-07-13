# 支持的 Agent

MUX 的 Agent 数据分为两层：

- **可配置目标**：39 个逐项核验的产品定义，其中 37 个有稳定的用户级全局配置文件，可由 MUX 安全读写。
- **客户端目录**：来自公开 MCP 客户端目录与官方客户端矩阵，只用于发现。和可配置目标去重后，界面共可搜索 **191 个**客户端。

没有确认全局文件路径、顶层键和条目结构的客户端只展示来源，不允许写入。这样可以持续扩大覆盖面，又不会把通用 JSON 猜测写进未知产品配置。

## 已核验列表

以下结果于 **2026-07-14** 逐项对照官方文档或官方源码，并对全部文档链接做了在线可达性检查。

| Agent | 格式 | 配置键 | 用户级全局路径 | 原生传输 |
|---|---|---|---|---|
| [Amp](https://ampcode.com/manual#model-context-protocol-mcp) | JSON | `amp.mcpServers` | `~/.config/amp/settings.json` | stdio / http |
| [Amazon Q Developer](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-mcp-configuration.html) | JSON | `mcpServers` | `~/.aws/amazonq/default.json` | stdio / http |
| [Google Antigravity](https://antigravity.google/docs/mcp) | JSON | `mcpServers` | `~/.gemini/config/mcp_config.json` | stdio / http |
| [Augment Code](https://docs.augmentcode.com/cli/integrations) | JSON | `mcpServers` | `~/.augment/settings.json` | stdio / http |
| [BoltAI](https://docs.boltai.com/docs/plugins/mcp-servers) | JSON | `mcpServers` | `~/.boltai/mcp.json` | stdio |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code/mcp) | JSON | `mcpServers` | `~/.claude.json` | stdio / http |
| [Claude Desktop](https://modelcontextprotocol.io/quickstart/user) | JSON | `mcpServers` | `~/Library/Application Support/Claude/claude_desktop_config.json` | stdio |
| [Cline](https://docs.cline.bot/mcp/configuring-mcp-servers) | JSON | `mcpServers` | `~/.cline/data/settings/cline_mcp_settings.json` | stdio / http |
| [CodeBuddy Code](https://www.codebuddy.ai/docs/cli/mcp) | JSON | `mcpServers` | `~/.codebuddy/.mcp.json` | stdio / http |
| [Codex](https://developers.openai.com/codex/mcp) | TOML | `mcp_servers` | `~/.codex/config.toml` | stdio / http |
| [Continue](https://docs.continue.dev/customize/deep-dives/mcp) | YAML | `mcpServers` | `~/.continue/config.yaml` | stdio / http |
| [GitHub Copilot CLI](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/extend-coding-agent-with-mcp) | JSON | `mcpServers` | `~/.copilot/mcp-config.json` | stdio / http |
| [Crush](https://github.com/charmbracelet/crush#model-context-protocol-mcp) | JSON | `mcp` | `~/.config/crush/crush.json` | stdio / http |
| [Cursor](https://docs.cursor.com/context/model-context-protocol) | JSON | `mcpServers` | `~/.cursor/mcp.json` | stdio / http |
| [Devin](https://docs.devin.ai/work-with-devin/mcp) | - | - | 只读目录 | - |
| [Factory Droid](https://docs.factory.ai/cli/configuration/mcp) | JSON | `mcpServers` | `~/.factory/mcp.json` | stdio / http |
| [Firebender](https://docs.firebender.com/context/mcp/overview) | JSON | `mcpServers` | `~/.firebender/firebender.json` | stdio / http |
| [Gemini CLI](https://geminicli.com/docs/tools/mcp-server/) | JSON | `mcpServers` | `~/.gemini/settings.json` | stdio / http |
| [Goose](https://goose-docs.ai/docs/guides/config-files/) | YAML | `extensions` | `~/Library/Application Support/Block/goose/config/config.yaml` | stdio / http |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/features/mcp.md) | YAML | `mcp_servers` | `~/.hermes/config.yaml` | stdio / http |
| [JetBrains Junie](https://www.jetbrains.com/help/junie/model-context-protocol-mcp.html) | JSON | `mcpServers` | `~/.junie/mcp/mcp.json` | stdio / http |
| [Kilo Code CLI](https://kilo.ai/docs/automate/mcp/using-in-kilo-code) | JSON | `mcp` | `~/.config/kilo/kilo.jsonc` | stdio / http |
| [Kimi Code CLI](https://moonshotai.github.io/kimi-code/en/customization/mcp) | JSON | `mcpServers` | `~/.kimi-code/mcp.json` | stdio / http |
| [Kiro](https://kiro.dev/docs/mcp/configuration/) | JSON | `mcpServers` | `~/.kiro/settings/mcp.json` | stdio / http |
| [LM Studio](https://lmstudio.ai/docs/app/plugins/mcp) | JSON | `mcpServers` | `~/.lmstudio/mcp.json` | stdio / http |
| [Mistral Vibe](https://docs.mistral.ai/vibe/code/cli/mcp-servers) | TOML | `mcp_servers` | `~/.vibe/config.toml` | stdio / http |
| [OpenCode](https://opencode.ai/docs/mcp-servers/) | JSON | `mcp` | `~/.config/opencode/opencode.json` | stdio / http |
| [OpenHands CLI](https://docs.openhands.dev/openhands/usage/cli/mcp-servers) | JSON | `mcpServers` | `~/.openhands/mcp.json` | stdio / http |
| [Pi Coding Agent (MCP Adapter)](https://github.com/nicobailon/pi-mcp-adapter) | JSON | `mcpServers` | `~/.pi/agent/mcp.json` | stdio / http |
| [Qoder](https://docs.qoder.com/user-guide/chat/model-context-protocol) | JSON | `mcpServers` | `~/.qoder/settings.json` | stdio / http |
| [QoderWork](https://qoder.com/qoderwork) | - | - | 只读目录 | - |
| [Qwen Code](https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/) | JSON | `mcpServers` | `~/.qwen/settings.json` | stdio / http |
| [Roo Code](https://docs.roocode.com/features/mcp/using-mcp-in-roo) | JSON | `mcpServers` | `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | stdio / http |
| [Atlassian Rovo Dev CLI](https://support.atlassian.com/rovo/docs/connect-to-an-mcp-server-in-rovo-dev-cli/) | JSON | `mcpServers` | `~/.rovodev/mcp.json` | stdio / http |
| [Tabnine](https://docs.tabnine.com/main/getting-started/tabnine-agent/mcp-intro-and-setup) | JSON | `mcpServers` | `~/.tabnine/mcp_servers.json` | stdio / http |
| [Visual Studio Code](https://code.visualstudio.com/docs/copilot/chat/mcp-servers) | JSON | `servers` | `~/Library/Application Support/Code/User/mcp.json` | stdio / http |
| [Warp](https://docs.warp.dev/knowledge-and-collaboration/mcp) | JSON | `mcpServers` | `~/.warp/.mcp.json` | stdio / http |
| [Windsurf](https://docs.windsurf.com/windsurf/cascade/mcp) | JSON | `mcpServers` | `~/.codeium/windsurf/mcp_config.json` | stdio / http |
| [Zed](https://zed.dev/docs/ai/mcp) | JSON | `context_servers` | `~/.config/zed/settings.json` | stdio / http |

### 需要特别区分的目标

- **Pi**：Pi 核心不内置 MCP。MUX 的定义只适用于已安装社区 `pi-mcp-adapter` 的环境，因此界面明确标为社区扩展。
- **Devin / QoderWork**：产品支持 MCP，但没有核验到稳定的用户级全局文件契约，只能查看来源，不能写入。
- **Claude Desktop / BoltAI**：列出的本地文件只原生支持 stdio。远程 MCP 分别由 Claude Connectors 或 BoltAI 的 `mcp-remote` 方案管理。
- **Goose**：通用文档示例使用 `~/.config/goose/config.yaml`，当前 macOS 源码实际采用 `~/Library/Application Support/Block/goose/config/config.yaml`；MUX 按运行时代码定位。

## 不同 Agent 的格式差异

MUX 不把所有客户端都当成同一种 `mcpServers` JSON：

- OpenCode / Kilo 使用 `type: local|remote`，本地 `command` 是数组。
- Codex 使用 TOML 表和 `http_headers`；Mistral Vibe 使用 `[[mcp_servers]]` TOML 列表。
- Continue 使用 YAML 列表并要求根级 `name`、`version`、`schema`；Goose 和 Hermes 也使用各自的 YAML map。
- Gemini / Qwen 使用 `httpUrl`；Windsurf 和 Antigravity 使用 `serverUrl`。
- Cline 把连接字段放在 `transport` 子对象；Tabnine 把 HTTP 头放在 `requestInit.headers`。
- Rovo、Amazon Q、Augment、OpenHands 等要求显式传输类型；Kimi / Hermes 只在旧 SSE 时写 `transport: sse`。

每个内置目标有独立 codec。升级时，MUX 会更新官方 schema 元数据，但保留用户对启用状态和全局路径的选择。

## 安全写入边界

MUX 会在本机解析 Agent 文件，但只把目标 MCP 条目的结构化连接字段提供给界面。完整配置文件不会进入界面、日志、来源缓存或网络，也不会通过“反序列化整份再重写”的方式覆盖用户配置。

- JSON / JSONC 使用语法树定位目标条目，保留注释、缩进、键顺序、其它 server 和其它顶层设置。
- TOML map 与 TOML list 都做局部编辑；YAML map / list 同样保留未受管内容和注释。
- `enabled`、OAuth、超时、工具白名单、审批策略等 Agent 私有字段原样保留。
- 无效文档、错误节点类型、重复目标键、YAML 多文档、备份失败或并发修改都会拒绝写入。
- 写前创建独立时间戳备份（Unix 下目录 `0700`、文件 `0600`），最终通过同目录临时文件原子替换；符号链接目标和原配置文件权限保持不变。

MUX 当前只管理用户级全局配置，不提供项目级写入。

## 自定义 Agent

桌面 App 的 Agent 选择器旁点 `+`，或在 TUI 的 Agents 屏幕按 `n`，可添加 JSON、TOML 或 YAML 的自定义全局目标。自定义目标使用标准 map 布局；只有已核验内置目标会启用产品专属字段转换。内置目标只允许覆盖路径，避免把官方 schema 意外改成不兼容格式。

下一步 → [常见问题](/guide/faq)
