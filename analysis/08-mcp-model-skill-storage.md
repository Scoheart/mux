# MUX 中 MCP、Model 与 Skill 资产的存储位置

## 结论

自 2026-08-09 起，三类中央资产统一存放在
`~/.mux/assets/{mcps,models,skills}`。Core 仍向调用方提供一份 hydrated
`Settings`，因此 Desktop、CLI 与 TUI 不需要分别理解物理文件；API Key
继续保存在 macOS Keychain，不进入资产目录。

## 当前结构

```text
~/.mux/                         # 或 $MUX_HOME
├── settings.json              # Agent、消费关系、启用状态、UI/network
├── assets/
│   ├── mcps/
│   │   ├── catalog.json       # registry 与 source 定义
│   │   └── sources/
│   │       ├── remote/        # 订阅缓存
│   │       └── local/         # 本地导入、manual、discovered
│   ├── models/
│   │   └── catalog.json       # Provider 与 Model Profile
│   └── skills/
│       ├── catalog.json       # Skill 来源、hash、risk 等 metadata
│       └── items/<name>/      # Skill 唯一中央正文
├── skills -> assets/skills/items
├── staging/                   # 审阅计划与事务暂存
├── backups/                   # 可恢复备份
└── journals/                  # 崩溃恢复 journal
```

`~/.mux/skills` 是旧版本 Agent 链接的兼容别名，不是第二份正文。

## 数据归属

- MCP 资产：`assets/mcps/catalog.json` 与 `assets/mcps/sources/`。
- Model 资产：`assets/models/catalog.json`；Provider API Key 使用
  `com.scoheart.mux.model-provider.<provider-id>` / `api-key` 存入 Keychain。
- Skill 资产：`assets/skills/catalog.json` 与 `assets/skills/items/<name>/`。
- Agent 消费关系：继续位于 `settings.json` 的 `mcp_consumptions`、
  `model_consumptions` / `model_assignments`、`skill_consumptions` /
  `skill_assignments`。

## 迁移与一致性

启动顺序先完成既有 Model schema migration，再执行资产目录迁移和 MCP
registry migration。旧 `~/.mux/sources` 与 `~/.mux/skills` 会幂等迁移；旧字段与
新 catalog 同时存在时只有语义一致才允许继续，否则 fail closed。中央 catalog
与 `settings.json` 使用 CAS 写入和逆序回滚，并纳入跨域资产事务快照。

CLI、TUI 与 Desktop 都通过 `mux-core` 的 `MuxCore::bootstrap()`、settings loader
和资源 API 访问同一布局；重新构建后的各前端不会维护第二套路径实现。
