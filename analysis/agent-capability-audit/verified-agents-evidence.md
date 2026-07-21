# MUX 已审计 Agent：MCP / Models / Skills 全量证据账本

> 审计日期：2026-07-22（Asia/Shanghai）
> 范围：`data/agents.json` 中 45 个深度审计 Agent。
> 状态：45/45 全量核验完成；新增 writer 仍须按本文门槛与 fixture 逐项落测试后才能进入 managed。

## 判定规则

- **已证实**：官方产品文档、官方 GitHub 仓库源码或官方发布包明确给出路径与 schema；若三者冲突，稳定发布包/对应 tag 源码优先。
- **未找到**：已检查官方入口，但没有找到稳定的 user-level、可无交互写入且不会落盘明文密钥的契约。这不等于产品永远不支持该能力。
- **冲突**：官方材料、稳定发布源码或 MUX 当前注册值互相矛盾；解决前不得扩大 writer。
- 社区资料只作线索；唯一已明确采用的社区能力是 Pi 的 `pi-mcp-adapter`，必须继续标注为非 Pi core 原生。
- 路径分三类单独核验：用户级、项目级、动态覆盖（环境变量/CLI）。不能从 MCP 路径推断 Models 或 Skills 路径。
- 凭据判定只接受 Keychain command、环境变量引用或产品自身安全凭据存储；需要把 API key 明文写进配置的能力不得成为 managed writer。

## 审计起始快照（实现前）

- 审计开始时有 45 个已审计 Agent；44 个有用户级 MCP writer，Devin 只读。
- 当时有 36 个已声明 user-level Skills capability。
- 14 个当前已注册 Model 目标：12 managed（Claude Code、Codex、Grok Build、Pi、OpenCode、Kilo Code、Qwen Code、Crush、Mistral Vibe、Hermes、Factory Droid、Goose）+ 2 guided（MiniMax Code、Qoder）。
- 新增三项源码复核已完成：VT Code 满足“持久化当前指针 + 实际执行的 Keychain command + 可逆局部写入”，可作为 managed 研究候选；CodeWhale 仅对“OpenAI Chat Completions + 已提供外部环境变量名”的 profile 满足条件，可作为 env-reference managed candidate；Stakpak 只在 canonical provider 环境变量或无认证 local endpoint 下安全，属于 guided/partial。审计还发现 Stakpak Skills 路径、Stakpak MCP precedence、VT Code MCP 总开关、CodeWhale Skills aliases 四处起始缺陷。最终交付只落地安全子集；CodeWhale / VT Code Models 均未纳入本版，实际边界以 `final-report.md` 与 capability baseline 为准。

## 逐 Agent 证据

下列清单覆盖安装探针、MCP 用户/项目路径与 schema、Models 路径与凭据、Skills 首选/兼容目录，以及明确结论。

### MCP 与 Skills 注册值逐项复核

2026-07-22 对 45 个 MCP 官方入口与 36 个 Skills 官方入口执行了实际 HTTP GET（跟随跳转，20 秒超时），81 个入口全部返回 HTTP 200。HTTP 可达只证明证据仍在线；字段语义另按下文源码/发布包核验。

| Agent id | 产品 | MCP 用户级路径 | 项目路径 | 格式 / key / layout / codec | transport | MCP 官方证据 | Skills 用户级首选目录 / 官方证据 | 证据类型与判定 | 核验日期 |
|---|---|---|---|---|---|---|---|---|---|
| `amp` | Amp | `~/.config/amp/settings.json` | — | `json` / `amp.mcpServers` / map / `url_inferred` | stdio/http | [官方文档](https://ampcode.com/manual#model-context-protocol-mcp) | `~/.config/agents/skills`; [官方文档](https://ampcode.com/manual#agent-skills) | 官方文档；**已证实** | 2026-07-22 |
| `amazon-q` | Amazon Q Developer IDE | `~/.aws/amazonq/default.json` | `.amazonq/default.json` | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [AWS 文档](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/mcp-ide.html) | **未找到**稳定 user-level Skills 契约 | 官方文档；MCP **已证实**、Skills **未找到** | 2026-07-22 |
| `antigravity` | Google Antigravity | `~/.gemini/config/mcp_config.json` | — | `json` / `mcpServers` / map / `server_url` | stdio/http | [官方文档](https://antigravity.google/docs/mcp) | `~/.gemini/config/skills`; [官方文档](https://antigravity.google/docs/skills) | 官方文档；**已证实** | 2026-07-22 |
| `augment` | Augment Code / Auggie | `~/.augment/settings.json` | — | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方文档](https://docs.augmentcode.com/cli/integrations) | `~/.augment/skills`; [官方文档](https://docs.augmentcode.com/cli/skills) | 官方文档；**已证实**；`augment` 与 `auggie` 是同一产品探针 | 2026-07-22 |
| `boltai` | BoltAI | `~/.boltai/mcp.json` | — | `json` / `mcpServers` / map / `stdio_only` | stdio | [官方文档](https://docs.boltai.com/docs/plugins/mcp-servers) | **未找到** | 官方文档；MCP **已证实**，远程需 `mcp-remote` | 2026-07-22 |
| `claude-code` | Claude Code | `~/.claude.json` | `.mcp.json` | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方文档](https://docs.anthropic.com/en/docs/claude-code/mcp) | `~/.claude/skills`; [官方文档](https://code.claude.com/docs/en/skills) | 官方文档；**已证实** | 2026-07-22 |
| `claude-desktop` | Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` | — | `json` / `mcpServers` / map / `claude_desktop` | stdio | [MCP 官方文档](https://modelcontextprotocol.io/quickstart/user) | **未找到** | 官方文档；文件 MCP 仅 stdio **已证实** | 2026-07-22 |
| `cline` | Cline | `~/.cline/data/settings/cline_mcp_settings.json` | — | `json` / `mcpServers` / map / `cline` | stdio/http | [官方文档](https://docs.cline.bot/mcp/configuring-mcp-servers) | `~/.cline/skills`; [官方文档](https://docs.cline.bot/customization/skills) | 官方文档；**已证实** | 2026-07-22 |
| `codebuddy-code` | CodeBuddy Code | `~/.codebuddy/.mcp.json` | — | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方文档](https://www.codebuddy.ai/docs/cli/mcp) | `~/.codebuddy/skills`; [官方文档](https://www.codebuddy.ai/docs/cli/skills) | 官方文档；**已证实** | 2026-07-22 |
| `codewhale` | CodeWhale | `~/.codewhale/mcp.json` | workspace overlay（不由 MUX 写） | `json` / `servers` / map / `standard` | stdio/http | [官方源码文档](https://github.com/Hmbown/CodeWhale/blob/9e1e967ba46a35f6eae337f09b399d938a080b81/docs/MCP.md) | `~/.codewhale/skills`; [官方源码文档](https://github.com/Hmbown/CodeWhale/blob/9e1e967ba46a35f6eae337f09b399d938a080b81/docs/CONFIGURATION.md) | 官方源码；MCP/Skills **已证实** | 2026-07-22 |
| `codex` | Codex | `~/.codex/config.toml` | `.codex/config.toml` | `toml` / `mcp_servers` / map / `codex` | stdio/http | [官方文档](https://developers.openai.com/codex/mcp) | `~/.agents/skills`; [官方文档](https://developers.openai.com/codex/skills) | 官方文档；**已证实** | 2026-07-22 |
| `continue` | Continue | `~/.continue/config.yaml` | — | `yaml` / `mcpServers` / list / `continue` | stdio/http | [官方文档](https://docs.continue.dev/customize/deep-dives/mcp) | **未找到**稳定 user-level Agent Skills | 官方文档；MCP **已证实** | 2026-07-22 |
| `copilot-cli` | GitHub Copilot CLI | `~/.copilot/mcp-config.json` | `.github/mcp.json` | `json` / `mcpServers` / map / `copilot` | stdio/http | [GitHub 文档](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/extend-coding-agent-with-mcp) | `~/.copilot/skills`; [GitHub 文档](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#skills-reference) | 官方文档；**已证实** | 2026-07-22 |
| `crush` | Crush | `~/.config/crush/crush.json` | — | `json` / `mcp` / map / `explicit_type` | stdio/http | [官方仓库](https://github.com/charmbracelet/crush/blob/9c4e2f673aeacd92040cad4981d832335ea0ad23/README.md#model-context-protocol-mcp) | `~/.config/crush/skills`; [官方仓库](https://github.com/charmbracelet/crush/blob/9c4e2f673aeacd92040cad4981d832335ea0ad23/README.md#skills) | 官方源码/README；**已证实** | 2026-07-22 |
| `cursor` | Cursor | `~/.cursor/mcp.json` | `.cursor/mcp.json` | `json` / `mcpServers` / map / `url_inferred` | stdio/http | [官方文档](https://docs.cursor.com/context/model-context-protocol) | `~/.cursor/skills`; [官方文档](https://cursor.com/docs/skills) | 官方文档；**已证实** | 2026-07-22 |
| `devin` | Devin | — | — | 无稳定文件契约 | — | [官方文档](https://docs.devin.ai/work-with-devin/mcp) | **未找到** | 官方文档；产品支持 MCP，但 user-level writer **未找到** | 2026-07-22 |
| `factory-droid` | Factory Droid | `~/.factory/mcp.json` | — | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方文档](https://docs.factory.ai/cli/configuration/mcp) | `~/.factory/skills`; [官方文档](https://docs.factory.ai/cli/configuration/skills) | 官方文档；**已证实** | 2026-07-22 |
| `firebender` | Firebender | `~/.firebender/firebender.json` | — | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方文档](https://docs.firebender.com/context/mcp/overview) | `~/.firebender/skills`; [官方文档](https://docs.firebender.com/multi-agent/skills) | 官方文档；**已证实** | 2026-07-22 |
| `gemini` | Gemini CLI | `~/.gemini/settings.json` | `.gemini/settings.json` | `json` / `mcpServers` / map / `gemini` | stdio/http | [官方文档](https://geminicli.com/docs/tools/mcp-server/) | `~/.gemini/skills`; [官方文档](https://geminicli.com/docs/cli/using-agent-skills/) | 官方文档；**已证实** | 2026-07-22 |
| `goose` | Goose | `~/Library/Application Support/Block/goose/config/config.yaml` | — | `yaml` / `extensions` / map / `goose` | stdio/http | [官方文档](https://goose-docs.ai/docs/guides/config-files/) | `~/.agents/skills`; [官方文档](https://goose-docs.ai/docs/guides/context-engineering/using-skills/) | 官方源码/文档；**已证实**（macOS runtime 路径） | 2026-07-22 |
| `grok-build` | Grok Build | `~/.grok/config.toml` | `.grok/config.toml` | `toml` / `mcp_servers` / map / `standard` | stdio/http | [官方文档](https://docs.x.ai/build/features/mcp-servers) | `~/.grok/skills`; [官方文档](https://docs.x.ai/build/features/skills-plugins-marketplaces) | 官方文档；**已证实** | 2026-07-22 |
| `hermes` | Hermes Agent | `~/.hermes/config.yaml` | — | `yaml` / `mcp_servers` / map / `url_transport` | stdio/http | [官方源码文档](https://github.com/NousResearch/hermes-agent/blob/a88512b114059fff642d60d54cbf30d5793c6c37/website/docs/user-guide/features/mcp.md) | `~/.hermes/skills`; [官方源码文档](https://github.com/NousResearch/hermes-agent/blob/a88512b114059fff642d60d54cbf30d5793c6c37/website/docs/user-guide/features/skills.md) | 官方源码；**已证实** | 2026-07-22 |
| `junie` | JetBrains Junie | `~/.junie/mcp/mcp.json` | `.junie/mcp/mcp.json` | `json` / `mcpServers` / map / `url_inferred` | stdio/http | [JetBrains 文档](https://www.jetbrains.com/help/junie/model-context-protocol-mcp.html) | **未找到** | 官方文档；MCP **已证实** | 2026-07-22 |
| `kilo-code` | Kilo Code CLI | `~/.config/kilo/kilo.jsonc` | — | `jsonc` / `mcp` / map / `opencode` | stdio/http | [官方文档](https://kilo.ai/docs/automate/mcp/using-in-kilo-code) | `~/.kilo/skills`; [官方文档](https://kilo.ai/docs/customize/skills) | 官方文档；**已证实** | 2026-07-22 |
| `kimi-code` | Kimi Code CLI | `~/.kimi-code/mcp.json` | — | `json` / `mcpServers` / map / `kimi` | stdio/http | [官方文档](https://moonshotai.github.io/kimi-code/en/customization/mcp) | `~/.kimi-code/skills`; [官方文档](https://moonshotai.github.io/kimi-code/en/customization/skills) | 官方文档；**已证实** | 2026-07-22 |
| `kiro` | Kiro | `~/.kiro/settings/mcp.json` | `.kiro/settings/mcp.json` | `json` / `mcpServers` / map / `url_inferred` | stdio/http | [官方文档](https://kiro.dev/docs/mcp/configuration/) | `~/.kiro/skills`; [官方文档](https://kiro.dev/docs/skills/) | 官方文档；**已证实** | 2026-07-22 |
| `lmstudio` | LM Studio | `~/.lmstudio/mcp.json` | — | `json` / `mcpServers` / map / `url_inferred` | stdio/http | [官方文档](https://lmstudio.ai/docs/app/plugins/mcp) | **未找到** | 官方文档；MCP **已证实** | 2026-07-22 |
| `minimax-code` | MiniMax Code | `~/.mavis/mcp.json` | — | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方签名包入口](https://agent.minimax.io/download) | **未找到** | 官方发布包；MCP **已证实** | 2026-07-22 |
| `mistral-vibe` | Mistral Vibe | `~/.vibe/config.toml` | — | `toml` / `mcp_servers` / list / `vibe` | stdio/http | [官方文档](https://docs.mistral.ai/vibe/code/cli/mcp-servers) | `~/.vibe/skills`; [官方文档](https://docs.mistral.ai/vibe/code/cli/commands-shortcuts) | 官方文档；**已证实** | 2026-07-22 |
| `opencode` | OpenCode | `~/.config/opencode/opencode.json` | `opencode.json` | `jsonc` / `mcp` / map / `opencode` | stdio/http | [官方文档](https://opencode.ai/docs/mcp-servers/) | `~/.config/opencode/skills`; [官方文档](https://opencode.ai/docs/skills/) | 官方文档/源码；**已证实** | 2026-07-22 |
| `openhands` | OpenHands CLI | `~/.openhands/mcp.json` | — | `json` / `mcpServers` / map / `explicit_type` | stdio/http | [官方文档](https://docs.openhands.dev/openhands/usage/cli/mcp-servers) | `~/.openhands/skills`; [官方文档](https://docs.openhands.dev/overview/skills/adding) | 官方文档；**已证实** | 2026-07-22 |
| `pi` | Pi Coding Agent + MCP Adapter | `~/.pi/agent/mcp.json` | `.pi/mcp.json` | `json` / `mcpServers` / map / `url_inferred` | stdio/http | [社区 adapter](https://github.com/nicobailon/pi-mcp-adapter) | `~/.pi/agent/skills`; [Pi 官方源码文档](https://github.com/earendil-works/pi/blob/dd6bea41efa8caa7a10fe5a6401676dc5699f83f/packages/coding-agent/docs/skills.md) | MCP 社区扩展；**已证实（限定）**；Skills 官方 **已证实** | 2026-07-22 |
| `qoder` | Qoder Desktop | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` | — | `json` / `mcpServers` / map / `qoder` | stdio/http | [官方文档](https://docs.qoder.com/user-guide/chat/model-context-protocol) | `~/.qoder/skills`; [官方文档](https://docs.qoder.com/zh/extensions/skills) | 官方文档/客户端；**已证实** | 2026-07-22 |
| `qoder-cli` | Qoder CLI | `~/.qoder/settings.json` | — | `json` / `mcpServers` / map / `qoder` | stdio/http | [官方文档](https://docs.qoder.com/en/cli/mcp-servers) | `~/.qoder/skills`; [官方文档](https://docs.qoder.com/zh/extensions/skills) | 官方文档；**已证实** | 2026-07-22 |
| `qoderwork` | QoderWork | `~/.qoderwork/mcp.json` | — | `json` / `mcpServers` / map / `qoderwork` | stdio/http | [官方文档](https://docs.qoder.com/qoderwork/connectors) | `~/.qoderwork/skills`; [官方文档](https://docs.qoder.com/qoderwork/skills) | 官方文档/客户端；**已证实** | 2026-07-22 |
| `qwen-code` | Qwen Code | `~/.qwen/settings.json` | — | `jsonc` / `mcpServers` / map / `gemini` | stdio/http | [官方文档](https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/) | `~/.qwen/skills`; [官方文档](https://qwenlm.github.io/qwen-code-docs/en/users/features/skills/) | 官方文档/稳定包；**已证实** | 2026-07-22 |
| `roo-code` | Roo Code | `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | `.roo/mcp.json` | `json` / `mcpServers` / map / `roo` | stdio/http | [官方文档](https://docs.roocode.com/features/mcp/using-mcp-in-roo) | `~/.roo/skills`; [官方文档](https://docs.roocode.com/features/skills) | 官方文档/源码；**已证实** | 2026-07-22 |
| `rovo-dev` | Atlassian Rovo Dev CLI | `~/.rovodev/mcp.json` | — | `json` / `mcpServers` / map / `transport` | stdio/http | [官方文档](https://support.atlassian.com/rovo/docs/connect-to-an-mcp-server-in-rovo-dev-cli/) | `~/.rovodev/skills`; [官方文档](https://support.atlassian.com/rovo/docs/extend-rovo-dev-cli-with-agent-skills/) | 官方文档；**已证实** | 2026-07-22 |
| `stakpak` | Stakpak | `~/.stakpak/mcp.toml`（仅最低优先级 fallback） | `./mcp.toml` / `./mcp.json`；其次 `./.stakpak/mcp.toml` / `.json` | `toml/json` / `mcpServers` / map / `standard` | stdio/http | [官方运行时源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/libs/mcp/config/src/lib.rs) | `~/.stakpak/skills`；[官方运行时源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/libs/api/src/local/skills/mod.rs) | 官方源码；MCP **已证实**；审计起始注册的 `~/.config/stakpak/skills` 与 runtime **冲突**，当前工作树已改为 runtime 路径 | 2026-07-22 |
| `tabnine` | Tabnine | `~/.tabnine/mcp_servers.json` | — | `json` / `mcpServers` / map / `tabnine` | stdio/http | [官方文档](https://docs.tabnine.com/main/getting-started/tabnine-agent/mcp-intro-and-setup) | **未找到** | 官方文档；MCP **已证实** | 2026-07-22 |
| `vt-code` | VT Code | `~/.vtcode/vtcode.toml` | workspace overlay（不由 MUX 写） | `toml` / `mcp.providers` / list / `vtcode` | stdio/http | [官方源码文档](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/docs/guides/mcp-integration.md) | `~/.agents/skills`; [官方源码文档](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/docs/skills/SKILL_AUTHORING_GUIDE.md) | 官方源码；MCP/Skills **已证实** | 2026-07-22 |
| `vscode` | Visual Studio Code | `~/Library/Application Support/Code/User/mcp.json` | `.vscode/mcp.json` | `json` / `servers` / map / `vscode` | stdio/http | [官方文档](https://code.visualstudio.com/docs/copilot/chat/mcp-servers) | `~/.copilot/skills`; [官方文档](https://code.visualstudio.com/docs/agent-customization/agent-skills) | 官方文档/源码；**已证实** | 2026-07-22 |
| `warp` | Warp | `~/.warp/.mcp.json` | `.warp/.mcp.json` | `json` / `mcpServers` / map / `warp` | stdio/http | [官方文档](https://docs.warp.dev/knowledge-and-collaboration/mcp) | `~/.agents/skills`; [官方文档](https://docs.warp.dev/agent-platform/capabilities/skills) | 官方文档；**已证实** | 2026-07-22 |
| `windsurf` | Windsurf | `~/.codeium/windsurf/mcp_config.json` | — | `json` / `mcpServers` / map / `windsurf` | stdio/http | [官方文档](https://docs.windsurf.com/windsurf/cascade/mcp) | `~/.codeium/windsurf/skills`; [官方文档](https://docs.windsurf.com/windsurf/cascade/skills) | 官方文档；**已证实** | 2026-07-22 |
| `zed` | Zed | `~/.config/zed/settings.json` | `.zed/settings.json` | `json` / `context_servers` / map / `url_inferred` | stdio/http | [官方文档](https://zed.dev/docs/ai/mcp) | `~/.agents/skills`; [官方文档](https://zed.dev/docs/ai/skills) | 官方文档/源码；**已证实** | 2026-07-22 |

### Skills 安装探针与兼容目录

这里的 probe 只决定“是否向本机展示/写入 Skills 目标”，不是安装器。没有 Skills writer 的 Agent 仍会按 MCP 配置路径被发现。兼容目录会扩大同一符号链接的实际影响范围，因此必须保留为显式证据，不能静默推断。

| Agent id | 官方能力对应的本机探针 | 兼容读取目录（alias） |
|---|---|---|
| `amp` | `amp` command；`~/.config/amp` path | `~/.agents/skills`、`~/.config/amp/skills`、`~/.claude/skills` |
| `amazon-q` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `antigravity` | `/Applications/Antigravity.app` path；`~/.gemini/config` path | — |
| `augment` | `augment` command；`auggie` command；`~/.augment` path | `~/.claude/skills`、`~/.agents/skills` |
| `boltai` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `claude-code` | `claude` command；`~/.claude` path | — |
| `claude-desktop` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `cline` | `cline` command；`~/.cline` path | — |
| `codebuddy-code` | `codebuddy` command；`~/.codebuddy` path | — |
| `codewhale` | `codewhale` command；`codewhale-tui` command；`~/.codewhale` path | `~/.agents/skills`、`~/.claude/skills`（另外存在 project 级兼容根，不能作为 user writer alias） |
| `codex` | `codex` command；`~/.codex` path | — |
| `continue` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `copilot-cli` | `copilot` command；`~/.copilot` path | `~/.agents/skills` |
| `crush` | `crush` command；`~/.config/crush` path | `~/.config/agents/skills`、`~/.agents/skills`、`~/.claude/skills` |
| `cursor` | `cursor` command；`/Applications/Cursor.app` path；`~/Library/Application Support/Cursor` path | `~/.agents/skills` |
| `devin` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `factory-droid` | `droid` command；`~/.factory` path | — |
| `firebender` | `/Applications/Firebender.app` path；`~/.firebender` path | `~/.goose/skills`、`~/.claude/skills`、`~/.codex/skills`、`~/.cursor/skills`、`~/.agents/skills` |
| `gemini` | `gemini` command；`~/.gemini` path | `~/.agents/skills` |
| `goose` | `goose` command；`~/Library/Application Support/Block/goose` path | `~/.claude/skills` |
| `grok-build` | `grok` command；`~/.grok` path | — |
| `hermes` | `hermes` command；`~/.hermes` path | — |
| `junie` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `kilo-code` | `kilo` command；`~/.config/kilo` path | — |
| `kimi-code` | `kimi` command；`~/.kimi-code` path | `~/.agents/skills` |
| `kiro` | `kiro-cli` command；`/Applications/Kiro.app` path；`~/.kiro` path | — |
| `lmstudio` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `minimax-code` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `mistral-vibe` | `vibe` command；`~/.vibe` path | — |
| `opencode` | `opencode` command；`~/.config/opencode` path | `~/.claude/skills`、`~/.agents/skills` |
| `openhands` | `openhands` command；`~/.openhands` path | — |
| `pi` | `pi` command；`~/.pi/agent` path | `~/.agents/skills` |
| `qoder` | `/Applications/Qoder.app` path；`~/Library/Application Support/Qoder` path | — |
| `qoder-cli` | `qoder` command；`~/.qoder` path | — |
| `qoderwork` | `/Applications/QoderWork.app` path；`~/.qoderwork` path | — |
| `qwen-code` | `qwen` command；`~/.qwen` path | — |
| `roo-code` | `~/.roo` path；`~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline` path | `~/.agents/skills` |
| `rovo-dev` | `~/.rovodev` path | `~/.agents/skills` |
| `stakpak` | `stakpak` command；`~/.stakpak` path | — |
| `tabnine` | 无 Skills writer，因此无 Skills 安装探针 | — |
| `vt-code` | `vtcode` command；`~/.vtcode` path | — |
| `vscode` | `code` command；`/Applications/Visual Studio Code.app` path；`~/Library/Application Support/Code` path | `~/.claude/skills`、`~/.agents/skills` |
| `warp` | `/Applications/Warp.app` path；`~/.warp` path | `~/.warp/skills`、`~/.claude/skills`、`~/.codex/skills`、`~/.cursor/skills`、`~/.gemini/skills`、`~/.copilot/skills`、`~/.factory/skills`、`~/.github/skills`、`~/.opencode/skills` |
| `windsurf` | `windsurf` command；`/Applications/Windsurf.app` path；`~/.codeium/windsurf` path | `~/.agents/skills` |
| `zed` | `zed` command；`/Applications/Zed.app` path；`~/.config/zed` path | — |

## 新增 Agent 的官方源码深挖

### CodeWhale（官方仓库 HEAD `065707a60a805fc67d6774018f43be925a3bc657`）

- **配置层级已证实**：用户配置为 `~/.codewhale/config.toml`，旧版 fallback 为 `~/.deepseek/config.toml`；`--config`、`CODEWHALE_CONFIG_PATH`、旧名 `DEEPSEEK_CONFIG_PATH` 可改实际文件。workspace 的 `.codewhale/config.toml` 只允许安全字段，并可用 `model` 覆盖用户的 `default_text_model`；provider、endpoint、credential、MCP、Skills 仍是 user-global。[官方配置文档](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/CONFIGURATION.md)；证据类型：官方源码文档；判定：**已证实**；核验日期：2026-07-22。
- **MCP 已证实**：默认 `~/.codewhale/mcp.json`，但 `mcp_config_path` 可动态改写，且有旧版 fallback。因此 MUX 的默认 writer 本身正确，但 observed state 必须能提示“运行时使用了自定义路径”，否则会出现写入成功但不生效。[官方配置文档](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/CONFIGURATION.md#where-it-looks)；证据类型：官方源码文档；判定：**已证实**。
- **Skills 首选与共享根已证实**：首选 user 根是 `~/.codewhale/skills`；runtime 还读取 `~/.agents/skills`、`~/.claude/skills`。project 读取 `.agents/skills`、`skills`、`.opencode/skills`、`.claude/skills`、`.cursor/skills`、`.codewhale/skills`，但这些 project 根不能成为 user writer alias。[官方运行时源码](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/tui/src/skills/mod.rs#L678-L686)；证据类型：官方源码；判定：**已证实**。审计起始快照 `aliases=[]` 与实际影响图不完整；当前工作树已补两个 user alias。
- **Models 路由已证实**：持久化当前路由是顶层 `provider` + `default_text_model`；命名 OpenAI-compatible provider 用 `[providers.<name>]`、`kind="openai-compatible"`、`base_url`、`model`。profiles 也存在，但激活靠 `--profile` / `DEEPSEEK_PROFILE`，并非单独的持久 current-profile 指针。[官方 provider 审计](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/MODEL_PROVIDER_AUDIT.md) 与 [配置文档](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/CONFIGURATION.md#custom-openai-compatible-gateways)；证据类型：官方源码/文档；判定：**已证实（OpenAI Chat Completions）**。native Anthropic/Responses 不可从该 custom schema 推断。
- **Models 凭据存在 typed/extras 分层**：共享 `ProviderConfigToml` 没有 typed `api_key_env`，但有 `#[serde(flatten)] extras` 保留未知 provider 字段；实际 TUI runtime 的 `ProviderConfig` 明确包含并消费 `api_key_env`，custom endpoint 缺失该环境变量时会 fail closed。[官方共享类型](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/config/src/lib.rs#L96-L127)、[runtime 字段](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/tui/src/config.rs#L2685-L2710) 与 [runtime 凭据解析](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/tui/src/config.rs#L5051-L5065)；证据类型：官方源码；判定：**已证实，但只能由保留 raw TOML 的 adapter 管理**。MUX 仅可在 profile 明确提供外部 `env_key` 时写 `api_key_env`；Keychain-only profile 不可假装兼容。
- **`auth.command` 当前不可用于运行凭据**：`[providers.<name>.auth] source="command"` 能反序列化和校验，但当前 HEAD 全仓没有执行 provider `auth.command` 的 resolver；官方文档也明确把执行命令与解析 secret 列为 follow-up work。[官方 auth schema](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/config/src/auth_source.rs) 与 [官方限制说明](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/CONFIGURATION.md#key-reference)；证据类型：官方源码/文档；判定：**冲突/未实现**。不得为复用 MUX Keychain 而写一个当前不会生效的 command auth。

### Stakpak（官方仓库 HEAD `760cd2b5984d29c2d513bb15ca33e995fae45f17`）

- **MCP 搜索顺序已证实**：`./mcp.toml|json` > `./.stakpak/mcp.toml|json` > `~/.stakpak/mcp.toml|json`。root 是 `mcpServers`；stdio 为 `command`、`args`、可选 `env`/`disabled`，HTTP 为 `url`、可选 `headers`/`disabled`。[官方运行时源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/libs/mcp/config/src/lib.rs)；证据类型：官方源码；判定：**已证实**。当前工作树已把用户配置标明为最低优先级 fallback；尚未实现项目文件探测与实际生效层 observation。
- **Skills 路径冲突**：当前运行时 `default_skill_directories()` 明确读取 project `.stakpak/skills` 和 user `~/.stakpak/skills`。[官方运行时源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/libs/api/src/local/skills/mod.rs#L153-L164)；证据类型：官方源码；判定：**已证实**。设计稿仍写 `~/.config/stakpak/skills`，[官方设计文档](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/docs/architecture-enhancements/08_unifying_knowledge_system.md)；判定：**冲突，runtime 胜出**。审计起始注册路径与 runtime 冲突；当前工作树已按 runtime 修正为 `~/.stakpak/skills`。
- **Models 部分可管理**：`~/.stakpak/config.toml` 使用 `[profiles.<name>]`、`model` 与 `[profiles.<name>.providers.<provider>]`；选择顺序是 `--profile` > `STAKPAK_PROFILE` > `default`，所以 `[profiles.default].model` 是无 CLI/env 覆盖时的持久默认指针。[官方 CLI 源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/cli/src/main.rs#L299-L305) 与 [配置类型源码](https://github.com/stakpak/agent/tree/760cd2b5984d29c2d513bb15ca33e995fae45f17/cli/src/config)；证据类型：官方源码；判定：**已证实**。
- **Models 凭据边界**：built-in `anthropic` / `openai` / `gemini` / `openrouter` 能从对应 canonical 环境变量取 key；任意 `type="custom"` provider 没有“环境变量名引用”字段，远程 custom 通常只能把 key/auth material 放进配置，local no-auth endpoint 例外。[官方凭据解析源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/cli/src/config/app.rs#L430-L470)；证据类型：官方源码；判定：**已证实**。建议先 guided/partial：仅允许 built-in canonical provider + canonical env，或无认证 local custom；不要把任意远程 custom 宣称为 managed。

### VT Code（官方仓库 HEAD `3f921f423d6bf1d08529badf8d27e9716371e245`）

- **配置层级已证实**：用户配置是 `~/.vtcode/vtcode.toml`，同时有 system、project/config-dir/workspace 层和 `VTCODE_CONFIG_PATH`；更高层可覆盖 user 值。因此 MUX 应显示实际生效层，而不能只显示“已写入用户配置”。[官方配置加载源码](https://github.com/vinhnx/VTCode/tree/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-config)；证据类型：官方源码；判定：**已证实**。
- **Models 候选已证实**：`[agent].provider` + `[agent].default_model` 是持久当前指针；`[[custom_providers]]` 有 `name`、`display_name`、`base_url`、`api_key_env`、`model`、`models`，并支持 `[custom_providers.auth] command/args/cwd/timeout_ms/refresh_interval_ms`。该结构明确只支持 OpenAI-compatible endpoint，provider 名须 lower-case 且不能与 built-in 冲突。[官方 custom provider schema](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-config/src/core/custom_provider.rs)；证据类型：官方源码；判定：**已证实（OpenAI Chat Completions）**，不得外推 Anthropic Messages 或 Responses。
- **command auth 确实运行**：runtime 用 `tokio::process::Command` 按 `command` + `args` 无 shell 执行，stdin 关闭、stdout 作为 bearer token，带 timeout/cache；非零退出、空 token、非 UTF-8 都 fail closed。OpenAI provider 构造器会在存在该配置时启用 `CustomCommand` backend，并有刷新/401 重试测试。[官方执行器](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-llm/src/providers/openai/custom_provider_auth.rs#L36-L118) 与 [provider 接线](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-llm/src/providers/openai/provider.rs#L235-L256)；证据类型：官方运行时源码/测试；判定：**已证实**。MUX 可安全写精确 argv 来读取自身 Keychain。
- **Skills 已证实**：当前 runtime loader 读取 project `.agents/skills` 与 user `~/.agents/skills`；旧 changelog/旁支模块出现的 `~/.vtcode/skills` 不应覆盖实际 loader。[官方运行时源码](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-core/src/skills/loader.rs#L290-L335)；证据类型：官方源码；判定：**已证实**，MUX 当前 path 正确。
- **MCP 总开关缺陷（当前工作树已修复）**：`[[mcp.providers]]` 每项默认 enabled，但顶层 `[mcp].enabled` 默认是 `false`。[官方 MCP 配置源码](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-config/src/mcp.rs#L11-L15) 与 [默认值测试](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-config/src/mcp.rs#L817-L829)；证据类型：官方源码；判定：**已证实**。缺失 `[mcp].enabled` 时写入 `true`；根开关或目标 provider 显式 `false`/非布尔时扫描跳过，更新、快照与恢复 fail closed，不替用户启用。当前只管理用户文件，尚不解析 workspace 或 `VTCODE_CONFIG_PATH` 的实际生效层。

### 可直接转为 fixture 的最小样例

下列内容只包含 adapter 必需字段；测试必须另加未知 sibling、注释、同名冲突、缺失文件与显式禁用场景，验证局部写入和 fail-closed。

**CodeWhale MCP（`~/.codewhale/mcp.json`）**

```json
{
  "servers": {
    "mux-clock": {
      "command": "npx",
      "args": ["-y", "@example/clock-mcp"]
    },
    "mux-docs": {
      "url": "https://mcp.example.test/mcp",
      "bearer_token_env_var": "MUX_MCP_TOKEN"
    }
  }
}
```

断言：也要读取兼容 root `mcpServers`；新写固定 `servers`；stdio/HTTP 判别不依赖臆造 `type`；`mcp_config_path` 被覆盖时静态默认文件不得标成 effective。[官方 MCP schema](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/MCP.md#remote-http-auth)；**已证实**。

**CodeWhale Model（`~/.codewhale/config.toml`，仅 OpenAI Chat Completions）**

```toml
provider = "mux-openrouter"
default_text_model = "anthropic/claude-sonnet-4"

[providers.mux-openrouter]
kind = "openai-compatible"
base_url = "https://openrouter.ai/api/v1"
model = "anthropic/claude-sonnet-4"
api_key_env = "MUX_MODEL_OPENROUTER_KEY"
```

断言：这是当前 HEAD 的最低**可运行**安全 fixture；`api_key_env` 必须按 raw TOML/extras 保留，不能依赖 `ProviderConfigToml` typed serializer 生成；只替换 `providers.mux-openrouter` 与两个顶层指针；保留其他 providers/profiles；project overlay 的 `model` 或 `CODEWHALE_CONFIG_PATH` 存在时 observed state 报 override，不谎报当前生效；profile 没有外部 `env_key` 时拒绝 apply，不回退到 Keychain-only 或明文 `api_key`。[官方 runtime 凭据解析](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/tui/src/config.rs#L5051-L5065) 与 [custom-provider 缺钥匙 fail-closed](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/tui/src/config.rs#L5189-L5209)；**已证实**。

**Stakpak MCP（默认 user fallback `~/.stakpak/mcp.toml`）**

```toml
[mcpServers.mux-clock]
command = "npx"
args = ["-y", "@example/clock-mcp"]

[mcpServers.mux-docs]
url = "https://mcp.example.test/mcp"
headers = { "X-MUX-Fixture" = "1" }
```

候选合同：TOML/JSON 两 codec 均应 round-trip；`./mcp.toml`、`./mcp.json`、`./.stakpak/mcp.*` 任一存在时 user file 只是 shadowed fallback；同一优先级 TOML/JSON 同时存在时必须显式冲突，不猜。[官方查找源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/libs/mcp/config/src/lib.rs#L81-L154)；**路径优先级已证实，当前实现尚未探测项目层**。

**Stakpak Model（安全边界内的 built-in OpenRouter）**

```toml
[profiles.default]
provider = "local"
model = "openrouter/anthropic/claude-sonnet-4"

[profiles.default.providers.openrouter]
type = "openrouter"
```

运行凭据来自 `OPENROUTER_API_KEY`；MUX 不能把自身 Keychain 值直接写入该配置。断言：存在 `--profile` / `STAKPAK_PROFILE` 时只报告 `[profiles.default]` 已配置而非当前 active；custom remote profile 无 literal key 时判 incomplete；no-auth local `type="custom"` 才能无凭据启用。[官方模板源码](https://github.com/stakpak/agent/blob/760cd2b5984d29c2d513bb15ca33e995fae45f17/cli/src/onboarding/config_templates.rs)；**已证实**。

**VT Code MCP（`~/.vtcode/vtcode.toml`）**

```toml
[mcp]
enabled = true

[[mcp.providers]]
name = "mux-clock"
enabled = true
command = "npx"
args = ["-y", "@example/clock-mcp"]

[[mcp.providers]]
name = "mux-docs"
enabled = true
endpoint = "https://mcp.example.test/mcp"
api_key_env = "MUX_MCP_TOKEN"
```

当前回归覆盖 missing-file/root switch、root/provider 显式禁用、按 `name` 局部维护与 `headers` / `http_headers` alias round-trip；workspace 与 `VTCODE_CONFIG_PATH` 的 effective-layer observation 仍未实现，保持延后。[官方 MCP 指南](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/docs/guides/mcp-integration.md#configuring-mcp-providers)；**上游契约已证实**。

**VT Code Model（OpenAI Chat Completions + MUX Keychain command）**

```toml
[agent]
provider = "mux_openrouter"
default_model = "anthropic/claude-sonnet-4"

[[custom_providers]]
name = "mux_openrouter"
display_name = "OpenRouter via MUX"
base_url = "https://openrouter.ai/api/v1"
model = "anthropic/claude-sonnet-4"
models = ["anthropic/claude-sonnet-4"]

[custom_providers.auth]
command = "/usr/bin/security"
args = [
  "find-generic-password", "-w", "-s",
  "com.scoheart.mux.model-profile.<profile-id>",
  "-a", "api-key"
]
timeout_ms = 5000
refresh_interval_ms = 300000
```

断言：只管理一个 stable `custom_providers.name` 与 `[agent]` 两指针；凭据缺失时不要创建 command auth；命令 argv 必须精确来自 MUX 的 `security_command(profile_id)`，不能接受用户拼接 shell；显式 workspace override 时不谎报 active；非 Chat-Completions profile 必须拒绝。[官方 command-auth schema](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-config/src/core/custom_provider.rs#L13-L131)；**已证实**。

## Models：45 个 Agent 全量证据矩阵

“不实施”只表示当前没有满足 MUX managed writer 门槛的公开稳定契约，不否定 Agent 自身支持模型切换。`guided/partial` 表示可以给用户打开官方入口、展示要求或在严格子集下生成配置，但 MUX 不应声称已统一托管凭据与当前指针。

| Agent id | 用户级配置 / 当前选择 | 协议与凭据边界 | 官方证据 | 判定 | 核验日期 |
|---|---|---|---|---|---|
| `amp` | 官方仅暴露 hosted mode/model picker；未找到可管理 BYOK provider 文件 | Amp 托管模型，不是中央 endpoint writer | [官方 Models](https://ampcode.com/models)、[官方 Manual](https://ampcode.com/manual) | **不实施** | 2026-07-22 |
| `amazon-q` | AWS 登录与 Amazon Q 托管模型；未找到用户级 provider/current-pointer 文件 | 无 MUX 三协议 BYOK 契约 | [AWS 官方文档](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/getting-started-q-dev.html) | **不实施** | 2026-07-22 |
| `antigravity` | 对话框内 sticky model selector；未找到稳定 BYOK 文件 | Google 托管的有限模型集 | [官方 Models](https://antigravity.google/docs/models) | **不实施** | 2026-07-22 |
| `augment` | IDE dropdown / Auggie `/model` / `--model`；选择按 workspace 或组织默认 | hosted catalog，无公开 provider credential writer | [官方 Available Models](https://docs.augmentcode.com/models/available-models) | **不实施** | 2026-07-22 |
| `boltai` | 原生 UI 支持多 provider、自定义 endpoint 与默认模型，但未公开稳定配置文件 schema | OpenAI 等；凭据由 BoltAI Keychain/加密数据库持有，MUX 不应直接写 | [官方 Features](https://docs.boltai.com/docs/features)、[官方凭据说明](https://docs.boltai.com/blog/how-boltai-handles-your-api-keys) | **guided/partial**（仅打开 UI） | 2026-07-22 |
| `claude-code` | `~/.claude/settings.json`；`model` | Anthropic Messages；`apiKeyHelper` 可执行 MUX Keychain command | [官方 Settings](https://code.claude.com/docs/en/settings) | **已 managed** | 2026-07-22 |
| `claude-desktop` | 官方 model selector；未找到受支持的 BYOK provider 文件 | Anthropic account models | [Claude 官方帮助](https://support.claude.com/en/articles/11049762-choosing-a-model-in-claude) | **不实施** | 2026-07-22 |
| `cline` | `~/.cline/data/settings/providers.json`；provider/model/API key 同文件；CLI `--provider`/`--model`/`--key` | OpenAI-compatible 等；官方未给环境变量名引用或安全非交互 secret writer | [官方 CLI Reference](https://docs.cline.bot/cli/cli-reference)、[OpenAI-compatible](https://docs.cline.bot/provider-config/openai-compatible) | **guided/partial** | 2026-07-22 |
| `codebuddy-code` | `/model` 或产品 picker；未找到稳定安全的 user-level BYOK writer | 产品托管/交互配置 | [官方 Model 文档](https://www.codebuddy.ai/docs/cli/model) | **不实施** | 2026-07-22 |
| `codewhale` | `~/.codewhale/config.toml`；顶层 `provider` + `default_text_model`；命名 `[providers.<id>]` | OpenAI Chat Completions；仅 raw/extras `api_key_env` 在 runtime 实际消费；Keychain-only 不兼容 | [官方配置](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/docs/CONFIGURATION.md)、[runtime](https://github.com/Hmbown/CodeWhale/blob/065707a60a805fc67d6774018f43be925a3bc657/crates/tui/src/config.rs#L5051-L5065) | **managed candidate（仅 env-reference）** | 2026-07-22 |
| `codex` | `~/.codex/config.toml`；`model` + `model_provider` + `[model_providers.<id>]` | OpenAI Responses；command auth 读取 MUX Keychain | [官方 Advanced Config](https://developers.openai.com/codex/config-advanced) | **已 managed** | 2026-07-22 |
| `continue` | `~/.continue/config.yaml` 的 `models[]`；各 role 的选中模型另存 `~/.continue/index/globalContext.json`；CLI 还可 `--config`/切换 config | 多 provider；`${{ secrets.NAME }}` 可来自 `.env`/process env，但 MUX Keychain 无直接 resolver；多 role/current profile 需专用 adapter | [官方 Config](https://docs.continue.dev/cli/configuration)、[官方 Reference](https://docs.continue.dev/reference)、[官方 active-pointer 源码](https://github.com/continuedev/continue/blob/5522c6f44ca0ac3528b37244818fbfa39b5af470/core/config/selectedModels.ts) | **guided/partial** | 2026-07-22 |
| `copilot-cli` | `/model`/CLI model picker；未找到稳定 BYOK provider file contract | GitHub Copilot hosted models | [GitHub CLI Reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#model) | **不实施** | 2026-07-22 |
| `crush` | `~/.config/crush/crush.json`；`providers` + `models.large` | Anthropic Messages / OpenAI Chat；`$VAR` 环境引用 | [官方仓库](https://github.com/charmbracelet/crush) | **已 managed** | 2026-07-22 |
| `cursor` | UI 支持 API keys/model selector；未找到官方支持的安全 user-level writer | 多 hosted/BYOK；凭据由 Cursor UI 管理 | [官方 API Keys](https://docs.cursor.com/settings/api-keys) | **不实施** | 2026-07-22 |
| `devin` | hosted Agent；未找到本机 user-level Model 配置 | Cognition 托管 | [官方文档](https://docs.devin.ai/) | **不实施** | 2026-07-22 |
| `factory-droid` | `~/.factory/settings.json`；`customModels[]` + `model` | Messages / Responses / Chat；`${VAR}` 环境引用 | [官方 BYOK](https://docs.factory.ai/cli/byok/overview) | **已 managed** | 2026-07-22 |
| `firebender` | 产品内模型设置；未找到官方稳定 user-level schema/credential reference | hosted/UI-managed | [官方文档](https://docs.firebender.com/) | **不实施** | 2026-07-22 |
| `gemini` | `~/.gemini/settings.json` 的 `model.name`，但 CLI/env 优先；模型配置仍是 Gemini 自身路由 | Google GenAI 固定面；`modelConfigs` 是参数/alias，不是任意 MUX provider endpoint | [官方 Model Routing](https://geminicli.com/docs/cli/model-routing/)、[官方 Advanced Model Config](https://geminicli.com/docs/cli/generation-settings/) | **不实施中央 provider writer** | 2026-07-22 |
| `goose` | macOS `~/Library/Application Support/Block/goose/config/config.yaml` + declarative custom provider JSON；`providers`/`active_provider` | Anthropic Messages / OpenAI Chat；外部 env | [官方 Providers](https://goose.ai/docs/getting-started/providers) | **已 managed**；现有旧 docs URL 应修正 | 2026-07-22 |
| `grok-build` | `~/.grok/config.toml`；`[model.<id>]` + `[models].default` | Messages / Responses / Chat；外部 env | [xAI Build Settings](https://docs.x.ai/build/settings#model-id) | **已 managed** | 2026-07-22 |
| `hermes` | `~/.hermes/config.yaml`；providers + model aliases + `model.default`/provider | Messages / Chat；`key_env` | [官方 Models](https://hermes-agent.nousresearch.com/docs/user-guide/configuring-models) | **已 managed** | 2026-07-22 |
| `junie` | IDE UI 支持指定 LLM providers/models；凭据由 IDE 安全存储 | 无公开稳定非交互文件契约 | [JetBrains 官方文档](https://www.jetbrains.com/help/junie/llm-providers-and-models.html) | **guided/partial**（打开 IDE UI） | 2026-07-22 |
| `kilo-code` | `~/.config/kilo/kilo.jsonc`；provider/models + `model` | Messages / Responses / Chat；`{env:VAR}` | [官方 Custom Models](https://kilo.ai/docs/code-with-ai/agents/custom-models) | **已 managed** | 2026-07-22 |
| `kimi-code` | `~/.kimi-code/config.toml`（`KIMI_CODE_HOME` 可迁移）；`default_model` + providers/models | 多协议，但 `api_key`/`providers.<id>.env` 都是配置内明文值；普通 shell env 不自动读 | [官方 Config Files](https://moonshotai.github.io/kimi-code/en/configuration/config-files.html)、[官方 Overrides](https://moonshotai.github.io/kimi-code/en/configuration/overrides.html) | **guided/partial**；no-auth local 例外 | 2026-07-22 |
| `kiro` | `/model`/产品 picker；未找到任意 BYOK provider file contract | Kiro 托管模型 | [官方 Model Selection](https://kiro.dev/docs/chat/model-selection/) | **不实施** | 2026-07-22 |
| `lmstudio` | LM Studio 是本地模型 runtime/server；不是消费远程中央 model profile 的 Agent | MUX 应把它当 endpoint provider 来源，不给 LM Studio 写 Agent current model | [官方 Docs](https://lmstudio.ai/docs) | **不实施此方向** | 2026-07-22 |
| `minimax-code` | `~/.mavis/config.yaml`；custom provider/current model 线索来自官方签名包 | 三协议，但当前 `options.apiKey` 明文 YAML | [官方下载](https://agent.minimax.io/download) | **现有 guided** | 2026-07-22 |
| `mistral-vibe` | `~/.vibe/config.toml`；providers/models + `active_model` | OpenAI Chat；`api_key_env_var` | [官方 API Keys/Profiles](https://docs.mistral.ai/vibe/code/cli/api-keys-profiles) | **已 managed** | 2026-07-22 |
| `opencode` | `~/.config/opencode/opencode.json`；provider/models + `model` | Messages / Responses / Chat；`{env:VAR}` | [官方 Models](https://opencode.ai/docs/models/) | **已 managed** | 2026-07-22 |
| `openhands` | `~/.openhands/agent_settings.json`；agent model/base URL/key | save 时 `expose_secrets=true`，默认 env 被忽略；`--override-with-envs` 只临时 | [官方仓库](https://github.com/OpenHands/OpenHands-CLI)、[官方 AgentStore 源码](https://github.com/OpenHands/OpenHands-CLI/blob/2df8a2835d3f1bd2f2eadf5a7a2e1ad0dfb0d271/openhands_cli/stores/agent_store.py) | **guided/partial** | 2026-07-22 |
| `pi` | `~/.pi/agent/models.json` + `settings.json`；custom providers + defaultProvider/defaultModel | Messages / Responses / Chat；command 读取 MUX Keychain | [Pi 官方 Models](https://github.com/earendil-works/pi/blob/dd6bea41efa8caa7a10fe5a6401676dc5699f83f/packages/coding-agent/docs/models.md) | **已 managed** | 2026-07-22 |
| `qoder` | 官方只证明 `/model` 交互流；无公开安全非交互 BYOK writer | 未建立协议/schema/credential 三元契约 | [官方文档](https://docs.qoder.com/) | **现有 guided** | 2026-07-22 |
| `qoder-cli` | `/model`/CLI selector；未找到公开稳定 provider file | 产品管理 | [官方 CLI 文档](https://docs.qoder.com/en/cli) | **不实施** | 2026-07-22 |
| `qoderwork` | 产品 UI/托管模型；未找到 stable local Model writer | hosted | [官方文档](https://docs.qoder.com/qoderwork) | **不实施** | 2026-07-22 |
| `qwen-code` | `~/.qwen/settings.json`；`modelProviders.<auth>[]` + `model.name`/selected auth | Messages / Chat；`envKey` | [官方 Model Providers](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/model-providers/) | **已 managed** | 2026-07-22 |
| `roo-code` | extension provider UI supports OpenAI-compatible models；未确认安全、稳定的 user-level credential writer | 多 provider；无受支持的 env-reference/current-pointer 文件合同 | [官方 Provider 文档](https://docs.roocode.com/providers/openai-compatible) | **guided/partial** | 2026-07-22 |
| `rovo-dev` | `/models` 选择 Atlassian 允许的 hosted models | 无任意 BYOK provider writer | [Atlassian 官方文档](https://support.atlassian.com/rovo/docs/use-rovo-dev-cli/) | **不实施** | 2026-07-22 |
| `stakpak` | `~/.stakpak/config.toml`；`[profiles.default]` + provider/model；`--profile`/`STAKPAK_PROFILE` 优先 | built-in provider 只认 canonical env；remote custom 无 env-name reference、常需明文 | [官方配置源码](https://github.com/stakpak/agent/tree/760cd2b5984d29c2d513bb15ca33e995fae45f17/cli/src/config) | **guided/partial**；canonical/no-auth 子集 | 2026-07-22 |
| `tabnine` | 产品托管模型；未找到 user-level BYOK endpoint/current pointer | hosted | [官方文档](https://docs.tabnine.com/) | **不实施** | 2026-07-22 |
| `vt-code` | `~/.vtcode/vtcode.toml`；`[agent].provider/default_model` + `[[custom_providers]]` | OpenAI Chat；command auth 有实际执行器，可读 MUX Keychain | [官方 schema](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-config/src/core/custom_provider.rs)、[官方执行器](https://github.com/vinhnx/VTCode/blob/3f921f423d6bf1d08529badf8d27e9716371e245/crates/codegen/vtcode-llm/src/providers/openai/custom_provider_auth.rs) | **managed candidate** | 2026-07-22 |
| `vscode` | UI + `chatLanguageModels.json` 可配 custom endpoint；当前 picker 是用户选择 | Chat / Responses / Messages；官方示例把 `apiKey` 写配置，UI secret 无 MUX writer 接口 | [VS Code 官方 Language Models](https://code.visualstudio.com/docs/agent-customization/language-models) | **guided/partial** | 2026-07-22 |
| `warp` | 产品 model picker / hosted catalog；付费层可 BYOK，但未找到安全 BYOK file contract | hosted/UI-managed | [官方 Agent Overview](https://docs.warp.dev/agent-platform/local-agents/overview)、[官方 FAQ](https://docs.warp.dev/agent-platform/getting-started/faqs) | **不实施** | 2026-07-22 |
| `windsurf` | 产品 model picker；未找到稳定 BYOK provider writer | hosted | [官方 Models](https://docs.windsurf.com/windsurf/models) | **不实施** | 2026-07-22 |
| `zed` | `~/.config/zed/settings.json` 可定义 `language_models.openai_compatible` / `anthropic_compatible`；官方当前页未给完整 active-pointer 写入合同 | Chat / Responses / Messages；custom provider key 来自 `<PROVIDER_ID>_API_KEY` 或 Zed 自身 Keychain，禁止写 settings | [Zed 官方 API Access](https://zed.dev/docs/ai/use-api-access) | **guided/partial；强 P1 候选** | 2026-07-22 |

## 研究建议与后续顺序（不代表本版实现）

> 以下是证据审计阶段的优先级建议，不是本版已实现清单。本版最终完成下列 P0 事实修正；VT Code / CodeWhale Model writer 因 effective-layer、动态路径与所有权边界不足而回撤。

### 审计时建议优先处理

1. **修正现有事实错误（P0）**
   - Stakpak Skills 从 `~/.config/stakpak/skills` 改为 runtime 实际读取的 `~/.stakpak/skills`。
   - Stakpak MCP registry 已把 user path 标为最低优先级 fallback；项目层探测与 effective-layer observation 仍保持延后。
   - VT Code MCP missing-file 写入补 `[mcp] enabled=true`；用户显式 `false` 时 fail closed，不替用户打开。
   - CodeWhale 补 user aliases `~/.agents/skills`、`~/.claude/skills`，project roots 只用于影响分析，不作为 user writer。
   - Goose Model docs 从已失效的 `https://block.github.io/goose/docs/getting-started/providers` 改为当前官方 `https://goose.ai/docs/getting-started/providers`。
2. **VT Code Model managed writer（P1）**
   - 证据已覆盖 config schema、持久 current pointer、真实 command-auth 执行、超时/缓存/401 刷新、配置层级和协议限制。
   - 仅接受 `OpenaiCompletions`；凭据优先使用 MUX Keychain command，已有外部 `env_key` 时也可选择 `api_key_env`，两者不能同时写；无认证仅允许明确的 loopback/local endpoint。
3. **CodeWhale env-reference Model writer（P1，严格条件）**
   - 仅接受 `OpenaiCompletions` 且 profile 有非空 `env_key`；Keychain-only profile 显示不兼容，不自动降级为明文。
   - 必须使用保留 raw TOML unknown fields/comments 的 adapter 写 `api_key_env`；不能依赖没有 typed 字段的 `ProviderConfigToml` serializer，也不能写当前不会执行的 `auth.command`。

### 只做 guided / partial

- **Stakpak**：可生成 built-in canonical provider + canonical env 的说明，或 no-auth local custom；任意 remote custom 不得托管，因为没有“环境变量名引用”字段。
- **Continue**：model list 和 per-role current pointer 均已证实，但 config/profile/role 层级多，`${{ secrets.* }}` 也没有 MUX Keychain resolver。先做发现、预览和打开配置；未来需独立 YAML + globalContext transaction adapter。
- **Zed**：三协议 custom endpoint 和衍生环境变量规则很清晰，但官方当前文档未完整固定 active-pointer 写入/清理契约，且 Zed Keychain 无外部 writer。先支持 env-reference 预览与引导，不先声称 managed。
- **Cline / Roo Code / VS Code**：产品支持 custom endpoint，但官方稳定文件或 secret ownership 不足；只能打开官方设置入口，不能把 API key 写到公开配置或 argv。
- **Kimi Code / OpenHands / MiniMax Code**：公开持久格式会把 key 明文落盘；仅引导产品自己的认证/UI，no-auth local endpoint 可另做显式实验能力。
- **BoltAI / Junie / Qoder**：产品自身 UI/Keychain 能安全配置，但没有 MUX 可调用的受支持 writer；只提供引导。

### 当前不要实施 Model writer

- hosted/账号托管且无 BYOK 文件合同：`amp`、`amazon-q`、`antigravity`、`augment`、`claude-desktop`、`codebuddy-code`、`copilot-cli`、`cursor`、`devin`、`firebender`、`kiro`、`qoder-cli`、`qoderwork`、`rovo-dev`、`tabnine`、`warp`、`windsurf`。
- 产品角色不是“消费中央远程 profile”：`lmstudio` 应作为 endpoint/runtime 来源，不应反向写 Agent Model。
- 只允许自身 native catalog、没有中央 provider endpoint 合同：`gemini`。可以继续展示/切换 Gemini 自身 `model.name`，但那不是把 MUX Model 资产添加给 Agent。

## CodeWhale 与 VT Code 候选 writer 所有权合同（本版未实现）

### CodeWhale

- **目标文件**：默认 `~/.codewhale/config.toml`。若 `CODEWHALE_CONFIG_PATH` / legacy override 或 CLI `--config` 改变位置，默认文件只能标为 shadowed；MUX 不在未知动态路径上自动写。
- **MUX 所有字段**：唯一稳定 provider id（建议 `mux-<profile-id-slug>`）对应的 `[providers.<id>]` 子树中的 `kind`、`base_url`、`model`、raw `api_key_env`，以及顶层 `provider`、`default_text_model` 两个 current-pointer 字段。其他 provider、profile、limits、headers、comments 都是外部所有。
- **apply 前置条件**：协议必须是 OpenAI Chat Completions；`base_url`、model id、外部 `env_key` 均非空；provider id 不与 built-in/现有不同内容 entry 冲突。若同名 entry 已存在但没有 MUX ownership fingerprint/transaction record，fail closed。
- **current pointer**：顶层两字段只代表默认 session；project `.codewhale/config.toml` 的 `model`、CLI/env model/provider、`--profile` / profile env 均可覆盖。observe 必须区分 `configured_default` 与 `effective_override`。
- **删除/回滚**：只删除 exact owned provider table；只有当前顶层指针仍等于本次写入值时，才恢复事务记录里的前值。用户在 apply 后改过任一指针或 entry 内容时标 `conflicted`，不覆盖。最后一个键删除后才清空空表，保留 sibling/comments。
- **凭据**：只写环境变量**名称**，绝不读取/复制该变量值，也不写 `api_key`。MUX Keychain 有值但 profile 无 `env_key` 时不兼容；`auth.command` 在当前 HEAD 不执行，不能作为兜底。
- **外部检测**：扫描所有 `[providers.*]`、顶层指针、动态 config path、project overlay 和 profile/CLI/env override 来源；只以 stable id + exact managed fields + transaction metadata 识别 MUX entry，不能把同 endpoint 的用户 entry 收编。

### VT Code

- **目标文件**：默认 `~/.vtcode/vtcode.toml`；system、project/config-dir/workspace 与 `VTCODE_CONFIG_PATH` 均可能覆盖。观察层必须返回 origin/effective layer，不能只读 user file。
- **MUX 所有字段**：exact `[[custom_providers]]` 元素（按稳定 `name`）中的 `name`、`display_name`、`base_url`、`model`、`models`，以及二选一的 `auth` 或 `api_key_env`；再管理 `[agent].provider`、`[agent].default_model` 两个默认 current pointer。不得改其他 `agent` 字段或 sibling list entries。
- **apply 前置条件**：仅 OpenAI Chat Completions；name 需小写字母/数字/`-`/`_` 且不撞 built-in；`auth` 和 `api_key_env` 不得并存。Keychain command 必须由 MUX 固定函数生成 `/usr/bin/security` + 精确 argv，禁止 shell 字符串/用户命令拼接。
- **current pointer**：`agent.provider` 直接等于 custom provider `name`，`agent.default_model` 等于 model id。更高 config layer 或 CLI override 存在时，user pointer 只标 configured，不标 effective。
- **删除/回滚**：按 exact name 删除一个 owned list element；只有两个 pointer 仍等于 MUX 值才恢复事务前值。列表顺序、未知字段、其他 provider 和 TOML comments 必须 round-trip；同名不同内容标冲突。
- **凭据**：Keychain credential 存在时写 command auth；外部 env 模式只写 `api_key_env`；明确 loopback/no-auth 可两者都省略。实际执行器关闭 stdin、捕获 stdout、trim token，并对空值/超时/非零退出 fail closed。
- **外部检测**：读取所有 config layers 的 origin metadata；同时对比 custom provider name、base URL、model、auth mode 和 current pointer。workspace/CLI 覆盖、同名外部 entry、用户修改过的 command argv 都必须呈现为 override/conflict，不自动修复。

## 候选能力启用前必须补齐的回归测试

1. CodeWhale：missing file apply；raw `api_key_env` round-trip；保留未知 sibling/comments；Keychain-only 拒绝；project model override；dynamic config path；同名外部 provider 冲突；用户改指针后 remove 不回滚。
2. VT Code Model：missing file apply；command auth 实际 TOML shape；`auth` 与 `api_key_env` 互斥；custom name 校验；多 list entry 保序；workspace/CLI override；用户改 pointer/argv 后 remove fail closed。
3. VT Code MCP：missing file 自动写 root enabled；root/provider 显式 `false` 时扫描跳过，更新与恢复 fail closed；effective-layer 的独立状态呈现仍待实现。
4. Stakpak：项目层探测、所有 project/user precedence 组合与同优先级 TOML+JSON 并存冲突仍待实现；Skills runtime path 已修正为 `~/.stakpak/skills`。
5. CodeWhale Skills：首选目录 + 两个 user aliases 影响图；project aliases 不进入 user writer targets。

## 剩余缺口

- Zed 的 active default model 序列化标识、清理语义和 project override origin 还需锁定到官方源码 commit 后，才可从 guided 升级 managed。
- Continue 的 `selectedModelsByProfileId` 是 per-role、多 profile 状态；需要先定义 MUX UI 如何映射 chat/edit/apply/autocomplete，不能用一个“当前模型”覆盖所有角色。
- Cline、Roo、VS Code 的 secret storage 没有受支持的外部写入 API；除非官方提供 stdin/keychain/SecretStorage command，否则不做 managed。
- CodeWhale 的 `api_key_env` 当前是 runtime 明确消费、共享 typed schema 通过 extras 保留的事实；如果上游将其正式提升为 typed field，需重跑 fixture，届时可移除 raw-field 特判。
