---
layout: home

hero:
  name: "MUX"
  text: "统一管理 MCP 与模型接口"
  tagline: 集中管理 Claude Code、Codex、Cursor、Pi、QoderWork 等 Agent 的 MCP 与常用模型端点，安全写入，不覆盖其他设置。
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/what-is-mux
    - theme: alt
      text: 安装
      link: /guide/install
    - theme: alt
      text: GitHub
      link: https://github.com/Scoheart/mux

features:
  - icon: 🗂️
    title: 一个目录，管所有 agent
    details: 39 个 Agent 已深度核验，其中 38 个可安全写入全局配置；更广的 191 条客户端资料继续保留用于后续核验。同一份 MCP 目录，逐个 Agent 安装、开关或删除。
  - icon: 🔗
    title: 来源驱动，不写死清单
    details: 目录由你订阅的远程 URL、导入的本地文件、手动添加与自动探索共同组成，随来源刷新更新。
  - icon: 🖥️
    title: 桌面 + 命令行
    details: macOS 桌面 App（可视化）与原生 Rust CLI / TUI，共享 ~/.mux，一处改动两端同步。
  - icon: 🔀
    title: 传输感知
    details: stdio / http / sse，还有自定义 type。同名的 stdio 与 http 视为两个独立条目。
  - icon: ✏️
    title: 编辑即传播
    details: 改一个目录条目，自动重刷进"干净"安装的 agent；被手改过的保留不动，也可显式"重新同步"。
  - icon: 🛟
    title: 安全写入
    details: 修改已有文件前先做独立备份；按条目原子更新，并保留其它 server、策略字段、注释与排版。
  - icon: 🔑
    title: 模型接口复用
    details: 首批支持 Claude Code、Codex 与 Pi 复用模型端点；API Key 只存 macOS Keychain。Qoder 保留官方交互配置入口。
---

<div style="max-width: 960px; margin: 48px auto 0; padding: 0 24px;">

## 30 秒了解

**MUX（MCP Multiplexer）** 是一款多 Agent 配置管理工具。它可以集中管理 Claude Code、Codex、Cursor、QoderWork、OpenCode 等 AI Agent 的 MCP 服务，并在桌面预览版中复用常用模型接口。

MUX 会适配不同 Agent 的配置路径与格式，只修改目标 MCP 部分，不覆盖用户的其他设置。桌面端与命令行共享同一份数据，让多 Agent 的 MCP 管理更简单、更安全。

它有两个前端，共享同一份数据目录 `~/.mux/`：

- **桌面 App**（macOS，Tauri + React）—— 可视化管理，鼠标操作。
- **CLI / TUI**（原生 Rust 二进制 `mux`）—— 终端里使用，可脚本化，无参数时进入交互式界面。

两者都构建在同一个 Rust 核心之上，所以数据模型只有一份。

> 新手从 [MUX 是什么](/guide/what-is-mux) 开始；想直接安装，请看 [安装](/guide/install)。

</div>
