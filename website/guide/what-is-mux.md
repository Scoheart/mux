# MUX 是什么

**MUX（MCP Multiplexer）** 是一个跨 AI 编码 agent 统一管理 **MCP（Model Context Protocol）服务器** 的工具。

## 它解决什么问题

如果你同时用多个 AI 编码工具（Claude Code、Cursor、VS Code、Codex、Zed……），每个工具都有自己的一份 MCP 配置文件，格式、路径、字段各不相同。给它们都配上同一个 MCP（比如 `filesystem`、`github`、`context7`），你得：

- 找到每个工具的配置文件路径；
- 用它各自的格式（JSON / TOML）、各自的键名（`mcpServers` / `servers` / `mcp_servers` / `context_servers` / `mcp`）写一遍；
- 想改一个 server 的参数，又得挨个改回去。

MUX 把这些 MCP 收进 **一个目录（Registry）**，让你从一个地方，把任意 MCP **安装 / 开关 / 编辑 / 删除** 到任意 agent —— 每个 agent 该用什么格式、写到哪个文件，MUX 替你处理。

## 两个前端，一份数据

MUX 有两个界面，它们**共享同一个数据目录 `~/.mux/`**：

| | 说明 |
|---|---|
| **桌面 App** | macOS 应用（Tauri + React）。可视化管理，适合鼠标操作。 |
| **命令行 / TUI** | 原生 Rust 二进制 `mux`。子命令可脚本化；无参数时进入交互式终端界面（TUI）。 |

因为两者都构建在**同一个 Rust 核心 crate（`mux-core`）**之上，数据模型只存在一处、两端永不分叉。你在桌面里的改动，命令行里立刻可见,反之亦然。

## 核心思路：来源驱动的目录

MUX **不内置**一份写死的 MCP 清单。你的目录是由**来源（Sources）**拼装出来的：

- **订阅**一个远程 URL（指向一份 MCP 配置文件），MUX 抓取并缓存；
- **导入**一个本地配置文件；
- **手动添加** / 粘贴一个 server；
- **自动探索**你各个 agent 里已有的 MCP。

目录 = 所有已启用来源的并集。删掉/停用一个来源，它的条目就从目录消失。详见 [核心概念](/guide/concepts)。

## 能做什么（功能一览）

- **浏览目录**：搜索、按来源过滤、看每个 MCP 的传输方式、来源、被哪些 agent 使用、GitHub 仓库。
- **安装到 agent**：选一个 MCP，勾选要装到哪些 agent，一键写入（自动备份原文件）。
- **开 / 关**：临时停用一个 MCP（从 agent 配置里移除，但记住它的配置，随时开回来）。
- **删除**：从 agent 卸载，或从目录彻底删除。
- **编辑 / 粘贴**：可视化编辑 MCP 配置，或粘贴一段 `mcpServers` JSON/TOML 自动识别。
- **重新同步**：把编辑后的配置显式推给所有已安装该 MCP 的 agent。
- **来源管理**：订阅、导入、刷新、启停、删除来源。
- **agent 管理**：新增自定义 agent、编辑其配置文件路径。

下一步 → [安装](/guide/install)
