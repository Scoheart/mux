# AGENTS.md — MUX

> 作用域：MUX 独立 Git 仓库。用户用法写 README；完整操作规则按任务读取父仓 `llm-wiki/wiki/knowledge/`。

## 架构边界

- Rust `core/` 是 Agent 发现、codec、MCP/Model/Skills 中央资产、消费关系与写入行为的唯一权威；CLI、TUI、Tauri command 和 React 只做薄适配。
- `data/` 是 Agent 与精选资源的 source of truth。新增可写 Agent 时同步 codec、发现、fixture、round-trip、图标 alias 与完整性检查。
- 当前只管理全局 Agent 配置和用户级 Skills；不得重新暴露项目级写入或在多个前端复制 core 编排。
- 顶层 `MCPs`、`Models`、`Skills` 是中央资产生命周期入口；Agent 页面只能选择和解除消费关系，不能创建、导入、编辑或安装资产。外部扫描结果保持只读，只有显式导入才进入中央资产库。

## 安全不变量

- 配置修改必须保留未知字段、注释、格式和非目标策略；损坏、歧义或并发变化时 fail closed，并经过备份、权限收紧、同目录临时文件和原子替换。
- MCP 与 model writer 只能修改各自拥有的字段。API key/token 只存系统 Keychain，不进入配置、日志、fixture、截图或仓库。
- Skills 只保留 `~/.mux/assets/skills/items/` 中央副本并通过已核验用户级目录链接分配；`~/.mux/skills` 仅是旧链接兼容别名。生命周期写操作必须由 core 先 plan，再以原 operation id、候选哈希和风险确认 commit。
- 中央 desired state 与 Agent 投影必须分离提交：中央变更先持久化，每个 `Agent × capability × physical target` 再独立收敛。跨 target 不得回滚已经成功的目标；失败只形成与最小物理 write set 绑定的 incident，并允许无关 Agent、无关能力和无关偏好继续写入。单个 target 内仍必须 fail closed、CAS、备份并原子替换。MCP/Skills 每个 Agent 为 `0..N`；原生支持多模型的 Agent 可安装 `0..N` 个 Model Profile 且最多一个为当前模型，单模型 Agent 仍为 `0..1`。
- 测试必须隔离 `HOME`/`MUX_HOME`，不得访问真实用户配置、Skills 或 Keychain。

## 产品与验证

- 顶层为 `MCPs`、`Models`、`Skills` 中央资产库；Agent 页面统一显示已添加资产与中央选择器，多模型 Agent 还需区分“已添加 / 已启用 / 当前模型”，三类状态由 core 的 desired/observed inventory 提供。UI 保持不透明、克制，并覆盖 `1200x820` 与 `900x600`。
- 当前使用极速交付模式：除非用户在当前任务中明确要求，不运行 `cargo test`、`npm test`、fmt、clippy、图标检查、changed-surface validator 或 push preflight。完成实现并检查 diff 后直接 commit、push `main`，由 Direct Stable 自动发布。
- 自动 Quality workflow 暂停。测试代码继续保留，便于用户明确要求时按需手动运行；生产编译、版本生成、签名、打包与 Release 资产发布仍属于交付必需步骤。
- UI 只验收 `/Applications/MUX.app`，不得用 target/Preview/dev/mock 冒充正式安装版。

## 按需路由

- Registry、codec、Models、更新与发布：[`mux-registry-release.md`](../../../llm-wiki/wiki/knowledge/mux-registry-release.md)
- 测试环境：[`mux-test-isolation.md`](../../../llm-wiki/wiki/knowledge/mux-test-isolation.md)
- 正式安装版 UI：[`mux-ui-review.md`](../../../llm-wiki/wiki/knowledge/mux-ui-review.md)
- Git、记忆与跨仓交付：[`repository-delivery.md`](../../../llm-wiki/wiki/knowledge/repository-delivery.md)

## Git

在本独立仓执行 status、commit、tag 和 push；父仓不得跟踪其内部文件。提交使用 `<type>(<scope>): <summary>` 并在 body 解释原因。不要提交 `target/`、`dist/`、临时 App、截图或本机配置。

- 永久使用 Direct Stable：实现完成并检查 diff 后直接提交并 push `main`。`direct-stable-release.yml` 只处理仍为当前 main head 的普通提交，自动递增 patch、提交 release metadata、创建 Draft，再创建不可变 Stable tag；自动 release commit 自身不会递归升版。
- 自动 Quality 暂停。Direct Stable 在 tag/Draft 落地后从 `main` 显式派发唯一一次 macOS build，使发布构建连续复用 default-branch Rust cache。发布仍必须完成版本、签名、App/DMG、Updater、CLI、完整资产集合和 latest 语义版本顺序检查。
- 不再维护日期窗口、Pre-release、Release Please PR 或 main PR Ruleset。PR 仅用于用户明确要求的可选评审，不再自动触发 Quality。用户明确要求暂不发布时停止 push；直推授权不覆盖无关改动、Stable tag 人工操作或 `/Applications/MUX.app` 替换。
- 功能提交不直接修改 `version.txt`、`CHANGELOG.md` 或 lockfile 版本；这些字段由自动 release commit 统一更新。npm lockfile 只能由 `release-version.mjs` 在无项目 `node_modules` 的临时目录更新；portable dependency closure 失败时不能绕过或手工补 JSON。
- 不手工创建、移动或覆盖 Stable tag，不直接发布 Draft，不以 `--clobber` 修复正式资产。发布缺陷使用新的 main commit 生成下一 patch。
- `RELEASE_PLEASE_TOKEN`、`COPILOT_PAT` 与 Tauri 签名材料只存在于 GitHub Secrets，不进入日志、fixture、文档或仓库。安装版替换仍需独立授权。
