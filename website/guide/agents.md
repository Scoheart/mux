# 支持的 Agent

MUX 的 Agent 数据分为两层：

- **可配置目标**：45 个逐项核验的产品定义，其中 44 个有稳定的用户级全局配置文件，可由 MUX 安全读写，并显示在 Agent 选择器中。
- **发现目录**：来自公开 MCP 客户端目录与官方客户端矩阵，只作为后续核验的数据储备。与可配置目标去重后共保留 **196 个**客户端记录，但不再作为单独标签页展示。

没有确认全局文件路径、顶层键和条目结构的客户端只保留来源数据，不进入选择器，也不允许写入。这样可以持续扩大覆盖面，又不会把通用 JSON 猜测写进未知产品配置。

桌面端把 MCP、Model 与 Skill 都视为中央资产：先在顶部 `MCPs`、`Models`、`Skills` 工作区统一创建、导入和维护，再由 Agent 建立消费关系。单个 Agent 页面使用 **MCPs → Model → Skills** 三个标签，只展示该 Agent 期望使用的中央资产；这里不会创建 MCP、填写 Model、解析 Skill 来源或重新安装 Skill。

消费关系通常只在 Agent 页面管理：MCP 与 Skills 每个 Agent 可选择多个，Model 同时最多一个。中央资产详情只负责资产生命周期和只读影响范围，不反向修改 Agent。MUX 再把 Agent 文件或 Skill link 作为 observed state 对账；仅在 Agent 中发现的外部配置保持只读，扫描不会静默接管。检测到历史 MCP / Skill 时，桌面端提供一次显式迁移，把中央资产与原有消费关系作为同一项可恢复事务导入。

## 已核验列表

以下结果基于截至 **2026-07-22** 的官方文档、官方源码或签名应用包；Grok Build 使用 xAI 官方文档核验，MiniMax Code 使用官方签名的 `3.0.51` macOS 应用包核验。

| Agent | 格式 | 配置键 | 用户级全局路径 | 原生传输 |
|---|---|---|---|---|
| [Amp](https://ampcode.com/manual#model-context-protocol-mcp) | JSON | `amp.mcpServers` | `~/.config/amp/settings.json` | stdio / http |
| [Amazon Q Developer IDE](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/mcp-ide.html) | JSON | `mcpServers` | `~/.aws/amazonq/default.json` | stdio / http |
| [Google Antigravity](https://antigravity.google/docs/mcp) | JSON | `mcpServers` | `~/.gemini/config/mcp_config.json` | stdio / http |
| [Augment Code](https://docs.augmentcode.com/cli/integrations) | JSON | `mcpServers` | `~/.augment/settings.json` | stdio / http |
| [BoltAI](https://docs.boltai.com/docs/plugins/mcp-servers) | JSON | `mcpServers` | `~/.boltai/mcp.json` | stdio |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code/mcp) | JSON | `mcpServers` | `~/.claude.json` | stdio / http |
| [Claude Desktop](https://modelcontextprotocol.io/quickstart/user) | JSON | `mcpServers` | `~/Library/Application Support/Claude/claude_desktop_config.json` | stdio |
| [Cline](https://docs.cline.bot/mcp/configuring-mcp-servers) | JSON | `mcpServers` | `~/.cline/data/settings/cline_mcp_settings.json` | stdio / http |
| [CodeBuddy Code](https://www.codebuddy.ai/docs/cli/mcp) | JSON | `mcpServers` | `~/.codebuddy/.mcp.json` | stdio / http |
| [CodeWhale](https://github.com/Hmbown/CodeWhale/blob/main/docs/MCP.md) | JSON | `servers` | `~/.codewhale/mcp.json` | stdio / http |
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
| [Grok Build](https://docs.x.ai/build/features/mcp-servers) | TOML | `mcp_servers` | `~/.grok/config.toml` | stdio / http |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/features/mcp.md) | YAML | `mcp_servers` | `~/.hermes/config.yaml` | stdio / http |
| [JetBrains Junie](https://www.jetbrains.com/help/junie/model-context-protocol-mcp.html) | JSON | `mcpServers` | `~/.junie/mcp/mcp.json` | stdio / http |
| [Kilo Code CLI](https://kilo.ai/docs/automate/mcp/using-in-kilo-code) | JSON | `mcp` | `~/.config/kilo/kilo.jsonc` | stdio / http |
| [Kimi Code CLI](https://moonshotai.github.io/kimi-code/en/customization/mcp) | JSON | `mcpServers` | `~/.kimi-code/mcp.json` | stdio / http |
| [Kiro](https://kiro.dev/docs/mcp/configuration/) | JSON | `mcpServers` | `~/.kiro/settings/mcp.json` | stdio / http |
| [LM Studio](https://lmstudio.ai/docs/app/plugins/mcp) | JSON | `mcpServers` | `~/.lmstudio/mcp.json` | stdio / http |
| [MiniMax Code](https://agent.minimax.io/download) | JSON | `mcpServers` | `~/.mavis/mcp.json` | stdio / http |
| [Mistral Vibe](https://docs.mistral.ai/vibe/code/cli/mcp-servers) | TOML | `mcp_servers` | `~/.vibe/config.toml` | stdio / http |
| [OpenCode](https://opencode.ai/docs/mcp-servers/) | JSON | `mcp` | `~/.config/opencode/opencode.json` | stdio / http |
| [OpenHands CLI](https://docs.openhands.dev/openhands/usage/cli/mcp-servers) | JSON | `mcpServers` | `~/.openhands/mcp.json` | stdio / http |
| [Pi Coding Agent (MCP Adapter)](https://github.com/nicobailon/pi-mcp-adapter) | JSON | `mcpServers` | `~/.pi/agent/mcp.json` | stdio / http |
| [Qoder Desktop](https://docs.qoder.com/user-guide/chat/model-context-protocol) | JSON | `mcpServers` | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` | stdio / http |
| [Qoder CLI](https://docs.qoder.com/en/cli/mcp-servers) | JSON | `mcpServers` | `~/.qoder/settings.json` | stdio / http |
| [QoderWork](https://docs.qoder.com/qoderwork/connectors) | JSON | `mcpServers` | `~/.qoderwork/mcp.json` | stdio / http |
| [Qwen Code](https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/) | JSON | `mcpServers` | `~/.qwen/settings.json` | stdio / http |
| [Roo Code](https://docs.roocode.com/features/mcp/using-mcp-in-roo) | JSON | `mcpServers` | `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | stdio / http |
| [Atlassian Rovo Dev CLI](https://support.atlassian.com/rovo/docs/connect-to-an-mcp-server-in-rovo-dev-cli/) | JSON | `mcpServers` | `~/.rovodev/mcp.json` | stdio / http |
| [Stakpak](https://github.com/stakpak/agent#mcp-proxy-server) | TOML | `mcpServers` | `~/.stakpak/mcp.toml` | stdio / http |
| [Tabnine](https://docs.tabnine.com/main/getting-started/tabnine-agent/mcp-intro-and-setup) | JSON | `mcpServers` | `~/.tabnine/mcp_servers.json` | stdio / http |
| [Visual Studio Code](https://code.visualstudio.com/docs/copilot/chat/mcp-servers) | JSON | `servers` | `~/Library/Application Support/Code/User/mcp.json` | stdio / http |
| [VT Code](https://github.com/vinhnx/VTCode/blob/main/docs/guides/mcp-integration.md) | TOML | `mcp.providers` | `~/.vtcode/vtcode.toml` | stdio / http |
| [Warp](https://docs.warp.dev/knowledge-and-collaboration/mcp) | JSON | `mcpServers` | `~/.warp/.mcp.json` | stdio / http |
| [Windsurf](https://docs.windsurf.com/windsurf/cascade/mcp) | JSON | `mcpServers` | `~/.codeium/windsurf/mcp_config.json` | stdio / http |
| [Zed](https://zed.dev/docs/ai/mcp) | JSON | `context_servers` | `~/.config/zed/settings.json` | stdio / http |

### 需要特别区分的目标

- **Pi**：Pi 核心不内置 MCP。MUX 的定义只适用于已安装社区 `pi-mcp-adapter` 的环境，因此界面明确标为社区扩展。
- **Devin**：产品支持 MCP，但没有核验到稳定的用户级全局文件契约，因此保留在发现数据中，不进入 Agent 选择器。
- **QoderWork**：用户自定义 MCP 保存在 `~/.qoderwork/mcp.json`，使用 `mcpServers`；MUX 不修改客户端数据目录中的内置 MCP。远程连接按官方导入格式写为 `streamable-http` 或 `sse`。
- **Qoder Desktop / Qoder CLI**：两者是独立 Agent。Qoder Desktop 的 MCP 页面编辑 `SharedClientCache/mcp.json`；Qoder CLI 的 user scope 使用 `~/.qoder/settings.json`，MUX 分别扫描和写入。
- **Claude Desktop / BoltAI**：列出的本地文件只原生支持 stdio。远程 MCP 分别由 Claude Connectors 或 BoltAI 的 `mcp-remote` 方案管理。
- **Goose**：通用文档示例使用 `~/.config/goose/config.yaml`，当前 macOS 源码实际采用 `~/Library/Application Support/Block/goose/config/config.yaml`；MUX 按运行时代码定位。
- **Grok Build**：MCP 与自定义模型共用 `~/.grok/config.toml`。MUX 分别局部管理 `mcp_servers`、`[models].default` 和独立的 MUX 模型表，支持三种官方 API backend，并保留其他模型、认证、超时、权限和工具策略。认证只写 `env_key` 变量名，不写密钥正文。
- **MiniMax Code**：主配置与 MCP 配置分离，分别是 `~/.mavis/config.yaml` 和 `~/.mavis/mcp.json`。MUX 可安全管理 `mcpServers`；Models 只提供引导，因为当前自定义 provider 会把 `options.apiKey` 明文写入 YAML。

## Skills 能力

Skills 路径与上表的 MCP 配置路径分别核验，不能互相推断。当前为 MUX 已审计 Agent 中 36 个具有稳定 user-level 契约的产品声明 Skills 能力；运行时只显示本机安装探针命中的 Agent。没有公开稳定用户级目录、只有项目级目录或仅提供 rules/prompts 的产品继续保持只读或不接入 Skills writer。

Skills 分配按物理目录而不是 Agent 名称执行。`~/.agents/skills` 现在同时是 Codex、Goose、Warp 与 Zed 的首选目录，也是多个 Agent 的兼容读取目录，因此一次写入可能影响更多已安装产品。MUX 会在审阅页展示真实影响并归一化重复链接。链接指向同一份可写中央内容，消费者侧修改会形成中央 drift；路径矩阵、安装来源、后台安全校验和当前边界见 [用户级 Skills](/guide/skills#已核验的-agent-路径)。

## 不同 Agent 的格式差异

MUX 不把所有客户端都当成同一种 `mcpServers` JSON：

- OpenCode / Kilo 使用 `type: local|remote`，本地 `command` 是数组。
- Codex 使用 TOML 表和 `http_headers`；Grok Build 使用 `mcp_servers` TOML 表和 `headers`；Mistral Vibe 使用 `[[mcp_servers]]` TOML 列表。
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
