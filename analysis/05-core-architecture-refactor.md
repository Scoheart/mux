# MUX Core 架构重构（2026-07-23）

## 结论

MUX 的 Core 已从“以 MCP Registry/codec 为中心、附带 Model 与 Skill 协调逻辑”重构为面向 Agent 资源的共享内核。新的稳定调用方向是：

```text
CLI / Tauri / future frontends
              │
              ▼
       application::MuxCore
       ├── bootstrap
       ├── snapshot
       └── plan / commit / cancel
              │
              ▼
 application workspace gate (read / write)
              │
              ▼
     domain contracts + assets coordinator
              │
              ▼
 resources::mcp / model / skill + storage adapters
```

MCP、Model、Skill 现在都是 Agent 可独立具备的能力，不再由 `mcp_path` 决定一个 Agent 能否进入统一配置与展示。CLI 与 Desktop 共用启动恢复顺序、能力图、只读工作区快照和统一操作入口；旧 Rust 模块路径只在 `lib.rs` 的兼容边缘保留，Core 实现不再反向依赖这些别名。

## 重构前的问题

### 1. 产品模型仍由 MCP 字段决定

旧 `AgentConfigurationInput` 要求调用者始终提供 `mcp_path`。因此只支持 Model 或 Skill 的 Agent，也必须伪造 MCP 配置才能参与配置流程。更深层的问题是前端需要分别读取 MCP Agent、Model Agent 和 Skill target，再自行拼成一张 Agent 卡片。

### 2. 模块名称掩盖了实际边界

原根模块中的 `adapter`、`codec`、`registry`、`scanner`、`ops` 实际都是 MCP 基础设施，却以通用名暴露在 `mux_core::*`。`consumption` 同时承载跨资源关系、生命周期、迁移和事务，名称不能说明其真正职责。

### 3. 前端组合业务流程

CLI 和 Tauri 可以直接调用 registry、scanner、ops、models writer 或 Skills 生命周期函数。启动时，两端对 Skills recovery、资产 recovery、Model migration/reconciliation 的顺序也不一致。这使恢复安全和 mutation 规则容易随前端漂移。

### 4. 查询可能隐式写入

Model inventory 读取曾顺带执行 active model reconciliation。调用者只想刷新页面时，也可能改 settings 或 Agent 文件，无法提供真正只读的工作区快照。

### 5. CI 没有保护架构边界

旧 CI 只运行根 workspace，而 Tauri crate 被排除在根 workspace 之外。未注册的旧 Tauri mutation commands 和与新漂移确认契约矛盾的测试因此长期存在。

## 新架构

### Domain：只表达身份、能力、资产与错误

`core/src/domain/` 保存不依赖文件系统、Keychain、网络或具体 adapter 的 contracts：

- `AgentConfigurationPatch` 的 `mcp`、`model`、`skill` 三个域均为可选，配置一个域不再要求另两个域存在（[`core/src/domain/agents.rs`](../core/src/domain/agents.rs)）。
- `AgentCapabilityView` 把身份与三个 typed capability 聚合成统一 Agent 投影（[`core/src/domain/agents.rs`](../core/src/domain/agents.rs)）。
- `CoreError` 提供稳定 `code`、message、details、retry 和 confirmation wire shape；前端不再解析任意错误字符串。
- `AssetRef`、`DomainPlan`、relationship/state diff 继续保持域类型，而不是压成一个 MCP-shaped map。

架构测试会扫描 `domain/`，禁止其重新依赖 settings、adapter、registry、models、skills 或其他基础设施模块（[`core/tests/application_architecture.rs`](../core/tests/application_architecture.rs)）。

### Application：前端唯一业务入口

`MuxCore` 暴露四组稳定用例：统一启动、只读快照、计划、提交/取消（[`core/src/application/mod.rs`](../core/src/application/mod.rs)）。

`PlanOperationRequest` 覆盖：

- MCP / Model 中央资产 create、update、delete、adopt；
- Agent 与 MCP / Model / Skill 的 desired relationship；
- MCP / Model enable/current 状态；
- MCP drift 的显式 reviewed reapply；
- Agent capability configuration；
- Skill install、import、assign、update、remove、repair。

这些请求统一返回带 domain tag 的 `OperationPlan`，再通过统一 commit/cancel envelope 进入已有的安全事务引擎（[`core/src/application/operations.rs`](../core/src/application/operations.rs)）。Skills 内部仍保留自己的 journal 和风险确认协议，因为它处理目录、链接、归档与 findings；Application 负责把它包装进同一个外层契约，而不是重复实现事务。

所有会读取或改变 MUX workspace 持久化状态的 Application 查询、计划和 mutation 都经过进程级 workspace gate（[`core/src/application/gate.rs`](../core/src/application/gate.rs)）；纯解析、静态 provider metadata 和独立 updater helper 不占用这个锁：

- snapshot、inventory、catalog 和 plan 使用 read lock；
- bootstrap、commit/cancel/recovery、Source 管理和自定义 Agent 管理使用 write lock；
- 各资源引擎原有的 settings lock、operation lock 和文件原子写继续负责更窄的持久化并发与崩溃恢复。

这个 gate 本身只协调同一 MUX 进程中的跨引擎观察窗口。workspace snapshot 还会同时获取已有 Skills 与 settings filesystem lock 的 shared 模式，因此中央 Skill tree/link/journal 和 settings-backed state 会与遵守同一协议的另一个 MUX 进程 Skills writer/asset transaction 串行化。普通 Skills transaction 与包含 Skill assignment 的 cross-asset transaction 锁序不同，snapshot 不假定唯一 writer 顺序：它只做 non-blocking try-lock；只要任一锁被占用，就先释放已取得的另一个 shared guard，再有界重试，从而不在两个 writer 顺序之间形成死锁。空 Home 则先无副作用地读取，读后若发现 writer 已创建 `skills.lock`、settings lockfile 或 settings 就重试。这个协议不会阻止外部 Agent 进程直接改自己的配置文件，也不等价于覆盖所有 Agent 文件的跨进程全局 snapshot transaction。

### Workspace：一次读取统一事实

`WorkspaceSnapshot` 一次返回：

- Agent capability graph；
- 中央 MCP、Model、Skill 资产；
- desired、observed、external relationships；
- 基于返回内容计算的 canonical revision。

每次 workspace projection 内，Skill inventory 扫描一次并复用于资产和关系投影（[`core/src/application/workspace.rs`](../core/src/application/workspace.rs)）；为检测读期间变化，单次 snapshot 调用可能执行多次 projection，因此不是“整个请求只扫描一次”。中央 Skills 状态和 settings-backed 中央资产/relationship 会通过有序 shared locks 与 cooperating MUX writer 协调；外部 Agent 文件不参与这些锁，仍依靠连续两次相等的 projection 做乐观稳定性检测。检测到持续变化时会以 `snapshot_unstable` 失败，但不能绝对排除外部文件在两次采样之间出现并恢复的短暂变化，因此整个 workspace 仍不是覆盖所有文件的跨进程原子快照。MCP registry 使用有序 map，保证同一事实状态生成稳定 revision。

revision 不是直接对 Rust 容器的迭代顺序做哈希。投影会先转换成 `serde_json::Value`，统一对象 key 的顺序，再序列化并计算 SHA-256；因此 MCP `env`、`headers` 等 `HashMap` 即使插入顺序不同，同一事实状态仍得到相同 revision（[`core/src/application/workspace.rs`](../core/src/application/workspace.rs)）。

快照不执行 recovery、migration 或 reconciliation；对应架构测试验证空隔离 Home 在 snapshot 后不会出现 settings 或 Skills 目录（[`core/tests/application_architecture.rs`](../core/tests/application_architecture.rs)）。shared read lock 只在相应 lockfile 或 settings 已存在时获取；空 Home 的首次 projection 完成后会重新检查 Skills 与 settings 初始化证据，既避免纯读取创建 `~/.mux`，也关闭 writer 在无锁读取期间初始化存储的竞态。锁序回归还固定了 `settings exclusive → Skills exclusive` writer 与 snapshot 并发时，snapshot 必须释放已取得的 shared guard，让 writer 完成后再重试，而不是互相等待到 timeout。

### Bootstrap：所有前端共用恢复顺序

`application::bootstrap` 固定执行：

1. storage migration；
2. Skill transaction recovery；
3. cross-asset transaction recovery；
4. Model schema migration；
5. active Model reconciliation。

CLI 对未解决事务 fail closed；Desktop 保持诊断界面可用，但禁止后台 Skill 更新，避免在损坏状态上继续 mutation（[`core/src/application/bootstrap.rs`](../core/src/application/bootstrap.rs)）。

### Resources：具体协议回到具体命名空间

原先位于 core 根部的 MCP adapter、codec、registry、scanner、source 和 apply 实现已移动到 `core/src/resources/mcp/`。Model 与 Skill 引擎也分别物理移动到 `core/src/resources/model/` 和 `core/src/resources/skill/`；根 `models`、`skills`、`adapter`、`registry`、`ops` 等路径仅由 `lib.rs` compatibility re-export 提供（[`core/src/lib.rs`](../core/src/lib.rs)）。

跨域 desired state、lifecycle、planner、transaction 和 migration 移入 `core/src/assets/`；纯 DTO 移入 `core/src/domain/assets.rs`。每个 `application::*` 模块只显式暴露前端需要的 use case 和 DTO，不再通过 glob 把底层 engine 重新泄漏出去。

`settings.json` 需要持久化、但不属于某个资源引擎行为的值对象也已下沉：MCP 的 `DisabledEntry` 位于 [`core/src/domain/mcp.rs`](../core/src/domain/mcp.rs)，Skill 的 `ManagedSkillRecord`、source/risk/update 等持久化 DTO 位于 [`core/src/domain/skill.rs`](../core/src/domain/skill.rs)。资源模块可以兼容 re-export 它们，但 [`core/src/settings.rs`](../core/src/settings.rs) 只依赖 domain DTO，不再依赖 MCP/Skill engine。

Core 内部直接使用 `domain`、`assets` 和 `resources::*` 的真实路径。`mux_core::models`、`mux_core::skills`、旧 MCP 根模块、`mux_core::types` 与 `mux_core::consumption` 只存在于 [`core/src/lib.rs`](../core/src/lib.rs) 的向后兼容 surface；架构测试禁止 `lib.rs` 之外的实现代码重新依赖这些别名。

### MCP 计划绑定中央 catalog

MCP 计划不仅保存 operation id、candidate hash 和目标文件 precondition，还会保存计划时完整中央 MCP catalog 的 canonical fingerprint（[`core/src/assets/planner.rs`](../core/src/assets/planner.rs)）。commit 会重新计算 fingerprint；如果 Source refresh、手动覆盖或其他进程在审阅后改变了 catalog，即使 Agent 目标文件没有变化也会以 `asset_operation_stale` 拒绝提交（[`core/src/assets/transaction.rs`](../core/src/assets/transaction.rs)）。

这避免了“用户审阅的是旧中央定义，提交时却把新定义传播给所有 consumer”的时间窗口。该约束只加在 `DomainPlan::Mcp`，Model/Skill 继续使用各自已有的 profile、tree、settings 和 target precondition。

### Source 管理：事务回滚与 fail closed

Source subscribe/add/refresh/enable/remove 仍是 MCP catalog administration，不伪装成中央资产 plan。它们通过 Application write gate 串行化，并在资源层执行受检查的 settings mutation、source cache 快照和 compare-before-write：

- add/register 先写私有 cache；settings 注册失败时按原快照回滚 cache；
- refresh 先解析不暴露的候选文件，替换 cache 后若 settings 检查失败则恢复旧 cache；
- remove 先移除 registration；cache 删除失败时尝试把 registration 放回原位置；
- 并发改变 source definition 或 cache 时拒绝覆盖。

Source 管理还会对比变更前后的 effective catalog。只要变化的 MCP key 已存在 desired Agent consumer，操作就 fail closed，并要求通过中央资产审阅流程更新，或先解除关系；因此 direct Source 管理只能改变未被消费的 catalog key（[`core/src/resources/mcp/sources.rs`](../core/src/resources/mcp/sources.rs)）。

这不是新的 Source operation journal：回滚覆盖已检测到的进程内错误，Application gate 也只约束同一进程。MCP 传播的崩溃恢复仍由 asset transaction journal 负责。

## 前端迁移

### CLI

CLI 的 Rust import 只进入 `application` 或 `domain`。产品描述与顶层命令已覆盖 MCP、Model、Skill，新增：

- `mux workspace [--json]`
- `mux models`
- `mux skills`
- unified Agent capabilities

启动统一调用 `MuxCore::bootstrap`（[`cli/src/main.rs`](../cli/src/main.rs)）。脚本式 `import/add/remove/apply/clean` 已改为 reviewed central plan/commit；存在 drift、冲突或需确认的计划会取消并提示到 Desktop 审查。

无子命令 TUI 仍保留 MCP catalog 编辑界面，这是兼容 surface，不代表产品 Core 仍以 MCP 为边界。该界面的 install、enable、delete、import、forget、resync 等写操作也已迁移到统一 asset plan/commit；resync 使用专门的 reapply plan，在不修改中央资产的前提下修复 consumer drift。后续 TUI 若新增 Model/Skill lifecycle，应继续只调用统一 Application API。

### Tauri / React

Tauri 注册了 `get_workspace_snapshot`、`list_agent_capabilities`、`plan_operation`、`commit_operation`、`cancel_operation`，并从共享 bootstrap 启动（[`desktop/src-tauri/src/lib.rs`](../desktop/src-tauri/src/lib.rs)）。

15 个已编译但未注册的 legacy mutation commands 已删除；其中直接覆盖 drifted config 的旧测试也已移除。TypeScript 已有与新 façade 对应的 typed DTO 和 API wrapper。

消费关系主 hook 已实际切换到统一 façade：[`desktop/src/hooks/useConsumptionState.ts`](../desktop/src/hooks/useConsumptionState.ts) 从 `getWorkspaceSnapshot().relationships` 取只读事实，并只通过 `planOperation`、`commitOperation`、`cancelOperation` 执行资产操作。Model-only Agent 不再被 Agent 页面降级成“仅供参考”，而是进入完整资源工作区；Skill-only 自定义 Agent 也能从 capability graph 得到独立的 Skill capability。

本轮没有把所有 React 状态强行合成一个 snapshot：例如 Skills 安装详情仍使用专用 Skills inventory/plan UI，Source 管理仍使用专用 Application command。它们必须委托 Core 判断状态，不能自行重新解释安全规则。

### MCP disabled reapply

`plan_reapply_mcp` 会保持 desired relationship 的 enabled 状态。对已禁用的 MCP，commit 会先用中央定义刷新保存的 disabled snapshot，再恢复禁用状态；它不会因为 reapply 暂时安装中央定义就把该 relationship 意外启用（[`core/src/assets/transaction.rs`](../core/src/assets/transaction.rs)）。

对应回归测试同时固定三项行为：普通 drift 可修复且中央资产不变；禁用 relationship reapply 后仍禁用；审阅后 catalog fingerprint 变化时目标文件保持不动（[`core/tests/central_assets_e2e.rs`](../core/tests/central_assets_e2e.rs)）。

### 自定义部分能力 Agent

自定义 Agent 不再必须声明 MCP `global/key/format`。`agents::put` 接受只有已核验 Skills capability 的定义，capability projection 会返回 `skill: Some`、`mcp: None`、`model: None`；Model-only 内建 Agent 也会独立加入同一投影（[`core/src/application/agents.rs`](../core/src/application/agents.rs)）。

Agent catalog administration 与 Source administration 一样采用受限直写边界：展示 metadata 和未消费的 target 可以直接管理；一旦 MCP writer、Model path 或 Skill target 已有关联的 desired resources，旧公开 writer 会以 `agent_definition_in_use` / `agent_configuration_in_use` fail closed，路径变更必须走统一 capability plan/commit。direct Agent definition/configuration writer 会在同一个 settings 文件锁内重新读取 expected state、检查 desired relationship 并合并写入；目标 Agent 已变化时拒绝 stale write，其他 Agent 的并发变更不会被旧 map 覆盖。仅切换 Agent catalog 的 `enabled` 标记不改变 writer contract，因此仍可直接保存（[`core/src/agents.rs`](../core/src/agents.rs)）。

当前自定义 Agent 持久化 schema 只新增了 Skill-only 路径；它还没有提供自定义 Model writer 的完整 schema。`AgentConfigurationPatch` 支持独立 Model patch，不等于用户可以任意定义新的 Model codec。

## 兼容与非目标

- settings、registry、operation plan、Agent config 的磁盘和 JSON wire shape 保持兼容；Box 只减少 Rust enum stack size，不改变 serde 结果。
- `mux_core::consumption::*`、根 MCP 模块、`models`、`skills` 与 `types` 路径通过 `lib.rs` compatibility façade 保留给旧 Rust integration；Core 实现和 Rust frontends 不使用它们。
- Skills 的内部 operation kind、journal 与 findings confirmation 不与 MCP/Model 强行合并。
- Model 与 Skill 引擎已进入各自的 resource namespace，但仍与 MCP 共处 `mux-core` crate，以保留跨资源原子事务；本轮不为了目录外观拆成独立 crates。
- CLI/TUI 现有 MCP 命令继续存在，产品“不局限于 MCP”不等于删除 MCP 专用工作流。
- Source 管理与自定义 Agent definition 管理经过 Application gate，但没有伪装成 asset plan；Source 只能改变无 desired consumer 的 key，Agent direct writer 只能改变未消费的 writer contract 或非 writer metadata。
- 本轮没有为自定义 Agent 引入任意 Model codec，也没有把所有 React domain hook 合并为一个 hook。

## 架构门禁

新增 `application_architecture` integration suite，验证：

- façade 可以完成中央资产 plan/commit 并在统一 snapshot 中观察结果；
- Model-only configuration patch 不要求 MCP 或 Skill 字段；
- snapshot 只读；
- snapshot revision 对等价 registry 状态稳定；
- snapshot revision 对 `env` / `headers` 等无序 map 的等价状态稳定；
- asset cancel 可安全重复调用；
- MCP reapply 能修复 drift 且不改中央资产；
- MCP reapply 保持 disabled relationship，且 commit 拒绝审阅后 catalog 变化；
- domain 不引用基础设施；
- settings 只依赖下沉后的 domain persistence DTO；
- CLI/Tauri Rust source 不绕过 Application boundary。
- Application 不以 glob 重新导出底层 engine writer。
- Core 实现不依赖 `lib.rs` 的 legacy aliases。

CI 现在执行 Rust format、strict Clippy、core/CLI tests、独立 macOS Tauri tests、Desktop tests/build、website build 和 workflow/release contracts，并要求所有 producer 成功（[`.github/workflows/quality-monitor.yml`](../.github/workflows/quality-monitor.yml)）。

## 验收标准

- 只支持 Model 或 Skill 的 Agent 能出现在统一 capability graph，并可独立配置。
- 中央资产、消费关系和已消费的 capability path 变更只能通过 Application 的 plan/commit/cancel；Source 与自定义 Agent administration 只能通过受 gate 保护、对 desired resources fail closed 的显式 Application API。
- workspace 查询没有隐式 migration、reconciliation 或目录创建。
- workspace revision 对等价无序 map 稳定，并在外部文件持续变化时 fail closed。
- CLI 与 Desktop 使用完全相同的恢复顺序。
- MCP commit 绑定审阅时 catalog；Source direct 管理不能改变已被 desired consumer 引用的 key。
- 旧存储和 wire payload 可继续读取；未知字段、注释、凭据与 drift 安全契约保持原行为。
- 架构测试与 CI 阻止 frontend 重新直连底层 writer，也阻止 Core 内部重新依赖 legacy compatibility aliases。
