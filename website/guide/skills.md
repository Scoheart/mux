# 用户级 Skills

MUX Desktop 把符合 Agent Skills 格式的用户级 Skill 作为中央资产统一管理。先把 Skill 添加到中央资产库，再单独选择哪些 Agent 消费它；Agent 页面不再解析来源或重新安装同一个 Skill。当前版本只管理用户主目录中的全局 Skill，不读取或写入项目目录中的 `.agents/skills`、`.claude/skills` 等内容。

> Skills 当前只有 Desktop 入口；CLI/TUI 暂不提供 Skills 命令。

## 添加到中央资产库

在顶部打开 **Skills**，点击 **添加 Skill**。选择 GitHub 来源后直接下载，选择本地文件夹或压缩包后直接导入；一个来源包含多个 Skill 时，只需勾选需要的项目。中央入库不再展示审核、风险证据或文件差异页面。这个流程只写 `~/.mux/skills/` 中央副本，不选择 Agent、不创建 link，也不建立消费关系。入库完成后，从对应 Agent 页的 Skills 标签单独选择消费者。

底层仍会校验来源、目录边界、压缩包结构、内容哈希和并发变化，并通过临时目录与原子事务写入；这些检查不再增加用户操作步骤。同名中央资产不会被静默覆盖，只有用户选择“备份并下载/导入”后才替换。

| 来源 | 行为 |
|---|---|
| 公开 GitHub | 支持 `owner/repo`、仓库 URL 和 GitHub tree 子目录 URL。MUX 通过 HTTPS 解析到不可变 commit 并下载归档，不调用本机 Git。 |
| 本地目录 | 只能通过 macOS 原生文件夹选择器选择。MUX 复制一份快照，不创建指向原目录的活链接，也不接受手输路径。 |
| 本地压缩包 | 通过原生文件选择器导入 `.zip`、`.tar.gz`、`.tgz` 或 `.tar`。MUX 安全解包并记录包内 Skill 路径，后续可重新检查、更新或修复。 |

来源中可以包含一个或多个具有有效 `SKILL.md` 的 Skill。解析与安全校验都由 MUX 自带的 Rust core 完成，因此运行功能不需要安装 Git、Node.js 或 `npx`。

私有 GitHub 仓库、GitLab、SSH Git 和远程压缩包 URL 当前不受支持。

## 一份中央副本，多处链接

下载或导入完成后，MUX 把每个 Skill 的唯一托管副本放在：

```text
~/.mux/skills/<skill-name>/
```

随后建立消费关系时，选中的 Agent 目录中只创建指向中央副本的受管链接。这样一次更新会被所有消费者看到，而解除某个消费关系只移除对应链接，不会删除中央内容。

MUX 按物理目录归一化消费关系。某些 Agent 会兼容读取另一个目录，例如 Cursor、Gemini CLI、OpenCode 和 GitHub Copilot CLI 都可以读取 `~/.agents/skills`。因此向 Codex 的首选目录写入链接时，其他已安装 Agent 也可能同时获得访问。共享同一物理 target 的 Agent 会作为不可拆分组一起选择，审阅页列出实际受影响的全部 Agent，并去掉会导致同一 Skill 被重复发现的冗余链接。

## 已核验的 Agent 路径

首版为以下六个 Agent 提供用户级 Skills 能力。MUX 只显示本机安装探针命中且能力资料已核验的 Agent；目录本身存在不等于对应 Agent 已安装。

| Agent | 首选用户级目录 | 兼容读取目录 |
|---|---|---|
| Claude Code | `~/.claude/skills` | — |
| Codex | `~/.agents/skills` | — |
| Cursor | `~/.cursor/skills` | `~/.agents/skills` |
| Gemini CLI | `~/.gemini/skills` | `~/.agents/skills` |
| OpenCode | `~/.config/opencode/skills` | `~/.claude/skills`、`~/.agents/skills` |
| GitHub Copilot CLI | `~/.copilot/skills` | `~/.agents/skills` |

Agent 的 MCP 配置路径和 Skills 路径是两套独立契约，MUX 不会从其中一个推断另一个。更多背景见 [支持的 Agent](/guide/agents#skills-能力)。

## 后台安全校验

MUX 在写盘前对候选文件做本地结构与静态校验。越界链接、路径穿越、特殊文件、超限压缩包和提交前发生变化的内容会直接拒绝；可执行文件与脚本等信息保留在资产详情中，但不会在下载或导入时增加审核步骤。

- Skill 正文、内容哈希、文件路径和风险 findings 不会上传。
- MUX 不运行候选脚本，也不会把“未发现高风险模式”解释为安全认证。
- `SKILL.md` 只以纯文本预览，不执行其中的 HTML、脚本或远程资源。

## 生命周期操作

下载与导入由用户动作直接提交内部计划；更新、删除、修复和 Agent 分配等已有资产变更仍会按适用情况展示影响。如果内容或设置在计划后变化，MUX 会拒绝旧操作并要求重试。

| 操作 | 结果 |
|---|---|
| 分配给 Agent | 从对应 Agent 页选择 Skill，生成独立的关系计划；中央副本本身不变。共享 target 的全部 Agent 会一起显示和变更。 |
| 检查 / 更新 | 后台和手动检查只读取 GitHub revision、本地目录或压缩包哈希，不改变正文。选择更新后才暂存候选、展示差异、重新审计并确认替换；中央副本的本地修改会要求“备份后替换”。 |
| 导入 | Agent 目录中的外部副本先只读展示。点击导入后 MUX 直接复制并校验内容、备份原目录，再用中央链接替换；成功前不会移动原副本。 |
| 停用 | 移除当前受管目标链接，保留中央副本和其他 Agent 的分配。共享目录会在审阅中列出所有失去访问的 Agent。 |
| 修复 | 对符合记录的断链重建链接；中央正文缺失时从已记录来源或只读导入备份重新解析，并再次展示完整差异与风险。 |
| 删除 | 先移除全部受管链接，再把中央副本移入带时间戳的 `~/.mux/backups/skills/`，最后移除托管记录。当前不提供永久清空备份操作。 |

候选和内部事务计划位于 `~/.mux/staging/skills/`，提交进度位于 `~/.mux/journals/skills/`。提交失败或 App 崩溃时，journal 会按已持久化阶段安全回滚或完成提交；无法完成恢复时，Skills 工作区进入只读恢复状态，不继续新的写操作。

## 当前边界

当前版本不支持：

- 项目级 Skills；
- 私有仓库或需要认证的 Git 来源；
- 在 MUX 中创建或编辑 `SKILL.md`；
- CLI/TUI Skills 命令。

返回 [桌面 App 指南](/guide/desktop#skills) 或查看 [支持的 Agent](/guide/agents#skills-能力)。
