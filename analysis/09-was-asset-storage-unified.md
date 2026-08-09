# MCP、Model、Skill 的资产存储是否完成统一

## 结论

已经完成。此前 MUX 只统一了领域模型、状态契约、消费关系与操作生命周期；
2026-08-09 的实现进一步把三类中央资产统一到：

```text
~/.mux/assets/
├── mcps/
├── models/
└── skills/
```

## 与旧架构的关系

旧架构的控制面统一仍然保留：`AssetRef`、`AssetStateStore`、Workspace Snapshot
和 Plan / Commit / Cancel 没有被替换。此次变更只整理权威数据面：

- MCP source cache 从 `~/.mux/sources` 移入 `assets/mcps/sources`。
- Provider/Profile 从 `settings.json` 移入 `assets/models/catalog.json`。
- Skill metadata 和正文分别移入 `assets/skills/catalog.json` 与
  `assets/skills/items`。
- `settings.json` 继续保存 Agent、消费关系与运行状态。
- Model secret 继续保存在 macOS Keychain。

三类资产仍保留适合各自领域的格式，而不是强行合并成一个 `assets.json`。

## 兼容性

启动 migration 会移动旧目录和旧 metadata；旧 `~/.mux/skills` 变为指向新正文
目录的兼容软链接，使早期 MUX 创建的 Agent Skill 链接继续有效。Desktop、CLI
与 TUI 共用同一 Core bootstrap，因此二进制升级后会使用相同的新布局。
