# MUX 用户级 Skills 管理 — 设计文档

- 日期：2026-07-16
- 状态：对话设计已确认，书面 spec 已完成自审
- 范围：Rust core + macOS Desktop；CLI/TUI 暂不增加 Skills 入口

## 1. 背景

MUX 已统一管理编码 Agent 的 MCP servers 和模型端点，但用户级 Agent Skills
仍散落在各工具自己的目录中。安装、更新、风险检查和跨 Agent 分配主要依赖手工
复制、符号链接或外部 CLI，缺少与 MUX 现有安全写入模型一致的可视化管理入口。

Agent Skills 已形成以 `SKILL.md` 为核心的开放格式。现有 `vercel-labs/skills`
证明了“中央副本 + Agent 目录链接”的跨 Agent 模型可行，但 MUX Desktop 不能
要求用户预装 Node.js 或 `npx`，也不能把关键写入和错误恢复委托给不稳定的命令行
输出。因此本功能采用原生 Rust core 实现。

参考：

- Agent Skills 规范：https://agentskills.io/specification
- OpenAI Skills：https://help.openai.com/en/articles/20001066-skills-in-chatgpt
- `vercel-labs/skills`：https://github.com/vercel-labs/skills

## 2. 已确认的产品决策

| 决策点 | 结论 |
|---|---|
| 首版范围 | 只管理用户级 Skills，不管理项目级 Skills |
| 运行依赖 | 完全自包含，不依赖系统 Node.js、`npx` 或 Git |
| 内容存储 | 一份中央副本，各 Agent 通过链接启用或停用 |
| Agent 范围 | 只展示本机已安装、且用户级 Skills 路径已核验的 Agent |
| 内容编辑 | 只预览，不创建或编辑 `SKILL.md` |
| 来源 | 公开 GitHub 仓库和用户主动选择的本地目录 |
| 私有仓库 | 首版不支持 |
| 更新 | 后台只检查；用户查看差异并确认后才更新 |
| 安装分配 | 安装时明确勾选目标 Agent，默认不全选 |
| 既有副本 | 只读扫描；用户确认导入前不移动、不替换 |
| 风险分析 | 完全本地，不上传内容、哈希或文件路径 |
| 高风险 | 展示证据并二次确认；允许用户明确覆盖 |
| Desktop 导航 | 新增与 MCPs、Models 并列的顶层 Skills 工作区 |
| CLI/TUI | 首版不增加入口；core 接口保持可复用 |

## 3. 目标与非目标

### 3.1 目标

1. 统一展示 MUX 托管和各 Agent 目录中发现的用户级 Skills。
2. 从公开 GitHub 仓库或本地目录安装一个或多个 Skills。
3. 将中央副本安全分配给一个或多个已核验 Agent。
4. 支持检查更新、查看文件差异、确认更新、停用和删除。
5. 在落盘前完成格式校验、路径安全检查和本地静态风险分析。
6. 对安装、导入、更新和删除提供可恢复的事务语义。
7. 保持 MUX 现有 Rust-core 权威、薄前端、隔离测试和配置保留约束。

### 3.2 非目标

- 不管理项目目录中的 `.agents/skills`、`.claude/skills` 等项目级内容。
- 不提供 Skills 市场、排行榜或 skills.sh 搜索。
- 不支持私有 GitHub 仓库、GitLab、任意压缩包 URL 或 SSH Git 来源。
- 不运行、测试、编辑或自动修复 Skill 中的脚本和说明。
- 不把静态风险分析包装成安全认证。
- 不在首版增加 CLI/TUI Skills 命令。
- 不在本功能内执行版本发布、签名、公证或 GitHub Release。

## 4. 架构

行为先进入 `mux-core`，Desktop 和未来 CLI 仅发送意图并展示计划或结果。

```text
GitHub / 本地目录 / Agent 外部副本
                 │
                 ▼
        mux-core::skills
        ├── resolver      来源与版本解析
        ├── inventory     中央库、分配和外部副本扫描
        ├── manifest      SKILL.md 解析与规范校验
        ├── audit         本地静态风险分析
        ├── store         暂存、备份和原子替换
        ├── assignments   Agent 链接、停用与修复
        └── ops           plan/commit 编排与 journal 恢复
                 │
       ┌─────────┼──────────────┐
       ▼         ▼              ▼
~/.mux/      ~/.mux/        Agent Skills 目录
skills/      settings.json  （只保存链接）
```

建议将 `core/src/skills.rs` 作为公共入口，并按职责拆到
`core/src/skills/` 子模块。`desktop/src-tauri` 的 command 只转换参数和错误，
React 只维护页面状态，不复制来源解析、风险判断或文件操作。

## 5. Agent Skills 能力声明

现有 `AgentDefinition` 增加可选的嵌套能力，而不是复用 MCP 配置路径：

```rust
pub struct AgentSkillsCapability {
    pub target_id: String,
    pub global_dir: String,
    pub docs: String,
    pub evidence: String,
    pub verified_at: String,
    pub probes: Vec<AgentInstallProbe>,
}

pub enum AgentInstallProbe {
    Path { path: String },
    Command { name: String },
    MacBundle { bundle_id: String },
}
```

`AgentDefinition.skills: Option<AgentSkillsCapability>` 为 `None` 时，该 Agent
不会出现在 Skills 分配列表。能力记录必须同时具备已核验的用户级目录、证据和
至少一个安装探针。

运行时满足任一探针才视为“本机已安装”。Skills 目录本身存在也算有效路径探针，
从而兼容仅安装 CLI 或已手工创建 Skills 目录的用户。探针只读，不启动 Agent。

Agent 配置路径和 Skills 路径分别展示、分别验证，不能由一方推断另一方。

`target_id` 标识一个物理分配目标，通常与 Agent id 相同。`global_dir` 在比较前
必须展开 `~`、解析父目录并规范化；同一 `target_id` 的声明必须解析到同一目录，
不同 `target_id` 不得解析到同一目录。多个已安装 Agent 共用一个 `target_id` 时，
UI 将它们显示为一个“共享目录”目标并列出全部受影响 Agent。用户不能对同一物理
目录做互相矛盾的开关选择。

## 6. 数据模型

### 6.1 托管记录

`Settings` 增加 MUX 自有字段：

```rust
pub struct ManagedSkillRecord {
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    pub resolved_revision: Option<String>,
    pub content_hash: String,
    pub installed_at: String,
    pub updated_at: String,
    pub risk: SkillRiskSummary,
    pub update: SkillUpdateState,
}

pub enum SkillSource {
    Github {
        owner: String,
        repo: String,
        subpath: String,
        requested_ref: String,
    },
    Local {
        path: String,
    },
}
```

- `managed_skills: BTreeMap<String, ManagedSkillRecord>` 按规范化 Skill 名称索引。
- `skill_assignments: BTreeMap<String, BTreeSet<String>>` 保存期望启用的
  `target_id`；UI 再从当前 Agent catalog 派生对应 Agent 列表。
- `skill_update_checked_at: Option<String>` 保存全局最近后台检查时间。
- `Settings.extra` 继续透传未知字段；旧版本无需迁移即可读取新增字段默认值。

`content_hash` 是中央目录规范化文件树的哈希。是否发生本地修改由实际哈希和该值
比较得出，不持久化一个可能过期的布尔值。

### 6.2 文件布局

```text
~/.mux/skills/<skill-name>/               # MUX 中央正文，每个 Skill 一份
~/.mux/staging/skills/<operation-id>/     # 下载、解析与确认候选
~/.mux/backups/skills/<timestamp>/<name>/ # 更新、导入或删除前的备份
~/.mux/journals/skills/<operation-id>.json # 未完成事务及恢复进度
~/.mux/settings.json                      # 来源、版本、哈希、风险与分配
```

中央目录由 MUX 独占管理，不是任何 Agent 的发现目录。这样所有 Agent（包括原生
读取 `~/.agents/skills` 的工具）都必须通过目标目录中的链接启用，安装时的 Agent
选择不会被中央副本绕过。MUX 只对 `managed_skills` 中的记录承担生命周期管理；
发现于中央目录、却没有 MUX 元数据的内容标记为“外部”，不能默认认领。

### 6.3 身份与冲突

Skill 名称遵循 Agent Skills 规范，是用户级唯一标识。同名不同来源不能同时成为
托管记录。安装或导入发现名称冲突时，计划必须同时展示现有来源和候选来源，用户
只能取消或明确选择“备份后替换”。

## 7. Inventory 状态

每次刷新都同时读取 settings 和文件系统，生成派生状态：

| 状态 | 含义 | 默认动作 |
|---|---|---|
| `managed` | 中央副本存在且哈希一致 | 正常管理 |
| `assigned` | Agent 路径链接到中央副本 | 显示已启用 |
| `external` | 中央或 Agent 路径存在独立副本 | 只读展示，可计划导入 |
| `locally_modified` | 中央内容哈希与记录不一致 | 阻止普通更新 |
| `broken_link` | 受管 Agent 链接目标不存在 | 展示修复操作 |
| `conflicting_link` | Agent 路径链接到其他位置 | 拒绝覆盖 |
| `missing` | 元数据存在但中央副本缺失 | 展示恢复或移除元数据 |
| `update_available` | 可移动来源解析出不同 revision/hash | 展示更新提示 |

`settings` 是期望状态，文件系统是实际状态。UI 必须显示两者差异，不能因为
settings 中存在分配就声称链接已生效。

## 8. 来源解析

### 8.1 公开 GitHub

接受：

- `owner/repo`
- `https://github.com/owner/repo`
- 指向仓库 tree 子目录的 GitHub URL

Resolver 使用 GitHub HTTPS API 解析 ref 到不可变 commit SHA，再下载该 commit
对应的 archive。MUX 不调用本机 Git。请求和重定向最终主机只允许
`github.com`、`api.github.com` 和 `codeload.github.com`，且必须使用 HTTPS。

安全解压后扫描包含合法 `SKILL.md` 的目录。一个仓库可返回多个候选，用户选择
需要安装的项。每个托管记录保存仓库、子目录、用户请求的 ref 和已解析 commit。
直接指向不可变 commit 的来源标记为 pinned，不提示普通更新。

### 8.2 本地目录

只允许通过 Tauri 原生文件选择器选择。MUX 复制快照，不建立指向原目录的活链接。
来源路径使用 `~` 规范化后存入 settings。后台检查可重新计算来源目录哈希并提示
变化，但更新仍需重新预览和确认。

### 8.3 后台更新检查

- App 启动且上次检查超过 24 小时时执行一次。
- 用户可随时手动“检查更新”。
- GitHub 仅请求 ref/ETag/commit 元数据；本地来源只读计算哈希。
- 检查不会下载候选正文、改变中央副本或重建链接。
- 用户打开更新确认时才下载候选、计算完整差异并重新执行风险分析。
- GitHub 限流必须显示原因和可重试时间，不循环请求。

## 9. Manifest 与文件校验

候选目录必须在根部包含 `SKILL.md`，其 YAML frontmatter 至少有规范有效的
`name` 和 `description`。目录名最终规范化为 manifest 名称。

校验必须拒绝：

- `..`、绝对路径或任何解压路径穿越。
- 指向候选根目录外的符号链接。
- 设备、socket、FIFO 等特殊文件。
- 超过集中定义上限的下载体积、展开体积、单文件体积或文件数量。
- 无法稳定读取、哈希或复制的候选。

首版限制值集中在 Rust 常量中：HTTP 下载体积 128 MiB、单个 archive 展开体积
512 MiB、单个候选 Skill 总体积 256 MiB、单文件 32 MiB、单个 archive 10,000 个
目录项。错误必须显示实际值和允许值，测试覆盖边界前后值。普通可执行文件和
二进制资源不直接拒绝，但进入风险报告。

## 10. 本地风险分析

Risk Audit 是确定性的静态规则引擎，不运行 Skill 内容，也不进行云端查询。

首版规则至少覆盖：

- 可执行文件、shell/Python/JavaScript 脚本和二进制文件。
- 下载后执行、管道到 shell、提权和系统级安装命令。
- 删除、覆盖、格式化磁盘或大范围文件修改命令。
- 读取常见凭据、Keychain、SSH、云配置或环境变量的指令。
- 向网络端点上传文件、日志、环境或身份数据的指令。
- 编码或混淆 payload、隐藏文件和候选根目录外链接。
- `SKILL.md` 中要求绕过 Agent 安全边界或隐瞒行为的指令。

每条 finding 包含规则 id、规则版本、等级、文件、行号和简短原因。Skill 总等级
取最高 finding，但 UI 使用“未发现高风险模式 / 中风险 / 高风险”等措辞，不声称
“安全”。

高风险安装或更新必须先展开证据，再勾选明确覆盖项并进行第二次确认。报告只留在
本机；不得上传正文、哈希、路径或 finding。

## 11. Plan / Commit 事务

所有有副作用的操作拆为 `plan_*` 与 `commit_*`：

- `plan_install` / `commit_install`
- `plan_import` / `commit_import`
- `plan_update` / `commit_update`
- `plan_remove` / `commit_remove`
- `plan_assignment` / `commit_assignment`
- `plan_repair` / `commit_repair`

`OperationPlan` 包含 operation id、候选哈希、当前中央哈希、目标目录状态、
文件变化、风险 finding、预期 settings 内容哈希和是否需要高风险覆盖。候选内容及
计划元数据位于 staging；取消、App 重启或重新计划都可安全清理未提交候选。

所有 `commit_*` 必须提交 operation id 和候选哈希。高风险覆盖还必须提交由当前
findings 摘要计算的确认摘要；任一值与 plan 不一致即拒绝执行并要求重新审阅。

Commit 流程：

1. 重新校验候选、settings 和全部目标路径前置条件。
2. 任一目标冲突则整体暂停，不产生部分安装。
3. 写入持久 operation journal。
4. 把候选复制到中央目录同级临时路径并 fsync。
5. 备份旧中央副本，使用同目录 rename 原子替换。
6. 通过“临时链接 + rename”为全部 Agent 创建或移除链接。
7. 在 `mutate_settings` 中写入元数据和期望分配。
8. 标记 journal 完成并清理 staging。

任一步失败都按 journal 撤销本操作已经创建的链接、恢复旧中央副本和 settings。
App 启动时必须先恢复未完成 journal，再向 UI 暴露 inventory。

## 12. 生命周期语义

### 12.1 安装

选择来源 → 解析仓库 Skills → 选择 Skills 与目标 Agent → 查看 revision、文件清单、
风险和冲突 → 确认 → 一个事务内安装中央副本并建立全部目标链接。

### 12.2 导入既有副本

外部副本始终先只读展示。用户选择一个外部副本作为导入来源后，MUX 复制并校验
候选，展示与其他同名副本的差异。确认后先建立中央副本，再备份原 Agent 目录，
最后用中央链接替换。导入成功前不移动原文件。

### 12.3 Agent 分配

- 启用：目标为空时创建指向中央副本的链接。
- 停用：只移除与 MUX 记录和中央目标都匹配的链接。
- 真实目录、未知链接或并发变化：拒绝覆盖，转入冲突处理。
- 分配失败时不改变 settings 的期望状态。

### 12.4 更新

更新确认页展示 revision、文件增删改、文本差异和重新计算的风险。中央副本有本地
修改时只能取消，或选择“备份后替换”；首版不做自动合并。更新成功后所有 Agent
链接自动看到新内容，无需逐个复制。

### 12.5 删除

删除计划列出所有受影响 Agent。确认后先移除受管链接，再把中央副本移入时间戳
备份，最后删除托管元数据。首版不提供永久清空备份按钮。

## 13. Desktop UI

### 13.1 顶层导航

`View` 新增 `{ kind: "skills" }`，在 MCPs、Models 旁增加 Skills。Skills 使用现有
`ResourceWorkspace`，保持搜索、左侧 facet、卡片网格和右侧 Inspector 的视觉与
交互层级一致。

### 13.2 Skills 工作区

左侧 facet：

- 状态：全部、有更新、需处理、外部。
- 来源：GitHub、本地。
- Agent：仅已安装且能力已核验的 Agent。

Toolbar：搜索、检查更新、安装 Skill。

卡片展示名称、描述、来源、revision、本地风险等级、更新状态和 Agent stack。
Inspector 展示：

- 来源、revision、内容哈希和更新时间。
- 文件树与纯文本 `SKILL.md` 预览。
- 风险 findings 及文件/行号证据。
- Agent 分配开关。
- 更新、导入、修复和删除操作。

预览必须按纯文本渲染，不执行 Skill 中的 HTML、脚本或远程资源。

### 13.3 安装与更新对话框

安装分三步：

1. 输入 GitHub 来源或选择本地目录。
2. 选择发现的 Skills 和目标 Agent；目标默认不全选。
3. 查看 revision、文件、差异、风险和冲突，确认执行。

更新和导入复用第三步的审阅组件。高风险在普通确认后增加明确的第二次确认。
对话框保持在 app chrome 和 Inspector 之上，Escape 只关闭最上层。

### 13.4 Agent 配置中心

Agent 页面增加简化 Skills section，只展示已分配 Skills、启停开关和“添加 Skill”。
来源、更新、风险和删除跳转至 Skills 工作区，避免两个完整管理入口。

### 13.5 响应式约束

- `1200×820`：左 sidebar、网格和 Inspector 同时显示。
- `900×600`：主操作、Agent picker 和更新入口仍可见；Inspector 使用现有窄屏策略，
  不允许横向溢出或通过放大窗口隐藏问题。
- 安装对话框内容可纵向滚动，底部确认操作固定可见。

## 14. Tauri API

建议 command：

```text
list_skills_inventory
list_skill_agents
resolve_skill_source
plan_skill_install        commit_skill_install
plan_skill_import         commit_skill_import
plan_skill_update         commit_skill_update
plan_skill_remove         commit_skill_remove
plan_skill_assignment     commit_skill_assignment
plan_skill_repair         commit_skill_repair
check_skill_updates
cancel_skill_operation
```

Command 返回结构化错误码和可展示消息，不要求前端解析字符串。网络、校验、冲突、
高风险确认、并发变化和恢复失败使用不同错误类型。

## 15. 错误处理

- 网络失败或 GitHub 限流：不改变本地状态，显示可重试时间。
- 计划之后来源或目标变化：计划失效，必须重新审阅。
- Agent 目标已有真实目录或未知链接：不覆盖，提供导入或取消。
- 中央副本本地修改：阻止普通更新，允许备份后替换。
- 损坏或丢失的中央副本：保留元数据，提供从来源恢复或移除记录。
- 断链：仅当目标路径为空且中央副本哈希有效时允许修复。
- journal 恢复失败：Skills 工作区进入只读恢复状态，不继续新的写操作。
- 多 Skill、多 Agent 操作：任一前置条件失败则不提交；运行时失败必须回滚。

## 16. 测试策略

### 16.1 Rust 单元测试

- manifest 正常、缺字段、非法名称、重复名称和编码错误。
- GitHub URL、tree 子目录、ref、commit、redirect allowlist 和 pinned 来源。
- 路径穿越、链接逃逸、特殊文件及所有大小/数量边界。
- 风险规则的等级、规则版本、文件和行号输出。
- inventory 的 managed、external、modified、broken、conflicting、missing 状态。

### 16.2 Rust 集成测试

- 安装、导入、更新、分配、停用、修复和删除完整流程。
- 多 Skill、多 Agent 全成功和任意步骤失败后的完整回滚。
- plan 后并发修改 settings、中央目录或 Agent 目标时拒绝 commit。
- 各 commit 阶段模拟崩溃后的 journal 恢复。
- settings 未知字段保留和旧 settings 默认读取。
- 全部文件测试使用隔离 `MUX_HOME`/TestHome；GitHub 测试使用本地 mock HTTP，
  不依赖实时网络。

### 16.3 Desktop 测试

增加最小化 Vitest、jsdom 和 React Testing Library，覆盖：

- 筛选、卡片、Inspector 和 Agent 分配状态。
- 安装三步流程、高风险二次确认和计划失效。
- 外部导入、更新差异、冲突、断链和恢复只读状态。
- Tauri invoke 使用 mock；真实文件行为只在 Rust 集成测试验证。

### 16.4 验证命令与真实验收

```bash
cargo fmt --check
cargo test --workspace
(cd desktop && npm test)
(cd desktop && npm run check:agent-icons)
(cd desktop && npm run build)
bash desktop/scripts/prepare-sidecar.sh
(cd desktop/src-tauri && cargo test)
```

UI 验收只在通过标准安装流程放入 `/Applications/MUX.app` 的真实应用中进行，不使用
浏览器 mock、Preview bundle 或 target app 冒充。验收覆盖 `1200×820`、`900×600`、
Escape 层级、横向 overflow、控制台错误和实际截图。

## 17. 完成标准

1. 没有 Node.js、`npx` 或 Git 时，公开 GitHub 和本地目录安装仍可完成。
2. 只显示本机已安装且 Skills 能力记录已核验的 Agent。
3. 未经确认，安装、导入、更新、分配和删除不会写盘。
4. 后台更新检查不改变 Skill 内容或 Agent 链接。
5. 一个中央副本可分配给多个 Agent，停用不会删除中央内容。
6. 高风险必须展示本地证据并二次确认，没有数据上传。
7. 并发变化、失败或崩溃后不存在不可恢复的半安装状态。
8. 既有独立副本在确认导入前保持原样。
9. Desktop 在两种规定 viewport 中操作完整、无裁切、无层级冲突。
10. 首版没有项目级 Skills、私有仓库、Skill 编辑器或 CLI/TUI 入口。

## 18. 实施边界

实施顺序应为：Agent 能力数据与 core 类型 → resolver/manifest/audit →
inventory/store/transaction ops → Tauri commands → Desktop 工作区与 Agent section →
组件测试和真实验收。具体任务拆分由后续 `writing-plans` 阶段完成。
