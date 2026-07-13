# 常见问题

## MUX 会改我 agent 配置里已有的其它 server 吗？

不会。MUX 只定位 MCP 节点中的目标条目，更新受管连接字段；其它顶层键、其它 server、目标条目里的权限 / OAuth / 工具策略、注释和排版都会保留。写入前会先备份，再以原子替换落盘；备份失败、配置结构不合法或文件在写入期间被其它进程修改时都会拒绝写入。

## 为什么 Claude Desktop 里看不到远程 HTTP MCP？

`claude_desktop_config.json` 是本地 MCP 配置，只接收 stdio server。远程 MCP 由 Claude Connector 管理，不是同一个本地文件接口；MUX 会隐藏并拒绝向 Claude Desktop 安装 HTTP 条目。

## 桌面 App 和命令行的数据是分开的吗？

不是。两者共享同一个数据目录 `~/.mux/`，构建在同一个 Rust 核心之上。一端的改动，另一端刷新后立刻可见。你可以只装一个，也可以都装。

## 提示「MUX 已损坏，无法打开」怎么办？

当前发布包未经过 Apple Developer ID 公证，macOS 可能因隔离属性阻止启动，不是应用内容损坏。确认文件来自本项目 Release 后执行：

```bash
xattr -dr com.apple.quarantine /Applications/MUX.app
```

或右键 App → 打开 → 在弹窗里再点「打开」。详见 [安装](/guide/install#提示-mux-已损坏-无法打开)。

## 有 Windows / Linux 版吗？

目前桌面 App 打包发布的是 **macOS（Apple Silicon）** 的 `.dmg`。CLI 是原生 Rust，理论上能在其它平台从源码编译（`cargo install --path cli`），但发布的预编译二进制目前是 macOS aarch64。

## 「停用」和「删除」有什么区别？

- **停用（Disable）**：先保存该 server 的完整语义配置（含 Agent 专属策略），再从 Agent 配置移除；恢复时不会覆盖期间重建的同名条目。适合临时关掉。
- **删除**：从 agent 卸载。对 manual / 探索 条目，还能从目录**彻底删除**（Forget），同时从所有 agent 卸载。

详见 [核心概念](/guide/concepts#安装-开关-删除)。

## 我改了一个目录条目，为什么某个 agent 没更新？

编辑目录条目的连接配置会自动重刷进所有正在使用它的全局 Agent，包括已手改的副本；每个文件都会先备份。仅修改描述或标签不会触发同步。

想强制推送，用**重新同步（Resync）**——桌面编辑器里的按钮，或 TUI Registry 屏幕的 `S` 键。定制过的会被跳过并报告，可选强制覆盖。详见 [编辑传播](/guide/concepts#编辑传播-edit-propagation)。

## 同名的 stdio 和 http 会冲突吗？

不会。MUX 的身份是 **`name::transport`** 组合键，`sse` 归入 `http`。同名的 stdio 与 http 是**两个独立条目**，各自安装、编辑、删除互不影响。

## 目录里的条目从哪来？我能只留一部分吗？

目录是所有**已启用来源**的并集，MUX 不内置写死的清单。TUI 来源屏幕可单独启停来源；桌面 `v1.1.5` 暂时只提供按来源过滤、刷新和删除。停用来源不会删除底层文件。详见 [来源](/guide/concepts#来源-sources)。

## Mux 精选是必须的吗？

不是。它只是一个**可选**的一键订阅（订阅一个内置的整理过的远程来源）。不加它，目录一样能靠你自己的订阅 / 导入 / 手动 / 探索来源工作。

## 数据能在多台机器间同步吗？

MUX 的目录、来源和状态都在 `~/.mux/` 下。主目录内的 Agent 路径会保存为 `~/…`，但主目录外的自定义绝对路径保持原值；跨机器同步后仍需核对 Agent 是否安装在相同位置。

## MUX 如何更新？

桌面 App 启动后静默检查最新正式版，也可以点击顶部 **检查更新**。独立安装的 CLI 使用 `mux upgrade`；随桌面 App 安装到 `~/.local/bin/mux` 的 CLI 会跟随 App 更新。

## 还有问题？

到 [GitHub Issues](https://github.com/Scoheart/mux/issues) 提问或反馈。
