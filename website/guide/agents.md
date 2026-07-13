# 支持的 Agent

MUX 内置 **21 个** AI 编码 Agent 定义，其中 **18 个**有经过核对的全局配置目标。不同产品不只配置文件路径和顶层键不同，单个 MCP 条目的字段也可能不同；MUX 会先转成统一模型，再按目标 Agent 的官方格式写回。

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
| **Qoder** | JSON | `mcpServers` | `~/.qoder/settings.json` |
| **Devin** | JSON | `mcpServers` | （无默认路径，需自行配置） |
| **Kiro** | JSON | `mcpServers` | `~/.kiro/settings/mcp.json` |
| **Junie** | JSON | `mcpServers` | `~/.junie/mcp/mcp.json` |
| **Amazon Q** | JSON | `mcpServers` | `~/.aws/amazonq/default.json` |
| **OpenCode** | JSON | `mcp` | `~/.config/opencode/opencode.json` |
| **Copilot CLI** | JSON | `mcpServers` | `~/.copilot/mcp-config.json` |
| **Cline** | JSON | `mcpServers` | `~/.cline/data/settings/cline_mcp_settings.json` |
| **Continue** | JSON | `mcpServers` | （当前官方方案是工作区 `.continue/mcpServers/`，不作为全局目标） |
| **Warp** | JSON | `mcpServers` | `~/.warp/.mcp.json` |
| **Pi** | JSON | `mcpServers` | `~/.pi/agent/mcp.json` |
| **QoderWork** <img src="/img/agent-qoderwork.png" width="18" style="vertical-align:-4px" /> | JSON | `mcpServers` | （未发现公开、稳定的用户级配置文件接口） |

除 **Claude Desktop** 外，上述可写目标均支持 stdio 与远程 HTTP。`claude_desktop_config.json` 是 Claude Desktop 的本地 MCP 配置，只支持 stdio；远程 MCP 应在 Claude 中作为 Connector 添加，因此 MUX 不会向该文件写入 HTTP 条目。Amazon Q 此处指当前 IDE 共享配置 `default.json`，旧版 `mcp.json` 路径会自动迁移。

## 配置路径可编辑

上面是**默认**路径。每个 Agent 的路径都能在 App / TUI 里改（比如你的 VS Code 装在非标准位置）。没有默认路径的 Agent 不会被安装、扫描或清理，除非用户明确配置路径。位于用户主目录内的路径会折叠成 `~/…`；主目录之外的自定义绝对路径会按原值保存。

## 新增自定义 agent

内置 21 个不够用？可以新增自定义 Agent：给它起名、指定全局配置文件路径、选格式（JSON / TOML）和配置键名。自定义 Agent 使用标准 MCP 条目格式；像 OpenCode 的 `command` 数组、Gemini 的 `httpUrl` 这类专属字段转换只由对应的内置适配器处理。MUX 当前只管理 Agent 的全局配置；桌面命令契约也固定写全局路径，旧调用方即使附带项目参数也不会写入项目文件。

- **桌面 App**：顶部 Agent 图标条右侧点虚线 `+`。
- **TUI**：Agents 屏幕按 `n`。

## 适配器如何工作

MUX 的读写分成两层：`Adapter` 只定位并修改指定 MCP 条目，Agent 专属 `Codec` 负责字段映射。例如 OpenCode 使用 `type: local/remote` 与命令数组，Codex 的远程请求头是 `http_headers`，Gemini 区分 `httpUrl` 与 SSE `url`，Windsurf 使用 `serverUrl`，Cline 的当前共享配置把连接参数放在 `transport` 子对象中，Warp 使用 `working_directory` 并让远程条目通过 `url` 推断传输。Claude Code、Amazon Q、Qoder 等写显式传输类型（Qoder 还保留 `ws`）；Zed、Kiro、Junie、Pi 等按 `command` / `url` 推断传输，不会被强行塞入另一套字段。

更新已有条目时，MUX 只接管连接字段（命令、参数、环境变量、工作目录、URL、请求头和传输类型）；`enabled`、`timeout`、OAuth、工具白名单、审批策略等 Agent 专属字段原样保留。其它顶层键、其它 server、注释、缩进和键顺序也不会因一次条目更新被整体重写。遇到无效 JSON / TOML、错误的 MCP 节点类型、非对象条目或重复 JSON 键时，MUX 会拒绝写入。

停用时，MUX 会先把目标 server 的完整语义条目（含 Agent 专属策略字段）持久化到私有权限的 `~/.mux/settings.json`，删除前再确认线上条目仍与快照一致；恢复时若发现同名条目已被用户或 Agent 重建，则拒绝覆盖。注释与原始排版不属于停用快照，但其它条目和 Agent 顶层配置始终不参与快照或重写。

修改已有文件前，MUX 会先在 `~/.mux/backups/` 创建带 Agent、作用域和时间戳的独立备份；备份失败就拒绝写入。最终内容通过同目录临时文件原子替换，并在替换前再次确认原文件没有被其它进程改动；文件权限与符号链接目标保持不变。Cline 的共享配置还会遵守其跨 IDE / CLI / SDK 的设置锁。

## 核对依据（2026-07-13）

路径与 wire format 以产品官方文档或官方源码为准：

- Claude：[Claude Code MCP](https://code.claude.com/docs/en/mcp)、[Claude Desktop 本地 MCP](https://modelcontextprotocol.io/docs/develop/connect-local-servers)、[Claude 远程 Connector](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp)
- OpenAI / 编辑器：[Codex MCP](https://developers.openai.com/codex/mcp/)、[Cursor MCP](https://docs.cursor.com/context/model-context-protocol)、[VS Code MCP](https://code.visualstudio.com/docs/agent-customization/mcp-servers)、[Zed MCP](https://zed.dev/docs/ai/mcp)
- Agent CLI：[OpenCode MCP](https://opencode.ai/docs/mcp-servers/)、[Gemini CLI MCP](https://geminicli.com/docs/tools/mcp-server/)、[GitHub Copilot CLI MCP](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers)、[Qoder CLI MCP](https://docs.qoder.com/en/cli/mcp-servers)
- 其它内置目标：[Windsurf MCP](https://docs.windsurf.com/windsurf/cascade/mcp)、[Roo Code MCP](https://docs.roocode.com/features/mcp/using-mcp-in-roo)、[Kiro MCP](https://kiro.dev/docs/cli/mcp/configuration/)、[Junie MCP](https://junie.jetbrains.com/docs/junie-cli-mcp-configuration.html)、[Amazon Q IDE MCP](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/mcp-ide.html)
- 源码级核对：[Cline config loader](https://github.com/cline/cline/blob/main/sdk/packages/core/src/extensions/mcp/config-loader.ts)、[Warp 官方 add-mcp-server skill](https://github.com/warpdotdev/warp/blob/master/resources/bundled/skills/add-mcp-server/SKILL.md)、[Pi MCP adapter](https://github.com/nicobailon/pi-mcp-adapter)
- Continue 当前使用 YAML / block 方案，暂无与 MUX JSON/TOML 单条目写入模型等价的稳定目标，因此默认不可写：[Continue MCP](https://docs.continue.dev/customize/mcp-tools)。Devin 与 QoderWork 也因未发现公开、稳定的用户级写入接口而保持不可写。

下一步 → [常见问题](/guide/faq)
