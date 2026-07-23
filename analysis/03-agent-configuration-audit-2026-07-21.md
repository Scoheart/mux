# Agent MCP / Skills / Model 配置审计（2026-07-21）

## 结论

本次按 MUX 当前 42 个深度审计 Agent 逐项对账了 MCP、user-level Agent Skills 和 Model Profile 配置契约：

- MCP：42 个定义中 41 个有可写用户级目标，Devin 保持只读发现。现有路径、根字段、格式、transport codec 与近期官方审计一致，本轮没有发现需要改写的 MCP schema。
- Skills：原实现只开放 6 个已核验 Agent，实际已有 33 个 MUX Agent 发布稳定的用户级 Skills 契约。已补齐首选目录、兼容目录、安装探针和物理影响图。
- Models：12 个自动写入目标、2 个安全引导目标。发现并修复一个高影响 Qwen 错误：MUX 原先写未来文档中的 `{ protocol, models }` wrapper，但本机 Qwen 0.14.3 与 npm stable 0.20.0 都仍消费数组。
- 文档：README、Agents、Skills、Models 指南已经与运行时代码重新对齐。

本轮只修改 MUX 的 source of truth、测试和文档，没有改写用户主目录下任何 Agent 配置，也没有发布、commit 或 push。

## 证据优先级与版本裁决

采用以下优先级：当前稳定发布包/对应 tag 源码 > 官方产品文档 > 官方仓库说明。社区扩展只用于明确标注的 Pi MCP adapter，不用来推断其他产品契约。

Qwen 是本轮唯一出现官方材料自相矛盾的项目：官网 Model Providers 概览已描述 `{ protocol, models }`，但同站 Quick Start、npm stable 0.20.0 发布包和运行时代码仍要求 `modelProviders.<auth>` 为 `ModelConfig[]`。发布包遇到非数组会跳过并给出 warning。因此 MUX 现在写数组，只对旧 MUX 生成的精确 wrapper 做迁移；wrapper 含未知字段时拒绝覆盖。

常规官方文档和发布源码已经足够完成裁决，因此没有启动 ChatGPT 网页版 Pro/xhigh 兜底搜索。

## 本轮修正

1. [`core/src/resources/model/adapters.rs`](../core/src/resources/model/adapters.rs)
   - Qwen 改写为 stable 数组结构。
   - 保留其他 auth group 与外部 model。
   - 按 `id + baseUrl` 识别 MUX model。
   - 精确迁移旧 `{ "protocol": <auth>, "models": [...] }`。
   - 非对象 `modelProviders`、非数组 provider、协议不匹配或含未知字段的 wrapper 全部 fail closed。
2. `data/agents.json` 与 Skills runtime allowlist
   - user-level Skills capability 从 6 个扩展到 33 个。
   - 同一路径统一使用同一 `target_id`，兼容目录进入物理影响图。
   - `~/.agents/skills` 的首选消费者现在包括 Codex、Goose、Warp、Zed；Cursor 等兼容消费者按安装状态加入影响范围。
3. 文档
   - 补齐 33 个 Skills 路径。
   - 补齐 12 managed + 2 guided Model 路径及所有字段。
   - 明确符号链接是可写的实时中央副本，不是隔离副本。

## MCP 配置矩阵

### 字段族

| codec / 产品族 | stdio 主要字段 | HTTP 主要字段 | 特殊约束 |
|---|---|---|---|
| 标准 JSON map | `command`、`args`、`env` | `url`、`headers` | 根通常为 `mcpServers` |
| Claude Code / explicit type | 标准字段，可带 `type: "stdio"` | `type: "http"`、`url`、`headers` | 只局部维护目标 server |
| Codex | TOML `command`、`args`、`env` | `url`、`http_headers` | 根表 `mcp_servers.<name>` |
| Gemini / Qwen | `command`、`args`、`env` | `httpUrl`、`headers` | 不能误写通用 `url` |
| Antigravity / Windsurf | `command`、`args`、`env` | `serverUrl`、`headers` | 不能误写 `httpUrl` |
| OpenCode / Kilo | `type="local"`、数组 `command`、`environment` | `type="remote"`、`url`、`headers` | 根为 `mcp` |
| Continue | YAML list，条目含 `name` 与连接字段 | 同一 list | 新文件还需根 `name/version/schema` |
| Cline | 连接字段位于 `transport` 子对象 | `transport.type/url/headers` | 保留 registration state |
| Goose | YAML `extensions` map | Streamable HTTP 字段 | 当前 macOS 路径来自 runtime 源码 |
| Hermes | YAML `mcp_servers` map | `url`；仅旧 SSE 写 `transport: sse` | Streamable HTTP 不写旧 SSE 标记 |
| Mistral Vibe | TOML `[[mcp_servers]]` | 同一数组表 | identity 为 `name` |
| VS Code | 根 `servers` | `type`/`url`/`headers` | 与常见 `mcpServers` 不同 |
| Tabnine | 标准本地字段 | HTTP header 位于 `requestInit.headers` | 保留其他 `requestInit` 字段 |
| QoderWork | 标准本地字段 | `type: streamable-http` 或 `sse` | 不碰内置客户端数据 |
| Claude Desktop / BoltAI 文件 | 标准 stdio | 不直接写远程 | 文件契约只原生接受 stdio |

### 42 个深度审计目标

| Agent id | 用户级全局路径 | 格式 / 根字段 | codec | transport | 结论 |
|---|---|---|---|---|---|
| `amp` | `~/.config/amp/settings.json` | `json` / `amp.mcpServers` | `url_inferred` | `stdio / http` | 准确 |
| `amazon-q` | `~/.aws/amazonq/default.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确；IDE 与 CLI discovery record 分离 |
| `antigravity` | `~/.gemini/config/mcp_config.json` | `json` / `mcpServers` | `server_url` | `stdio / http` | 准确 |
| `augment` | `~/.augment/settings.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确 |
| `boltai` | `~/.boltai/mcp.json` | `json` / `mcpServers` | `stdio_only` | `stdio` | 准确；远程需 `mcp-remote` |
| `claude-code` | `~/.claude.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确；项目级另为 `.mcp.json` |
| `claude-desktop` | `~/Library/Application Support/Claude/claude_desktop_config.json` | `json` / `mcpServers` | `claude_desktop` | `stdio` | 准确；远程在 Connectors 管理 |
| `cline` | `~/.cline/data/settings/cline_mcp_settings.json` | `json` / `mcpServers` | `cline` | `stdio / http` | 准确 |
| `codebuddy-code` | `~/.codebuddy/.mcp.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确 |
| `codex` | `~/.codex/config.toml` | `toml` / `mcp_servers` | `codex` | `stdio / http` | 准确；项目级另为 `.codex/config.toml` |
| `continue` | `~/.continue/config.yaml` | `yaml` / `mcpServers` | `continue` | `stdio / http` | 准确 |
| `copilot-cli` | `~/.copilot/mcp-config.json` | `json` / `mcpServers` | `copilot` | `stdio / http` | 准确 |
| `crush` | `~/.config/crush/crush.json` | `json` / `mcp` | `explicit_type` | `stdio / http` | 准确 |
| `cursor` | `~/.cursor/mcp.json` | `json` / `mcpServers` | `url_inferred` | `stdio / http` | 准确 |
| `devin` | — | `unknown` | — | — | 只读发现；无稳定 user-level 全局文件契约 |
| `factory-droid` | `~/.factory/mcp.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确 |
| `firebender` | `~/.firebender/firebender.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确 |
| `gemini` | `~/.gemini/settings.json` | `json` / `mcpServers` | `gemini` | `stdio / http` | 准确；项目级另为 `.gemini/settings.json` |
| `goose` | `~/Library/Application Support/Block/goose/config/config.yaml` | `yaml` / `extensions` | `goose` | `stdio / http` | 准确；macOS runtime 路径，不是通用文档的 Linux 路径 |
| `grok-build` | `~/.grok/config.toml` | `toml` / `mcp_servers` | `standard` | `stdio / http` | 准确 |
| `hermes` | `~/.hermes/config.yaml` | `yaml` / `mcp_servers` | `url_transport` | `stdio / http` | 准确 |
| `junie` | `~/.junie/mcp/mcp.json` | `json` / `mcpServers` | `url_inferred` | `stdio / http` | 准确 |
| `kilo-code` | `~/.config/kilo/kilo.jsonc` | `jsonc` / `mcp` | `opencode` | `stdio / http` | 准确 |
| `kimi-code` | `~/.kimi-code/mcp.json` | `json` / `mcpServers` | `kimi` | `stdio / http` | 准确；这是新 Kimi Code，不是 legacy `~/.kimi` CLI |
| `kiro` | `~/.kiro/settings/mcp.json` | `json` / `mcpServers` | `url_inferred` | `stdio / http` | 准确 |
| `lmstudio` | `~/.lmstudio/mcp.json` | `json` / `mcpServers` | `url_inferred` | `stdio / http` | 准确 |
| `minimax-code` | `~/.mavis/mcp.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确；与 Model 的 `~/.mavis/config.yaml` 分离 |
| `mistral-vibe` | `~/.vibe/config.toml` | `toml` / `mcp_servers` | `vibe` | `stdio / http` | 准确 |
| `opencode` | `~/.config/opencode/opencode.json` | `jsonc` / `mcp` | `opencode` | `stdio / http` | 准确 |
| `openhands` | `~/.openhands/mcp.json` | `json` / `mcpServers` | `explicit_type` | `stdio / http` | 准确 |
| `pi` | `~/.pi/agent/mcp.json` | `json` / `mcpServers` | `url_inferred` | `stdio / http` | 仅适用于社区 `pi-mcp-adapter`；Pi core 不内置 MCP |
| `qoder` | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` | `json` / `mcpServers` | `qoder` | `stdio / http` | 准确；Qoder Desktop My Servers |
| `qoder-cli` | `~/.qoder/settings.json` | `json` / `mcpServers` | `qoder` | `stdio / http` | 准确；不要与 Desktop 路径混用 |
| `qoderwork` | `~/.qoderwork/mcp.json` | `json` / `mcpServers` | `qoderwork` | `stdio / http` | 准确 |
| `qwen-code` | `~/.qwen/settings.json` | `jsonc` / `mcpServers` | `gemini` | `stdio / http` | 准确 |
| `roo-code` | `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | `json` / `mcpServers` | `roo` | `stdio / http` | 准确 |
| `rovo-dev` | `~/.rovodev/mcp.json` | `json` / `mcpServers` | `transport` | `stdio / http` | 准确 |
| `tabnine` | `~/.tabnine/mcp_servers.json` | `json` / `mcpServers` | `tabnine` | `stdio / http` | 准确 |
| `vscode` | `~/Library/Application Support/Code/User/mcp.json` | `json` / `servers` | `vscode` | `stdio / http` | 准确 |
| `warp` | `~/.warp/.mcp.json` | `json` / `mcpServers` | `warp` | `stdio / http` | 准确 |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `json` / `mcpServers` | `windsurf` | `stdio / http` | 准确 |
| `zed` | `~/.config/zed/settings.json` | `json` / `context_servers` | `url_inferred` | `stdio / http` | 准确 |

每项的官方证据 URL、审计日期、layout、identity field 与 transport allowlist 仍以 `data/agents.json` 为机器可读 source of truth；`website/guide/agents.md` 是用户可读投影。

## User-level Skills 配置矩阵

| Agent id | 首选目录 | 兼容读取目录 |
|---|---|---|
| `amp` | `~/.config/agents/skills` | `~/.agents/skills`、`~/.config/amp/skills`、`~/.claude/skills` |
| `antigravity` | `~/.gemini/config/skills` | — |
| `augment` | `~/.augment/skills` | `~/.claude/skills`、`~/.agents/skills` |
| `claude-code` | `~/.claude/skills` | — |
| `cline` | `~/.cline/skills` | — |
| `codebuddy-code` | `~/.codebuddy/skills` | — |
| `codex` | `~/.agents/skills` | — |
| `copilot-cli` | `~/.copilot/skills` | `~/.agents/skills` |
| `crush` | `~/.config/crush/skills` | `~/.config/agents/skills`、`~/.agents/skills`、`~/.claude/skills` |
| `cursor` | `~/.cursor/skills` | `~/.agents/skills` |
| `factory-droid` | `~/.factory/skills` | — |
| `firebender` | `~/.firebender/skills` | `~/.goose/skills`、`~/.claude/skills`、`~/.codex/skills`、`~/.cursor/skills`、`~/.agents/skills` |
| `gemini` | `~/.gemini/skills` | `~/.agents/skills` |
| `goose` | `~/.agents/skills` | `~/.claude/skills` |
| `grok-build` | `~/.grok/skills` | — |
| `hermes` | `~/.hermes/skills` | — |
| `kilo-code` | `~/.kilo/skills` | — |
| `kimi-code` | `~/.kimi-code/skills` | `~/.agents/skills` |
| `kiro` | `~/.kiro/skills` | — |
| `mistral-vibe` | `~/.vibe/skills` | — |
| `opencode` | `~/.config/opencode/skills` | `~/.claude/skills`、`~/.agents/skills` |
| `openhands` | `~/.openhands/skills` | — |
| `pi` | `~/.pi/agent/skills` | `~/.agents/skills` |
| `qoder` | `~/.qoder/skills` | — |
| `qoder-cli` | `~/.qoder/skills` | — |
| `qoderwork` | `~/.qoderwork/skills` | — |
| `qwen-code` | `~/.qwen/skills` | — |
| `roo-code` | `~/.roo/skills` | `~/.agents/skills` |
| `rovo-dev` | `~/.rovodev/skills` | `~/.agents/skills` |
| `vscode` | `~/.copilot/skills` | `~/.claude/skills`、`~/.agents/skills` |
| `warp` | `~/.agents/skills` | `~/.warp/skills`、`~/.claude/skills`、`~/.codex/skills`、`~/.cursor/skills`、`~/.gemini/skills`、`~/.copilot/skills`、`~/.factory/skills`、`~/.github/skills`、`~/.opencode/skills` |
| `windsurf` | `~/.codeium/windsurf/skills` | `~/.agents/skills` |
| `zed` | `~/.agents/skills` | — |

注意：`~/.codex/skills` 是 Firebender/Warp 的兼容目录，不是 Codex 当前官方首选目录；Codex 自身使用 `~/.agents/skills`。任意 `CRUSH_SKILLS_DIR`、`KIMI_CODE_HOME`、Grok `[skills].paths` 等动态覆盖不适合作为静态 MUX target，继续由产品自身管理。

没有接入 Skills writer 的 9 个深度审计 Agent：Amazon Q IDE、BoltAI、Claude Desktop、Continue、Devin、Junie、LM Studio、MiniMax Code、Tabnine。当前公开资料要么只提供 rules/prompts，要么只有 project scope，要么没有稳定的 user-level Agent Skills 契约；这表示“未找到足够证据”，不表示产品永远不支持。

### 中央链接的写入风险

MUX 在目标目录创建指向 `~/.mux/skills/<name>` 的符号链接。它保证目录边界、事务恢复和 drift 检测，但不会把链接变成只读。任何消费者只要跟随链接并写文件，就会直接修改中央副本；更新/替换前必须把这种变化当作中央 drift 审阅和备份。

## Model 配置矩阵

| Agent | 模式 | 路径 / 格式 | MUX 所有字段 | 协议 | 凭据契约 | 结论 |
|---|---|---|---|---|---|---|
| Claude Code | managed | `~/.claude/settings.json` / JSONC | `model`、`apiKeyHelper`、`env.ANTHROPIC_BASE_URL` | Anthropic Messages | Keychain read command | 准确 |
| Codex | managed | `~/.codex/config.toml` / TOML | `model`、`model_provider`、`model_providers.<mux-id>`；`wire_api="responses"` | OpenAI Responses | provider `auth.command/args` 读取 Keychain | 准确 |
| Grok Build | managed | `~/.grok/config.toml` / TOML | `model.<mux-id>`、`models.default`；可选 `context_window/max_completion_tokens` | Messages / Responses / Chat Completions | `env_key` | 准确 |
| Pi | managed | `~/.pi/agent/models.json` + `settings.json` / JSONC | `providers.<mux-id>`、`defaultProvider`、`defaultModel` | 三种 | `apiKey: "!<Keychain command>"` | 准确 |
| OpenCode | managed | `~/.config/opencode/opencode.json` / JSONC | `provider.<mux-id>`、`model="<provider>/<model>"` | 三种 | `{env:VAR}` | 准确 |
| Kilo Code CLI | managed | `~/.config/kilo/kilo.jsonc` / JSONC | 同 OpenCode | 三种 | `{env:VAR}` | 准确 |
| Qwen Code | managed | `~/.qwen/settings.json` / JSONC | `modelProviders.<auth>[]`、`model.name`、`security.auth.selectedType` | Messages / Chat Completions | model `envKey` | **已修正** stable 数组结构 |
| Crush | managed | `~/.config/crush/crush.json` / JSONC | `providers.<mux-id>`、`models.large` | Messages / Chat Completions | `$VAR` | 准确；不改 `small` 等槽位 |
| Mistral Vibe | managed | `~/.vibe/config.toml` / TOML | `[[providers]]`、`[[models]]`、`active_model` | Chat Completions | `api_key_env_var` | 准确 |
| Hermes Agent | managed | `~/.hermes/config.yaml` / YAML | `providers.<mux-id>`、`model_aliases.<mux-id>`、`model.default/provider` | Messages / Chat Completions | provider `key_env` | 准确；不接管辅助 task model |
| Factory Droid | managed | `~/.factory/settings.json` / JSONC | `customModels[]`、`model` | 三种 | `${VAR}` | 准确 |
| Goose | managed | macOS config YAML + `custom_providers/<mux-id>.json` | `providers.<mux-id>`、`active_provider`、declarative provider JSON | Messages / Chat Completions | `api_key_env` | 准确；custom provider 跟随 config parent |
| MiniMax Code | guided | `~/.mavis/config.yaml` / YAML | MUX 不自动写 Model | 三种产品能力 | 当前自定义 provider 会持久化明文 `options.apiKey` | 正确保持引导 |
| Qoder | guided | `~/.qoder/settings.json` / JSON | MUX 不自动写 Model | 由产品 `/model` 管理 | 未发现公开安全的 non-interactive BYOK writer | 正确保持引导 |

### Qwen 修复后的稳定结构

```json
{
  "modelProviders": {
    "openai": [
      {
        "id": "model-id",
        "name": "Display Name",
        "baseUrl": "https://example.test/v1",
        "envKey": "MODEL_API_KEY",
        "generationConfig": {
          "contextWindowSize": 128000
        }
      }
    ]
  },
  "model": { "name": "model-id" },
  "security": { "auth": { "selectedType": "openai" } }
}
```

同一 auth type 内的唯一性是 `id + baseUrl`。`envKey` 可选；MUX 不把 API key 明文写入 Qwen settings。

## 保留边界与后续观察点

- MUX 当前只管理 macOS user-level 默认路径；项目级 MCP/Skills/Model 配置不在本轮写入范围。
- 任意产品支持的动态路径环境变量或额外 search path 不会被静态 catalog 猜测。
- Qwen 官网的新 wrapper 描述如果未来真正进入 stable，需按已安装版本引入显式 schema/version 迁移；当前不能提前写未来格式。
- Pi MCP 仍是社区 adapter 能力，不能描述成 Pi core 原生支持。
- Devin 仍无稳定 user-level MCP 文件；不应为了“覆盖率”猜路径。

## 主要官方依据

- [Codex configuration](https://developers.openai.com/codex/config-reference)、[MCP](https://developers.openai.com/codex/mcp)、[Skills](https://developers.openai.com/codex/skills)
- [Claude Code settings](https://code.claude.com/docs/en/settings)、[MCP](https://code.claude.com/docs/en/mcp)、[Skills](https://code.claude.com/docs/en/skills)
- [Qwen stable Model Providers / Quick Start](https://qwenlm.github.io/qwen-code-docs/en/blog/quickstart/getting-started/)、[Skills](https://qwenlm.github.io/qwen-code-docs/en/users/features/skills/)、[官方源码](https://github.com/QwenLM/qwen-code)
- [Grok Build overview/models](https://docs.x.ai/build/overview)、[Skills](https://docs.x.ai/build/features/skills-plugins-marketplaces)
- [Goose config](https://goose-docs.ai/docs/guides/config-files/)、[Skills](https://goose-docs.ai/docs/guides/context-engineering/using-skills/)、[官方源码](https://github.com/block/goose)
- [Kimi Code MCP](https://moonshotai.github.io/kimi-code/en/customization/mcp)、[Skills](https://moonshotai.github.io/kimi-code/en/customization/skills)
- [Warp Skills](https://docs.warp.dev/agent-platform/capabilities/skills)、[VS Code Skills](https://code.visualstudio.com/docs/agent-customization/agent-skills)、[Firebender Skills](https://docs.firebender.com/multi-agent/skills)
- 其余逐 Agent 官方 URL 保存在 `data/agents.json` 的 `docs` 与 `skills.docs` 字段中。

## 验证记录

- `jq empty data/agents.json`：通过。
- catalog 计数：42 个 Agent、41 个可写 MCP 目标、33 个 Skills capability；唯一只读 MCP 目标为 Devin。
- 42 个 MCP `docs` URL、33 个 `skills.docs` URL：全部 HTTP 200。
- `cargo test -p mux-core verified_skill_capabilities_are_data_driven --lib`：通过。
- `cargo test -p mux-core qwen_ --lib`：4 项通过。
- `cargo test -p mux-core model_migration --lib`：7 项通过，覆盖 12 个可管理 Model importer、Qwen 稳定数组格式、旧 wrapper 精确兼容和隐私路径折叠。
- `cargo test --workspace --locked`：通过（core unit 297 项通过、2 项 ignored；CLI 35 项及全部 integration suites 通过）。
- `cargo test --manifest-path desktop/src-tauri/Cargo.toml`：通过。
- Desktop `npm test`：40 个测试文件、204 项通过；`npm run build`：通过，并验证 41 个可配置 Agent 的图标。
- Website `npm run build`：通过。
- `cargo fmt --all -- --check`、`git diff --check`：通过。
- 根仓 `python3 scripts/agent-startup-context.py --check`：通过。
