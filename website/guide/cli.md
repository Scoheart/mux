# 命令行 / TUI

`mux` 是一个原生 Rust 二进制，和桌面 App 构建在同一个 `mux-core` 之上，共用 `~/.mux/` 中的中央资产、Agent 关系和事务状态。

> 还没装？看 [安装 · 命令行](/guide/install#命令行-tui-mux)。

它有两个入口：

- **无参数**：进入面向 MCP 的兼容性终端管理器（TUI）；
- **带子命令**：用统一的 MCP、Model、Skill 和 Agent 命令进行查询或脚本化写入。

## 交互式 TUI

```bash
mux
```

无参数 TUI 聚焦 MCP 兼容性管理，有三个屏幕：

| 键 | 屏幕 |
|---|---|
| `1` | Registry（MCP 目录） |
| `2` | Sources（MCP 来源） |
| `3` | Agents（Agent 的 MCP 状态） |

它可以搜索和维护 MCP 目录、来源以及 Agent 的 MCP 安装状态。Model 和 Skill 的脚本化关系管理与显式物理修复使用下文子命令；完整的可视化资产生命周期仍可在 Desktop 中操作。

### 通用键位

| 键 | 作用 |
|---|---|
| `↑`/`k`、`↓`/`j` | 上下移动 |
| `Tab` / `Shift-Tab` | 在三个屏幕间前后切换 |
| `?` | 显示帮助 / 键位表 |
| `q` 或 `Ctrl-C` | 退出 |
| `Ctrl-R` | 刷新 |

### Registry 屏幕

| 键 | 作用 |
|---|---|
| `/` | 搜索 |
| `[`/`]` 或 `←`/`→` | 切换过滤 |
| `i` | 安装向导（多选 Agent，空格勾选，`Ctrl-S` 确认） |
| `n` | 新建条目 |
| `e` | 编辑选中条目 |
| `p` | 粘贴一段 `mcpServers` 配置 |
| `S` | 重新同步选中条目 |
| `d` | 删除选中条目并确认影响 |

### Sources 屏幕

| 键 | 作用 |
|---|---|
| `Space`/`Enter` | 启停选中来源 |
| `r` | 刷新来源；外部发现只重新识别，不自动导入 |
| `s` | 订阅一个 URL |
| `l` | 导入本地文件 |
| `o` | 添加 MUX 精选 |
| `d` | 删除来源并确认影响 |

### Agents 屏幕

| 键 | 作用 |
|---|---|
| `Enter`/`→`/`l` | 进入 Agent，查看它的 MCP |
| `Space` | 启停 Agent（列表层）或已分配的 MCP（详情层） |
| `a` | 给该 Agent 添加 MCP |
| `e` | 编辑 Agent 配置路径 |
| `n` | 新增自定义 Agent |
| `d` | 从该 Agent 解除选中的 MCP |

## 子命令总览

在脚本里可设置 `MUX_NO_TUI=1`，让无参数运行时打印帮助而不进入 TUI。

```text
mux mcp {list,show,status,assign,unassign,enable,disable,reapply,add,delete,export}
mux model {list,show,status,assign,unassign,enable,disable,reapply,use}
mux skill {list,show,status,assign,unassign,enable,disable,reapply}
mux agent {list,enable,disable}
mux discover [mcp|model|skill]
mux adopt {mcp,model,skill}
mux migration {review,resolve}
mux workspace
mux upgrade
```

MCP、Model、Skill 使用相同的查询与消费关系动词；各自独有的操作保留在对应域中。

| 能力层 | MCP | Model | Skill |
|---|---|---|---|
| 通用查询 | `list` / `show` / `status` | `list` / `show` / `status` | `list` / `show` / `status` |
| 通用关系 | `assign` / `unassign` / `enable` / `disable` | `assign` / `unassign` / `enable` / `disable` | `assign` / `unassign` / `enable` / `disable` |
| 显式物理修复 | `reapply --agent`（准确关系；另有显式 `--all`） | `reapply --agent`（准确关系） | `reapply --agent`（准确关系 / 共享 target） |
| 域专属 | `add` / `delete` / `export` | `use`（选择 current） | — |

命令数量不追求机械相等：统一的是关系语义、审阅和事务保证。MCP 的目录导出与手工中央资产维护、Model 的 current pointer 都没有可伪造的跨域对应物；中央 Model / Skill 的富创建、编辑、更新和删除仍由 Desktop 提供。

## 全局参数

全局参数可以写在域命令前或后，并和任意适用的子命令组合：

| 参数 | 作用 |
|---|---|
| `--json` | 只输出机器可读 JSON，不混入表格、颜色或确认提示；写操作还必须配合 `--yes` 或 `--dry-run` |
| `--yes` | 接受已生成的写入计划；未解决的漂移、冲突和并发变化仍会拒绝 |
| `--dry-run` | 生成并输出计划，但不提交该命令请求的领域变更 |
| `--no-color` | 关闭 ANSI 颜色 |

例如：

```bash
mux --json skill status --agent codex
mux --dry-run mcp assign github::stdio --agent claude-code
mux --yes --no-color model use work --agent pi
```

`--yes` 和 `--dry-run` 互斥，且只适用于写操作；把它们传给 `list`、`show`、`status`、`discover`、`workspace` 或输出到 stdout 的 `mcp export` 会直接报错。`mcp export --out <path>` 会创建文件，因此同样必须交互确认，或显式使用 `--yes` / `--dry-run`。`mux --json --help` 与 `mux --json --version` 也返回 schema v1 成功 envelope；作为 `mcp add --arg --json` 值出现的字样不会误切换输出模式。

## 启动迁移审核

当旧 Model schema 可以安全生成计划、但 Agent 中的 MUX 管理字段已经变化时，CLI 对 Model 写入返回稳定的 `migration_review_required`；MCP、Skill 以及语言、网络代理、CLI 工具安装等独立设置继续可用。会同时改动多种能力路径的 Agent 配置仍按共享写入处理，避免绕过 Model 阻断。单个 Agent / Provider 的 Model 错误或已经完整回滚的 Model 提交只关闭 Model 能力；只有共享事务未完成、提交后清理未完成、回滚证据不安全或共享 settings 状态不确定时，才进入全局只读。审核与 Desktop 复用同一个 Core 契约：

```bash
mux migration review
mux --json migration review

# 使用 review 输出中对应策略的 candidate_hash
mux migration resolve use-mux --yes --candidate-hash <hash>
mux migration resolve keep-agent --yes --candidate-hash <hash>

mux migration resolve use-mux --dry-run
mux migration resolve recheck
mux migration resolve later
```

`use-mux` 只替换已审核的 MUX-owned Model 字段，并在原子事务中迁移 Model ID、关系和 Keychain 引用；未知字段、注释、权限和非 MUX 内容继续保留。`keep-agent` 保持受影响 Agent 文件及其现有 Keychain 引用逐字节不变，将该 Agent 的全部 Model 管理关系解除为外部观察，其他 Agent 和中央 Profile 继续迁移。审核输出会列出所有被解除的 Profile。`recheck` 会取消旧候选并按最新 settings / target hash 重新生成；`later` 不写任何内容，Model 保持只读但 MCP 与 Skill 不受影响。`--dry-run` 只输出所选候选的影响，不提交、不替换当前审核状态。任何 candidate、settings 或目标文件变化都会返回 `migration_review_stale`，不能沿用旧确认。

## 稳定 ID 与 Agent 选择

所有现有资产写操作都要求准确稳定 ID，不按显示名称模糊匹配：

| 域 | 稳定 ID | 示例 |
|---|---|---|
| MCP | `name::transport` | `github::stdio`、`github::http` |
| Model | Model Profile ID | `work` |
| Skill | 中央 Skill 名称 | `review-changes` |
| Agent | Agent ID | `claude-code`、`codex` |

所有 Agent 消费关系写操作都必须带且只带一个明确的 `--agent <id>`。`assign` 和 `unassign` 一次可以增量处理多个准确资产 ID；`enable`、`disable` 和 `use` 一次处理一个。关系命令不会默认影响全部 Agent。三个域的 `reapply` 同样默认要求一个准确 Agent；只有 MCP 额外提供必须明确写出的 `--all` 批量修复：

```bash
mux mcp assign github::stdio filesystem::stdio --agent claude-code
mux skill unassign review-changes source-explainer --agent codex
```

如果 `github::stdio` 和 `github::http` 同时存在，它们是两个独立资产。命令必须逐个写出准确 ID，不会因为名称相同而自动选择两个 transport。

## 统一的消费关系语义

| 动词 | 语义 |
|---|---|
| `assign` | 增量增加指定资产与 Agent 的 desired relationship，不移除其他已分配资产 |
| `unassign` | 只解除指定关系，不删除中央资产 |
| `enable` | 保留关系并让指定资产在 Agent 中生效 |
| `disable` | 保留关系但暂停在 Agent 中生效，之后可以原位恢复 |
| `status` | 对照 desired / observed 状态，显示 pending、synced、drifted、conflicted 等结果 |

关系写入统一经过 `plan → review → commit`，并在同一事务中更新中央关系与 Agent 目标。`--dry-run` 停在 review，`--yes` 只省略交互确认，不会绕过安全检查。

## MCP

```bash
mux mcp list
mux mcp show github::stdio
mux mcp status
mux mcp status --agent claude-code

mux mcp assign github::stdio --agent claude-code
mux mcp unassign github::stdio --agent claude-code
mux mcp disable github::stdio --agent claude-code
mux mcp enable github::stdio --agent claude-code
```

MCP 的专有操作：

```bash
mux mcp add github::stdio --command npx --arg -y --arg @example/server
mux mcp add docs::http --url https://mcp.example.com --http-type streamable-http
mux mcp delete github::stdio       # 删除中央资产并审阅全部消费者影响
mux mcp export                     # 将生效目录输出到 stdout
mux mcp export --out mcp.json --yes # 经写入门禁创建文件
mux mcp reapply github::stdio --agent claude-code
mux mcp reapply github::stdio --all # 显式批量修复所有未同步 desired consumers
```

`add` 通过参数完整定义 MCP，不再交互询问连接字段，并且必须直接给出完整稳定 ID；提交仍遵循统一计划确认。stdio 资产要求 `--command`，可重复 `--arg` 并选填 `--cwd`；HTTP 资产要求 `--url`，可用 `--http-type` 设置原生类型。两者都可选填 `--description` 和可重复的 `--tag`。

`export` 输出完整的生效 MCP 目录；同一个 `name::transport` 只保留来源优先级最高的副本。由于内容可能含凭据，`--out` 也遵守统一写入确认，只新建权限为 `0600` 的文件，目标已存在时拒绝写入，绝不静默覆盖；JSON 文件模式只回传脱敏路径和权限，不会再把完整目录复制到 stdout。`reapply --agent` 只重新同步指定 desired relationship；`reapply --all` 是显式批量入口，只把当前未同步的 desired consumers 纳入计划，不会重写已同步 Agent。两种形式都检查目标漂移、冲突、Agent 是否启用以及计划后的并发变化。

## Model

```bash
mux model list
mux model show work
mux model status --agent pi

mux model assign work backup --agent pi
mux model assign work --agent claude-code --replace
mux model unassign backup --agent pi
mux model disable backup --agent pi
mux model enable backup --agent pi
mux model use work --agent pi
mux model reapply work --agent pi
```

`assign` 默认只负责增量分配 Model Profile。单模型目标已经分配其他 Profile 时，可以显式加 `--replace`，用本次准确 ID 替换完整 Model selection；没有该参数时不会隐式移除。`use` 单独设置 Agent 的 **current model**，不会把“已分配”和“当前”混成一个动作。原生多模型 Agent 可以有多个已分配 / 已启用 Profile，但最多一个 current；单模型 Agent 仍按自身能力限制计划。

关系动词只改变 desired state：重复执行 `assign`、`enable` 或 `use` 是幂等 no-op，即使观测配置已经漂移也不会借机覆盖。`reapply` 是唯一显式物理修复入口；它只重同步指定 Agent 上的准确 Profile，并对漂移生成绑定候选哈希的审阅计划。若请求的是“实际 current、但 desired 非 current”的 Profile，命令会要求改为 reapply desired current Profile，避免暗中扩大修复范围。已禁用 Profile 仅在目标仍可证明为 MUX 管理内容时安全清理，定制或歧义内容会被阻断。

## Skill

```bash
mux skill list
mux skill show review-changes
mux skill status --agent codex

mux skill assign review-changes source-explainer --agent codex
mux skill unassign source-explainer --agent codex
mux skill disable review-changes --agent codex
mux skill enable review-changes --agent codex
mux skill reapply review-changes --agent codex
```

Skill 分配的是 `~/.mux/skills/` 中央副本到已核验用户级目录的受管链接。部分 Agent 读取同一个物理目录，因此给一个 Agent 分配、启停或解除 Skill 时，其他已安装 Agent 也可能同时受影响。计划会列出完整的物理 target 和实际影响 Agent；共享目录带来的影响不能被隐藏或拆成相互矛盾的关系。

`reapply` 只修复已经存在的 desired 关系：缺失或损坏的受管链接可从中央副本重建；外部目录、普通文件或指向其他位置的链接不会被覆盖。对共享目录执行 reapply 时，审阅结果会列出该物理写入影响到的全部 Agent。

## Agent

```bash
mux agent list
mux agent enable claude-code
mux agent disable cursor
```

这里的 enable / disable 控制 Agent 是否参与 MUX 管理，不等同于某个资产关系的 enable / disable。Agent ID 必须准确匹配目录中的稳定 ID。

## 外部发现与纳管

```bash
mux discover
mux discover mcp
mux discover model
mux discover skill
```

`discover` 本身只刷新或列出 Agent 配置中尚未纳管的观察结果，不创建 ownership，也不改 Agent 配置。纳管必须逐项指定准确候选：

```bash
mux adopt mcp github::stdio --agent claude-code
mux adopt model <candidate-id>
mux adopt skill <identity>
```

MCP 候选需要用来源 Agent 作为明确锚点；如果同一 MCP identity 在多个 Agent 中存在，MUX 会把当前全部同 key 观察结果交给 core 对账，完全一致的副本及原始关系原子纳管，内容不同则保持冲突。Model 与 Skill 候选本身已经绑定来源 Agent / 物理 target，因此不再接受 `--agent`。纳管会先展示中央资产、原始关系、目标文件和风险；一次只处理一个逻辑资产，不提供跨资产批量接管。

所有子命令启动前都会执行同一套安全 bootstrap；它可能完成旧数据迁移、未完成事务恢复或状态 reconcile。非 JSON 的普通命令还可能执行每日一次的正式版检查。因此这里的“只读”和 `--dry-run` 指不提交当前命令请求的领域/ownership 变更，不承诺进程绝对没有维护性文件或网络副作用；需要完全关闭版本检查时设置 `MUX_NO_UPDATE_CHECK=1`。

## JSON 与自动化

`--json` 成功时在 stdout 输出一个稳定 envelope：

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "skill.assign",
  "changed": true,
  "data": {}
}
```

失败结果写到 stderr，包含 `ok: false` 和稳定的 `error.code`、`error.message`，必要时还有经过安全投影的 `error.details`。所有非导出 JSON 都隐藏配置值、凭据、原始解析诊断和绝对路径；低层错误的完整诊断只留在人类输出中。只有不带 `--out` 的 `mcp export` 会按命令语义输出完整 MCP 配置。成功的幂等 no-op 仍返回成功，并以 `changed: false` 区分。`--json` 不会隐式同意写入；mutation 必须明确选择 `--yes` 或 `--dry-run`。

## 工作区与更新

```bash
mux workspace
mux --json workspace
mux upgrade
```

`workspace` 查看统一 revision、中央资产、desired relationships、observed inventory 和外部发现。独立下载或通过 `cargo install` 安装的 CLI 可以用 `mux upgrade` 跟随最新正式版；桌面 App 内置的 CLI 由 App 更新。

普通子命令执行后每天最多检查一次最新正式版；设置 `MUX_NO_UPDATE_CHECK=1` 可关闭。

## 和桌面 App 的关系

CLI 与 Desktop 读写同一个 `~/.mux/`，并调用同一个 core planner。CLI 分配的关系会出现在 Desktop；Desktop 维护的中央资产也会出现在对应的 `list` 和 `status` 输出中，数据模型不会分叉。

下一步 → [支持的 Agent](/guide/agents)
