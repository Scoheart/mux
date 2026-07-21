# Agent 能力审计：实现边界与差距

> 这是 2026-07-22 审计完成后的实现边界记录。最终工作树统计为 56 个 audited definitions、46 个 MCP writer、45 个 Skills target 与 14 个 Model target（12 managed、2 guided）；身份覆盖与实现统计的机器证据分别见 validator 和 capability baseline。

## 判定原则

MUX 只有在以下条件同时满足时才开启 writer：

1. 官方文档、官方源码或官方发布包给出稳定路径和 schema；
2. 配置属于用户明确可管理的文件或目录，而不是应用数据库、云端账户状态或未公开的 credential store；
3. MUX 可以只修改自己拥有的字段，并保留未知字段、同级条目和用户显式的 `false`；
4. API Key 不需要从中央资产复制为明文；必须能使用 Keychain command、环境变量引用或“仅保留外部凭据”；
5. Agent 的“已安装连接”“默认模型”“当前会话模型”语义可以被准确表达，不能把连接存在误报为当前模型；
6. 删除、覆盖和迁移可以 fail closed，并有 fixture 覆盖 round-trip、冲突与损坏输入。

目录、社区帖子和 ACP manifest 只用于发现身份，不能单独开启 writer。

## 当前可以安全落地的类型

| 能力形态 | MUX 当前处理 | 说明 |
|---|---|---|
| 用户级 JSON / TOML / YAML MCP map | 可写 | 需要 Agent-specific codec 验证 transport、字段名和默认值 |
| 用户级 MCP list | 可写 | 必须有稳定 identity field，并拒绝重复身份 |
| 标准 `SKILL.md` 用户目录 | 可写 | 记录真实写入目录、共享读取者、别名和重名优先级 |
| OpenAI-compatible Model + 环境变量引用 | 可写 | 仅限存在稳定 provider inventory 与 active/default pointer 的 Agent |
| Model + 官方可执行 credential command | 可写 | 仅在 Agent 运行时确实执行该 command，并对 argv、超时和失败行为有官方源码证据时使用 |
| 外部明文凭据 | 只发现 | 只记录“存在”，不复制、不回显、不哈希原值到用户可见内容 |

## 已核验但本版未接入的 Model 候选

### CodeWhale Models

- 配置：`~/.codewhale/config.toml`。
- 只接管 `kind = "openai-compatible"` provider。
- 官方 runtime 已证明 `api_key_env` 与 OpenAI Chat Completions 路径，但配置根可被 CLI/env 改写，project model 还能覆盖默认值。
- 本版不写入、不迁移、不观察为 active；等待 resolved-path/effective-layer resolver、provider ownership fingerprint 和局部 clear 契约。

### VT Code Models

- 配置：`~/.vtcode/vtcode.toml`。
- 官方 runtime 已证明 `[[custom_providers]]`、`[agent]` pointer 与 command auth，但 system/project/workspace/VTCODE_CONFIG_PATH 都可能覆盖用户文件。
- 本版不写入、不迁移、不观察为 active；等待 effective-layer observation、credential argv ownership 和非破坏性 clear。
- VT Code 已有 MCP/Skills 能力保持不变；这里的回撤只涉及 Models。

## 既有 writer 的 scope 与 precedence 约束

### Stakpak

- 用户 MCP 是最低优先级 `~/.stakpak/mcp.toml`；项目根与 `.stakpak/` 内配置可覆盖它。
- Skills 用户目录为 `~/.stakpak/skills`。
- MUX 当前只写用户层，并把它标明为最低优先级 fallback；尚未实现项目文件探测与实际生效层 observation，因此不能把“已写入”误报为“已生效”。

## 本版新增文件型 Agent

A–M 与 N–Z 报告中的 candidate 只有在现有 schema 能无损表达时才会从 discovery catalog 提升为 audited definition。每个提升项必须同时加入：

- 官方证据、核验日期和安装 probe；
- transport capability；
- 专用或经过证明可复用的 codec；
- round-trip、未知字段保留、非法输入和 transport 拒绝测试；
- Skills evidence allowlist（如适用）；
- 图标或明确的中性 fallback 契约，不能冒用其他品牌图标。

本版按上述门禁实际新增 2 个 MCP writer：`agentkube`、`chatmcp`；新增 9 个 Skills-only Agent：`docker-agent`、`cortex-code`、`dirac`、`minion-code`、`poolside`、`raycast`、`theiaai-theiaide`、`trae-ide`、`zencoder`。这些提升使 registry 从 45 增至 56；A–M/N–Z 分片中的其余 research candidate 不计入当前 writer/target 数量。

## 明确延后的能力

| 类别 | 典型 Agent / 情况 | 延后原因 | 需要的产品能力 |
|---|---|---|---|
| 应用数据库或云端状态 | Raycast MCP/Models、Tome、部分桌面客户端 | 无官方外部写 API；直接改 SQLite/内部状态会绕过迁移、加密和运行时生命周期 | 官方 import/export API 或 versioned management API |
| 每会话模型状态 | oterm 等 | provider connection 可存在，但当前模型保存在 chat/SQLite；不存在全局 pointer | “连接已安装但无全局当前模型”的独立状态模型 |
| 强制明文 API Key | siGit Models 等 | Agent schema 没有 env/keychain reference；默认写入会降低安全性 | 上游安全引用，或明确的逐次明文导出同意与文件权限门禁 |
| 项目级唯一配置 | Replit Agent、TRAE Agent、项目 MCP/Skills | 当前中央关系主要按用户级路径建模，不能凭当前工作目录猜项目所有权 | 显式项目选择、workspace trust、project-scoped binding 与 precedence inspector |
| Windows-only 路径 | Visual Studio | 当前发布目标和验证环境为 macOS，无法证明 Windows path/encoding/ACL round-trip | 平台条件 capability 与 Windows CI/fixture |
| JSONC 及复杂嵌套设置 | Theia、Zencoder VS Code 等 | 普通 JSON rewrite 会破坏注释；嵌套 preference 还需局部 CST 修改 | JSONC/CST 局部编辑器与 workspace scope |
| 带 OAuth/密文的复杂数组 | Witsy MCP、Raycast MCP | 条目包含 UUID、OAuth、token、额外状态；通用 map codec 会丢字段或接管 secret | 专用 array codec、secret-field exclusion、live reload 验证 |
| 多模型但 writer 会收敛 | 某些 VT/自定义 provider | 接管一个 provider 可能删除 sibling model | 一对多 provider/model binding 与非破坏性 active pointer |
| 运行参数决定配置根 | `XDG_CONFIG_HOME`、`OTERM_DATA_DIR`、`SIGIT_CONFIG_DIR` 等 | 固定默认路径可能创建第二份无效配置 | Agent environment/profile 选择和 resolved-path probe |

## Catalog 卫生差距

本轮发现目录中混有 IDE framework、MCP server/demo、云端工作台、旧名和重复别名。它们可以作为 discovery lead，但不能都叫“可配置 Agent”。后续 schema 应至少区分：

- `agent-client`：可检测、可能存在 writer；
- `host-framework`：由下游产品决定配置；
- `server-or-sdk`：不是 Agent client；
- `hosted-service`：仅 UI/API 管理；
- `stale-or-alias`：合并到 canonical identity；
- `unverified-lead`：只有社区目录证据。

在分类字段落地前，报告用 `audited-writable`、`audited-read-only`、`catalog-only`、`misclassified`、`duplicate` 和 `rename-or-stale-identity` 保留这一差异，UI 不应把 discovery-only 数量解读为可写支持数量。

## 仍需补齐的架构能力

1. **统一 capability descriptor**：MCP、Models、Skills 目前分别散落在 registry、adapter 和硬编码分支中。
2. **项目级 binding**：显式选择项目、记录 trust 与 precedence，并让扫描/计划/验证使用同一个 scope。
3. **连接与 active state 解耦**：Model provider inventory、默认模型和会话内当前模型必须是三个状态。
4. **credential strategy 枚举**：environment reference、Keychain command、external-managed、plaintext-required 需要统一、可审计的表达。
5. **lossless structured editors**：JSONC、复杂 TOML array、YAML 注释与应用特定数组需要 CST/typed codec。
6. **环境解析**：配置根被环境变量、命令参数或多 profile 决定时，应显示实际 resolved path，而不是只展示默认路径。
7. **跨进程事务与恢复**：Desktop、CLI 和将来的后台扫描必须共享文件锁、journal、计划与验证。

这些差距不阻止安全的用户级子集先落地，但会阻止 MUX 对不具备真实契约的 Agent 宣称“完整支持”。
