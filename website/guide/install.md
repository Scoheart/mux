# 安装

MUX 有桌面 App 和 CLI / TUI 两个入口，共享 `~/.mux/`。桌面 App 已内置 CLI，通常无需重复安装。

## 桌面 App（macOS）

1. 打开[最新正式版 Release](https://github.com/Scoheart/mux/releases/latest)，选择标注为 **Desktop installer · Apple Silicon** 的资源（文件名形如 `MUX-Desktop-Installer-*-macOS-Apple-Silicon.dmg`）。
2. 打开 dmg，把 **MUX.app** 拖进 `/Applications`。
3. 首次打开。

### 提示「MUX 已损坏，无法打开」？

当前发布包未经过 Apple Developer ID 公证，macOS 可能因隔离属性（quarantine）阻止首次启动，并不代表文件损坏。确认下载来源是本项目 Release 后，可以清掉隔离属性：

```bash
xattr -dr com.apple.quarantine /Applications/MUX.app
```

然后正常打开。（或者：右键 App → 打开 → 在弹窗里再点"打开"。）

## 命令行 / TUI（`mux`）

`mux` 是一个原生 Rust 二进制，和桌面 App 共用同一个核心。

### 方式一：随桌面 App 安装（推荐）

正式版 App 启动后会把包内 CLI 软链到 `~/.local/bin/mux`。如果终端找不到 `mux`，把目录加入 `PATH`：

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
mux --version
```

这个软链随 App 自动修复；App 更新后，CLI 也同步更新。

### 方式二：单独下载预编译二进制

在正式版 Release 中选择标注为 **Command-line tool · Apple Silicon** 的资源。为兼容旧版 `mux upgrade`，实际文件名仍是 `mux_v<版本>_aarch64-apple-darwin.tar.gz`：

```bash
# 到 Releases 下载后：
tar xzf mux_v*_aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin
mv mux ~/.local/bin/mux
mux --version
```

独立安装的 CLI 可以运行 `mux upgrade` 跟随最新正式版；桌面 App 自带的 CLI 会提示改由 App 更新。

### 方式三：从源码安装（需要 Rust）

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
mux apply <名字…>   # 非交互安装到全局配置（--agent）
mux export --out mcp.json  # 导出生效配置
mux agents list     # 列出所有 agent
mux upgrade         # 升级独立安装的 CLI
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
└── backups/                # 修改已有 Agent 配置前的独立时间戳备份
```

桌面和 CLI 都读写这里，所以两端天然同步。

下一步 → [核心概念](/guide/concepts)
