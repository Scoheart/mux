# Supported agents

MUX's agent data comes in two layers:

- **Configurable targets**: 45 individually verified product definitions, 44 of which have a stable user-level global config file that MUX can safely read and write.
- **Client directory**: sourced from public MCP client directories and the official client matrix, used for discovery only. After deduplication against the configurable targets, the UI can search **196** clients in total.

Clients whose global file path, top-level key, and entry structure have not been confirmed are shown for discovery only and cannot be written to. This keeps expanding coverage without writing a generic JSON guess into an unknown product's config.

MUX Desktop treats MCPs, Models, and Skills as central assets. Create, import, and maintain them in the top-level libraries first; an Agent page is the only place that chooses which compatible assets that Agent should consume. MCPs and Skills are `0..N` per Agent, while Model is `0..1`. Asset Inspectors own lifecycle actions and show impact without editing Agent relationships.

Agent files and Skill links are observed state, not an alternate asset database. MUX reconciles them as synced, pending, drifted, or conflicted. Scanning never silently takes ownership. When historical MCPs or Skills are detected, Desktop offers an explicit migration that imports the central asset and adopts its original Agent relationships as one recoverable per-asset transaction.

## Verified list

The results below are based on official docs, official source, or signed application bundles through **2026-07-22**. Grok Build was verified against xAI's official documentation; MiniMax Code was verified from the official signed `3.0.51` macOS bundle.

| Agent | Format | Config key | User-level global path | Native transports |
|---|---|---|---|---|
| [Amp](https://ampcode.com/manual#model-context-protocol-mcp) | JSON | `amp.mcpServers` | `~/.config/amp/settings.json` | stdio / http |
| [Amazon Q Developer IDE](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/mcp-ide.html) | JSON | `mcpServers` | `~/.aws/amazonq/default.json` | stdio / http |
| [Google Antigravity](https://antigravity.google/docs/mcp) | JSON | `mcpServers` | `~/.gemini/config/mcp_config.json` | stdio / http |
| [Augment Code](https://docs.augmentcode.com/cli/integrations) | JSON | `mcpServers` | `~/.augment/settings.json` | stdio / http |
| [BoltAI](https://docs.boltai.com/docs/plugins/mcp-servers) | JSON | `mcpServers` | `~/.boltai/mcp.json` | stdio |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code/mcp) | JSON | `mcpServers` | `~/.claude.json` | stdio / http |
| [Claude Desktop](https://modelcontextprotocol.io/quickstart/user) | JSON | `mcpServers` | `~/Library/Application Support/Claude/claude_desktop_config.json` | stdio |
| [Cline](https://docs.cline.bot/mcp/configuring-mcp-servers) | JSON | `mcpServers` | `~/.cline/data/settings/cline_mcp_settings.json` | stdio / http |
| [CodeBuddy Code](https://www.codebuddy.ai/docs/cli/mcp) | JSON | `mcpServers` | `~/.codebuddy/.mcp.json` | stdio / http |
| [CodeWhale](https://github.com/Hmbown/CodeWhale/blob/main/docs/MCP.md) | JSON | `servers` | `~/.codewhale/mcp.json` | stdio / http |
| [Codex](https://developers.openai.com/codex/mcp) | TOML | `mcp_servers` | `~/.codex/config.toml` | stdio / http |
| [Continue](https://docs.continue.dev/customize/deep-dives/mcp) | YAML | `mcpServers` | `~/.continue/config.yaml` | stdio / http |
| [GitHub Copilot CLI](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/extend-coding-agent-with-mcp) | JSON | `mcpServers` | `~/.copilot/mcp-config.json` | stdio / http |
| [Crush](https://github.com/charmbracelet/crush#model-context-protocol-mcp) | JSON | `mcp` | `~/.config/crush/crush.json` | stdio / http |
| [Cursor](https://docs.cursor.com/context/model-context-protocol) | JSON | `mcpServers` | `~/.cursor/mcp.json` | stdio / http |
| [Devin](https://docs.devin.ai/work-with-devin/mcp) | - | - | discovery only | - |
| [Factory Droid](https://docs.factory.ai/cli/configuration/mcp) | JSON | `mcpServers` | `~/.factory/mcp.json` | stdio / http |
| [Firebender](https://docs.firebender.com/context/mcp/overview) | JSON | `mcpServers` | `~/.firebender/firebender.json` | stdio / http |
| [Gemini CLI](https://geminicli.com/docs/tools/mcp-server/) | JSON | `mcpServers` | `~/.gemini/settings.json` | stdio / http |
| [Goose](https://goose-docs.ai/docs/guides/config-files/) | YAML | `extensions` | `~/Library/Application Support/Block/goose/config/config.yaml` | stdio / http |
| [Grok Build](https://docs.x.ai/build/features/mcp-servers) | TOML | `mcp_servers` | `~/.grok/config.toml` | stdio / http |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/features/mcp.md) | YAML | `mcp_servers` | `~/.hermes/config.yaml` | stdio / http |
| [JetBrains Junie](https://www.jetbrains.com/help/junie/model-context-protocol-mcp.html) | JSON | `mcpServers` | `~/.junie/mcp/mcp.json` | stdio / http |
| [Kilo Code CLI](https://kilo.ai/docs/automate/mcp/using-in-kilo-code) | JSON | `mcp` | `~/.config/kilo/kilo.jsonc` | stdio / http |
| [Kimi Code CLI](https://moonshotai.github.io/kimi-code/en/customization/mcp) | JSON | `mcpServers` | `~/.kimi-code/mcp.json` | stdio / http |
| [Kiro](https://kiro.dev/docs/mcp/configuration/) | JSON | `mcpServers` | `~/.kiro/settings/mcp.json` | stdio / http |
| [LM Studio](https://lmstudio.ai/docs/app/plugins/mcp) | JSON | `mcpServers` | `~/.lmstudio/mcp.json` | stdio / http |
| [MiniMax Code](https://agent.minimax.io/download) | JSON | `mcpServers` | `~/.mavis/mcp.json` | stdio / http |
| [Mistral Vibe](https://docs.mistral.ai/vibe/code/cli/mcp-servers) | TOML | `mcp_servers` | `~/.vibe/config.toml` | stdio / http |
| [OpenCode](https://opencode.ai/docs/mcp-servers/) | JSON | `mcp` | `~/.config/opencode/opencode.json` | stdio / http |
| [OpenHands CLI](https://docs.openhands.dev/openhands/usage/cli/mcp-servers) | JSON | `mcpServers` | `~/.openhands/mcp.json` | stdio / http |
| [Pi Coding Agent (MCP Adapter)](https://github.com/nicobailon/pi-mcp-adapter) | JSON | `mcpServers` | `~/.pi/agent/mcp.json` | stdio / http |
| [Qoder Desktop](https://docs.qoder.com/user-guide/chat/model-context-protocol) | JSON | `mcpServers` | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` | stdio / http |
| [Qoder CLI](https://docs.qoder.com/en/cli/mcp-servers) | JSON | `mcpServers` | `~/.qoder/settings.json` | stdio / http |
| [QoderWork](https://docs.qoder.com/qoderwork/connectors) | JSON | `mcpServers` | `~/.qoderwork/mcp.json` | stdio / http |
| [Qwen Code](https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/) | JSON | `mcpServers` | `~/.qwen/settings.json` | stdio / http |
| [Roo Code](https://docs.roocode.com/features/mcp/using-mcp-in-roo) | JSON | `mcpServers` | `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | stdio / http |
| [Atlassian Rovo Dev CLI](https://support.atlassian.com/rovo/docs/connect-to-an-mcp-server-in-rovo-dev-cli/) | JSON | `mcpServers` | `~/.rovodev/mcp.json` | stdio / http |
| [Stakpak](https://github.com/stakpak/agent#mcp-proxy-server) | TOML | `mcpServers` | `~/.stakpak/mcp.toml` | stdio / http |
| [Tabnine](https://docs.tabnine.com/main/getting-started/tabnine-agent/mcp-intro-and-setup) | JSON | `mcpServers` | `~/.tabnine/mcp_servers.json` | stdio / http |
| [Visual Studio Code](https://code.visualstudio.com/docs/copilot/chat/mcp-servers) | JSON | `servers` | `~/Library/Application Support/Code/User/mcp.json` | stdio / http |
| [VT Code](https://github.com/vinhnx/VTCode/blob/main/docs/guides/mcp-integration.md) | TOML | `mcp.providers` | `~/.vtcode/vtcode.toml` | stdio / http |
| [Warp](https://docs.warp.dev/knowledge-and-collaboration/mcp) | JSON | `mcpServers` | `~/.warp/.mcp.json` | stdio / http |
| [Windsurf](https://docs.windsurf.com/windsurf/cascade/mcp) | JSON | `mcpServers` | `~/.codeium/windsurf/mcp_config.json` | stdio / http |
| [Zed](https://zed.dev/docs/ai/mcp) | JSON | `context_servers` | `~/.config/zed/settings.json` | stdio / http |

### Targets that need special distinction

- **Pi**: Pi's core does not include MCP. MUX's definition applies only to environments with the community `pi-mcp-adapter` installed, so the UI clearly labels it a community extension.
- **Qoder Desktop / Qoder CLI**: these are separate Agents. Qoder Desktop edits `SharedClientCache/mcp.json`, while Qoder CLI's user scope uses `~/.qoder/settings.json`; MUX scans and writes them independently.
- **Devin**: the product supports MCP, but no stable user-level global file contract was verified, so it can only be viewed for discovery and not written to.
- **QoderWork**: user-defined MCP servers live in `~/.qoderwork/mcp.json`; MUX does not modify the client's built-in MCP data.
- **Claude Desktop / BoltAI**: the local files listed natively support stdio only. Remote MCP is managed by Claude Connectors or BoltAI's `mcp-remote` approach, respectively.
- **Goose**: the generic docs example uses `~/.config/goose/config.yaml`, but the current macOS source actually uses `~/Library/Application Support/Block/goose/config/config.yaml`; MUX locates it by the runtime code.
- **Grok Build**: MCP and custom models share `~/.grok/config.toml`. MUX separately manages `mcp_servers`, `[models].default`, and one dedicated MUX model table across all three documented API backends while preserving other models, auth, timeout, permission, and tool settings. Authentication writes only an `env_key` name, never a secret value.
- **MiniMax Code**: the main and MCP configurations are separate at `~/.mavis/config.yaml` and `~/.mavis/mcp.json`. MUX safely manages `mcpServers`; Models remains guided because the current custom-provider flow persists `options.apiKey` as plaintext YAML.

## Skills capabilities

Skills paths are verified separately from the MCP config paths in the table above; MUX never infers one from the other. MUX currently declares Skills capabilities for 36 audited Agents with stable user-level contracts, and shows only Agents whose local installation probes succeed.

Skills assignments operate on physical directories, not Agent names. Cursor, Gemini CLI, OpenCode, and GitHub Copilot CLI may all read the `~/.agents/skills` compatibility directory, so an operation on Codex's preferred directory can affect several installed Agents. MUX shows the real impact during review and normalizes duplicate links. See [User-level Skills](/en/guide/skills#verified-agent-paths) for the path matrix, installation sources, background safety checks, and current boundaries.

## Format differences across agents

MUX does not treat every client as the same `mcpServers` JSON:

- OpenCode / Kilo use `type: local|remote`, with a local `command` as an array.
- Codex uses TOML tables and `http_headers`; Grok Build uses `mcp_servers` TOML tables and `headers`; Mistral Vibe uses a `[[mcp_servers]]` TOML list.
- Continue uses a YAML list and requires root-level `name`, `version`, and `schema`; Goose and Hermes also use their own YAML maps.
- Gemini / Qwen use `httpUrl`; Windsurf and Antigravity use `serverUrl`.
- Cline puts connection fields in a `transport` sub-object; Tabnine puts HTTP headers in `requestInit.headers`.
- Rovo, Amazon Q, Augment, OpenHands, etc. require an explicit transport type; Kimi / Hermes only write `transport: sse` for legacy SSE.

Each built-in target has its own codec. On upgrade, MUX updates the official schema metadata but preserves your choices for enabled state and global path.

## Safe-write boundary

MUX parses agent files locally, but only provides the structured connection fields of the target MCP entry to the UI. The complete config file never enters the UI, logs, source cache, or the network, and MUX never overwrites your config by "deserializing the whole file and rewriting it."

- JSON / JSONC use a syntax tree to locate the target entry, preserving comments, indentation, key order, other servers, and other top-level settings.
- Both TOML maps and TOML lists are edited locally; YAML maps / lists likewise preserve unmanaged content and comments.
- Agent-private fields like `enabled`, OAuth, timeouts, tool allowlists, and approval policies are preserved as-is.
- Writes are refused on an invalid document, a wrong node type, duplicate target keys, a YAML multi-document file, a failed backup, or a concurrent modification.
- A timestamped independent backup is created before writing (directory `0700`, file `0600` on Unix), and the final replacement is atomic via a temp file in the same directory; symlink targets and the original config file's permissions are left unchanged.

MUX currently manages only user-level global config and does not offer project-level writes.

## Custom agents

Click `+` next to the desktop app's agent selector, or press `n` in the TUI's Agents screen, to add a custom JSON, TOML, or YAML global target. Custom targets use the standard map layout; only verified built-in targets enable product-specific field conversion. Built-in targets allow only overriding the path, to avoid accidentally turning an official schema into an incompatible format.

Next → [FAQ](/en/guide/faq)
