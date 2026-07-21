# MUX catalog-only Agent capability audit — N–Z

> Audit date: 2026-07-22
>
> Scope: IDs that were not in `data/agents.json` at audit start, sorted `navigator` through `zin-mcp-client`; identities promoted in this release remain in this shard for provenance.
>
> Evidence rule: official repository/source and official product documentation are authoritative; package/release artifacts are supporting evidence; community pages are discovery leads only. A missing public filesystem contract is recorded as **not verified**, never inferred from a screenshot or marketing claim.

## Status and decision vocabulary

- **audited-writable**: official evidence proves at least one local filesystem contract with a plausible lossless adapter. This is a research candidate classification, not proof that the current MUX release has a path resolver, codec, scope model, or writer wired for it.
- **audited-read-only**: official capability exists, but it is cloud-managed, database-managed, extension-storage-only, or lacks a stable public config contract.
- **catalog-only**: identity or capability evidence is too weak/ambiguous for promotion.
- Model and Skill support are evaluated independently from MCP. “Supports custom models” does not prove a file-backed Model adapter; “supports prompts/instructions” does not prove Agent Skills compatibility.

## Inventory checkpoint

52 identities that were catalog-only at audit start are in scope:

| Batch | IDs | Research state |
|---|---|---|
| N–O | `navigator`, `needle`, `nerve`, `nextchat`, `nova`, `nvidia-agent-intelligence-aiq-toolkit`, `ollamac-pro`, `open-webui`, `opensumi`, `oterm` | complete |
| P–R | `poolside`, `posthog-code`, `postman`, `proxyman`, `qodo`, `qordinate`, `ravenala`, `raycast`, `recurse-chat`, `replit`, `replit-agent`, `rtrvr-ai`, `runbear` | complete |
| S | `seekchat`, `sema4`, `shelbula`, `shortwave`, `sigit`, `simtheory`, `slack-mcp-client`, `smithery-playground`, `spinai`, `superinterface`, `superjoin`, `swarms`, `systemprompt` | complete |
| T–Z | `tambo`, `tencent-cloudbase-ai-devkit`, `tester-mcp-client`, `theiaai-theiaide`, `tiles-notebook`, `tome`, `trae-agent`, `trae-ide`, `typingmind-app`, `v0`, `visual-studio`, `whatsmcp`, `witsy`, `y-cli`, `zencoder`, `zin-mcp-client` | complete |

## Per-Agent evidence

Each entry records: official identity/aliases; install probes; global/project paths; MCP schema; Model schema/switching/credentials; Skills directories; MUX write suitability; evidence and verification state.

<!-- Entries are appended as each source is verified. -->

### `navigator` — Navigator (identity collision; catalog-only)

- **Official identity / aliases:** the catalog label cannot be bound confidently to a current public product. The current Navigator Labs site describes persistent-memory chat apps and points to `github.com/navigatorlabs`, but that organization was archived on 2026-07-19 and has no public repositories. This is insufficient to prove it is the older Glama-listed MCP client.
- **Install probes:** none verified. Do not probe a generic `navigator` command because it collides with unrelated packages and system concepts.
- **Global / project config:** no official filesystem path found.
- **MCP:** the catalog/Glama listing is only a discovery lead; no official schema, transport, enable/disable or credential contract was recovered.
- **Models:** no official provider/model schema or persistent switch contract recovered.
- **Skills:** no Agent Skills directory recovered.
- **MUX suitability:** **catalog-only**. Keep display-only; do not create a writable entry until the product identity and an official/local source contract are recovered.
- **Evidence:** [current product site](https://getnavigator.app/), [archived GitHub organization](https://github.com/navigatorlabs), [catalog lead](https://glama.ai/mcp/clients/navigator).
- **Verified:** identity conflict checked 2026-07-22; configuration not verified.

### `needle` — Needle Web (audited-read-only)

- **Official identity / aliases:** Needle Web at `needle.app`; distinct from the official `needle-ai/needle-mcp` **server** repository.
- **Install probes:** none: the client is a hosted web application. A local `needle-mcp` command would detect its server, not the client.
- **Global / project config:** none. MCP connectors are created in the hosted UI at `Collections → Connectors → New Connector`.
- **MCP:** remote URL connector with OAuth authorization; the official client guide does not expose a local config file or import/export schema. The separate Needle server supports Streamable HTTP/SSE and local stdio, but that is not a client storage contract.
- **Models:** hosted product behavior; no local provider/model schema or persistent file switch was published.
- **Skills:** no Agent Skills filesystem directory published.
- **MUX suitability:** **audited-read-only**. MUX can link to docs, but must not write a guessed Needle config. Do not confuse the server API key with client credentials.
- **Evidence:** [official MCP client guide](https://docs.needle.app/docs/guides/mcp/getting-started/), [official MCP server guide](https://docs.needle.app/docs/guides/mcp/needle-mcp-server/), [official server source](https://github.com/needle-ai/needle-mcp).
- **Verified:** official docs checked 2026-07-22.

### `nerve` — `evilsocket/nerve` / Nerve ADK (audited-read-only)

- **Official identity / aliases:** `evilsocket/nerve`, Python package `nerve-adk`, command `nerve`. The repository was archived on 2026-02-17. It must not be confused with the newer unrelated `daggerhashimoto/openclaw-nerve` cockpit.
- **Install probes:** command `nerve`; path `~/.nerve`. Official install: `pip install nerve-adk`.
- **Global / project config:** Nerve loads installed agent definitions from `~/.nerve/agents`, then falls back to the current directory. Each agent is a directory or YAML file (`agent.yml` / `task.yml`); there is no single global MCP registry. Reusable prompts live in `~/.nerve/prompts`.
- **MCP:** per-agent YAML map under `mcp`. Stdio fields: `command`, `args`, `env`, `session_timeout`; remote fields: `url`, `headers`, `timeout`, `read_timeout`. Plain `http(s)://` means legacy SSE; `stream://` prefix means Streamable HTTP. Nerve can also expose an agent with `nerve serve <agent> --mcp`.
- **Models:** per-agent `generator`, e.g. `openai/gpt-4o`; LiteLLM supplies providers and provider environment variables. There is no global profile inventory/current-model pointer; switching means editing an individual agent YAML or using a CLI override.
- **Skills:** no Agent Skills loader. `~/.nerve/prompts` and installed agent recipes are different concepts and must not be surfaced as SKILL.md assets.
- **MUX suitability:** **audited-read-only** for the current architecture. A future project/recipe adapter could edit one explicitly selected agent definition, but a global MUX write would fan out ambiguously across arbitrary YAML files. The archived upstream also raises maintenance risk.
- **Evidence:** [official archived source](https://github.com/evilsocket/nerve), [`docs/mcp.md`](https://github.com/evilsocket/nerve/blob/3c8b9c351948d01c7113925213515ddfee52802b/docs/mcp.md), [`docs/index.md`](https://github.com/evilsocket/nerve/blob/3c8b9c351948d01c7113925213515ddfee52802b/docs/index.md), source `nerve/defaults.py` and `nerve/models.py` at audited commit `3c8b9c3`.
- **Verified:** official source cloned and searched 2026-07-22.

### `nextchat` — ChatGPTNextWeb/NextChat (audited-read-only)

- **Official identity / aliases:** `ChatGPTNextWeb/NextChat`, formerly ChatGPT-Next-Web; self-hosted Web/PWA plus packaged desktop clients.
- **Install probes:** no reliable cross-install command. Possible app probe `/Applications/NextChat.app` is identity-only and does not establish a writable config location.
- **Global / project config:** MCP is enabled at build/deploy time with `ENABLE_MCP=true`. Current source hard-codes `path.join(process.cwd(), "app/mcp/mcp_config.json")`; this is deployment/source-tree state, not a stable per-user path. Browser preferences/models are persisted by the client store; deployments may also supply environment variables.
- **MCP:** JSON `{ "mcpServers": { name: { command, args, env?, status? }}}`; current `ServerConfig` is stdio-only. UI mutations rewrite the server-side application file. There is no supported user-global path or lossless desktop import contract.
- **Models:** deployment env includes provider keys/URLs, `CUSTOM_MODELS`, and `DEFAULT_MODEL`; per-user selected model/provider are client-store state (`modelConfig.model`, `providerName`). This mixes deploy-time and browser-local scopes and is not a MUX file-backed profile/current-model contract.
- **Credentials:** provider secrets are deployment env or browser-entered settings; MCP env values may be stored in the server file. MUX must not inject secrets into a guessed deployed tree.
- **Skills:** no Agent Skills loader or SKILL.md directory found. NextChat masks/prompts are product-native templates, not Agent Skills.
- **MUX suitability:** **audited-read-only**. A future NextChat deployment integration would need an explicit deployment root/API and scope selection; do not treat `app/mcp/mcp_config.json` as `~` global config.
- **Evidence:** [official source](https://github.com/ChatGPTNextWeb/NextChat), source `app/mcp/actions.ts`, `app/mcp/types.ts`, `app/store/config.ts`, and README env reference at audited commit `706a18b`.
- **Verified:** official source cloned and searched 2026-07-22.

### `nova` — Compass Agentic Platform Nova CLI (audited-read-only)

- **Official identity / aliases:** `Compass-Agentic-Platform/nova`, npm package `@compass-ai/nova`, command `nova`; this is the Compass Nova coding-agent CLI, not the unrelated Panic Nova editor or Amazon Nova model family.
- **Install probes:** command `nova`; official install `npm install -g @compass-ai/nova`; `nova setup` configures providers/preferences and `npx @compass-ai/nova@latest acp` starts its ACP server.
- **Global / project config:** the public repository ships documentation and release notes but not the implementation or a versioned config reference. The changelog mentions project settings and global/project `.compass/skills/`, yet does not resolve the global directory to an absolute home path or publish a complete configuration schema.
- **MCP:** current release notes prove an MCP client with stdio/Streamable HTTP-SSE behavior, OAuth/token authentication, reconnect/auth commands, built-in servers and per-agent/project restrictions. No authoritative MCP file path, serialized server schema, merge precedence, enable/disable field or secret-storage contract is public.
- **Models / credentials:** Nova supports multiple hosted/local providers, `nova setup`, multi-provider API keys, runtime `/model` switching and plan/subscription-dependent fallback. Public material does not expose the provider inventory, active-model persistence, base-URL schema or credential backend; some configuration is synchronized from Compass.
- **Skills:** release notes say skills are discovered from global and project `.compass/skills/` directories and describe markdown files, while the README also calls the feature Agent Skills. Without source or a current format/path reference, MUX cannot safely assume `$HOME/.compass/skills/<name>/SKILL.md` or write into an ambiguous project root.
- **MUX suitability:** **audited-read-only**. Detect the command and link to setup/MCP/Skills guidance, but do not invent paths from release-note shorthand. Promote only after Compass publishes the implementation or a versioned storage/import contract.
- **Evidence:** [official repository](https://github.com/Compass-Agentic-Platform/nova), repository README and CHANGELOG at audited commit `5db8a29`.
- **Verified:** official repository cloned and searched 2026-07-22; capabilities verified, writable storage contract not verified.

### `nvidia-agent-intelligence-aiq-toolkit` — NVIDIA NeMo Agent Toolkit (audited-read-only; rename required)

- **Official identity / aliases:** NVIDIA AIQ Toolkit was renamed to **NVIDIA NeMo Agent Toolkit**; repository redirects from `NVIDIA/AIQToolkit` to `NVIDIA/NeMo-Agent-Toolkit`. Current CLI is `nat` (historical `aiq`). The catalog name is stale.
- **Install probes:** command `nat`; historical command `aiq` only as a compatibility lead. No single user config directory is the authoritative workflow inventory.
- **Global / project config:** workflows are explicit YAML files selected by `nat run --config_file ...`; examples package their own `configs/config.yml`. Configuration variants may inherit another YAML and CLI `--override` can change fields. There is no canonical global MCP/models file.
- **MCP:** project/workflow YAML uses a function group such as `function_groups.<name>._type: mcp_client`; transport-specific fields include server URL/command arguments, tool filters and authentication options. It is a workflow component, not a top-level `mcpServers` user registry.
- **Models:** workflow YAML `llms.<name>` entries use `_type` plus `model_name` and provider-specific endpoint/options; workflow/agent sections reference a named LLM. Switching is workflow edit or `--override`, not one persistent global current pointer. Keys are provider environment variables or config-gated secret fields.
- **Skills:** current repository publishes **coding-agent skills** under repository `skills/` for Claude/Codex and similar hosts. Official instructions copy them into those other agents' skill directories; NAT itself does not consume a dedicated user-level SKILL.md directory. A2A “skills” are protocol capabilities, a different concept.
- **MUX suitability:** **audited-read-only** under current global Agent adapters. Rename the display identity, docs and probes, but do not write arbitrary workflow YAML without a future project/workflow selector and typed NAT adapter.
- **Evidence:** [official repository/releases](https://github.com/NVIDIA/NeMo-Agent-Toolkit), [MCP client docs](https://docs.nvidia.com/nemo/agent-toolkit/latest/build-workflows/mcp-client.html), [LLM configuration docs](https://docs.nvidia.com/nemo/agent-toolkit/latest/build-workflows/llms/index.html), [official coding-agent skills docs](https://docs.nvidia.com/nemo/agent-toolkit/latest/resources/contributing/agent-skills.html), audited source commit from 2026-07-22 clone.
- **Verified:** official docs, release redirect, and source searched 2026-07-22.

### `ollamac-pro` — Ollamac Pro (audited-read-only)

- **Official identity / aliases:** commercial native macOS app “Ollamac Pro” (the docs sometimes shorten it to Ollamac/OllamaC); not the unrelated open-source Ollamac clients.
- **Install probes:** `/Applications/Ollamac Pro.app` is a reasonable identity probe; no local installation was present on the audited machine. Do not add a command probe.
- **Global / project config:** official docs expose MCP servers, Ollama server profiles and model selection through Settings/UI, but publish no supported filesystem config/import schema. The roadmap describes “Remote config” as future/planned, reinforcing that no stable external management contract is documented.
- **MCP:** local and remote servers are supported; chats select enabled servers. Official public docs do not specify serialized fields, transport encoding, database location or safe write lifecycle.
- **Models:** local/remote Ollama server profiles and per-chat model/parameter switching are UI-managed. Default local endpoint is `http://localhost:11434`; no stable file-backed profile/current pointer is published.
- **Credentials:** remote-server authentication is UI-managed; storage/Keychain behavior is not documented sufficiently for external writes.
- **Skills:** no Agent Skills/SKILL.md loader documented.
- **MUX suitability:** **audited-read-only**. Detect the app and link to docs; do not reverse-engineer private app storage into a default writable adapter without an official export contract or bundle audit plus versioned migration tests.
- **Evidence:** [official product](https://ollamac.com/), [official setup/model docs](https://ollamac.com/docs/getting-started/setup), [official MCP usage](https://ollamac.com/docs/mcp/usage), [official roadmap](https://ollamac.com/roadmap).
- **Verified:** official docs and local-install absence checked 2026-07-22.

### `open-webui` — Open WebUI (audited-read-only)

- **Official identity / aliases:** `open-webui/open-webui`, Python/Docker service `open-webui`; hosted/self-hosted multi-user web application.
- **Install probes:** command `open-webui`, Docker container/image, or deployment URL are only install/runtime probes. A local data directory does not imply file-backed config ownership.
- **Global / project config:** MCP and model connections are administered through the Web UI/API and persisted in Open WebUI's application database/config state. They are not represented by a public `mcpServers` file. `WEBUI_SECRET_KEY` is required to preserve encrypted OAuth tokens across container recreation.
- **MCP:** v0.6.31+ supports **Streamable HTTP only**, admin-created at `Admin Settings → External Tools`. Fields include URL, auth mode (none/bearer/OAuth 2.1/static), OAuth metadata/scopes/resource behavior, custom headers and function-name filters. Stdio/SSE are not native; older `mcpo` is a bridge. Official docs explicitly warn that pasting `mcpServers` JSON is invalid.
- **Models:** connections/models are admin/API/database-managed (Ollama and OpenAI-compatible connections, per-model metadata/default tools). There is no stable local profile file/current-model pointer suitable for MUX's filesystem transaction adapter.
- **Credentials:** secrets/OAuth tokens are encrypted with deployment secret and user/admin state. External file mutation would bypass access control and encryption.
- **Skills:** Open WebUI tools/functions/prompts are product plugins, not an Agent Skills SKILL.md directory.
- **MUX suitability:** **audited-read-only** unless MUX later gains an authenticated Open WebUI Admin API connector. Never write its SQLite/database files directly.
- **Evidence:** [official MCP docs](https://docs.openwebui.com/features/extensibility/mcp/), [official source](https://github.com/open-webui/open-webui).
- **Verified:** official docs/source identity checked 2026-07-22.

### `opensumi` — OpenSumi framework (catalog-only as an Agent identity)

- **Official identity / aliases:** `opensumi/core` is an IDE framework/SDK used to build Web/Electron IDE products, not one canonical installed end-user Agent. `opensumi/ide-electron` and `ide-startup` are samples/distributions with host-controlled modules.
- **Install probes:** developer CLI `sumi` and `~/.sumi-dev/extensions` detect the SDK development environment, not an MCP-capable Agent product.
- **Global / project config:** host applications choose storage, extension directories and modules. `.sumi/launch.json` is an extension debug file, not an MCP registry.
- **MCP / Models / Skills:** no official core-user contract was found for a universal OpenSumi MCP registry, model provider profile, model switch, or Agent Skills directory. A downstream product based on OpenSumi may define all of these differently.
- **MUX suitability:** **catalog-only** and arguably remove/replace with concrete downstream products. A framework-level writable entry would point multiple incompatible hosts at invented paths.
- **Evidence:** [official framework docs](https://opensumi.com/en/docs/), [official source](https://github.com/opensumi/core), [official extension quick start](https://opensumi.com/en/docs/extension/quick-start/).
- **Verified:** official docs/source searched 2026-07-22; no universal capability contract found.

### `oterm` — oterm terminal client (audited-writable candidate: MCP; partial Models)

- **Official identity / aliases:** `ggozad/oterm`, Python app/command `oterm`; official installation can be run with `uvx oterm`.
- **Install probes:** command `oterm`; resolved data directory printed by `oterm --data-dir`. On macOS default directory is `~/Library/Application Support/oterm`; `OTERM_DATA_DIR` overrides it and `XDG_DATA_HOME/oterm` is honored.
- **Global / project config:** macOS canonical file `~/Library/Application Support/oterm/config.json`; no project config. Stateful chats/current per-chat provider/model live in sibling `store.db`, and must not be edited by MUX.
- **MCP:** JSON map at `mcpServers`. Stdio fields `command`, `args`, `env`; HTTP fields `url`, `headers`. URL ending `/sse` selects legacy SSE; other HTTP URLs use Streamable HTTP. Environment interpolation supports `${VAR}` and `${VAR:-default}`. Current docs say `cwd` is no longer recognized even though an internal helper still accepts it; MUX should not author `cwd`, but must preserve unknown fields losslessly. No WebSocket support.
- **Models:** `openaiCompatible.<id>` entries accept required `base_url` and optional `api_key` (prefer `${ENV_KEY}`). Built-in providers are activated by environment variables (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.); provider/model choice is per chat and persisted in SQLite, so MUX can safely manage compatible **connections** but cannot claim a global active/default model switch. OpenAI Chat and Responses are distinct provider IDs.
- **Credentials:** built-in provider keys are env-only; custom compatible endpoints may use an env interpolation reference or literal. MUX should require/reference an env key and never read/write `store.db` or silently copy a central secret into JSON.
- **Skills:** no Agent Skills/SKILL.md loader found. Built-in tools and MCP tools are not Skills.
- **MUX suitability:** promote to **audited-writable MCP** using standard JSON map codec at the resolved macOS path, with a note about `OTERM_DATA_DIR`. Model support should be limited to `openaiCompatible` connection materialization until MUX can represent “installed connection but no global active pointer.”
- **Evidence:** [official source](https://github.com/ggozad/oterm), [official app configuration](https://ggozad.github.io/oterm/app_config/), [official MCP reference](https://ggozad.github.io/oterm/mcp/), source `src/oterm/config.py` and `src/oterm/tools/mcp/setup.py` at audited commit `aac9672`.
- **Verified:** official source cloned and searched 2026-07-22.

### `poolside` — Poolside `pool` coding agent (audited-writable candidate: MCP and Skills)

- **Official identity / aliases:** `poolsideai/pool`, Poolside's coding-agent CLI/ACP agent and client; command `pool`. It is distinct from the Poolside inference service/model names used behind the product.
- **Install probes:** command `pool`; official signed/checksummed download script for macOS/Linux and PowerShell installer for Windows. `pool config` prints the effective config, credential, log and trajectory directories; `pool acp` is the ACP-server command.
- **Global / project config:** user configuration is `~/.config/poolside/settings.yaml`; project overrides are `.poolside/settings.yaml` and gitignored `.poolside/settings.local.yaml`, with the more specific file winning. Credentials are separate at `~/.config/poolside/credentials.json`; `POOLSIDE_API_KEY` takes precedence for automation.
- **MCP:** YAML map `mcp_servers`. Official `pool mcp add/get/list/remove` supports stdio (`command`, `args`, optional `env`) and remote `transport: { type: http|sse, url, headers? }`. An isolated run of the checksummed official v1.0.14 artifact reproduced that schema and a `.backup` sidecar. No persistent enable/disable field is documented, so MUX should expose add/remove rather than synthesize a toggle.
- **Models / credentials:** OpenRouter login, Ollama launch, ACP model selection and OpenAI-compatible env overrides are supported. Model/mode may be persisted in `settings.yaml`, but the public reference does not document a provider inventory or stable active-model field; the standalone endpoint uses `POOLSIDE_STANDALONE_BASE_URL`, `POOLSIDE_API_KEY` and optionally `POOLSIDE_STANDALONE_MODEL`. Keep Models read-only until that schema is public. MCP env/header values are inline YAML and must not be populated from MUX secrets implicitly.
- **Skills:** the agent follows the Agent Skills specification. Current changelog documents user-global `~/.agents/skills`; project discovery uses `.agent/skills`. These are one-level shared/project roots and should use duplicate-name and project-trust checks.
- **MUX suitability:** promote to **audited-writable user/project MCP and Skills** using a lossless YAML map codec. Resolve scope explicitly, preserve unknown fields/comments and external secret material, create a backup/rollback plan, and verify through `pool mcp get/list`; Models stay read-only.
- **Evidence:** [official repository](https://github.com/poolsideai/pool), README/CHANGELOG at audited commit `39e9094`; official v1.0.14 macOS arm64 artifact and published checksum (`a388ac305da89dfe831e22986c9b78ea10d00347aa5465c398f26c67a4fd3b44`) checked 2026-07-22.
- **Verified:** official docs plus isolated artifact-generated stdio/HTTP/SSE fixtures verified 2026-07-22.

### `posthog-code` — PostHog Code (audited-read-only)

- **Official identity / aliases:** PostHog Code is PostHog's proactive desktop coding agent. It is distinct from the official PostHog remote MCP server, `posthog-mcp`, and from the `@posthog/cli` command. Those adjacent products are not installation/configuration probes for this desktop Agent.
- **Install probes:** a signed PostHog Code desktop application may be used as an identity probe when its stable bundle ID is verified from an installed build. No official CLI command or public user-config root was found.
- **Global / project config:** the public product documentation explains repository access and product behavior but does not publish a supported global/project filesystem schema for MCP servers, model profiles, or Agent Skills. Product state is managed by the desktop/service UI.
- **MCP:** PostHog publishes an official MCP **server** for letting other agents query PostHog. That does not prove PostHog Code is an MCP client or stores a writable `mcpServers` registry. No client-side schema/path was recovered.
- **Models / credentials:** no official local provider/base-URL/model-switch schema was recovered. Authentication and runtime model policy are product-managed.
- **Skills:** PostHog's public repository contains coding-agent guidance/skills for contributors, including material that explicitly distinguishes working through PostHog Code from working through MCP/IDE tools. That repository content is not evidence that the PostHog Code app consumes a user `SKILL.md` directory.
- **MUX suitability:** **audited-read-only**. Detect/link only after an official bundle identity is verified. Never map the PostHog MCP server's settings onto the PostHog Code client.
- **Evidence:** [official PostHog Code docs](https://posthog.com/docs/code), [official PostHog source](https://github.com/PostHog/posthog), [official PostHog MCP server](https://github.com/PostHog/mcp).
- **Verified:** official product/repository boundaries checked 2026-07-22; local configuration contract not published.

### `postman` — Postman desktop/web app (audited-read-only)

- **Official identity / aliases:** Postman API Platform desktop/web application. MCP request support is part of Postman collections/workspaces and Agent Mode, not a standalone Postman Agent CLI.
- **Install probes:** macOS `/Applications/Postman.app`; command-line tools such as `postman`, Newman, or Postman CLI are adjacent automation tools and do not establish the desktop app's MCP/model storage schema.
- **Global / project config:** MCP requests are created and saved through Postman's UI in collections/workspaces. Workspaces may synchronize to Postman Cloud. The official MCP docs do not define a canonical global `mcpServers` file or a filesystem-safe import/write contract.
- **MCP:** Postman can connect to MCP servers, configure supported transports/authentication in a request, inspect tools/resources/prompts, and save requests into collections. Agent Mode can also use configured MCP servers. Serialized collection data is a Postman domain model rather than the standard client registry map, and ownership/sync can be cloud or team-scoped.
- **Models / credentials:** AI requests and Agent Mode use Postman's product/provider configuration. No supported file-backed provider inventory/current-model pointer or external secret-write contract was found.
- **Skills:** Postman collections, Flows, scripts, prompts, and MCP resources are not Agent Skills; no `SKILL.md` directory is documented.
- **MUX suitability:** **audited-read-only**. A future integration should use a documented Postman API/export workflow with workspace/collection selection, not mutate desktop app storage or synthesize a global config file.
- **Evidence:** [official MCP request overview](https://learning.postman.com/latest-v-12/docs/use/send-requests/protocols/mcp-requests/overview), [official MCP support announcement](https://blog.postman.com/postman-launches-full-support-for-model-context-protocol-mcp-build-better-ai-agents-faster/), [official Postman source organization](https://github.com/postmanlabs).
- **Verified:** official docs checked 2026-07-22.

### `proxyman` — Proxyman MCP server provider (misclassified; catalog-only as Agent)

- **Official identity / aliases:** `ProxymanApp/Proxyman`, native macOS HTTP debugging application. Its MCP feature exposes Proxyman **as an MCP server to an external AI client**; it is not an MCP host/Agent configuration manager.
- **Install probes:** `/Applications/Proxyman.app`; bundled server executable `/Applications/Proxyman.app/Contents/MacOS/mcp-server`. Setapp installs use `/Applications/Setapp/Proxyman.app/Contents/MacOS/mcp-server`.
- **Global / project config:** server enablement is controlled in Proxyman Settings. A per-session handshake/token file is written at `~/Library/Application Support/com.proxyman.NSProxy/mcp-handshake.json` with restrictive permissions. This is ephemeral security material, not a client registry and must never be imported, displayed, copied, or edited by MUX.
- **MCP:** server-side stdio integration. External clients register Proxyman's bundled `mcp-server` command. Proxyman itself does not publish an inbound `mcpServers` configuration schema.
- **Models / Skills:** none as an Agent host. No model-provider profiles/current switch or Agent Skills directory applies.
- **MUX suitability:** **catalog-only** as an Agent and recommended for removal from the Agent catalog. It may later appear in an MCP-server discovery catalog, using only the executable path and leaving the handshake lifecycle to Proxyman.
- **Evidence:** [official MCP documentation](https://docs.proxyman.com/mcp), [official GitHub organization/repository](https://github.com/ProxymanApp/Proxyman).
- **Verified:** official docs checked 2026-07-22; role classification is conclusive.

### `qodo` — Qodo IDE / Qodo Command family (audited-read-only globally; typed project adapter possible)

- **Official identity / aliases:** “Qodo” now covers Qodo IDE (formerly Qodo Gen IDE plugin), Qodo Command/CLI (earlier docs call it Qodo Gen CLI), and Git Integration (formerly Qodo Merge). The catalog URL currently points at Qodo Merge's server integration, which does not accurately describe the interactive IDE/CLI Agent.
- **Install probes:** CLI command `qodo`, npm packages documented historically as `@qodo/gen` and currently as Qodo Command; VS Code/JetBrains extension identity should be probed separately. `~/.qodo` proves Qodo authentication/history state, but must not be treated as the MCP inventory.
- **Global / project config:** Qodo Command accepts explicit `--agent-file=<path>` and `--mcp-file=<path>`. A project convention uses `agent.toml`, imported per-command TOML files under `agents/`, and `mcp.json`. Because paths can be arbitrary and selected by flags, there is no single safe user-global MCP target. Authentication is stored beneath `~/.qodo`; official changelog identifies `~/.qodo/history`. Secrets are outside the project MCP schema.
- **MCP:** reusable project `mcp.json` uses standard `{ "mcpServers": { ... } }`; stdio entries use `command`, `args`, `env`, while official agent examples also show remote `url` and `headers`. A per-agent TOML can embed the same JSON as a multiline `mcpServers` string and select tools via `available_tools`/`tools`. Qodo IDE also supports local and remote/SSE MCPs through its UI, with sub-tool enable/disable and approval settings, but no stable extension-storage file is published.
- **Models / credentials:** Qodo Command lists models with `qodo models`, overrides a run with `--model`, and can set `model = "..."` in top-level agent configuration. Qodo IDE modes persist a preferred model through UI/exported mode TOML. Qodo-managed SaaS models are not arbitrary provider profiles; on-prem model secrets/config are a separate administrative deployment contract and must not be folded into a desktop adapter.
- **Skills:** Qodo publishes `qodo-ai/qodo-skills`, but those skills are installed into the **target coding agent's** monitored directory with `npx skills add`; this does not prove that Qodo IDE/Command itself scans a Qodo-owned SKILL.md directory.
- **MUX suitability:** **audited-read-only** for the generic installed Agent. A future project-scoped adapter can safely manage an explicitly selected `mcp.json` and selected agent TOML, with path selection shown in review and no implicit write to `~/.qodo`. Do not promote a guessed global path.
- **Evidence:** [Qodo custom agents/MCP file](https://docs.qodo.ai/qodo-documentation/qodo-gen/cli/creating-and-managing-agents), [CLI flags](https://docs.qodo.ai/qodo-documentation/qodo-gen/qodo-gen-cli/list-of-cli-commands-and-flags), [official agent examples](https://github.com/qodo-ai/agents), [Qodo IDE MCP docs](https://docs.qodo.ai/qodo-ide/tools-mcps/agentic-tools-mcps), [Qodo Agent Skills](https://docs.qodo.ai/agent-skills), [official Qodo Command repository](https://github.com/qodo-ai/command).
- **Verified:** official docs and source searched 2026-07-22.

### `qordinate` — Qordinate hosted coordination assistant (audited-read-only)

- **Official identity / aliases:** Qordinate by Identimental Technologies, a hosted personal/team coordination assistant reached through WhatsApp, iMessage, Slack, Telegram, and its web app.
- **Install probes:** none on the local machine. Browser cookies, messaging apps, or a generic `qordinate` name are not valid probes.
- **Global / project config:** integrations and MCP connections are account/cloud state managed from Qordinate's web application. The official site publishes no local configuration/export schema.
- **MCP:** third-party directory screenshots indicate Qordinate can connect a remote MCP server; official product copy confirms a hosted integrations layer but does not document its serialized MCP fields or a filesystem location. Treat the directory listing as discovery evidence only.
- **Models / credentials / Skills:** product-managed cloud agent; no supported local provider profile, model switch file, secret store contract, or SKILL.md directory was found.
- **MUX suitability:** **audited-read-only**. Link to the official application; a future connector would require a documented Qordinate API, not browser/session-storage mutation.
- **Evidence:** [official Qordinate site](https://www.qordinate.ai/), [Glama discovery lead](https://glama.ai/mcp/clients/qordinate).
- **Verified:** official identity and hosted architecture checked 2026-07-22; capability serialization not verified.

### `ravenala` — Ravenala hosted AI OS (catalog-only)

- **Official identity / aliases:** the Glama listing describes Ravenala as a paid web “AI-first operating system” that runs agents/apps/MCP tools. Its linked product domain currently redirects without exposing crawlable public technical documentation, and no official repository was recovered.
- **Install probes:** none; web-only according to the discovery record.
- **Global / project config:** no official local path or export schema found.
- **MCP:** directory claims broad client features, but it does not publish transport-specific serialized fields, ownership, credentials, enablement, or an API. These claims are not enough for an audited adapter.
- **Models / Skills:** no official provider/current-model contract or SKILL.md directory recovered.
- **MUX suitability:** **catalog-only**. Retain only as an unverified directory lead or remove until Ravenala publishes stable official documentation/API evidence.
- **Evidence:** [product domain](https://ravenala.ai/), [Glama discovery lead](https://glama.ai/mcp/clients/ravenala).
- **Verified:** discovery lead checked and official technical evidence searched 2026-07-22; configuration unverified.

### `raycast` — Raycast AI (audited-writable candidate: Skills; MCP/Models read-only)

- **Official identity / aliases:** Raycast desktop launcher and its Raycast AI surfaces (Quick AI, AI Chat, Agents, AI Commands). Agents were called Presets in Raycast v1. Current Raycast v2 documentation now includes native MCP Servers and Agent Skills.
- **Install probes:** macOS `/Applications/Raycast.app`; bundle/process identity preferred. CLI/script commands from Raycast extensions are not reliable Agent probes.
- **Global / project config:** Raycast MCP servers, AI Agents, models, commands and encrypted export are application/account state managed in Settings. The supported `.rayconfig` export is passphrase-encrypted and includes MCP servers; it is a backup/import boundary, not a direct JSON registry.
- **MCP:** native desktop supports stdio (`command`, arguments as string or JSON array, environment) and HTTP/Streamable HTTP (`url`, headers, dynamic/static OAuth, client ID/secret/scopes), plus description/custom instructions and permissions. Tokens are encrypted per server. Official docs publish UI fields but no stable plaintext storage path/schema, so MUX must not reverse-engineer the application database.
- **Models / credentials:** Raycast manages default models independently for Quick AI, AI Chat and AI Commands; conversations can switch models, while an Agent can lock a model or inherit the chat model. BYOK supports provider keys through Settings and stores them securely; only Raycast-available models are exposed. No arbitrary base-URL profile file or safe external secret-write contract exists.
- **Skills:** **verified writable filesystem contract.** Raycast scans `~/.claude/skills`, `~/.config/agents/skills`, `~/.config/raycast/skills`, and `~/.agents/skills`, plus user-added folders. Each direct child must contain case-sensitive `SKILL.md`; folder name must match frontmatter `name`; scanning is one level deep and cached about 60 seconds. Duplicate names keep the first discovered copy. `~/.config/raycast/skills` is the clean dedicated target; `~/.agents/skills` is preferable when the user intentionally wants a shared canonical library.
- **MUX suitability:** promote to **audited-writable Skills**, with shared-reader semantics and duplicate-name checks. Keep MCP and Models **audited-read-only** until Raycast publishes an external management API/plaintext config contract. MUX must distinguish one skill-directory write from multiple Agents reading it.
- **Evidence:** [official MCP manual](https://manual.raycast.com/ai/model-context-protocol), [official Skills manual](https://manual.raycast.com/ai/skills), [official Agents manual](https://manual.raycast.com/ai/agents), [model switching](https://manual.raycast.com/ai/chat), [BYOK](https://manual.raycast.com/ai/bring-your-own-keys), [encrypted import/export](https://manual.raycast.com/import-export).
- **Verified:** current official manual checked 2026-07-22.

### `recurse-chat` — RecurseChat (audited-read-only)

- **Official identity / aliases:** RecurseChat, commercial sandboxed macOS personal AI client by Xiaoyi Chen; App Store bundle/product rather than an open-source CLI.
- **Install probes:** `/Applications/RecurseChat.app` or receipt/bundle identity; no command probe. App Sandbox container presence is identity-only and does not authorize external database writes.
- **Global / project config:** model and MCP setup are managed through the app's Model page. Official docs expose chat export but no supported global config path or lossless external write schema.
- **MCP:** beta client can add/import “MCP Server JSON Config,” enable/disable a server, and use tools. Current releases support SSE and Streamable HTTP; official docs do not publish the complete imported schema/storage contract or token lifecycle.
- **Models / credentials:** built-in local llama.cpp, Ollama, Hugging Face/GGUF, OpenAI, Anthropic, and OpenAI-compatible endpoints are app-managed. Users may switch models per next message/regeneration; a new chat defaults to the last-used model. API keys/private state remain sandbox-managed.
- **Skills:** no Agent Skills directory is documented.
- **MUX suitability:** **audited-read-only**. The JSON import UI could support a future user-mediated export/import flow, but MUX should not directly mutate an App Sandbox container or infer its database schema.
- **Evidence:** [official MCP docs](https://recurse.chat/docs/features/mcp/), [official introduction/security model](https://recurse.chat/docs/introduction/introduction/), [official model switching](https://recurse.chat/docs/features/multiple-models-session/), [official product site](https://recurse.chat/).
- **Verified:** official docs and App Store identity checked 2026-07-22.

### `replit` — generic Replit catalog entry (duplicate; consolidate)

- **Official identity / aliases:** generic Replit web IDE/platform. Current Agent capability documentation is explicitly branded **Replit Agent**, already represented by the adjacent `replit-agent` ID.
- **Install probes / paths:** no distinct local Agent installation or configuration contract separates this entry from Replit Agent.
- **MCP / Models / Skills:** use the `replit-agent` evidence below; the generic product also exposes a separate Replit MCP **server** for external clients, which must not be mistaken for an inbound client configuration.
- **MUX suitability:** remove or alias to `replit-agent` to avoid two entries representing the same hosted Agent. Do not create parallel bindings.
- **Evidence:** [official Replit Agent MCP client docs](https://docs.replit.com/build/connect-via-mcp), [official Replit MCP server docs](https://docs.replit.com/platforms/mcp-server).
- **Verified:** product boundary checked 2026-07-22.

### `replit-agent` — Replit Agent (audited-writable candidate: project Skills; MCP/Models read-only)

- **Official identity / aliases:** Replit Agent inside the hosted Replit project editor; not a locally installed CLI Agent. The official external `https://replit-mcp.com/server/mcp` endpoint exposes Replit Agent **as a server** to other clients and is a separate direction.
- **Install probes:** none on local macOS. A Replit workspace/project URL is account state, not a machine probe.
- **Global / project config:** hosted MCP connections live at `replit.com/integrations`. Project filesystem content is real versionable project state; Agent instructions include `replit.md`, runtime `.replit`, and Agent Skills under project `.agents/skills/`.
- **MCP:** the hosted Agent connects pre-listed OAuth integrations or a custom remote MCP URL from the Integrations pane. The published flow is remote/account-managed; no stdio client config or local `mcpServers` file is documented.
- **Models / credentials:** Replit chooses/operates Agent models and account usage. No arbitrary provider/base URL inventory or persistent file-backed current-model switch is published. Integration OAuth/secrets remain in Replit's managed stores.
- **Skills:** **verified project contract**: `<project>/.agents/skills/<name>/SKILL.md`; each skill follows the Agent Skills specification, persists across sessions and may be committed. Official `npx skills ... -a replit` installs to that directory. The hosted one-message skill picker is separate and does not install files.
- **MUX suitability:** **audited-writable project Skills** when the user explicitly selects a local checkout/project root that will be synchronized to Replit. Do not claim user-global support or attempt to modify hosted MCP/model state. Surface the security review because Replit warns that skills can contain arbitrary instructions/code.
- **Evidence:** [official Agent Skills usage](https://docs.replit.com/build/use-agent-skills), [official Agent Skills concepts/security](https://docs.replit.com/learn/agent-skills), [official MCP client flow](https://docs.replit.com/build/connect-via-mcp), [official MCP server boundary](https://docs.replit.com/platforms/mcp-server).
- **Verified:** official docs checked 2026-07-22.

### `rtrvr-ai` — rtrvr.ai browser MCP server (misclassified; catalog-only as Agent)

- **Official identity / aliases:** rtrvr.ai/Retriever Chrome extension and cloud browser automation. Its documented MCP capability turns the user's Chrome profile into a remote MCP **server/API endpoint** at `https://mcp.rtrvr.ai`; it is not an inbound MCP client registry.
- **Install probes:** Chrome extension identity and optional `rtrvr` CLI are product probes. `rtrvr mcp init --client claude` configures **another client** to consume rtrvr, so it must not be interpreted as rtrvr's own MCP config.
- **Global / project config:** remote-tool permissions, device routing and credentials are managed in the extension/cloud dashboard; no local Agent registry schema is published.
- **MCP:** server-side Streamable HTTP/API with OAuth (preferred) or API key/device ID. Generated URLs can embed secret API keys and must never be scanned into logs/catalog metadata. Per-tool allow/deny state stays in extension/cloud settings.
- **Models / Skills:** no file-backed model provider/current switch or Agent Skills loader applies to this server product.
- **MUX suitability:** **catalog-only as Agent** and recommended for reclassification into a central MCP-server catalog. MUX may offer the clean OAuth endpoint as an installable MCP asset, but must not persist secret query URLs.
- **Evidence:** [official browser-as-MCP documentation](https://www.rtrvr.ai/docs/mcp), [official product](https://www.rtrvr.ai/).
- **Verified:** official role and endpoint checked 2026-07-22.

### `runbear` — Runbear hosted team-agent platform (audited-read-only; API integration candidate)

- **Official identity / aliases:** Runbear Inbox Agent/Team Agent hosted platform. It is both an MCP **client** for attaching tools to Runbear agents and an OAuth-protected MCP **server** that lets external clients manage Runbear agent configuration.
- **Install probes:** none local; account/organization scoped web service.
- **Global / project config:** agents, instructions, models, contexts, integrations and triggers are cloud state. Credentials are configured in Runbear's dashboard vault, not files.
- **MCP:** inbound custom MCP integrations are attached to individual hosted agents from the catalog/dashboard. In the reverse direction, the official management endpoint `https://api.runbear.io/mcp` exposes read/write agent operations with scopes (`read:agents`, `write:agents`, `deploy:agents`, `read:traces`) and OAuth 2.0. This is a genuine supported management API boundary, but not a filesystem adapter.
- **Models / credentials:** the management MCP can read/update an agent's model and max tokens; advanced settings remain dashboard-only. It does not represent arbitrary provider/base-URL profiles. Integration secrets must be entered in the vault and never sent through chat or copied by MUX.
- **Skills:** Runbear contexts/instructions are product-native; no Agent Skills directory is documented.
- **MUX suitability:** **audited-read-only** in the current file-backed catalog. Strong candidate for a future remote connector using the official management MCP with an explicit OAuth/scopes review and plan/apply confirmation; never emulate it by writing local files.
- **Evidence:** [official Runbear MCP management server](https://docs.runbear.io/api-reference/mcp-server), [official product architecture](https://docs.runbear.io/).
- **Verified:** official API/docs checked 2026-07-22.

### `seekchat` — `seekrays/seekchat` (audited-read-only)

- **Official identity / aliases:** open-source Electron desktop assistant `seekrays/seekchat`; latest public release is v0.0.2 from March 2025. The repository is the authoritative source behind the catalog listing.
- **Install probes:** application bundle/name “SeekChat”; no stable CLI. Electron product metadata plus bundle identity should be used if promoted for detection.
- **Global / project config:** no project config. Electron `app.getPath("userData")` contains `seekchat.db`; provider settings/current selection are Chromium `localStorage`, not a supported standalone file. On macOS the resolved `userData` root depends on the packaged product name and Electron version, so MUX must not guess it from the repo name.
- **MCP:** SQLite table `mcp_servers(id,name,url,type,active,tools,description,timestamps)`. `stdio` overloads `url` as a whitespace-split command line; `sse` stores a URL. The source has no safe JSON import/export transaction and no Streamable HTTP implementation. Direct SQLite writes would race the live app, bypass migrations, and cannot round-trip an `env` field that runtime code references but the table schema does not store.
- **Models / credentials:** providers, `baseUrl`, plaintext `apiKey`, models and enablement live in renderer `localStorage` under `providers_config`; selected `providerId`/`modelId` also live in renderer state and message rows record them. This is browser storage, not a stable file profile/current pointer, and secrets are not Keychain-backed in the audited revision.
- **Skills:** no Agent Skills/SKILL.md loader found.
- **MUX suitability:** **audited-read-only** despite open source. Detect/link only; do not write SQLite or Chromium storage. A future upstream export/import API would be required for safe ownership.
- **Evidence:** [official source](https://github.com/seekrays/seekchat), source `src/main/database.js`, `src/main/services/mcpService.js`, `src/renderer/hooks/useUserConfig.js`, and provider services at audited commit `aa14b28`.
- **Verified:** official source cloned/searched 2026-07-22.

### `sema4` — Sema4.ai Studio / Agent Platform (audited-read-only; project/API connector candidates)

- **Official identity / aliases:** Sema4.ai Studio desktop builder plus hosted Team/Enterprise Agent Platform/Control Room. It is both an MCP client for agent tools and an MCP server surface for exposing agents.
- **Install probes:** Sema4.ai Studio application; its user-selected “Agent Source Folder” is configuration, not a fixed probe. Do not infer a default beyond what Studio reports.
- **Global / project config:** Studio “Agent as Folder” projects contain `agent-spec.yaml`, `runbook.md`, and `actions/` under a configurable source root (illustrated as `~/sema4ai-agents`). Current v2 hosted state is API/workspace managed. The official docs do not publish a stable global plaintext MCP/model registry for Studio.
- **MCP:** current hosted/Studio workflow attaches remote MCP servers to agents; deployment keeps secret header values out of exported agent packages. v2 API supports list/attach/detach per-agent MCP server and export/import/diff. Agent-as-server endpoints are separate outbound assets. Local stdio client lifecycle is not the current production contract.
- **Models / credentials:** Studio Settings manages provider/model credentials and a default model; individual agents may override it. Hosted workspace models and secrets are organization-managed. No supported plaintext model inventory/current pointer was recovered.
- **Skills:** runbooks, Actions, SAI and agent packages are Sema4-native concepts; no Agent Skills directory is documented.
- **MUX suitability:** **audited-read-only** for generic local file management. Strong future candidates: (1) explicit project adapter once `agent-spec.yaml` schema/version is formally documented; (2) remote v2 connector using `diff → import/attach → publish`, with OAuth/API scope review. Do not edit a user-selected source folder generically.
- **Evidence:** [Agent as Folder](https://sema4.ai/docs/studio/agent-as-a-folder), [MCP usage](https://sema4.ai/docs/build-agents/mcp), [v2 Agent API and dry-run](https://sema4.ai/docs/v2/api/agents), [Studio model configuration](https://sema4.ai/docs/studio/setting-your-studio).
- **Verified:** current official docs/API checked 2026-07-22.

### `shelbula` — Shelbula hosted universal chat (audited-read-only)

- **Official identity / aliases:** Shelbula personal AI-enabled cloud workspaces, delivered through web/Telegram rather than a local Agent binary.
- **Install probes:** none local.
- **Global / project config:** product/account state; official site requires its JavaScript application and publishes no local config/export contract.
- **MCP:** third-party directory evidence shows a hosted remote MCP connector UI. No official schema/path/API or secret lifecycle was recovered.
- **Models / credentials / Skills:** hosted service state; no file-backed provider profiles/current switch or Agent Skills directory found.
- **MUX suitability:** **audited-read-only**; keep only a product link until an official management API is published.
- **Evidence:** [official product](https://shelbula.com/), [Glama discovery lead](https://glama.ai/mcp/clients/shelbula).
- **Verified:** identity checked 2026-07-22; configuration serialization not verified.

### `shortwave` — Shortwave AI email assistant (audited-read-only)

- **Official identity / aliases:** Shortwave email client/AI Agent across web, desktop and mobile.
- **Install probes:** desktop application bundle is identity-only; web/mobile accounts share cloud product state.
- **Global / project config:** AI integrations are configured in Shortwave's Integrations UI/account. No supported local registry or export schema is published.
- **MCP:** official Shortwave material confirms MCP-backed AI integrations. Web supports remote servers; the desktop product has also advertised local stdio. Regardless of transport, configuration/credentials are app/account managed and no external write path is documented.
- **Models / credentials:** Shortwave selects current platform models and learns user-specific email context; it does not publish arbitrary local provider/base-URL profiles or an external secret store contract.
- **Skills:** saved prompts/templates and AI memory are product-native, not SKILL.md assets.
- **MUX suitability:** **audited-read-only**. Never mutate desktop app storage or treat Gmail/account tokens as MCP credentials.
- **Evidence:** [official MCP integration announcement](https://www.shortwave.com/blog/integrate-ai-with-all-your-apps-mcp/), [official AI assistant guide](https://www.shortwave.com/docs/guides/ai-assistant/), [official product](https://www.shortwave.com/).
- **Verified:** official product/docs checked 2026-07-22.

### `sigit` — siGit Code local coding agent (audited-writable candidate: MCP and Skills; Models deferred)

- **Official identity / aliases:** `getsigit/sigit`, package/crate names `sigit`, `sigit-code` and `@smbcloud/sigit`, command `sigit`; marketed as **siGit Code**. Terminal mode is macOS/Linux; ACP mode also supports Windows.
- **Install probes:** command `sigit`; official Homebrew, Cargo, pip/uv and npm installs. `SIGIT_CONFIG_DIR` overrides the default user configuration root `~/.config/sigit`.
- **Global / project config:** user MCP file `$SIGIT_CONFIG_DIR/mcp.toml` (default `~/.config/sigit/mcp.toml`); project override `<cwd>/.sigit/mcp.toml`, with project entries winning by normalized name. Preferences use user `settings.toml`; cloud login state is a separate `credentials.toml` with Unix mode `0600` and must never be imported or changed by MUX.
- **MCP:** TOML uses optional top-level `official` plus repeated `[[server]]` records: required `name`, exactly one of `url` or `command`, optional `args`, `enabled`, `[server.env]` and `[server.headers]`. URL means Streamable HTTP (including SSE responses); command means stdio. `SIGIT_MCP=off` disables the whole client and `SIGIT_MCP_OFFICIAL=off` disables the built-in siGit server. Changes require process restart because discovery runs once at startup.
- **Models / credentials:** user `providers.toml` has top-level `active` and repeated `[[provider]] { name, base_url, api_key, model }`; `OPENAI_BASE_URL` + `OPENAI_API_KEY` and `SIGIT_MODEL` override it. Local/cloud mode persists as `settings.toml: local_inference`; local GGUF selection uses `selected-model.txt` in the Onde App Group or cache. The external-provider schema requires a **plaintext `api_key`** and offers no env/keychain-reference field, so MUX should inventory it but defer central-secret materialization unless the user explicitly accepts plaintext export and file permissions are hardened.
- **Skills:** first-party source implements the Agent Skills format and scans, in precedence order, project `.sigit/skills`, project `.claude/skills`, user `$SIGIT_CONFIG_DIR/skills` and user `~/.claude/skills`. Each immediate child must contain a valid `SKILL.md`; earlier roots win name collisions.
- **MUX suitability:** promote to **audited-writable user/project MCP and Skills** using lossless TOML and shared-root collision checks. Models are schema-verifiable but are **not** an immediate safe writer because siGit requires inline plaintext keys; keep them observed/guided until MUX exposes explicit plaintext-export consent or siGit adds an environment/keychain reference.
- **Evidence:** [official source](https://github.com/getsigit/sigit), README and source `src/mcp.rs`, `src/provider.rs`, `src/settings.rs`, `src/setup.rs`, `src/skills.rs`, `src/credentials.rs` at audited commit `dd4e49e`.
- **Verified:** official source cloned, schemas and path precedence traced in code 2026-07-22.

### `simtheory` — Simtheory hosted AI workspace (audited-read-only)

- **Official identity / aliases:** Simtheory multi-model AI workspace/platform, currently transitioning toward asynchronous workflows and agentic tasks.
- **Install probes:** no authoritative local CLI/config probe recovered; product is account/service oriented.
- **Global / project config:** models, assistants/workflows and MCP connections are configured in Simtheory's web/application support flows. No public filesystem schema or lossless export contract was found.
- **MCP:** official support/changelog acknowledges MCP connections and token/authentication state, but fields and storage are hosted/app-managed.
- **Models / credentials:** multi-model selection is a service capability, not proof of user-supplied base-URL profiles or a writable current-model file.
- **Skills:** no Agent Skills directory documented.
- **MUX suitability:** **audited-read-only**; link only unless Simtheory publishes a management API.
- **Evidence:** [official Simtheory support/version guide](https://help.simtheory.ai/a-20-version-2-0), [official product](https://simtheory.ai/), [Glama discovery lead](https://glama.ai/mcp/clients/simtheory).
- **Verified:** official support/product boundary checked 2026-07-22.

### `slack-mcp-client` — Slackbot MCP Client (rename; audited-read-only)

- **Official identity / aliases:** the generic catalog label should be updated to **Slackbot MCP Client**, Slack/Salesforce's current first-party feature. It is distinct from Slack's official MCP **server**, which exposes Slack data/actions to external clients.
- **Install probes:** none local: Slackbot is a workspace/cloud feature. Slack desktop installation does not prove entitlement or client configuration.
- **Global / project config:** external MCP applications are approved/managed through Slack's app, workspace administration and native compliance controls; there is no user filesystem registry.
- **MCP:** Slackbot connects outward to HTTPS-hosted MCP servers. Admin/app approval and per-user OAuth/permissions govern access. Slack's own MCP server is the reverse direction and should be cataloged as a central MCP asset, not merged into the Slackbot client adapter.
- **Models / credentials:** model policy, Slack authorization and enterprise data access are service-managed; no arbitrary file-backed model profile/current switch exists.
- **Skills:** Slackbot also offers reusable/schedulable product skills, but no evidence ties them to the open Agent Skills `SKILL.md` filesystem standard.
- **MUX suitability:** **audited-read-only**. Rename/rebind the identity, link to admin docs, and do not infer configuration from the Slack desktop app.
- **Evidence:** [official Slack MCP guide](https://slack.com/help/articles/48855576908307-Guide-to-Model-Context-Protocol-in-Slack), [official Slackbot MCP Client announcement](https://slack.com/blog/news/slackbots-mcp-client), [official developer architecture](https://docs.slack.dev/ai/slack-mcp-server/).
- **Verified:** current official Slack sources checked 2026-07-22.

### `smithery-playground` — Smithery web playground (audited-read-only)

- **Official identity / aliases:** hosted Smithery Playground/registry testing surface. It is distinct from the `smithery` CLI and Smithery Connect API used by developers to manage connections.
- **Install probes:** none for the web playground. A local `smithery` command indicates the CLI, not the playground Agent.
- **Global / project config:** playground sessions/connections are hosted account/browser state. Smithery Connect intentionally owns OAuth, token refresh and write-only encrypted credentials.
- **MCP:** the playground can inspect/call hosted servers; no user-global file is published. The CLI/API support named remote connections, but adopting those would require a separate Smithery CLI/Connect adapter with namespace/user scope.
- **Models / credentials / Skills:** no playground-local provider profile/current model or Agent Skills directory contract found.
- **MUX suitability:** **audited-read-only**. A future Smithery Connect remote adapter could use its official REST/SDK and scoped service tokens; do not map the web playground to a guessed local file.
- **Evidence:** [official Smithery docs](https://smithery.ai/docs), [official Connect API/CLI](https://smithery.ai/docs/use/connect), [official Playground](https://smithery.ai/playground).
- **Verified:** official docs checked 2026-07-22.

### `spinai` — SpinAI TypeScript framework (catalog-only as Agent)

- **Official identity / aliases:** open-source TypeScript SDK/framework for building custom agents, scaffolded with `npx create-spinai`; not one canonical installed end-user Agent.
- **Install probes:** project dependency/source import, not a global Agent executable/config.
- **Global / project config:** individual TypeScript projects define agents in source and keep provider secrets in `.env`. No universal global registry exists.
- **MCP:** applications call framework helpers such as `createActionsFromMcpConfig`; examples install/provider-wrap MCP packages per project. This is program code, not a standardized host config MUX can patch globally.
- **Models / credentials:** model is a Vercel AI SDK object in project code (for example `openai("gpt-4o")`); credentials are project environment variables. There is no shared provider inventory/current-model pointer.
- **Skills:** no Agent Skills loader contract found.
- **MUX suitability:** **catalog-only** and recommended for removal/reclassification as an SDK. A code-aware project generator is outside the current Agent adapter contract.
- **Evidence:** [official product/framework](https://www.spinai.dev/), [official introduction](https://docs.spinai.dev/get-started/introduction), [official MCP example](https://docs.spinai.dev/mcp/example).
- **Verified:** official docs checked 2026-07-22.

### `superinterface` — Superinterface hosted assistant platform (audited-read-only; REST candidate)

- **Official identity / aliases:** Superinterface cloud platform and open UI components for embedding AI assistants; not a local desktop Agent.
- **Install probes:** none local. npm/UI component presence identifies a consuming application, not an Agent configuration root.
- **Global / project config:** assistant/provider/interface/MCP state is stored in Superinterface Cloud and managed through its dashboard/REST APIs.
- **MCP:** official REST API can create a per-assistant MCP server using `transportType` `SSE` or `HTTP`, nested URL and JSON-string headers. This is a supported remote write boundary, not a file schema; stdio is not represented by that endpoint.
- **Models / credentials:** assistants choose provider/model in cloud; provider API keys are configured in Superinterface. This supports multiple providers and switching but does not expose MUX-style local profile/current pointer semantics.
- **Skills:** no Agent Skills directory documented.
- **MUX suitability:** **audited-read-only** in the current file-backed catalog. Good future remote connector candidate via authenticated REST, with assistant selection and secret redaction in plan/review.
- **Evidence:** [official docs](https://superinterface.ai/docs), [official MCP create API](https://superinterface.ai/docs/api-reference/rest/mcp-servers/create), [assistant/provider model setup](https://superinterface.ai/docs/assistants/create).
- **Verified:** official REST/product docs checked 2026-07-22.

### `superjoin` — Superjoin spreadsheet AI (catalog-only for MCP)

- **Official identity / aliases:** Superjoin AI Assistant/Agent Mode for Google Sheets and Excel.
- **Install probes:** Google Workspace/Excel add-in and account state; no local Agent CLI/config.
- **Global / project config:** integrations, data sources and agent behavior are managed in the spreadsheet add-in/service.
- **MCP:** the current official product/docs found during audit do not document a general MCP client, transport fields or management API. The Glama listing is therefore insufficient and may describe an older/internal capability.
- **Models / credentials / Skills:** product-managed custom models/integrations; no filesystem provider profiles, current pointer or Agent Skills directory.
- **MUX suitability:** **catalog-only** until Superjoin publishes first-party MCP documentation. Consider keeping Superjoin as a general Agent/product entry only if MUX's catalog scope extends beyond configurable assets.
- **Evidence:** [official product](https://www.superjoin.ai/), [official quickstart](https://docs.superjoin.ai/quickstart), [Glama discovery lead](https://glama.ai/mcp/clients/superjoin).
- **Verified:** official docs searched 2026-07-22; MCP contract not verified.

### `swarms` — Swarms framework/API (misclassified; catalog-only as Agent)

- **Official identity / aliases:** Swarms Python/TypeScript framework and hosted API for constructing agents/multi-agent systems. The documented `swarms-ts-mcp` is an MCP **server** exposing the Swarms API to external clients.
- **Install probes:** Python/npm dependency or project source; no single end-user Agent installation/config.
- **Global / project config:** agents are instantiated in code with parameters; projects may define their own YAML/Pydantic models, but no canonical user-global registry applies across applications.
- **MCP:** `swarms-ts-mcp` is configured as a command in Claude/Cursor/etc., with `SWARMS_API_KEY` and filtering flags. Swarms' Python schemas can represent MCP connections, but that is SDK input inside an application, not a MUX-safe global host file.
- **Models / credentials:** per-agent `model_name`/LLM object, often via LiteLLM; provider keys come from environment. Switching means changing code/config for a specific application.
- **Skills:** no universal Agent Skills user directory verified for the Swarms runtime.
- **MUX suitability:** **catalog-only** and reclassify as framework/server, not a concrete Agent client.
- **Evidence:** [official Agent configuration](https://docs.swarms.world/agents/agent-configuration), [official Swarms API MCP server](https://docs.swarms.ai/docs/documentation/clients/swarms-api-mcp), [official source organization](https://github.com/kyegomez/swarms).
- **Verified:** official docs checked 2026-07-22.

### `systemprompt` — systemprompt.io platform/mobile MCP client (audited-read-only)

- **Official identity / aliases:** systemprompt.io hosted/self-hostable platform plus SystemPrompt MCP mobile client. Current documentation also covers hosting MCP servers; server manifests must not be confused with the mobile/client inventory.
- **Install probes:** iOS/Android app identity or a deployed systemprompt.io service; no universal desktop command/config.
- **Global / project config:** mobile client connections and hosted platform configuration are app/service state. Self-hosted server extensions use project YAML manifests, but those describe servers the platform hosts, not the client's consumed server list.
- **MCP:** mobile client promises compatibility with HTTPS Streamable HTTP servers; platform services expose OAuth-secured Streamable HTTP endpoints. No supported local plaintext client registry/export schema was recovered.
- **Models / credentials:** hosted platform models/secrets are service-managed; mobile auth/token storage is not an external file contract.
- **Skills:** platform “skills”/marketplace services are product-native unless separately exported to Agent Skills; no mobile SKILL.md directory applies.
- **MUX suitability:** **audited-read-only**. If self-hosted systemprompt deployments later expose a management API, model them as remote deployments with explicit endpoint/auth scope, not local Agent files.
- **Evidence:** [official mobile-client getting started](https://systemprompt.io/documentation/starting), [official MCP service architecture](https://systemprompt.io/documentation/services/mcp), [official custom integration](https://systemprompt.io/documentation/modules/custom).
- **Verified:** official docs checked 2026-07-22.

### `tambo` — Tambo generative-UI SDK (misclassified; catalog-only as Agent)

- **Official identity / aliases:** `tambo-ai/tambo`, a React/TypeScript SDK and hosted backend for building generative-UI applications. It is not one canonical installed end-user Agent.
- **Install probes:** npm dependency such as `@tambo-ai/react` or application source import; neither establishes a global Agent installation or config root.
- **Global / project config:** consuming applications initialize Tambo in source and pass configuration to `TamboProvider`; deployment keys and backend settings are application environment/configuration. There is no user-global registry.
- **MCP:** applications can pass an `mcpServers` array to `TamboProvider`; official examples use named remote HTTP endpoints. This is application source/runtime input, not a stable MUX-owned host file.
- **Models / credentials:** model choice and provider credentials are backend/deployment concerns. No shared provider inventory or persistent end-user current-model pointer is published.
- **Skills:** no Agent Skills `SKILL.md` loader or user directory documented.
- **MUX suitability:** **catalog-only** and reclassify as an SDK/platform. A future Tambo project generator would need explicit source-code ownership and is outside the generic Agent adapter contract.
- **Evidence:** [official source](https://github.com/tambo-ai/tambo), [official integration guide](https://docs.tambo.co/getting-started/integrate), [official MCP guide](https://docs.tambo.co/concepts/model-context-protocol).
- **Verified:** official source/docs checked 2026-07-22.

### `tencent-cloudbase-ai-devkit` — Tencent CloudBase MCP server/toolkit (misclassified; catalog-only as Agent)

- **Official identity / aliases:** the historical CloudBase AI ToolKit repository now presents itself as **CloudBase MCP** and redirects to `TencentCloudBase/CloudBase-AI-ToolKit`. It supplies CloudBase capabilities to other AI clients; it is not itself an Agent client.
- **Install probes:** package/command invocation detects an MCP server or development toolkit, not a standalone host Agent.
- **Global / project config:** installation instructions generate snippets for Claude Code, Cursor and other clients. Any durable config belongs to those consuming clients.
- **MCP:** CloudBase is on the **server** side of the connection. Its command, arguments, environment and remote endpoint belong in MUX's central MCP asset catalog, not in an Agent adapter under this ID.
- **Models / credentials:** CloudBase deployment and Tencent credentials configure the service/toolkit; they do not form a local multi-model Agent profile/current-selection schema.
- **Skills:** no independently installed Agent Skills client directory belongs to this identity.
- **MUX suitability:** **catalog-only as Agent**; remove/reclassify to a central MCP server asset and redirect documentation to the current CloudBase MCP identity.
- **Evidence:** [official repository](https://github.com/TencentCloudBase/CloudBase-AI-ToolKit), [official CloudBase MCP documentation](https://docs.cloudbase.net/ai/mcp/), [Tencent CloudBase documentation](https://docs.cloudbase.net/).
- **Verified:** official repository/product identity checked 2026-07-22.

### `tester-mcp-client` — Apify Tester MCP Client (audited-read-only)

- **Official identity / aliases:** `apify/tester-mcp-client`, a browser UI/Apify Actor for testing one MCP endpoint; package name `tester-mcp-client`.
- **Install probes:** cloned application plus `npm start`, or the hosted Apify Actor. This is a test utility, not a persistent desktop Agent installation.
- **Global / project config:** connection/model inputs are supplied per run through the Actor input or local web UI. No canonical user-level registry or project config is persisted for external ownership.
- **MCP:** one endpoint at a time. Input fields include `mcpUrl`, `mcpTransportType` (`http` or legacy `sse`) and custom headers; no stdio/global multi-server map.
- **Models / credentials:** Anthropic-backed evaluator with supported model names and token controls; key is supplied as Actor input or `LLM_PROVIDER_API_KEY`. It does not expose arbitrary provider/base-URL profiles or a persistent active pointer.
- **Skills:** no Agent Skills loader found.
- **MUX suitability:** **audited-read-only**. It may be linked as a test tool, but MUX should not fabricate a durable registry from transient Actor inputs.
- **Evidence:** [official source](https://github.com/apify/tester-mcp-client), [official Apify Actor](https://apify.com/apify/tester-mcp-client), source `src/` and input schema at audited commit `81c5ba8`.
- **Verified:** official source cloned and searched 2026-07-22.

### `theiaai-theiaide` — Eclipse Theia AI / Theia IDE (audited-writable candidate: MCP and Skills)

- **Official identity / aliases:** Eclipse Theia IDE with the Theia AI feature set; distinct from downstream products built on the Theia platform.
- **Install probes:** concrete Theia IDE application/distribution plus its preferences service. A generic downstream Theia app must not be assumed compatible unless it ships the same AI preference contributions.
- **Global / project config:** user preferences are `$HOME/.theia/settings.json`; workspace/folder preferences are `<root>/.theia/settings.json`. Files can contain comments/JSONC, so MUX must use a lossless JSONC transaction rather than plain JSON rewrite.
- **MCP:** preference `ai-features.mcp.mcpServers` is a named map. Local entries use `command`, `args`, `env`, `autostart`; remote entries use `serverUrl`, optional `serverAuthToken`, `serverAuthTokenHeader`, OAuth settings, `autostart` and `deferLoading`. Workspace trust prevents untrusted workspace autostart; OAuth tokens are stored by Theia's credential service rather than authored by MUX.
- **Models:** custom OpenAI-compatible connections use `ai-features.openAiCustom.customOpenAiModels` entries with `model`, `url`, optional `id`, `apiKey` and developer-message settings. Vercel AI custom entries additionally carry `provider`, structured-output and streaming flags. OpenAI/Anthropic/Google keys can be preferences but official guidance favors environment variables; per-agent selection is managed by AI Configuration/agent settings and its complete stable write schema is not public enough for MUX to claim active-model switching yet.
- **Skills:** project `.prompts/skills/` and `.agents/skills/`; global `~/.theia/skills/` and `~/.agents/skills/`; extra roots from `ai-features.skills.skillDirectories`. Standard `SKILL.md` structure is documented, with project/user/global precedence.
- **MUX suitability:** promote to **audited-writable MCP and Skills** with JSONC preservation, user/workspace scope selection, trust-aware plan warnings and secret-field refusal. Limit Models to read/inventory until a typed, versioned selection schema and safe secret references are implemented.
- **Evidence:** [official Theia AI user guide](https://theia-ide.org/docs/user_ai/), [official preferences guide](https://theia-ide.org/docs/preferences/), [official workspace trust guide](https://theia-ide.org/docs/workspace_trust/), [official source](https://github.com/eclipse-theia/theia).
- **Verified:** official current docs/source checked 2026-07-22.

### `tiles-notebook` — Tiles Notebook (catalog-only)

- **Official identity / aliases:** Tiles is presented at `tiles.run` as an offline-first, multiplayer notebook with AI-generated widgets. A Glama client listing is the only MCP-specific discovery lead recovered.
- **Install probes:** no current official package, CLI or stable desktop bundle identifier was recovered.
- **Global / project config:** no official filesystem path, export schema or management API was published in sources available to this audit.
- **MCP:** the directory listing claims MCP use, but no first-party transport/schema/credential documentation could be verified. That claim is therefore not sufficient for a writable adapter.
- **Models / credentials / Skills:** no official provider profile/current-model contract or Agent Skills directory recovered.
- **MUX suitability:** **catalog-only**. Retain only as an unverified discovery entry or remove until first-party technical documentation/source becomes available.
- **Evidence:** [official product site](https://www.tiles.run/), [Glama discovery lead](https://glama.ai/mcp/clients/tiles-notebook).
- **Verified:** official identity checked 2026-07-22; capability serialization not verified.

### `tome` — `runebookai/tome` desktop MCP client (audited-read-only)

- **Official identity / aliases:** `runebookai/tome`, an open-source Tauri desktop MCP client/LLM chat application; current repository describes it as a technical preview.
- **Install probes:** Tome desktop application/release; no authoritative CLI probe. Tauri bundle metadata is suitable for identity detection, not configuration ownership.
- **Global / project config:** persistent state is Tauri application data in SQLite database `tome.db`. The README explicitly positions server/model management through the application UI rather than hand-edited JSON.
- **MCP:** database-backed server records contain name, command, JSON args, JSON env, metadata and relationships; audited runtime is stdio-oriented. There is no supported filesystem import/export registry for lossless external writes.
- **Models / credentials:** engines, models, default model and API-key data are database/config records. Direct writes would bypass migrations and live-app lifecycle; API-key storage must not be reverse-engineered into a MUX plaintext profile.
- **Skills:** no Agent Skills/SKILL.md loader found.
- **MUX suitability:** **audited-read-only**. Detect/link only; never mutate `tome.db`. Promotion would require an upstream API or versioned import/export contract.
- **Evidence:** [official source](https://github.com/runebookai/tome), source database migrations/models and README at audited commit `1d6d6c0`, [official releases](https://github.com/runebookai/tome/releases).
- **Verified:** official source cloned and searched 2026-07-22.

### `trae-agent` — ByteDance TRAE Agent / `trae-cli` (audited-read-only globally; typed project candidate)

- **Official identity / aliases:** `bytedance/trae-agent`, installed command `trae-cli`; this open-source CLI runtime is distinct from the commercial TRAE IDE entry below.
- **Install probes:** command `trae-cli`; official installation supports Git/uv. A config is normally supplied from the current project as `trae_config.yaml`, with `--config-file` for another path and legacy JSON fallback.
- **Global / project config:** no canonical user-global registry. Project YAML has `agents`, `model_providers`, `models`, `mcp_servers` and an `allow_mcp_servers` list. A MUX adapter must preserve arbitrary workflow sections and update the server map and allow-list atomically.
- **MCP:** each named entry can contain `command`, `args`, `env`, `cwd`, `url`, `http_url`, `headers`, `tcp`, `timeout`, `trust` and `description`. However, the audited `MCPClient` implementation actually supports stdio (`command`) and raises `NotImplementedError` for `url`/`http_url`; MUX must advertise runtime reality rather than the broader dataclass shape.
- **Models:** `model_providers.<name>` includes `provider`, `api_key`, optional `base_url`/`api_version`; `models.<name>` references a provider and model ID plus generation/retry/token settings; `agents.trae_agent.model` selects the named model. CLI flags can override provider/model/base URL/key. Environment fallbacks use provider-derived `*_API_KEY` and `*_BASE_URL` names.
- **Credentials:** provider keys may be referenced through environment interpolation; MUX should never materialize a central secret as YAML plaintext by default.
- **Skills:** no Agent Skills/SKILL.md loader found in the audited source.
- **MUX suitability:** **audited-read-only globally**. A future explicitly selected project-file adapter is feasible, but requires a typed YAML round-trip transaction, coordinated `mcp_servers` + `allow_mcp_servers` updates, runtime transport validation and project ownership confirmation; the generic map codec is insufficient.
- **Evidence:** [official source](https://github.com/bytedance/trae-agent), [official configuration examples](https://github.com/bytedance/trae-agent/tree/e839e559ac61bdd0e057c375dd1dee391fee797d/configs), source configuration and MCP client modules at audited commit `e839e55`.
- **Verified:** official source cloned and searched 2026-07-22.

### `trae-ide` — TRAE IDE (audited-writable candidate: project MCP and global/project Skills)

- **Official identity / aliases:** ByteDance TRAE IDE/SOLO desktop development environment; distinct from the open-source `trae-agent` CLI.
- **Install probes:** TRAE desktop application/bundle. The app probe identifies the host, while writable scope must still be an explicitly selected project root or documented global Skills directory.
- **Global / project config:** official docs expose project MCP at `<root>/.trae/mcp.json`; project Skills at `<root>/.trae/skills/`; global Skills at `~/.trae/skills/` on macOS/Linux and `%USERPROFILE%/.trae/skills/` on Windows. TRAE also has UI-managed global MCP “Raw Config,” but the docs do not publish that file's stable filesystem path, so MUX must not guess it.
- **MCP:** project JSON is `{ "mcpServers": { ... } }`. Stdio entries use required `command`, optional string-array `args` and string-map `env`; HTTP entries use required `url` and optional `headers`. `${workspaceFolder}` is the only documented variable. `START_MCP_TIMEOUT_MS` and `RUN_MCP_TIMEOUT_MS` are unusually encoded in `env` for stdio and `headers` for HTTP. TRAE supports stdio, legacy SSE and Streamable HTTP at product level, but the published project schema shows stdio/HTTP. Project MCP must also be explicitly enabled in Settings and untrusted workspaces are a security boundary.
- **Models:** built-in and custom models are configured in Settings and switched from the chat selector. Custom models support OpenAI Chat Completions or Anthropic Messages, full URL/base URL, model ID, multimodal flag, display name, token/tool limits, API key and model-series optimizations. No stable external file or credential storage contract is published, so Models remain read-only/UI-linked.
- **Skills:** native standard `SKILL.md` folders at the documented global/project roots, plus opt-in project `.agents/skills/`. Native `.trae/skills/` wins on name collision. Disabled project skills are listed in `<root>/.trae/skill-config.json`; disabled global state is not stored there, so MUX should initially own install/remove only and avoid claiming a cross-scope enable switch.
- **MUX suitability:** promote to **audited-writable project MCP and global/project Skills**. Use lossless JSON, explicit project scope, trust warnings, collision checks and plan/apply verification; do not write global MCP or Models until their storage contracts are official.
- **Evidence:** [official MCP overview](https://docs.trae.ai/ide/model-context-protocol), [official Add MCP servers guide](https://docs.trae.ai/ide/add-mcp-servers), [official Skills guide](https://docs.trae.ai/ide/skills), [official Models guide](https://docs.trae.ai/ide/models).
- **Verified:** current official docs and exact project/global paths checked 2026-07-22.

### `typingmind-app` — TypingMind web/PWA client (audited-read-only)

- **Official identity / aliases:** TypingMind Personal/Teams web application and static self-host package; the official docs supersede the catalog's Glama-only link.
- **Install probes:** installed PWA/static deployment/browser session are identity clues only. Account/browser state may sync across devices and is not a supported host filesystem registry.
- **Global / project config:** MCP connectors, models, agents, plugins and Skills are managed through TypingMind UI, Cloud Sync and workspace state. No stable plaintext path or public lossless export schema is documented.
- **MCP:** remote Streamable HTTP plus local commands through TypingMind's Private MCP Connector. It supports OAuth dynamic registration/custom clients, custom authentication, multiple accounts/connections, per-server tool enablement and persistent sessions scoped per chat/user. Those tokens/sessions are service/browser state, not safe MUX file fields.
- **Models:** 12 documented provider families plus custom model endpoints are managed in UI; chats select models and can request multi-model responses. No externally writable provider inventory/current-model file is published.
- **Skills:** Personal now supports the open Agent Skills standard, installed from its store, public GitHub URL or zip, enabled per chat, and optionally synced. These are imported frozen packages inside TypingMind storage—not a documented filesystem directory—so MUX can link to/import instructions but cannot safely write them directly. Teams does not currently support Skills.
- **MUX suitability:** **audited-read-only**. Do not reverse-engineer browser IndexedDB/local storage or connector token state. A future official management/export API could enable remote writes.
- **Evidence:** [official MCP guide](https://docs.typingmind.com/model-context-protocol-%28mcp%29-in-typingmind), [official Agent Skills guide](https://docs.typingmind.com/skills), [official model management](https://docs.typingmind.com/manage-and-connect-ai-models), [official static self-host docs](https://docs.typingmind.com/static-self-host/static-self-host-package-and-updates).
- **Verified:** current official docs checked 2026-07-22.

### `v0` — Vercel v0 hosted agent (audited-read-only)

- **Official identity / aliases:** Vercel's hosted v0 AI agent and v0 Platform API; not the shadcn registry MCP server shown in older design-system examples.
- **Install probes:** none local. v0 projects/chats are account/cloud resources.
- **Global / project config:** MCP connections and project settings are account/project resources; no local user/project registry belongs to the hosted client. The official Platform API does expose remote CRUD for MCP server resources, but those resources and credentials remain service state.
- **MCP:** v0 connects to remote MCP servers from presets or custom endpoints. The Platform API can create/list/update/delete named HTTPS connections with user/team scope, enablement and none/bearer/custom-header/OAuth authentication; sensitive values are redacted and OAuth tokens remain server-managed. The current v2 API is explicitly beta, and local stdio is not a portable hosted-client contract.
- **Models / credentials:** v0's own model/service policy and API keys are cloud-managed. The v0 Model API is an outbound API for consumers, not an arbitrary local provider inventory or current-model file for the hosted v0 agent.
- **Skills:** no local Agent Skills/SKILL.md directory documented for the hosted agent.
- **MUX suitability:** **audited-read-only** in the current file-backed architecture, but a strong future remote-connector candidate. Such a connector should use the stable Platform API version, explicit user/team scope, plan/apply review and secret redaction rather than filesystem writes; do not build production ownership on beta v2 alone.
- **Evidence:** [official v0 overview](https://v0.app/docs), [official MCP integrations](https://api2.v0.dev/docs/MCP), [official Platform API overview](https://v0.app/docs/api/platform/overview), [official MCP create API](https://v0.app/docs/api/platform/reference/mcp-servers/create), [v2 OAuth lifecycle](https://v0.app/docs/api/v2/guides/oauth-mcp-servers).
- **Verified:** official docs checked 2026-07-22.

### `visual-studio` — Microsoft Visual Studio + GitHub Copilot (audited-writable candidate: MCP and Skills; Windows only)

- **Official identity / aliases:** Microsoft Visual Studio 2022/2026 for Windows with GitHub Copilot agent mode. This is not Visual Studio Code.
- **Install probes:** installed Visual Studio instance plus a supported version (MCP: VS 2022 17.14+; Agent Skills: VS 2026 18.5+). MUX running on macOS can catalog these paths for remote/Windows use but cannot claim a local installation.
- **Global / project config:** MCP discovery order is `%USERPROFILE%\.mcp.json`, `<SOLUTIONDIR>\.vs\mcp.json`, `<SOLUTIONDIR>\.mcp.json`, `<SOLUTIONDIR>\.vscode\mcp.json`, then `<SOLUTIONDIR>\.cursor\mcp.json`. Because later entries may be shared with other clients, MUX must show the exact target and resulting cross-Agent impact before writing.
- **MCP:** current official examples use top-level `servers` keyed by name, with remote `url` or local command/arguments/environment/transport fields. Visual Studio reloads valid edits, disables tools by default, and prompts again when changed server configuration/capabilities require trust. Remote OAuth uses CodeLens/browser plus the Visual Studio keychain; MUX should author endpoint definitions only, never OAuth tokens.
- **Models:** Copilot models and BYOM (OpenAI, Anthropic, Google) are selected in the UI; the selected model persists across chats. Microsoft publishes no stable external provider profile/key/current-model file, so Models are read-only/UI-linked.
- **Skills:** workspace `.github/skills/`, `.claude/skills/`, `.agents/skills/`; personal `~/.copilot/skills/`, `~/.claude/skills/`, `~/.agents/skills/`. Each skill is an Agent Skills-standard directory containing `SKILL.md`.
- **MUX suitability:** promote to **audited-writable MCP and Skills for Windows**, guarded by OS/version probes, scope/source-control warnings, lossless JSON and secret refusal. Model management remains read-only.
- **Evidence:** [official MCP guide](https://learn.microsoft.com/en-us/visualstudio/ide/mcp-servers?view=visualstudio), [official Agent Skills guide](https://learn.microsoft.com/en-us/visualstudio/ide/copilot-agent-skills?view=visualstudio), [official model/BYOM guide](https://learn.microsoft.com/en-us/visualstudio/ide/copilot-select-add-models?view=visualstudio).
- **Verified:** exact paths, version gates and schemas checked in current Microsoft documentation 2026-07-22.

### `whatsmcp` — WhatsMCP WhatsApp agent (catalog-only)

- **Official identity / aliases:** the Glama listing describes a paid WhatsApp-hosted MCP client named WhatsMCP, but no verifiable first-party product domain, repository, package, publisher identity or current documentation was recovered.
- **Install probes:** none. Do not confuse it with `lharries/whatsapp-mcp` or other WhatsApp MCP **servers**.
- **Global / project config:** no official path, export schema or management API.
- **MCP / Models / credentials / Skills:** directory claims are only discovery leads; transport, provider, secret lifecycle and Agent Skills support are unverified.
- **MUX suitability:** **catalog-only**. Remove or retain as explicitly unverified until a first-party identity/config contract is available; never create a writable adapter from the directory description.
- **Evidence:** [Glama discovery lead](https://glama.ai/mcp/clients/whatsmcp).
- **Verified:** public identity/source search completed 2026-07-22; no first-party evidence found.

### `witsy` — Kochava Studios Witsy (audited-writable candidate: MCP and Skills)

- **Official identity / aliases:** `Kochava-Studios/witsy`, formerly published under another GitHub owner; Electron desktop “Witsy,” marketed as a universal MCP client.
- **Install probes:** `/Applications/Witsy.app`, command/package release as applicable; official macOS install is `brew install --cask witsy`.
- **Global / project config:** on macOS the canonical Electron/CLI user-data root is `~/Library/Application Support/Witsy`; primary config is `settings.json`. API keys were split into `apiKeys.json` and encrypted with Electron `safeStorage`. There is no project config. MUX must preserve the entire settings document because it is app-monitored and contains unrelated state.
- **MCP:** modern settings are `mcp.servers`, an array of objects with `uuid`, `registryId`, `state`, `type` (`stdio`, `sse`, `http`), required/overloaded `url`, and optional `label`, `command`, `cwd`, `env`, `headers`, `oauth`, `timeout`, `toolSelection`, `toolMappings`. Witsy also reads a legacy Claude-style `mcpServers` map and augments registry entries through `mcp.mcpServersExtra`. OAuth tokens/client secrets can appear nested in settings; MUX must redact and refuse ownership of those fields.
- **Models:** provider engine settings are split between `settings.json`, `engines/<engine>.json`, encrypted `apiKeys.json`, and several feature-specific active/default fields. This is not safely representable as one generic central Model profile/current pointer; Models should be inventory/read-only until Witsy gains a stable import API.
- **Skills:** native global root is `<userData>/skills`, i.e. `~/Library/Application Support/Witsy/skills` on macOS. Extra roots come from `skills.locations` (defaults include `~/.claude/skills` and `~/.agents/skills`); each configured location is also checked for nested `.claude/skills` and `.github/skills`. Standard `SKILL.md` packages are parsed.
- **MUX suitability:** promote to **audited-writable MCP and Skills** with a Witsy-specific array codec, UUID generation/duplicate checks, lossless full-document writes, live-reload verification, and strict exclusion of OAuth/API-key material. Do not run the legacy map and modern array as two independent inventories.
- **Evidence:** [official source](https://github.com/Kochava-Studios/witsy), source `src/cli/config.ts`, `src/main/config.ts`, `src/main/mcp.ts`, `src/main/skills.ts`, `src/types/mcp.ts` and defaults at audited commit `c81c7de`, [official releases](https://github.com/Kochava-Studios/witsy/releases).
- **Verified:** official source cloned and schema/path behavior searched 2026-07-22.

### `y-cli` — renamed `luohy15/y-agent` (stale identity; catalog-only)

- **Official identity / aliases:** upstream explicitly says **y-cli was renamed to y-agent**; current command remains `y`. It is a hosted/self-hosted orchestration app that wraps Claude Code, Codex, Gemini, pi and Grok rather than a generic standalone model/MCP host.
- **Install probes:** command `y`; official local development install uses `uv tool install --force -e ./cli`.
- **Global / project config:** CLI service configuration is `~/.y-agent/config.toml` (or `Y_AGENT_HOME`) and authentication is `~/.y-agent/auth.json`. These files hold service URL/timezone/proxy/integration data, not the remote bot definitions that users manage through `y bot add/update` and backend state.
- **MCP:** y-agent launches wrapped Claude paths with strict MCP disabled in the audited source and owns no independent user MCP registry. Configuring the underlying Claude/Codex host belongs to those Agent adapters, not this stale ID.
- **Models / credentials:** bot agent/model/base-URL choices are hosted/Postgres resources; local config is not a provider inventory/current-model pointer. Wrapped agent credentials/config belong to their native tools.
- **Skills:** docs describe `~/.agents/skills/<name>/SKILL.md`, but the audited API source hard-codes the maintainer path `/Users/roy/luohy15/.agents/skills`. Until upstream makes this portable/configurable, MUX cannot treat it as a reliable cross-user write contract.
- **MUX suitability:** **catalog-only**. Rename/rebind to y-agent if retained, but do not promote to writable until the local/remote ownership boundary and portable Skills path are fixed upstream.
- **Evidence:** [official source](https://github.com/luohy15/y-agent), [official CLI docs](https://github.com/luohy15/y-agent/blob/75eba78bcc23025de450c55d450093fdb4731b28/docs/cli.md), [official getting-started guide](https://github.com/luohy15/y-agent/blob/75eba78bcc23025de450c55d450093fdb4731b28/docs/getting-started.md), audited source commit `75eba78`.
- **Verified:** official source cloned and searched 2026-07-22; stale name and portability defect confirmed.

### `zencoder` — Zencoder IDE Agents / Zenflow (audited-writable candidate: project MCP and Skills)

- **Official identity / aliases:** Zencoder IDE plugins for VS Code and JetBrains plus the Zenflow desktop orchestrator. The current product is not the unrelated legacy video-encoding service.
- **Install probes:** Zencoder extension in VS Code, official JetBrains plugin (marketplace ID 24782), or Zenflow desktop app. Capability scope differs by host and must be shown in MUX.
- **Global / project config:** in VS Code, MCP is the `zencoder.mcpServers` preference in User Settings or `<root>/.vscode/settings.json`; the latter is JSONC and is the safest explicit project target. JetBrains exposes the same named map through `Settings → Tools → Zencoder → MCP Servers`, but no supported file path is published.
- **MCP:** named entries accept local `command`, `args`, `env` or remote `url`; OAuth2 is also documented. JetBrains has full stdio/HTTP/OAuth2 support, while VS Code transport rollout is version-dependent. MUX must preserve JSONC, avoid plaintext secrets in `env`, and advertise only transports supported by the detected host/version.
- **Models:** Zencoder's selector, entitlements, BYOK UI and custom/private endpoints are product/account/credential-store state. Anthropic/OpenAI keys are explicitly entered only in the official UI; named model presets in Zenflow workflows are also product state. No stable file-backed provider inventory/current pointer is published.
- **Skills:** project `<root>/.agents/skills/`, user `~/.agents/skills/`, and project `<root>/.claude/skills/`; legacy `.zencoder/skills/` remains supported but is deprecated. Standard `SKILL.md` folders work in both Zenflow and IDE Agents.
- **MUX suitability:** promote to **audited-writable Skills** at documented roots and **project MCP for VS Code** via a JSONC nested-map codec. Keep JetBrains MCP and all Models read-only/UI-linked until their storage/credential contracts are published.
- **Evidence:** [official MCP configuration guide](https://docs.zencoder.ai/features/mcp-deep-dive), [official Skills guide](https://docs.zencoder.ai/features/skills), [official Models guide](https://docs.zencoder.ai/features/models), [official model configuration](https://docs.zencoder.ai/features/custom-models-configuration), [official installation guide](https://docs.zencoder.ai/get-started/install-ide-plugin).
- **Verified:** current official docs, exact keys/paths and host-specific transports checked 2026-07-22.

### `zin-mcp-client` — `zinja-coder/zin-mcp-client` (audited-read-only globally; archived)

- **Official identity / aliases:** open-source Zin MCP Client, a lightweight CLI/web bridge from local Ollama models to stdio MCP servers. The repository is archived/read-only and upstream explicitly ended development.
- **Install probes:** release directory/script `zin_mcp_client.py`; official execution is `uv run zin_mcp_client.py`. There is no installed global command/profile convention.
- **Global / project config:** default is a file in the current application directory; code uses `mcp_config.json` while release-tree README text also says `mcp-config.json`. `--config/-c` selects an arbitrary file, so no canonical user-global path exists.
- **MCP:** Claude-style JSON `{ "mcpServers": { name: { "command": ..., "args": [...] } } }`; audited runtime is stdio-only. `--server` selects a configured server.
- **Models / credentials:** models are discovered dynamically from local Ollama and `--model` chooses one for the run; there is no provider profile inventory, credential store or persistent current-model pointer.
- **Skills:** no Agent Skills loader found.
- **MUX suitability:** **audited-read-only globally**. A user could explicitly select one config file for a future project/file adapter, but the archived upstream, filename inconsistency and arbitrary path make automatic writes inappropriate.
- **Evidence:** [official archived source](https://github.com/zinja-coder/zin-mcp-client), README and source `zin_mcp_client.py`, `src/mcp_tools_client.py` at audited commit `1cbea5f`.
- **Verified:** official source cloned and searched 2026-07-22.

## Promotion decisions

Ten identities below have at least one stable, first-party filesystem contract. Promotion is capability-scoped: it does **not** mean every MCP/Model/Skill surface of that product becomes writable.

| ID | Verified writable capability | Required adapter guardrails |
|---|---|---|
| `oterm` | user MCP; OpenAI-compatible connection inventory | resolve `OTERM_DATA_DIR`; preserve unknown fields; no SQLite/current-model writes; env-reference secrets |
| `poolside` | user/project MCP and Skills | lossless YAML; scope/precedence warning; preserve inline external secrets; no invented enable flag or model schema |
| `raycast` | user Skills | dedicated/shared-root choice; one-level scan and duplicate-name checks; MCP/Models stay read-only |
| `replit-agent` | project Skills | explicit checkout/project root; no hosted MCP/model mutation |
| `sigit` | user/project MCP and Skills | lossless TOML; resolve `SIGIT_CONFIG_DIR`; restart warning; never touch `credentials.toml`; Models remain guided/read-only while keys require plaintext |
| `theiaai-theiaide` | user/project MCP and Skills | JSONC preservation; workspace-trust warning; credential-service fields excluded |
| `trae-ide` | project MCP; user/project Skills | never guess global MCP path; project trust warning; native-vs-`.agents` collision checks |
| `visual-studio` | Windows user/project MCP and Skills | Windows/version gate; target-precedence/cross-client warning; OAuth stays in keychain |
| `witsy` | user MCP and Skills | Witsy array codec; full-document preservation; UUID/legacy de-duplication; exclude OAuth/API-key material |
| `zencoder` | VS Code project MCP; user/project Skills | JSONC nested-map codec; host/transport gate; JetBrains MCP and Models stay read-only |

At the research layer, eight macOS user-level identities had candidate surfaces: `oterm`, `poolside`, `raycast`, `sigit`, `theiaai-theiaide`, `trae-ide`, `witsy`, and `zencoder`. This is not the current writer list. Dynamic roots, project overlays, JSONC, UUID/OAuth arrays and effective-layer ambiguity remove several items from the release-safe subset.

The release implementation adds Skills-only targets for `poolside`, `raycast`, `theiaai-theiaide`, `trae-ide`, and `zencoder`. Their MCP/Models surfaces remain read-only. `replit-agent` is project-only and `visual-studio` is Windows-only; neither is wired into the current macOS user-level registry. Together with the A–M implementation, the final registry contains 56 audited definitions, 46 MCP writers and 45 Skills targets; the 14 Model targets (12 managed, 2 guided) are unchanged by this N–Z shard.

Remote/API opportunities such as Runbear management MCP, Sema4 v2 and Superinterface REST are valid future connectors, but they are deliberately not counted as immediately writable filesystem adapters.

## Machine-checkable coverage summary

Primary status is mutually exclusive and sums to 52. Secondary flags are independent subsets.

```json
{
  "schema_version": 1,
  "audit_date": "2026-07-22",
  "scope": {
    "sort_range": ["navigator", "zin-mcp-client"],
    "identity_count": 52,
    "entry_audit_complete_count": 52
  },
  "primary_status_counts": {
    "audited-writable": 10,
    "audited-read-only": 28,
    "catalog-only": 14
  },
  "primary_status_members": {
    "audited-writable": [
      "oterm",
      "poolside",
      "raycast",
      "replit-agent",
      "sigit",
      "theiaai-theiaide",
      "trae-ide",
      "visual-studio",
      "witsy",
      "zencoder"
    ],
    "audited-read-only": [
      "needle",
      "nerve",
      "nextchat",
      "nova",
      "nvidia-agent-intelligence-aiq-toolkit",
      "ollamac-pro",
      "open-webui",
      "posthog-code",
      "postman",
      "qodo",
      "qordinate",
      "recurse-chat",
      "runbear",
      "seekchat",
      "sema4",
      "shelbula",
      "shortwave",
      "simtheory",
      "slack-mcp-client",
      "smithery-playground",
      "superinterface",
      "systemprompt",
      "tester-mcp-client",
      "tome",
      "trae-agent",
      "typingmind-app",
      "v0",
      "zin-mcp-client"
    ],
    "catalog-only": [
      "navigator",
      "opensumi",
      "proxyman",
      "ravenala",
      "replit",
      "rtrvr-ai",
      "spinai",
      "superjoin",
      "swarms",
      "tambo",
      "tencent-cloudbase-ai-devkit",
      "tiles-notebook",
      "whatsmcp",
      "y-cli"
    ]
  },
  "secondary_flag_counts": {
    "misclassified": 7,
    "duplicate": 1,
    "rename-or-stale-identity": 3
  },
  "secondary_flag_members": {
    "misclassified": [
      "opensumi",
      "proxyman",
      "rtrvr-ai",
      "spinai",
      "swarms",
      "tambo",
      "tencent-cloudbase-ai-devkit"
    ],
    "duplicate": ["replit"],
    "rename-or-stale-identity": [
      "nvidia-agent-intelligence-aiq-toolkit",
      "slack-mcp-client",
      "y-cli"
    ]
  },
  "research_candidate_scope": {
    "platform": "macos",
    "config_scope": "user",
    "identity_count": 8,
    "rule": "Only stable user-level files/directories that current MUX asset types can represent safely; project-only, Windows-only, internal-database and plaintext-secret-only writes are excluded."
  },
  "research_candidate_items": [
    {
      "id": "oterm",
      "capabilities": ["mcp.user", "models.connections.openaiCompatible"]
    },
    {
      "id": "poolside",
      "capabilities": ["mcp.user", "skills.user"]
    },
    {
      "id": "raycast",
      "capabilities": ["skills.user"]
    },
    {
      "id": "sigit",
      "capabilities": ["mcp.user", "skills.user"]
    },
    {
      "id": "theiaai-theiaide",
      "capabilities": ["mcp.user", "skills.user"]
    },
    {
      "id": "trae-ide",
      "capabilities": ["skills.user"]
    },
    {
      "id": "witsy",
      "capabilities": ["mcp.user", "skills.user"]
    },
    {
      "id": "zencoder",
      "capabilities": ["skills.user"]
    }
  ],
  "implemented_in_this_release": [
    { "id": "poolside", "capabilities": ["skills.user"] },
    { "id": "raycast", "capabilities": ["skills.user"] },
    { "id": "theiaai-theiaide", "capabilities": ["skills.user"] },
    { "id": "trae-ide", "capabilities": ["skills.user"] },
    { "id": "zencoder", "capabilities": ["skills.user"] }
  ],
  "explicitly_deferred_writes": [
    {
      "id": "replit-agent",
      "reason": "project-only",
      "capabilities": ["skills.project"]
    },
    {
      "id": "visual-studio",
      "reason": "windows-only",
      "capabilities": ["mcp.user", "mcp.project", "skills.user", "skills.project"]
    },
    {
      "id": "sigit",
      "reason": "provider profiles require plaintext api_key materialization",
      "capabilities": ["models.connections", "models.active"]
    },
    {
      "id": "oterm",
      "reason": "active model is internal SQLite per-chat state",
      "capabilities": ["models.active"]
    }
  ],
  "immediate_catalog_hygiene": {
    "reclassify-or-remove": [
      "opensumi",
      "proxyman",
      "rtrvr-ai",
      "spinai",
      "swarms",
      "tambo",
      "tencent-cloudbase-ai-devkit"
    ],
    "consolidate": {
      "replit": "replit-agent"
    },
    "rename": {
      "nvidia-agent-intelligence-aiq-toolkit": "NVIDIA NeMo Agent Toolkit",
      "slack-mcp-client": "Slackbot MCP Client",
      "y-cli": "y-agent"
    },
    "replace-directory-links-with-official-docs": [
      "typingmind-app",
      "v0",
      "witsy",
      "zencoder"
    ]
  }
}
```
