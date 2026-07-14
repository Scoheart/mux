---
layout: home

hero:
  name: "MUX"
  text: "统一管理 MCP 服务器"
  tagline: 一处配置，管好你所有 AI 编码 agent 的 MCP —— 桌面 App 与命令行共享同一份数据。
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
---

<div style="max-width: 960px; margin: 48px auto 0; padding: 0 24px;">

## 30 秒了解

**MUX（MCP Multiplexer）** 解决一个具体痛点：你在十几个 AI 编码工具里重复配置同一批 MCP 服务器，改一处要改很多遍。MUX 把这些 MCP 收进**一个目录**，让你从一个地方把它们**安装、开关、编辑、删除**到任意 agent。

它有两个前端，共享同一份数据目录 `~/.mux/`：

- **桌面 App**（macOS，Tauri + React）—— 可视化管理，鼠标操作。
- **CLI / TUI**（原生 Rust 二进制 `mux`）—— 终端里使用，可脚本化，无参数时进入交互式界面。

两者都构建在同一个 Rust 核心之上，所以数据模型只有一份。

> 新手从 [MUX 是什么](/guide/what-is-mux) 开始；想直接安装，请看 [安装](/guide/install)。

</div>
