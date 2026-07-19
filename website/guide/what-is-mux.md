# MUX 是什么

**MUX（MCP Multiplexer）** 是一款面向多 Agent 的中央资产与配置管理工具。它把 MCP、Model Profile 和用户级 Skill 统一放进中央资产库，再让 Claude Code、Codex、Cursor、QoderWork、OpenCode 等 Agent 选择消费。

MUX 会适配不同 Agent 的配置路径、文件格式和字段结构，只修改自己拥有的字段，不覆盖用户的其他设置。Desktop 的 Agent 页面只管理 desired relationship；Agent 文件与 Skill link 作为 observed state 对账，不会因扫描而自动变成中央资产。

![MUX 桌面 App 界面总览](/img/mcps-overview.png)

> 上图各区域详解见 [桌面 App 图文教程](/guide/desktop#界面总览)。

## 它解决什么问题

如果你同时用多个 AI 编码工具（Claude Code、Cursor、VS Code、Codex、Zed……），每个工具都有自己的一份 MCP 配置文件，格式、路径、字段各不相同。给它们都配上同一个 MCP（比如 `filesystem`、`github`、`context7`），你得：

- 找到每个工具的配置文件路径；
- 用它各自的格式（JSON / TOML / YAML）、各自的键名和 map/list 布局写一遍；
- 想改一个 server 的参数，又得挨个改回去。

MUX 把这些 MCP 收进 **一个目录（Registry）**，并用同一种产品逻辑管理 Model 与 Skill：**中央配置一次 → 选择消费者 → 审阅影响 → 事务写入并验证**。MCP/Skills 每个 Agent 可消费多个，Model 同时最多一个。

## 两个前端，一份数据

MUX 有两个界面，它们**共享同一个数据目录 `~/.mux/`**：

| | 说明 |
|---|---|
| **桌面 App** | macOS 应用（Tauri + React）。可视化管理，适合鼠标操作。 |
| **命令行 / TUI** | 原生 Rust 二进制 `mux`。子命令可脚本化；无参数时进入交互式终端界面（TUI）。 |

因为两者都构建在**同一个 Rust 核心 crate（`mux-core`）**之上，数据模型只存在一处。你在桌面里的改动，命令行刷新后可见，反之亦然。

## 核心思路：中央资产与消费关系

MUX **不内置**一份写死的 MCP 清单。你的目录是由**来源（Sources）**拼装出来的：

- **订阅**一个远程 URL（指向一份 MCP 配置文件），MUX 抓取并缓存；
- **导入**一个本地配置文件；
- **手动添加** / 粘贴一个 server；
- 在 Agent 中发现但未纳管的 MCP 只作为**只读外部状态**展示，必须显式导入才进入中央目录。

目录 = 所有已启用受管来源的并集。消费关系单独记录“哪个 Agent 应该使用哪个资产”，不再由文件扫描反推。详见 [核心概念](/guide/concepts)。

## 能做什么（功能一览）

- **浏览目录**：搜索、按来源过滤、看每个 MCP 的传输方式、来源、被哪些 agent 使用、GitHub 仓库。
- **管理消费者**：只在对应 Agent 页修改 desired relationship，审阅后写入（自动备份原文件）；资产详情只读展示影响。
- **状态对账**：区分已同步、待同步、漂移、冲突与只读外部配置，不后台静默覆盖。
- **级联生命周期**：中央更新传播到全部消费者；中央删除同时清理关系和受管 Agent 目标。
- **编辑 / 粘贴**：可视化编辑 MCP 配置，或粘贴一段 `mcpServers` JSON/TOML 自动识别。
- **事务恢复**：中央变化、关系和全部目标一起提交；崩溃重启后验证完整结果，否则从持久化快照回滚。
- **导出生效目录**：把去重后的完整目录导出为标准 MCP JSON。
- **来源管理**：订阅、导入、刷新、启停、删除来源。
- **agent 管理**：新增自定义 agent、编辑其配置文件路径。
- **自动更新**：桌面 App 与独立 CLI 都跟随最新正式版通道。

下一步 → [安装](/guide/install)
