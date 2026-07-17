# AGENTS.md — MUX

> 作用域：MUX 独立 Git 仓库。用户用法写 README；完整操作规则按任务读取父仓 `memory/reference/`。

## 架构边界

- Rust `core/` 是 Agent 发现、codec、MCP/model/Skills 数据与写入行为的唯一权威；CLI、TUI、Tauri command 和 React 只做薄适配。
- `data/` 是 Agent 与精选资源的 source of truth。新增可写 Agent 时同步 codec、发现、fixture、round-trip、图标 alias 与完整性检查。
- 当前只管理全局 Agent 配置和用户级 Skills；不得重新暴露项目级写入或在多个前端复制 core 编排。

## 安全不变量

- 配置修改必须保留未知字段、注释、格式和非目标策略；损坏、歧义或并发变化时 fail closed，并经过备份、权限收紧、同目录临时文件和原子替换。
- MCP 与 model writer 只能修改各自拥有的字段。API key/token 只存系统 Keychain，不进入配置、日志、fixture、截图或仓库。
- Skills 只保留 `~/.mux/skills/` 中央副本并通过已核验用户级目录链接分配；生命周期写操作必须由 core 先 plan，再以原 operation id、候选哈希和风险确认 commit。
- 测试必须隔离 `HOME`/`MUX_HOME`，不得访问真实用户配置、Skills 或 Keychain。

## 产品与验证

- 顶层为 `MCPs`、`Models`、`Skills`；Agent 页面是三类配置的简化入口。UI 保持不透明、克制，并覆盖 `1200x820` 与 `900x600`。
- 按改动范围运行 `cargo fmt --check`、`cargo test --workspace`、Desktop test/build/icon check、Tauri test 或 Website build；共享契约变更扩大验证。
- UI 只验收 `/Applications/MUX.app`，不得用 target/Preview/dev/mock 冒充正式安装版。

## 按需路由

- Registry、codec、Models、更新与发布：[`mux-registry-release.md`](../../../memory/reference/mux-registry-release.md)
- 测试环境：[`mux-test-isolation.md`](../../../memory/reference/mux-test-isolation.md)
- 正式安装版 UI：[`mux-ui-review.md`](../../../memory/reference/mux-ui-review.md)
- Git、记忆与跨仓交付：[`repository-delivery.md`](../../../memory/reference/repository-delivery.md)

## Git

在本独立仓执行 status、commit、tag 和 push；父仓不得跟踪其内部文件。提交使用 `<type>(<scope>): <summary>` 并在 body 解释原因。不要提交 `target/`、`dist/`、临时 App、截图或本机配置。`main` 生成预发布，正式版只由 annotated `vX.Y.Z` tag 触发。
