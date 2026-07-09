# 安装

MUX 有两个前端，可以只装一个，也可以都装 —— 它们共享 `~/.mux/`。

## 桌面 App（macOS）

1. 到 [**Releases**](https://github.com/Scoheart/mux/releases) 页面，下载最新的 **`MUX_*_aarch64.dmg`**（Apple Silicon）。
2. 打开 dmg，把 **MUX.app** 拖进 `/Applications`。
3. 首次打开。

### 提示「MUX 已损坏，无法打开」？

这是 macOS 对**未签名应用**的隔离（quarantine），不是应用真的坏了。清掉隔离属性即可：

```bash
xattr -dr com.apple.quarantine /Applications/MUX.app
```

然后正常打开。（或者：右键 App → 打开 → 在弹窗里再点"打开"。）

## 命令行 / TUI（`mux`）

`mux` 是一个原生 Rust 二进制，和桌面 App 共用同一个核心。

### 方式一：下载预编译二进制（无需工具链）

每个 Release 都附带一个 `mux_*_aarch64-apple-darwin.tar.gz`：

```bash
# 到 Releases 下载后：
tar xzf mux_*.tar.gz
# 放到 PATH 上，例如：
mv mux ~/.cargo/bin/mux        # 或 /usr/local/bin
mux --version
```

### 方式二：从源码安装（需要 Rust）

```bash
git clone https://github.com/Scoheart/mux
cd mux
cargo install --path cli       # 装到 ~/.cargo/bin/mux
```

### 用法

无参数运行进入**交互式 TUI**：

```bash
mux
```

也可以用子命令脚本化（设 `MUX_NO_TUI=1` 让无参数时打印帮助而不进 TUI）：

```bash
mux list            # 列出目录里的 MCP
mux status          # 各 agent 当前生效的 MCP
mux apply <名字…>   # 非交互安装（--scope / --agent / --project）
mux agents list     # 列出所有 agent
```

详见 [命令行 / TUI](/guide/cli)。

## 数据放在哪

所有用户数据都在 `~/.mux/`：

```
~/.mux/
├── settings.json           # 单一文档：agents · sources · disabled · state
├── sources/
│   ├── remote/<id>.json    # 订阅 URL 的缓存
│   └── local/<id>.(json|toml)  # 导入的本地文件 + 手动/探索两个托管来源
└── backups/                # 每次写入 agent 配置前的时间戳备份
```

桌面和 CLI 都读写这里，所以两端天然同步。

下一步 → [核心概念](/guide/concepts)
