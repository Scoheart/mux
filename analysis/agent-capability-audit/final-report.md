# MUX Agent MCP / Models / Skills 全量审计报告

> 审计日期：2026-07-22。本文是总入口；逐 Agent 的字段、路径、证据链接与判定保留在分片报告中，避免把细节压缩成无法复核的汇总。

## 结论先行

- 本轮审计集合是 MUX audited definitions、Glama discovery catalog 与 ACP Registry manifest 的规范化并集，共 **211 个 Agent 身份**。
- 每个身份都按 MCP、Models、Skills 三类能力分别检索；“支持 MCP”不会被用来推断“支持 Models/Skills”。
- 社区目录和 ACP manifest 只用于发现身份与官方仓库，不会单独开启写入。
- 代码只实现有稳定官方路径/schema、可保真修改、可安全引用凭据、可验证回滚的能力；其余保留为只读发现或 research candidate。
- 对应用数据库、云端账户状态、强制明文密钥、仅项目级路径、Windows-only 路径和多 profile 歧义采用 fail-closed，不以“支持数量”换取错误写入。

身份覆盖机器门禁为：

```bash
node scripts/validate-agent-capability-audit.mjs
```

该脚本要求 211 个规范化身份各出现且只出现一次，并校验三份报告的分片归属。它不证明实现数量；MCP、Model、Skills 的实现统计由最终工作树重新生成的 capability baseline 单独证明。

## 详细证据入口

| 报告 | 范围 | 内容 |
|---|---|---|
| [`verified-agents-evidence.md`](./verified-agents-evidence.md) | 原 45 个 audited Agent | 对现有 writer/read-only 定义重新核对官方路径、schema、凭据与 Skills |
| [`catalog-a-m-evidence.md`](./catalog-a-m-evidence.md) | 审计开始时 A–M catalog-only / ACP 新身份 | 逐 Agent 官方 repo/docs 搜索、安装 probe、三类资产配置和实现判定；本版提升项仍留在原分片 |
| [`catalog-n-z-evidence.md`](./catalog-n-z-evidence.md) | 审计开始时 N–Z catalog-only / ACP 新身份 | 同上，含路径优先级、桌面内部存储与平台限制；本版提升项仍留在原分片 |
| [`implementation-gap.md`](./implementation-gap.md) | 跨 Agent 实现边界 | writer 门禁、已落地改造、明确延后项和架构缺口 |
| [`coverage-baseline.md`](./coverage-baseline.md) | 机器基线 | 每个 identity 的 audited/catalog/ACP 与能力集成状态 |

机器证据包括固定 commit repository tree、按 blob SHA 提取的高信号源码片段、ACP manifest 与 catalog source discovery。它们用于定位和复核；最终语义结论仍由分片报告中的官方证据逐项确认。

## 调研方法

1. 合并 `data/agents.json`、`data/agent-catalog.json` 和 ACP Registry identity，并处理已知 alias（例如 `qoder` / `qoder-cli` 的产品边界）。
2. 从官方产品文档、官方 GitHub 仓库、官方发布包和包内运行结果检索：
   - 用户级/项目级路径、环境变量覆盖和 precedence；
   - MCP transport、顶层 key、map/list layout、enablement 与未知字段；
   - Model provider inventory、base URL、protocol、credential reference、default/current semantics；
   - Skills 根目录、别名、加载深度、冲突优先级和共享读取者。
3. 搜索不到稳定契约时记录 `not-found`；来源冲突、运行参数决定路径或 writer 会损失 sibling 时记录 `conflict`。
4. 只有达到 writer 门禁的候选才进入 `data/agents.json` 或 Model adapter；每个新增 codec 必须有正向、未知字段保留、非法输入与 transport 拒绝测试。
5. 扫描研究产物中的 credential-shaped 内容，报告不落盘真实 secret，不回显解析错误中的源码行。

## 关键发现

### “Agent 产品”与目录条目不是一回事

目录中同时存在可安装 Agent client、IDE framework、MCP server/SDK、演示项目、托管服务、旧名和重复别名。发现目录适合扩大检索面，但不适合作为 writer registry。MUX 后续需要显式的 product kind；本轮先在报告中保留 `audited-writable`、`audited-read-only`、`catalog-only`、`misclassified`、`duplicate` 和 `rename-or-stale-identity`。

### MCP 配置不是统一的 `mcpServers`

官方实现横跨 JSON/JSONC/TOML/YAML，既有 map 也有 array of tables；URL 字段可能是 `url`、`serverUrl`、`httpUrl` 或嵌套 transport，对 SSE/Streamable HTTP 的表达也不同。复用 codec 只有在字段、transport 与保留策略完全匹配时才安全。

### Model 的三种状态必须分开

调研反复出现：

1. provider connection 已存在；
2. 新会话默认模型；
3. 当前 chat/session 的模型。

有的 Agent 只公开连接配置，当前模型在 SQLite 或应用状态中；有的 Agent 有 global pointer；有的只允许 UI 切换。MUX 只在第 2 项存在稳定文件契约时提供“设为当前/默认”写入，不能把第 1 项冒充第 2/3 项。

### Skills 的共享读取不是多次写入

大量 Agent 同时扫描 `~/.agents/skills`、`~/.claude/skills` 或 `~/.config/agents/skills`。MUX 需要分别展示：实际写入一个目录、哪些 Agent 显式绑定、哪些 Agent 因兼容扫描而可读取。共享 reader 不能写成“另影响 N 个 Agent”。

### 凭据能力决定 Model writer 上限

安全组合是 Keychain command、环境变量引用或 external-managed credential。若 Agent 只接受明文 `api_key`，MUX 会发现并提示，但不会默认导出中央密钥。若 auth command 仅被 parser 接受、运行时并未执行，也不会被当作安全策略。

## 本版实现

### Agent registry 与 MCP codec

- 新增 `agentkube` MCP writer：只写官方用户级 `mcp.json`，SSE/stdio schema 专用 codec；现存 `enabled=false` 时扫描跳过、更新拒绝。
- 新增 `chatmcp` MCP writer：含 OAuth/token/client secret 的条目不进入 MUX inventory；只要文件中存在这类条目，更新、停用、快照与备份整个文件都在写前 fail closed，凭据继续完全由 ChatMCP 管理。
- 两个 writer 均有 round-trip、未知字段保留、非法 transport 与 fail-closed 测试。

### Skills-only Agent

- A–M 新增 `docker-agent`、`cortex-code`、`dirac`、`minion-code` Skills target。
- N–Z 新增 `poolside`、`raycast`、`theiaai-theiaide`、`trae-ide`、`zencoder` Skills target。
- Skills-only Agent 可以出现在 Agent picker 并直接进入 Skills 页，但不会伪装出 MCP/Models writer。
- Skills 证据允许官方文档或官方源码两种一等证据，仍要求 docs、verified date 和 install probe。

### CodeWhale 与 VT Code Models 的处置

两者的 Model 文件契约、凭据边界和所有权规则已完成官方证据核验，但本版没有进入 `default_config_paths`、Model apply/clear/observe 或 migration dispatch。原因是当前架构不能可靠解析动态配置根、project/workspace 高优先级层、active pointer 漂移和同一 provider 内的外部凭据字段；它们仍是后续 managed writer 候选，不计入本版 Model target。

### Agent 配置 UX

- Agent 配置位置现在可以同时审阅 MCP path/key、Model paths 与 Skills directory。
- 共享 Skills 目录的确认文案区分“实际写入位置”和“其他 Agent 也会读取”，不再笼统写“影响其他 Agent”。
- 添加 Skill 的审阅页使用“同一目录也被 N 个 Agent 读取”；修改 Agent 配置路径时使用中性的“Skills 目录变更涉及 N 个其他 Agent”，两者都不会把共享读取误写成第二次安装。
- Desktop：`npm test -- --run` 通过 43 个文件、228 项测试，`npm run build` 通过。

## 明确没有实现的范围

- 不直接修改 SQLite、Electron/Tauri 内部数据库、加密导出或云端账户状态。
- 不为没有稳定全局路径的 Agent 猜测 `~/.config/<name>/...`。
- 不把 project-only 文件伪装成用户级全局 writer。
- 不给 macOS 发布物宣称 Windows-only Agent 已完成可写验证。
- 不写官方 schema 强制要求的明文 API Key。
- 不把连接 inventory 冒充全局 current model。
- 不通过删除未知字段、OAuth 内容、同级 provider/model 或注释来“统一格式”。

## 交付结果

- 身份审计：211/211，分片为原 audited 45、A–M 114、N–Z 52；无遗漏、无重复。
- 当前 registry：56 个 audited definitions；审计集合中其余 155 个身份未进入 audited registry，保持 discovery-only/read-only、重分类或去重状态。
- 当前能力：46 个用户级 MCP writer、45 个 Skills target、14 个 Model target（12 managed、2 guided；不能把 guided 计作 writer）。
- 本轮实际新增：2 个 MCP writer、9 个 Skills-only Agent；CodeWhale/VT Code Models 明确回撤。
- 图标：只有可写 MCP Agent 需要品牌图标；Skills-only Agent 使用现有中性 fallback，不冒用品牌素材。
- ACP Registry 来源固定到 commit `cc4eb37906eb477a0b0a5e46a7312cbf25366aef`；39/39 manifest URL 均固定到该 commit。
- 本报告记录源码可复现的研究与实现证据，不预写尚未产生的 commit/tag。完整测试、commit、Stable workflow、tag 与 Release URL 以最终交付消息中的发布验证结果为准。

## 发布前验证

- 审计覆盖：`validate-agent-capability-audit.mjs` 通过，211/211 identities；分片 45/114/52，missing 0、overlaps 0。
- 仓库源码证据：116/116 repositories；762 个候选文件、423 个匹配文件、fetch failure 0。
- 研究产物：5 个 JSON 全部可解析；6 个生成/校验脚本通过 `node --check`；证据脱敏 6/6 回归通过，credential-shaped 扫描 0 hits。
- 基线可复现：连续生成两次的 SHA-256 一致；JSON `369f52a9d6e8872015573699f858c76eb0721e8c4cfc1f159761aba8829002e3`，Markdown `cf04c1349c7936bf1355fe08497c98bbd0561a9f897d38d4ff17b2cf4e0b7e85`。
- Core：`cargo test --workspace --locked` 全部通过，覆盖 CLI、Core unit、Agent formats、中央资产、停用恢复与 Skills 全事务套件。
- Desktop：43 个测试文件、228 项测试通过；46 个可配置 Agent 图标通过检查；TypeScript 与 Vite production build 通过。
- Tauri：`cargo test --locked` 共 20 项测试通过；Website production build 通过。
- Release contracts：版本一致性检查通过，workflow/release helper 22/22 测试通过；`cargo fmt --check` 与 `git diff --check` 通过。
