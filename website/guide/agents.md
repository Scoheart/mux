# 支持的 Agent

MUX 内置了 **20 个** AI 编码 agent 的配置适配。每个 agent 的配置文件路径、格式、键名各不相同——MUX 替你处理这些差异，你只管选「装到哪个 agent」。

## 内置列表

| Agent | 格式 | 配置键 | 全局配置路径 | 项目作用域 |
|---|---|---|---|:---:|
| **Claude Code** | JSON | `mcpServers` | `~/.claude.json` | ✅ |
| **Claude Desktop** | JSON | `mcpServers` | `~/Library/Application Support/Claude/claude_desktop_config.json` | — |
| **Cursor** | JSON | `mcpServers` | `~/.cursor/mcp.json` | ✅ |
| **VS Code** | JSON | `servers` | `~/Library/Application Support/Code/User/mcp.json` | ✅ |
| **Codex** | TOML | `mcp_servers` | `~/.codex/config.toml` | ✅ |
| **Zed** | JSON | `context_servers` | `~/.config/zed/settings.json` | ✅ |
| **Windsurf** | JSON | `mcpServers` | `~/.codeium/windsurf/mcp_config.json` | — |
| **Roo Code** | JSON | `mcpServers` | `~/Library/…/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | ✅ |
| **Gemini CLI** | JSON | `mcpServers` | `~/.gemini/settings.json` | ✅ |
| **Qoder** | JSON | `mcpServers` | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` | — |
| **Devin** | JSON | `mcpServers` | （无默认路径，需自行配置） | — |
| **Kiro** | JSON | `mcpServers` | `~/.kiro/settings/mcp.json` | ✅ |
| **Junie** | JSON | `mcpServers` | `~/.junie/mcp/mcp.json` | ✅ |
| **Amazon Q** | JSON | `mcpServers` | `~/.aws/amazonq/mcp.json` | ✅ |
| **OpenCode** | JSON | `mcp` | `~/.config/opencode/opencode.json` | ✅ |
| **Copilot CLI** | JSON | `mcpServers` | `~/.copilot/mcp-config.json` | ✅ |
| **Cline** | JSON | `mcpServers` | `~/Library/…/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` | — |
| **Continue** | JSON | `mcpServers` | `~/.continue/config.json` | ✅ |
| **Warp** | JSON | `mcpServers` | `~/.warp/.mcp.json` | ✅ |
| **Pi** | JSON | `mcpServers` | `~/.pi/agent/mcp.json` | ✅ |

> 「项目作用域」= 该 agent 支持在项目目录下放一份局部配置（如 `.mcp.json` / `.cursor/mcp.json`），可用 `mux apply --scope project --project <目录>` 写入。

## 配置路径可编辑

上面是**默认**路径。每个 agent 的路径都能在 App / TUI 里改（比如你的 VS Code 装在非标准位置）。存储时路径一律用 `~/…` 的可移植形式，绝不写死成 `/Users/你的名字/…`。

## 新增自定义 agent

内置 20 个不够用？可以新增自定义 agent：给它起名、指定配置文件路径、选格式（JSON / TOML）和配置键名。

- **桌面 App**：Agents 页 → 新增。
- **TUI**：Agents 屏幕按 `n`。

## 适配器如何工作

MUX 通过一个 `Adapter` 抽象来读写每个 agent 的文件，核心保证是**按单个 server 操作**：安装 / 卸载只动目标那一个条目，**保留同文件里其它 server 的原始字节**。这样即使某个 agent 的配置文件里还有你手写的、MUX 不认识的 server，也不会被破坏。

每次写入前都会先给目标文件做一个时间戳备份，存到 `~/.mux/backups/`。

下一步 → [常见问题](/guide/faq)
