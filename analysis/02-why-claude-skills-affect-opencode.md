# 为什么给 Claude Code 添加 Skill 会显示影响 OpenCode

> 状态：2026-07-19 已在工作树中修正确认框语义与真实写入路径；OpenCode 运行时禁用变量的探测尚未实现。

## 结论

MUX 没有从 Claude Code 页面推断出额外目录，也不需要用户在 OpenCode 配置里手动声明 `~/.claude/skills`。

原因是 OpenCode 自身默认兼容 Claude Code Skills：它会扫描 `~/.claude/skills/<name>/SKILL.md`。因此，只要 OpenCode 已被 MUX 检测为已安装，向 Claude Code 的首选目录写入一个 Skill，就会让 OpenCode 同时发现它。

```text
MUX 写入 ~/.claude/skills/frontend-design
                  │
          ┌───────┴────────┐
          ▼                ▼
   Claude Code 发现     OpenCode 默认兼容扫描
```

这是一份物理 Skill、两个实际消费者，不是必须复制两份内容。

## 代码依据

### 1. Agent 目录表声明了反向兼容关系

[`data/agents.json`](../data/agents.json) 中：

- Claude Code 的首选目标是 `claude-user`，路径为 `~/.claude/skills`，没有 aliases（第 7 行）。
- OpenCode 的首选目标是 `opencode-user`，路径为 `~/.config/opencode/skills`；同时把 `claude-user` / `~/.claude/skills` 和 `agents-user` / `~/.agents/skills` 声明为兼容读取目录（第 30 行）。

这里的关系是“OpenCode 读取 Claude Code 的目录”，并不是“Claude Code 配置了 OpenCode 的目录”。

### 2. MUX 把兼容读取者加入物理目标的影响范围

[`core/src/skills/inventory.rs`](../core/src/skills/inventory.rs) 的 `build_target_graph`：

- 第 1441–1460 行注册每个 Agent 的首选目标和 aliases。
- 第 1473–1480 行把所有已安装 Agent 加入其声明目录的 `affected_agent_ids`。

所以 `claude-user` 这个物理目标的影响集合会成为：

```text
claude-user -> { claude-code, opencode }
```

前提是 MUX 通过 command/path probe 检测到 OpenCode 已安装。

### 3. 确认计划主动扩展到全部真实消费者

[`core/src/consumption/planner.rs`](../core/src/consumption/planner.rs) 第 581–600 行把目标的 `affected_agent_ids` 合并进计划，并为这些 Agent 生成关系变化。因此选择 Claude Code 后，确认框会同时出现 `claude-code` 与 `opencode`。

[`desktop/src/components/AssetOperationReviewDialog.tsx`](../desktop/src/components/AssetOperationReviewDialog.tsx) 第 75–79、140–150 行只是渲染核心计划，所以显示为“另影响 1 个 Agent”和“添加 opencode”。

## OpenCode 官方行为

OpenCode 官方 Skills 文档明确列出六类搜索位置，其中包含全局 Claude-compatible 路径 `~/.claude/skills/<name>/SKILL.md`。这不依赖 `opencode.json` 中增加一条目录配置。

如果不希望 OpenCode 加载 Claude Code Skills，OpenCode 官方提供环境变量：

```sh
OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1
```

但 MUX 当前只根据静态能力目录和安装探针计算影响，没有检测这个运行时环境变量。因此即便用户禁用了 OpenCode 的 Claude Skills 兼容，MUX 仍可能显示 OpenCode 受影响。

## 原确认框的两个表达问题

### “添加 opencode”语义过重

实际含义更接近：

> 写入 Claude Code 目录后，OpenCode 也能读取这个 Skill。

它不等价于“单独向 OpenCode 首选目录安装一份”。UI 应区分：

- 主动写入：Claude Code
- 被动可见：OpenCode（兼容读取 `~/.claude/skills`）

### “将更新的位置”列出了三个目录

原实现中，[`core/src/consumption/planner.rs`](../core/src/consumption/planner.rs) 会遍历所有受影响 Agent 声明过的全部 Skills 目标，因此计划预览会列出：

- `~/.claude/skills/frontend-design`
- `~/.agents/skills/frontend-design`
- `~/.config/opencode/skills/frontend-design`

这比实际最小物理分配目标更宽。底层 assignment normalization 会保留覆盖所选消费者的最小目标；此场景中 `~/.claude/skills/frontend-design` 已同时覆盖 Claude Code 和 OpenCode，不需要再向 OpenCode 首选目录复制一份。

因此原确认框的路径列表不是对实际最小写入集的准确表达。现已改为根据最终归一化后的 `target_id` 生成，只展示真正可能发生变更的物理路径。

## 已实施的修正

1. 将关系变化拆成“直接添加”和“兼容可见”，避免把被动消费者写成主动安装。
2. 将“将更新的位置”改为“实际写入位置”，只展示最终归一化后的真实物理目标。
3. 增加共享目录说明：兼容 Agent 读取同一份 Skill，不会重复安装。
4. 增加 Core 与 Desktop 回归测试，固定 Claude Code → OpenCode 的共享目录语义。

后续若要精确支持 OpenCode 禁用兼容的环境变量，应把运行时禁用状态纳入能力探测；当前提示仍基于 OpenCode 默认行为。

## 参考

- [OpenCode 官方 Agent Skills 文档](https://opencode.ai/docs/skills/)
- [OpenCode 官方 Rules 文档](https://opencode.ai/docs/rules/)
