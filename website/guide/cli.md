# 命令行 / TUI

`mux` 是一个原生 Rust 二进制，和桌面 App 构建在同一个 `mux-core` 之上，一切都跑在共享的 `~/.mux/` 上。

> 还没装？看 [安装 · 命令行](/guide/install#命令行-tui-mux)。

它有两种用法：

- **无参数** → 进入交互式 **TUI**（键盘驱动的终端管理器）；
- **带子命令** → 非交互，可脚本化。

## 交互式 TUI

```bash
mux
```

TUI 有三个屏幕，顶部用数字键切换：

| 键 | 屏幕 |
|---|---|
| `1` | Registry（目录） |
| `2` | 来源（Sources） |
| `3` | Agents |

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
| `i` | 安装向导（多选 agent，空格勾选，`Ctrl-S` 确认） |
| `n` | 新建条目 |
| `e` | 编辑选中条目 |
| `p` | 粘贴一段 `mcpServers` 配置 |
| `S` | 重新同步选中条目（定制过的会先确认，可强制） |
| `d` | 删除选中条目（→ 确认；remote/local 会给出提示） |

### 来源屏幕

| 键 | 作用 |
|---|---|
| `Space`/`Enter` | 启停选中来源 |
| `r` | 刷新来源 |
| `s` | 订阅一个 URL |
| `l` | 导入本地文件 |
| `o` | 添加 Mux 精选 |
| `d` | 删除来源（→ 确认） |

### Agents 屏幕

| 键 | 作用 |
|---|---|
| `Enter`/`→`/`l` | 进入某个 agent，查看它装了哪些 MCP |
| `Space` | 启停 agent（列表层）/ 启停某个已装 MCP（详情层） |
| `a` | 给该 agent 添加 MCP |
| `e` | 编辑 agent 配置路径 |
| `n` | 新增自定义 agent |
| `d` | 从该 agent 卸载选中 MCP（详情层） |

## 子命令（脚本化）

在脚本里用时，设 `MUX_NO_TUI=1` 让无参数运行时打印帮助而不进 TUI。

```bash
mux import                       # 扫描各 agent，导入探测到的 server
mux list                         # 列出目录里的条目
mux status                       # 各 agent 当前生效的 MCP
mux add <名字>                   # 交互式添加一个 server 到手动来源
mux remove <名字>                # 从手动来源删除一个条目
mux apply <名字…>                # 非交互安装
mux export [--out <文件>]        # 导出去重后的生效目录；默认输出到 stdout
mux clean [--agent <名字>]       # 清空（已启用）agent 的 MCP
mux agents                       # 列出所有 agent
mux agents enable <名字>         # 启用一个 agent
mux agents disable <名字>        # 停用一个 agent
mux upgrade                      # 升级独立安装的 CLI
```

### `mux apply` 的参数

```bash
mux apply github filesystem \
  --agent all             # 逗号分隔的 agent 名，或 "all"（默认 all）
```

`mux apply` 只写入 Agent 的全局配置。

举例：只把 `github` 装到 Claude Code 和 Cursor：

```bash
mux apply github --agent "claude-code,cursor"
```

如果同名条目同时存在 stdio 和 HTTP 版本，`mux apply <名字>` 会处理该名字下的所有传输版本。

### 导出

```bash
mux export                    # JSON 输出到 stdout
mux export --out mcp.json     # 保存到文件
```

导出内容是完整的**生效目录**：每个 `name::transport` 只保留优先级最高的副本。

### 更新

独立下载或 `cargo install` 的 CLI 可运行：

```bash
mux upgrade
```

普通子命令执行后每天最多检查一次最新正式版；设置 `MUX_NO_UPDATE_CHECK=1` 可关闭。桌面 App 自带的 CLI 由 App 更新，不会自行替换包内二进制。

## 和桌面 App 的关系

两者读写同一个 `~/.mux/`。CLI 里 `mux add` 的条目，桌面 App 刷新后立刻可见；桌面里的安装，`mux status` 也能看到。数据模型只有一份，永不分叉。

下一步 → [支持的 Agent](/guide/agents)
