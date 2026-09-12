# 命令行 / TUI

`mux` 是和桌面 App 共用 `mux-core` 与 `~/.mux/` 的原生 Rust CLI。它把 MCP、Model、Skill 都视为“中央资产 + Agent 消费关系 + 外部观测”，不会为三个域维护互相矛盾的状态模型。

> 还没安装？看 [安装 · 命令行](/guide/install#命令行-tui-mux)。

## 两个入口

- `mux`：进入 MCP 终端工作区；适合交互维护 MCP 目录、来源和 Agent 关系。
- `mux <domain> <command>`：使用可脚本化的统一资产命令。

脚本中可设置 `MUX_NO_TUI=1`，让无参数调用打印帮助而不是进入 TUI。

## 统一命令模型

```text
mux mcp {list,show,status,assign,unassign,enable,disable,converge,add,save,delete,export,source}
mux mcp source {list,subscribe,add-local,add-builtin,refresh,enable,disable,remove}
mux model {list,show,status,save,delete,import,assign,unassign,enable,disable,converge,use,delivery,provider}
mux model provider {list,show,templates,models,save,delete}
mux skill {list,show,status,inspect-source,install,import,update,remove,repair,check-updates,assign,unassign,enable,disable,converge}
mux agent {list,save,configure,enable,disable}
mux network proxy {show,set,clear}
mux discover [mcp|model|skill]
mux workspace
mux upgrade
```

三个资产域共享以下关系语义：

| 命令 | 语义 |
|---|---|
| `list` / `show` | 查询中央资产 |
| `status` | 对照 desired / observed，显示 ownership、启停/current、状态和可用动作 |
| `assign` | 只新增指定 Agent 的 desired relationship，不删除其他关系 |
| `unassign` | 只移除指定关系，不删除中央资产 |
| `enable` / `disable` | 保留关系，只改变 desired 启停状态 |
| `converge` | 对一个准确 observation 执行采用外部、恢复 MUX 或解除管理 |

域专属命令表达真实差异：MCP 支持中央条目、来源和完整配置导出；Model 支持 Profile、共享 Provider、凭据交付方式与 current；Skill 支持来源解析、安装、导入、更新、删除和修复。这些命令调用 Desktop 使用的 Core 用例。

## 外部变更与收敛

Agent 文件和用户级 Skill 目录是合法输入源。用户在外部新增、删除、启停或修改配置不会让 MUX 进入迁移冲突；下一次状态查询会重新扫描，Desktop 也会实时刷新。

```bash
mux mcp converge github::stdio --agent claude-code adopt
mux mcp converge github::stdio --agent claude-code restore
mux mcp converge github::stdio --agent claude-code detach

mux model converge external-<candidate-id> --agent codex adopt
mux model converge work --agent codex restore
mux model converge work --agent codex detach

mux skill converge review-changes --agent codex adopt
mux skill converge review-changes --agent codex restore
mux skill converge review-changes --agent codex detach
```

| 动作 | 含义 |
|---|---|
| `adopt` | 采用当前 Agent 现场，更新中央资产或 desired state；原现场字节保持不变 |
| `restore` | 只把选中的准确关系恢复成 MUX desired state |
| `detach` | 解除 MUX ownership；漂移或外部内容保留，并重新显示为外部观测 |

每个 convergence 请求都绑定 `status` 返回的 inventory revision。计划生成后 core 会再扫描一次，提交时还会校验 candidate hash 与目标快照；现场变化会返回 `observation_stale`，不会把旧审阅应用到新配置。

旧 `reapply`、顶层 `adopt` 和 `migration review/resolve` 命令已删除。明确的 convergence action 就是收敛意图，不再使用第二套漂移确认令牌。

## 状态含义

| 状态 | 含义 |
|---|---|
| `synced` | desired 与 observed 一致 |
| `external-added` | Agent 中新增、MUX 尚未管理 |
| `external-changed` | 受管字段、启停或 current 被外部修改 |
| `external-removed` | desired 仍在，但 Agent 目标已删除 |
| `unparseable` | 单个现场目标无法解析 |
| `ambiguous` | 单个现场目标存在身份或值歧义 |
| `unsupported` | 现场状态无法无损接管或表达 |

后三类状态只隔离对应资产。普通外部修改也只影响对应关系；MCP、Model 或 Skill 的单点错误不会锁住其他域。只有共享 settings 或未完成事务无法安全恢复时，整个 workspace 才进入只读恢复。

JSON 状态中的 `capability_errors` 表示能力域局部不可用；`recovery_error` 只表示共享事务恢复边界。前者不会阻止其他域的查询或 mutation。

## 稳定 ID 与 Agent 选择

所有 mutation 使用准确稳定 ID，不按显示名模糊匹配：

| 域 | ID | 示例 |
|---|---|---|
| MCP | `name::transport` | `github::stdio` |
| Model | Profile ID 或 `status` 返回的 external ID | `work` |
| Skill | 中央 Skill 名称 | `review-changes` |
| Agent | Agent ID | `claude-code`、`codex` |

关系命令必须显式带一个 `--agent <id>`。`assign` / `unassign` 可在一次命令中处理多个准确资产 ID；`enable` / `disable` / `use` / `converge` 一次处理一个。
MCP 的 `unassign --all` 审阅后清空该 Agent 的受管项、停用快照和外部项。Model 也支持 `unassign --all`，按 Core 声明的 storage authority 清空；原生注册表 Agent 包括外部模型。中央资产、Provider 和凭据保留；共享物理配置冲突由 Core 拒绝。

```bash
mux mcp assign github::stdio filesystem::stdio --agent claude-code
mux mcp unassign --all --agent qoder
mux skill unassign source-explainer --agent codex
mux model assign work backup --agent pi
mux model use work --agent pi
```

同名 `github::stdio` 与 `github::http` 是两个资产，必须分别指定。

## MCP

```bash
mux mcp list
mux mcp show github::stdio
mux mcp status --agent claude-code
mux mcp assign github::stdio --agent claude-code
mux mcp disable github::stdio --agent claude-code
```

中央 MCP 专属操作：

```bash
mux mcp add github::stdio --command npx --arg -y --arg @example/server
mux mcp add docs::http --url https://mcp.example.com --http-type streamable-http
mux mcp save --file mcp.json --key github::stdio --dry-run
mux mcp delete github::stdio
mux mcp export
mux mcp export --out mcp.json --yes
```

`export --out` 只创建权限为 `0600` 的新文件，拒绝覆盖已有目标。stdout 导出按命令定义包含完整 MCP 配置，其他 JSON 投影保持脱敏。

MCP 来源管理：

```bash
mux mcp source list
mux mcp source subscribe --url https://example.com/mcp.json --name work --yes
mux mcp source add-local --path /path/to/mcp.json --yes
mux mcp source add-builtin --yes
mux mcp source refresh <source-id> --yes
mux mcp source disable <source-id> --yes
mux mcp source remove <source-id> --yes
```

`manual` / `discovered` 是自动管理来源，Core 拒绝通过来源删除或刷新接口改写它们。重新发现外部 MCP 使用 `mux discover mcp`。有消费关系的来源变更仍受 Core 的影响校验约束。

## Model

```bash
mux model list
mux model show work
mux model status --agent pi
mux model assign work backup --agent pi
mux model assign work --agent claude-code --replace
mux model disable backup --agent pi
mux model use work --agent pi
```

多模型 Agent 可保留多个已分配 Profile，但最多一个 current；单模型 Agent 由 capability 在计划阶段约束。重复 `assign`、`enable` 或 `use` 是 desired-state 幂等操作，不会借机覆盖外部漂移；需要收敛时使用准确的 `converge`。

外部 Model 按 candidate identity 分别显示。无法安全搬入 Keychain、需要环境变量改造或存在身份歧义的候选会显示 `unsupported` / `ambiguous`，不会猜测接管。

创建 Provider 后再创建引用它的 Model Profile：

```json
{
  "id": "work-provider",
  "name": "Work",
  "provider": "custom",
  "base_url": "https://api.example.com/v1",
  "protocols": {"openai-completions": {"endpoint_path": "/chat/completions"}},
  "auth_requirement": "required",
  "api_key_source": {"kind": "env", "name": "WORK_API_KEY"}
}
```

将上面的文档保存为 `provider.json`。Profile 文档 `model.json`：

```json
{
  "id": "work",
  "name": "Work model",
  "provider_id": "work-provider",
  "protocol": "openai-completions",
  "model": "your-model-id"
}
```

```bash
mux model provider templates
mux model provider save --file provider.json --dry-run
mux model provider save --file provider.json --yes
mux model provider list
mux model provider models work-provider
mux model save --file model.json --yes
mux model save --file model.json --id work --dry-run
mux model delivery env --agent qoder-cli --profile work --dry-run
mux model unassign --all --agent qoder-cli --dry-run
mux model delete work --dry-run
mux model provider delete work-provider --dry-run
mux model import <candidate-id> --dry-run
```

`save` 不带 `--id` 表示创建；编辑必须指定准确旧 ID。Profile 引用已存在 Provider，连接地址和凭据归 Provider 管理。`provider show` 输出脱敏概览，不能直接充当完整编辑文档。`import` 的 candidate ID 来自 `mux discover model`，Core 会绑定当前 fingerprint。

如果使用 Keychain，将 Provider 的 `api_key_source` 设置为 `{"kind":"mux-store"}`，通过 `--credential-stdin` 从管道接收凭据。该选项不能与 `--file -` 共用 stdin；没有凭据选项时保留原密钥，只有 `--clear-credential` 才请求清除。不要把密钥放进命令参数。`delivery plaintext` 仅对 Core 支持的 Agent 生效，并且额外要求 `--confirm-plaintext`。

`mux agent list --json` 现在包含 `storage_authority` 和 `supports_global_selection`。例如 Qoder Desktop 在会话里选模型，Qoder CLI 可全局选模型；多模型能力不等于全局 current 能力。

## Skill

```bash
mux skill list
mux skill show review-changes
mux skill status --agent codex
mux skill assign review-changes source-explainer --agent codex
mux skill disable review-changes --agent codex
```

Skill relationship 把 `~/.mux/assets/skills/items/` 中央副本链接到已核验用户级 target。多个 Agent 可能共用同一物理目录，因此计划会列出完整 `affected_agent_ids`。`restore` 只重建可证明安全的受管链接；外部目录、普通文件或异向链接不会被覆盖。`detach` 会保留这些外部内容。

Skill 中央生命周期：

```bash
mux skill inspect-source --github owner/repo
mux skill inspect-source --path /path/to/skills
mux skill inspect-source --archive /path/to/skills.zip
mux skill install --github owner/repo --name review-changes --dry-run
mux skill install --github owner/repo --name review-changes --yes
mux skill import <external-identity> --dry-run
mux skill check-updates --yes
mux skill update review-changes --dry-run
mux skill repair review-changes --dry-run
mux skill repair review-changes --target <target-id> --dry-run
mux skill remove review-changes --dry-run
```

先用 `inspect-source` 获得准确候选名称，再用一个或多个 `--name` 明确选择安装项。来源只能指定 GitHub、本地目录、压缩包中的一种。解析暂存会在查询结束、取消或失败后清理；安装和外部导入先进入中央库，分配 Agent 使用单独的 `assign`。

存在本地修改或冲突时，只有明确的 `--replace-local-changes` / `--replace-conflicts` 才允许生成对应替换计划。高风险计划先用 `--dry-run` 阅读 findings；决定接受后，在提交命令中额外传 `--accept-risk`，Core 仍以本次实际计划的 findings hash 校验。普通 `--yes` 不再自动同意风险。

## Agent 与网络设置

```bash
mux agent save my-agent --file agent.json --dry-run
mux agent configure codex --file agent-patch.json --dry-run
mux network proxy show
mux network proxy set socks5://127.0.0.1:7890 --yes
mux network proxy clear --yes
```

`agent save` 接收 Core 的 `AgentDefinition`，修改已有 ID 还需 `--replace`。`agent configure` 接收 `AgentConfigurationPatch`，仅修改文档中出现的能力，例如 `{"model":{"paths":["~/.codex/config.toml"]}}`。Core 校验路径、共享目录影响和已消费资源；CLI 不自行拼 Agent 文件。代理配置由 CLI 与 Desktop 共用，不接受 URL 中的凭据。

## 只读发现

```bash
mux discover
mux discover mcp
mux discover model
mux discover skill
```

`discover` 只列出外部观测和候选详情，不创建 ownership、不写 Agent 配置。需要改变归属时，从 `status` 取得准确 asset ID 后执行 `converge`。

## 通用选项

| 选项 | 作用 |
|---|---|
| `--json` | 输出稳定 JSON envelope；mutation 仍必须同时选择 `--yes` 或 `--dry-run` |
| `--yes` | 跳过交互提示，但不绕过安全校验或自动同意 Skill 风险 |
| `--accept-risk` | 明确接受当前 Skill 计划的风险 findings；仍需正常提交确认 |
| `--dry-run` | 生成并展示计划，随后取消，不提交当前 mutation |
| `--no-color` | 关闭 ANSI 颜色 |

`--yes` 与 `--dry-run` 互斥。传给纯查询命令会报错，避免脚本把无效参数当成已确认行为。

## Workspace 与 JSON

```bash
mux workspace
mux --json workspace
```

`workspace` 返回统一 revision、Agent capability、中央资产、desired relationships、observed inventory 与每项 `available_actions`。JSON 成功 envelope 示例：

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "model.status",
  "changed": false,
  "data": {}
}
```

失败写 stderr，并包含稳定的 `error.code` 与脱敏 details。若中央配置已保存而部分 Agent 尚未收敛，退出码仍为 1，`error.code` 是 `pending_convergence`，`error.details` 包含 `changed: true`、`operation_id` 和本次操作的 `target_incidents`。先查看对应域的 `status`，再使用列出的恢复动作，不能把它当作“什么都没保存”直接重复创建资产。API key、token、原始配置值和绝对私有路径不会进入普通状态 JSON。

## TUI 键位

无参数 TUI 目前聚焦 MCP：

| 键 | 作用 |
|---|---|
| `1` / `2` / `3` | Registry / Sources / Agents |
| `↑` `↓` 或 `j` `k` | 移动 |
| `/` | 搜索 |
| `i` / `a` | 安装或给 Agent 添加 MCP |
| `Space` | 启停来源、Agent 或 MCP |
| `d` | 删除中央 MCP 或解除 Agent 关系 |
| `Ctrl-R` | 重新扫描 |
| `?` | 帮助 |
| `q` | 退出 |

资源变更调用同一个 Core planner；来源注册、Agent 定义和代理偏好使用共享 Core 用例。后者的 `--dry-run` 展示操作意图，不会进行远程刷新或声称已完成写入时才执行的完整校验。CLI 无参数 TUI 仍聚焦 MCP；图标、固定 Agent、语言、桌面启动、文件选择和凭据显示／校验界面留在 Desktop。

## 更新

```bash
mux upgrade
```

独立下载或 `cargo install` 的 CLI 可用 `mux upgrade` 跟随 Stable；Desktop 内置 CLI 随 App 更新。设置 `MUX_NO_UPDATE_CHECK=1` 可关闭普通命令后的每日版本检查。

下一步 → [支持的 Agent](/guide/agents)
