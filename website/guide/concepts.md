# 核心概念

理解这几个概念，MUX 的所有操作就都说得通了。

## 目录（Registry）

**目录**是 MUX 的中心——你所有 MCP 服务器的聚合视图。它**不是**一份写死在程序里的清单，而是由你启用的所有 [来源](#来源-sources) **实时拼装**出来的并集。

在目录里你可以搜索、按来源过滤、检查被覆盖副本，并查看每个条目的传输方式、来源和使用情况。

## 来源（Sources）

目录里的每个条目都来自某个来源。MUX 有四种来源：

| 来源 | 是什么 | 存放位置 |
|---|---|---|
| **订阅（remote）** | 一个指向 MCP 配置文件的 **URL**，MUX 抓取并缓存；刷新时重新拉取上游。 | `~/.mux/sources/remote/<id>.json` |
| **本地（local）** | 从磁盘**导入**的配置文件，复制进 MUX；刷新时重新读取原文件。 | `~/.mux/sources/local/<id>.(json\|toml)` |
| **手动添加（manual）** | 你手写或**粘贴**创建的 server，存为一个受管的本地来源（`manual.json`）。 | `~/.mux/sources/local/manual.json` |
| **历史探索（discovered）** | 旧版本已写入 `discovered.json` 的受管条目，升级后继续作为中央资产保留；新扫描不再自动写入。 | `~/.mux/sources/local/discovered.json` |

另外还有一个一键的 **Mux 精选**——它其实就是订阅一个内置的、经过整理的远程来源，属于**可选**订阅，而不是默认基座。

Agent 文件中的新发现不属于第五种来源，而是独立的只读 **external observed state**。用户显式导入后，它才成为中央资产；导入也不会顺手创建消费关系。

只有**已启用**来源参与目录拼装。来源模型支持启停，TUI 的来源屏幕可直接切换；桌面 `v1.2.0` 目前提供过滤、刷新和删除，暂未暴露启停开关。

### 优先级（去重规则）

同一个 MCP 可能出现在多个来源里。目录按 **`name::transport`** 组合键去重，优先级 **从低到高**：

```
外部来源（remote / local） < 自动探索（discovered） < 手动添加（manual）
```

也就是**你自己的手动编辑永远赢**——即使某个远程来源也定义了同名条目。

### 全部与被覆盖筛选

桌面 Registry 默认显示每个来源中的全部副本：

- 优先级最高的副本正常显示，并参与安装与导出。
- 其余副本标记为 **被覆盖**，显示“以某来源为准”。
- 当前范围存在冲突时，工具栏出现 **被覆盖 N**，可只看这些副本。

左侧“全部”表示全部来源，不再有单独的“生效中”入口。点具体来源后，冲突筛选只作用于该来源。

## 身份、传输与来源标签

- **组合键 `name::transport`**。`transport ∈ {stdio, http}`（sse 归入 http）。同名的 stdio 和 http 是**两个独立条目**。
- **传输方式自动识别**：MUX 会识别标准字段和 Agent 专属字段，例如 OpenCode 命令数组、Gemini `httpUrl`、Windsurf `serverUrl`。目录统一为 stdio / http 模型，写入时再转换为目标 Agent 的字段和传输名称。
- **来源标签**：每个条目带一个来源类型（`discovered` / `manual` / `remote` / `local`）+ 来源 id，驱动界面上的来源徽章。

## 中央资产与消费关系

中央资产定义“它是什么”，消费关系定义“哪个 Agent 应该使用它”。Desktop 的 Agent 页面和资产详情修改同一份 desired state：

- **MCP / Skills**：一个 Agent 可消费 `0..N` 个中央资产。
- **Model**：一个 Agent 同时最多消费 `0..1` 个 Profile。
- **解除使用**：删除关系和该 Agent 的受管目标，不删除中央资产。
- **observed state**：Agent 文件与 Skill link 只用于对账；外部内容、漂移和冲突不会自动反写 desired state。

## 编辑传播（Edit propagation）

编辑中央资产时，MUX 先枚举所有 desired consumers，并把中央变化、关系和目标文件放进同一影响计划。干净目标可一次确认后全部传播；漂移目标必须显式确认覆盖，冲突或并发变化阻止整个提交，不做部分更新。

## 删除目录条目（Forget）

**彻底删除**一个用户拥有的中央条目，会先展示全部消费者，再从相应受管 source copy 删除它，并原子清理所有 desired relationship 与 Agent 受管目标。

只有 **manual / discovered** 条目能这样删——remote / local 来源的条目没有"用户拥有"的部分可删，请通过管理它们的来源来处理。

## 数据布局

所有用户数据都在 `~/.mux/`：

```
~/.mux/
├── settings.json           # 单一文档：agents · sources · 中央 metadata · desired state
├── update-check.json       # 独立 CLI 的每日更新检查缓存（按需生成）
├── sources/
│   ├── remote/<id>.json    # 订阅 URL 的缓存
│   └── local/<id>.(json|toml)  # 导入文件 + manual.json / discovered.json
├── staging/consumption/    # 审阅计划与事务回滚快照
└── backups/                # 修改已有 Agent 配置前的独立时间戳备份
```

`settings.json` 是**一个文档**：MUX 只修改自己拥有的部分，其余字段透传。每次修改都是**读整份 → 改一段 → 原子写回**（临时文件 + 重命名）。

Agent 的 JSON / TOML / YAML 配置采用“定位 MCP 节点 → 定位目标条目 → 修改受管连接字段”的方式更新。其它顶层键、其它 server、目标条目里的权限 / OAuth / 工具策略等未建模字段，以及注释和原有排版都会保留；已有文件先备份，再通过同目录临时文件原子替换。备份失败、并发修改、文件或节点结构不合法时都会拒绝写入，不会尝试修复或覆盖整个配置。

## 配置路径可移植

用户主目录内的 Agent 配置路径会折叠为 `~/…`，便于迁移；主目录之外的自定义绝对路径会保持原值。跨机器同步 `~/.mux/` 时仍需核对这些外部路径。

下一步 → [桌面 App 指南](/guide/desktop) 或 [命令行 / TUI](/guide/cli)
