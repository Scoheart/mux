# 用户级 Skills

MUX Desktop 把符合 Agent Skills 格式的用户级 Skill 作为中央资产统一管理。先把 Skill 添加到中央资产库，再单独选择哪些 Agent 消费它；Agent 页面不再解析来源或重新安装同一个 Skill。当前版本只管理用户主目录中的全局 Skill，不读取或写入项目目录中的 `.agents/skills`、`.claude/skills` 等内容。

> Skills 当前只有 Desktop 入口；CLI/TUI 暂不提供 Skills 命令。

## 添加到中央资产库

在顶部打开 **Skills**，点击 **添加 Skill**。中央入库分为三步：选择来源、选择发现的 Skills、审阅计划后确认。这个流程只写 `~/.mux/skills/` 中央副本，不选择 Agent、不创建 link，也不建立消费关系。入库完成后，从对应 Agent 页的 Skills 标签单独选择消费者。

中央入库、审阅和高风险二次确认使用与 MCP、Models 一致的对话框结构，但不会简化下面的安全流程：候选仍由 core 生成不可变计划，提交仍校验 operation id、内容哈希和风险确认。提交期间不能通过遮罩或 `Escape` 关闭，失败后保留当前计划与错误信息。

| 来源 | 行为 |
|---|---|
| 公开 GitHub | 支持 `owner/repo`、仓库 URL 和 GitHub tree 子目录 URL。MUX 通过 HTTPS 解析到不可变 commit 并下载归档，不调用本机 Git。 |
| 本地目录 | 只能通过 macOS 原生文件夹选择器选择。MUX 复制一份快照，不创建指向原目录的活链接，也不接受手输路径。 |
| 本地压缩包 | 通过原生文件选择器导入 `.zip`、`.tar.gz`、`.tgz` 或 `.tar`。MUX 安全解包并记录包内 Skill 路径，后续可重新检查、更新或修复。 |

来源中可以包含一个或多个具有有效 `SKILL.md` 的 Skill。解析、校验、差异和风险分析都由 MUX 自带的 Rust core 完成，因此运行功能不需要安装 Git、Node.js 或 `npx`。

私有 GitHub 仓库、GitLab、SSH Git 和远程压缩包 URL 当前不受支持。

## 一份中央副本，多处链接

确认中央入库后，MUX 把每个 Skill 的唯一托管副本放在：

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

## 本地风险审阅

MUX 在写盘前对候选文件做确定性的本地静态分析。越界链接会在结构校验阶段直接拒绝；对于可审计内容，MUX 会标记可执行文件、脚本、下载后执行、提权、破坏性文件操作、凭据读取、数据上传和混淆载荷等模式，并展示规则、文件、适用时的行号和原因。

- Skill 正文、内容哈希、文件路径和风险 findings 不会上传。
- MUX 不运行候选脚本，也不会把“未发现高风险模式”解释为安全认证。
- 高风险操作必须审阅已显示的证据、明确勾选覆盖项，再通过独立的第二次确认。
- `SKILL.md` 只以纯文本预览，不执行其中的 HTML、脚本或远程资源。

## 生命周期操作

所有写操作都先生成计划，再由用户确认提交。计划会按适用情况显示文件变化、风险、中央副本冲突、目标路径、共享影响和将保留备份的事实；如果内容或设置在审阅后变化，MUX 会拒绝旧计划并要求重新审阅。

| 操作 | 结果 |
|---|---|
| 分配给 Agent | 从对应 Agent 页选择 Skill，生成独立的关系计划；中央副本本身不变。共享 target 的全部 Agent 会一起显示和变更。 |
| 检查 / 更新 | 后台和手动检查只读取 GitHub revision、本地目录或压缩包哈希，不改变正文。选择更新后才暂存候选、展示差异、重新审计并确认替换；中央副本的本地修改会要求“备份后替换”。 |
| 导入 | Agent 目录中的外部副本先只读展示。确认后 MUX 复制并校验内容、备份原目录，再用中央链接替换；成功前不会移动原副本。 |
| 停用 | 移除当前受管目标链接，保留中央副本和其他 Agent 的分配。共享目录会在审阅中列出所有失去访问的 Agent。 |
| 修复 | 对符合记录的断链重建链接；中央正文缺失时从已记录来源或只读导入备份重新解析，并再次展示完整差异与风险。 |
| 删除 | 先移除全部受管链接，再把中央副本移入带时间戳的 `~/.mux/backups/skills/`，最后移除托管记录。当前不提供永久清空备份操作。 |

候选和审阅计划位于 `~/.mux/staging/skills/`，提交进度位于 `~/.mux/journals/skills/`。提交失败或 App 崩溃时，journal 会按已持久化阶段安全回滚或完成提交；无法完成恢复时，Skills 工作区进入只读恢复状态，不继续新的写操作。

## 当前边界

当前版本不支持：

- 项目级 Skills；
- 私有仓库或需要认证的 Git 来源；
- 在 MUX 中创建或编辑 `SKILL.md`；
- CLI/TUI Skills 命令。

返回 [桌面 App 指南](/guide/desktop#skills) 或查看 [支持的 Agent](/guide/agents#skills-能力)。
