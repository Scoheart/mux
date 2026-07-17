# AGENTS.md — MUX

> 作用域：MUX 独立 Git 仓库。本文记录架构边界、配置安全和交付规则；用户安装与使用方式写入 README。

## 1. 产品与架构

MUX 统一管理编码 Agent 的 MCP servers、模型端点和用户级 Skills。Rust core 是行为与数据的唯一权威，CLI/TUI 和 Tauri command 只做薄适配；当前 Skills 只在 Desktop 提供入口。

| 路径 | 归属 |
|---|---|
| `core/` | Agent 发现、配置 codec、MCP/model/Skills 写入、备份与安全不变量 |
| `cli/` | CLI/TUI 展示与命令路由 |
| `desktop/src/` | React UI、状态与交互 |
| `desktop/src-tauri/` | Tauri command、sidecar、更新器与桌面集成 |
| `data/` | Agent 与精选资源的 source of truth |
| `website/` | 产品站与文档页面 |
| `.github/workflows/` | CI、预发布、正式发布与 Issue 自动化 |

新增行为先进入 `core/` 并由测试覆盖；不要在 React、Tauri command 或 CLI 中复制配置解析和写入逻辑。

## 2. 数据与操作契约

- catalog 由启用的 sources 组装，优先级为 external remote/local < discovered < manual；用户手工内容优先。
- MCP 身份是 `name::transport`；同名 stdio 与 http 是不同资源，不得只按名称合并、删除或同步。
- manual/discovered 是 `~/.mux/sources/local/` 下的托管 source，不回写 legacy `settings.registry`。
- `settings.json` 只强类型化 MUX 自有 section，其余字段透传；单项变更必须在 `mutate_settings` 锁内完成，不能写回过期的整节快照。
- install/remove/import/clean、registry 变更、同步和启停等编排属于 `core/src/ops.rs`；source 管理属于 `core/src/sources.rs`。
- Agent wire schema 与 transport 限制属于 `core/src/codec.rs`；adapter 选择必须走 `get_agent_adapter`，不能退回通用 serde 重写。
- catalog entry 的连接配置变更会同步到已安装 Agent；说明和标签变更不触发写入。删除远程/本地 source 资源应管理 source，不得伪装为用户自有 entry 删除。
- Skills 使用 `~/.mux/skills/` 中的一份中央副本，通过已核验的用户级 Agent 目录链接分配；不得把中央目录直接作为 Agent 发现目录，也不得复制多份正文代替链接。
- Skills 的安装、导入、更新、分配、修复和删除必须先由 core 生成 plan，再用原 operation id、候选哈希和必要的风险确认 commit；前端不得自行拼写文件变更或绕过 journal 恢复。

## 3. 配置安全

- 当前只管理 Agent 的全局配置和用户级 Skills。项目级 metadata 仅作兼容读取，不新增项目级 MCP、model 或 Skills 写入入口。
- 只修改目标配置文件中的 MCP 节点或 model/provider 节点；必须保留其余用户配置、未知字段、注释、格式和策略。
- 写入必须经过解析校验、权限收紧、备份、同目录临时文件、原子替换和乐观并发检查；损坏或歧义配置应拒写并给出恢复信息。
- Model writer 与 MCP writer 各自维护字段所有权，不得互相覆盖。
- 所有用户路径使用 `~` 或运行时 home 展开，不硬编码 `/Users/...`。
- API key/token 仅存系统 Keychain；不得进入 profile、日志、UI、测试 fixture、截图或仓库。
- Skill 风险分析必须完全在本机进行，不上传正文、内容哈希、文件路径或 findings；预览只按纯文本展示，不执行候选内容。
- 测试必须设置隔离的 `MUX_HOME`/TestHome，禁止读取或写入真实 `~/.mux`、Agent 配置和 Keychain。

## 4. Agent 契约

- Qoder Desktop 与 Qoder CLI 是两个独立 Agent；分别维护名称、图标、检测方式和配置路径。
- Agent 的主配置路径与 MCP 配置路径可能相同，也可能独立；Agent 页面必须同时准确展示，不得推断为同一文件。
- Agent 的 Skills 路径是独立、可选的已核验能力，不能从 MCP 路径推断。只有本机安装探针命中且能力数据含证据的 Agent 才能进入 Skills 分配列表；共享 alias 必须按物理目录归一化并展示全部受影响 Agent。
- 新增/修改 Agent 时同步检查 `data/agents.json`、codec、发现逻辑、图标 alias 和构建时图标完整性门禁。
- 不使用会漂移的 Agent 数量作为文档约束；以当前 data source 和校验脚本为准。

## 5. UI 契约

- 顶层产品区为 `MCPs`、`Models` 与 `Skills`；三者复用一致的 resource workspace、筛选和卡片层级，Skills 生命周期写操作必须经过审阅对话框。
- Agent 页面是配置中心：同页展示配置路径、MCP 路径、模型分配、Skills 简化分配区与 MCP 管理；Skills 的来源、风险、更新、导入、修复和删除仍集中在 Skills 工作区。
- 界面保持不透明、克制、中性；不用背景透穿、卡片套卡片或大量重复说明。
- 顶栏 Agent picker、更新操作与资源主操作在 `1200x820` 和最小 `900x600` 都必须可见，不得横向溢出或裁切。
- 对话框位于 app chrome 之上，背景有完整遮罩；Escape 只关闭最上层，不得同时关闭 inspector 和 dialog。
- 更新入口使用明确的“检查更新”命令，不依赖用户猜测版本号可点击。
- 图标优先使用已有官方品牌资产与 alias；新增 Agent 必须通过图标完整性检查。
- 当前版本不得在 CLI/TUI 增加 Skills 命令或入口；core 接口保留给未来前端复用。

## 6. UI 验收与自动化

涉及打开 MUX、切换 Agent、截图或排查桌面自动化时，先读 `../../../skills/tool/mux-ui-review/SKILL.md`。

- 截图前比较最新稳定 Release、安装 app 与源码版本；安装版落后时，先校验官方资产并直接更新 `/Applications/MUX.app`。
- 遇到 `cgWindowNotFound`、空白或黑屏时，先检查进程、LaunchServices、CGWindow、DOM 和截图合成层；同一假设证据不变时不得重复重试。
- UI 验收只允许启动 `/Applications/MUX.app`；不得启动 target bundle、重命名 Preview app、dev server 或合成 Tauri IPC/browser 页面。
- 真实安装版无法附着或截图时，保留原生进程/窗口证据并报告阻塞；不得用 mock 结果冒充真实界面。
- UI 验收必须覆盖代表性 viewport 与 `900x600`；不能通过放大窗口隐藏裁切。
- 更新成功并完成截图后，清理下载、staging 和回滚副本，保持安装版在验收状态并检查工作树。

## 7. 验证

按改动范围执行；共享契约变更需扩大验证范围。

```bash
cargo fmt --check
cargo test --workspace
(cd desktop && npm test)
(cd desktop && npm run check:agent-icons)
(cd desktop && npm run build)
bash desktop/scripts/prepare-sidecar.sh
(cd desktop/src-tauri && cargo test)
(cd website && npm run build)
```

Tauri 测试前先按项目脚本准备 sidecar。UI 改动还需验证 `1200x820`、`900x600`、控制台错误、横向 overflow 和实际截图。无法执行的验证必须明确说明。

## 8. 发布

- `main` 只生成预发布；正式版只由 annotated `vX.Y.Z` tag 触发。
- 发布前同步 Cargo、npm、Tauri 等版本 source of truth 与 lockfile，运行完整构建和测试。
- 发布后重新下载全部资产，核对版本、哈希、codesign、DMG 内容和 `latest.json`，不要只看 workflow 绿色。
- 更新器检测到只读 DMG 或 App Translocation 时，引导用户替换 `/Applications` 中的 app；不得原地循环重试。

## 9. Git 与文档

- MUX 是独立仓库，在本目录内执行 status、commit、tag 和 push；不要把改动提交到父仓库。
- 提交格式：`<type>(<scope>): <summary>`，body 解释原因。保持 feature、docs 和 release metadata 边界清晰。
- 不提交 `target/`、`dist/`、临时 preview 文件、验收截图或本机配置；仅允许提交放在 `website/public/img/`、经脱敏审查批准的正式文档截图。
- `CLAUDE.md` 仅保留 `@AGENTS.md`；README 写用户用法，本文写 Agent 边界，历史排障进入父仓 `memory/`。
