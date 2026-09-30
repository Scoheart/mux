# Pi 原生 MCP 与 MUX 适配

核对日期：2026-09-30。

## 结论

Pi 的 [v0.99.0 官方发布记录](https://github.com/earendil-works/pi/releases/tag/v0.99.0)明确加入内置 MCP、codemode 和 `pi mcp` 命令。官方 [MCP 文档](https://pi.dev/docs/latest/mcp)确认：用户级配置仍是 `~/.pi/agent/mcp.json`，顶层键仍是 `mcpServers`。因此 MUX 原有的文件位置和基本 stdio/HTTP 结构无需迁移；需要更新能力证据及 Pi 专属行为。

本机核查时 `pi --version` 为 `0.82.1`，`pi list` 显示已安装 `pi-mcp-adapter`。这些是本机当前状态，不代表 MUX 的内置定义。要使用 Pi 原生 MCP，需先让 Pi 本身达到 `0.99.0+`；[官方文档](https://pi.dev/docs/latest/mcp)还说明，注册 `/mcp` 的旧扩展会替换原生会话行为。本文与本次代码改动均不自动升级 Pi 或移除用户扩展。

## 代码路径

```mermaid
flowchart LR
  A[data/agents.json Pi 定义] --> B[core/src/agents.rs 合并内置能力]
  B --> C[Codec::Pi]
  C --> D[JsonAdapter 读取/写入]
  D --> E[~/.pi/agent/mcp.json]
  E --> F[Pi 0.99.0+ /reload 后读取]
```

Pi 定义仍位于 `data/agents.json:49`。MUX 从该记录获取官方证据、路径和专属 `pi` codec；已保存的内置定义会由 `core/src/agents.rs:343` 更新审计元数据，同时保留用户启用状态和合法的自定义全局路径。CLI 与 Desktop 都通过同一 core 消费这些能力，没有两份独立的 Pi writer。

`core/src/resources/mcp/codec.rs:126` 增加 Pi codec。它继续将 `command`/`args`/`env`/`cwd` 及 `url`/`headers` 写成官方兼容的 JSON，拒绝 Pi 新版不支持的旧 SSE 类型。`core/src/resources/mcp/json_adapter.rs:22` 按官方规则检查服务名；非法名称不当成已连接的 MCP，也不会写入。`core/src/resources/mcp/codec.rs:319` 将缺失的 `enabled` 当作启用，读取 `enabled: false` 时明确标为关闭。

`core/src/resources/mcp/json_adapter.rs:348` 复用现有原生开关能力，`set_enabled` 只修改 Pi 的 `enabled` 字段。该操作仍依赖完整条目快照比对，其他进程在计划与提交之间改动该条目时拒绝覆盖。普通 `upsert` 只修改连接字段；`exposure`、`toolExposure`、`timeout`、其他 MCP 项和根目录其他键原样保留。隔离 HOME 的回归用例见 `core/tests/agent_formats.rs:142`。

## 使用边界

- MUX 只写用户级全局 `mcp.json`；项目级 `.pi/mcp.json` 仍由可信项目和 Pi 自己管理。
- `mcp-auth.json` 中的 OAuth 凭据归 Pi 管理，MUX 不读写它。MUX 不替 Pi 连接服务器，也不宣称 `pi mcp list` 已实际连通。
- 新版 Pi 运行中的会话需 `/reload` 才会应用外部文件变化。若安装的旧 `pi-mcp-adapter` 抢占 `/mcp`，原生配置在交互会话里可能不生效，需按官方说明处理扩展。
- 本次按仓库极速交付规则不运行测试套件；生产编译、发布文件复验和正式安装版验收记录于交付结果。
