# AGENTS.md — MUX

> 作用域：MUX 独立 Git 仓库。用户用法写 README；完整操作规则按任务读取父仓 `memory/reference/`。

## 架构边界

- Rust `core/` 是 Agent 发现、codec、MCP/Model/Skills 中央资产、消费关系与写入行为的唯一权威；CLI、TUI、Tauri command 和 React 只做薄适配。
- `data/` 是 Agent 与精选资源的 source of truth。新增可写 Agent 时同步 codec、发现、fixture、round-trip、图标 alias 与完整性检查。
- 当前只管理全局 Agent 配置和用户级 Skills；不得重新暴露项目级写入或在多个前端复制 core 编排。
- 顶层 `MCPs`、`Models`、`Skills` 是中央资产生命周期入口；Agent 页面只能选择和解除消费关系，不能创建、导入、编辑或安装资产。外部扫描结果保持只读，只有显式导入才进入中央资产库。

## 安全不变量

- 配置修改必须保留未知字段、注释、格式和非目标策略；损坏、歧义或并发变化时 fail closed，并经过备份、权限收紧、同目录临时文件和原子替换。
- MCP 与 model writer 只能修改各自拥有的字段。API key/token 只存系统 Keychain，不进入配置、日志、fixture、截图或仓库。
- Skills 只保留 `~/.mux/skills/` 中央副本并通过已核验用户级目录链接分配；生命周期写操作必须由 core 先 plan，再以原 operation id、候选哈希和风险确认 commit。
- 中央资产更新和删除必须通过统一计划覆盖全部 desired consumers；未解决的漂移、冲突或并发变化不得部分提交。MCP/Skills 每个 Agent 为 `0..N`，Model 为 `0..1`。
- 测试必须隔离 `HOME`/`MUX_HOME`，不得访问真实用户配置、Skills 或 Keychain。

## 产品与验证

- 顶层为 `MCPs`、`Models`、`Skills` 中央资产库；Agent 页面统一显示“正在使用”与中央选择器，三类状态由 core 的 desired/observed inventory 提供。UI 保持不透明、克制，并覆盖 `1200x820` 与 `900x600`。
- 按改动范围运行 `cargo fmt --check`、`cargo test --workspace`、Desktop test/build/icon check、Tauri test 或 Website build；共享契约变更扩大验证。
- UI 只验收 `/Applications/MUX.app`，不得用 target/Preview/dev/mock 冒充正式安装版。

## 按需路由

- Registry、codec、Models、更新与发布：[`mux-registry-release.md`](../../../memory/reference/mux-registry-release.md)
- 测试环境：[`mux-test-isolation.md`](../../../memory/reference/mux-test-isolation.md)
- 正式安装版 UI：[`mux-ui-review.md`](../../../memory/reference/mux-ui-review.md)
- Git、记忆与跨仓交付：[`repository-delivery.md`](../../../memory/reference/repository-delivery.md)

## Git

在本独立仓执行 status、commit、tag 和 push；父仓不得跟踪其内部文件。提交使用 `<type>(<scope>): <summary>` 并在 body 解释原因。不要提交 `target/`、`dist/`、临时 App、截图或本机配置。

- 临时 Fast Lane：`2026-07-20T08:27:23Z` 至 `2026-07-30T08:27:23Z` 允许直接向 `main` 开发；期间 Release Please PR 仅在当前 head 的 `verify` 通过后自动 squash merge。到期自动恢复 `MUX main delivery` Ruleset 和下述标准 PR/人工批准正式发布流程；Stable tag 不可变规则与所有质量校验始终不变。权威期限记录在 `.github/fast-lane.json`。
- 普通功能改动通过 PR 进入 `main`；`main` 的普通合并生成 Pre-release，并更新唯一一个 Release Please PR。功能 PR 不直接修改 `version.txt`、release-owned manifest 或 lockfile 版本。
- npm lockfile 只能由 `release-version.mjs refresh-locks` 在无项目 `node_modules` 的临时目录生成；`check` 的 portable dependency closure 失败时不能绕过或手工补 JSON。完整 lock 在 Release PR 中只更新版本元数据，不重新解析依赖。
- 只有用户明确批准并合并 `chore(main): release X.Y.Z` Release PR 才进入 Stable 路径。Release Please 创建不可移动的 `vX.Y.Z` tag 和 Draft Release；Desktop workflow 只在相同 SHA 的 `verify` 成功且签名资产完整后发布 Draft。
- 不手工创建、移动或覆盖 Stable tag，不直接发布 Draft，不以 `--clobber` 修复正式资产。发布缺陷使用新的 patch Release PR。
- `RELEASE_PLEASE_TOKEN`、`COPILOT_PAT` 与 Tauri 签名材料只存在于 GitHub Secrets，不进入日志、fixture、文档或仓库。Ruleset 激活、Release PR 合并、正式发布和安装版替换均需当次明确授权。
