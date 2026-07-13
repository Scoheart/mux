# 支持的 Agent

MUX 内置了 **21 个** AI 编码 agent 的配置适配。每个 agent 的配置文件路径、格式、键名各不相同——MUX 替你处理这些差异，你只管选「装到哪个 agent」。

## 内置列表

| Agent | 格式 | 配置键 | 全局配置路径 |
|---|---|---|---|
| **Claude Code** | JSON | `mcpServers` | `~/.claude.json` |
| **Claude Desktop** | JSON | `mcpServers` | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| **Cursor** | JSON | `mcpServers` | `~/.cursor/mcp.json` |
| **VS Code** | JSON | `servers` | `~/Library/Application Support/Code/User/mcp.json` |
| **Codex** | TOML | `mcp_servers` | `~/.codex/config.toml` |
| **Zed** | JSON | `context_servers` | `~/.config/zed/settings.json` |
| **Windsurf** | JSON | `mcpServers` | `~/.codeium/windsurf/mcp_config.json` |
| **Roo Code** | JSON | `mcpServers` | `~/Library/…/rooveterinaryinc.roo-cline/settings/mcp_settings.json` |
| **Gemini CLI** | JSON | `mcpServers` | `~/.gemini/settings.json` |
| **Qoder** | JSON | `mcpServers` | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` |
| **Devin** | JSON | `mcpServers` | （无默认路径，需自行配置） |
| **Kiro** | JSON | `mcpServers` | `~/.kiro/settings/mcp.json` |
| **Junie** | JSON | `mcpServers` | `~/.junie/mcp/mcp.json` |
| **Amazon Q** | JSON | `mcpServers` | `~/.aws/amazonq/mcp.json` |
| **OpenCode** | JSON | `mcp` | `~/.config/opencode/opencode.json` |
| **Copilot CLI** | JSON | `mcpServers` | `~/.copilot/mcp-config.json` |
| **Cline** | JSON | `mcpServers` | `~/Library/…/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` |
| **Continue** | JSON | `mcpServers` | `~/.continue/config.json` |
| **Warp** | JSON | `mcpServers` | `~/.warp/.mcp.json` |
| **Pi** | JSON | `mcpServers` | `~/.pi/agent/mcp.json` |
| **QoderWork** <img src="/img/agent-qoderwork.png" width="18" style="vertical-align:-4px" /> | JSON | `mcpServers` | `~/.qoderwork/mcp.json` |

## 配置路径可编辑

上面是**默认**路径。每个 agent 的路径都能在 App / TUI 里改（比如你的 VS Code 装在非标准位置）。位于用户主目录内的路径会折叠成 `~/…`；主目录之外的自定义绝对路径会按原值保存。

## 新增自定义 agent

内置 21 个不够用？可以新增自定义 agent：给它起名、指定全局配置文件路径、选格式（JSON / TOML）和配置键名。MUX 当前只管理 Agent 的全局配置。

- **桌面 App**：顶部 Agent 图标条右侧点虚线 `+`。
- **TUI**：Agents 屏幕按 `n`。

## 适配器如何工作

MUX 通过一个 `Adapter` 抽象来读写每个 agent 的文件，安装 / 卸载只增删目标条目，并保留同文件里的其它顶层键、其它 server 和未建模字段。JSON / TOML 会重新序列化，因此缩进、键顺序等纯格式可能变化。

每次写入前都会先给目标文件做一个时间戳备份，存到 `~/.mux/backups/`。

下一步 → [常见问题](/guide/faq)
