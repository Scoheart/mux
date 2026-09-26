# Agent 名称核对（2026-09-27）

桌面 Agent 使用实际 App 名称（不含 `.app` 扩展名），CLI / 插件使用正式产品名。地区、版本、Desktop / IDE 等形态词不额外拼进名字；若形态词本来就是产品名则保留。内部 ID、消费关系、配置路径、启动目标不因改名而改变。

## 范围与证据

核对当前全部 229 个定义：73 个内置能力定义以及 156 个目录补充项。先与本机 App 包名称、安装探针、启动定义对账，再参考已保存的官方包审计；Cline / Theia 另核实官方构建配置。CLI、插件和未安装的来源目录条目保留其正式产品/来源名称，不把插件改成宿主编辑器名，也不猜测不存在的本机 App。

本轮修正 18 个名称；未重新安装所有第三方 App。

| ID | 原名称 | 修正名称 | 依据 |
|---|---|---|---|
| `antigravity` | Google Antigravity | Antigravity | 本机 Antigravity.app / Info.plist |
| `claude-desktop` | Claude Desktop | Claude | 现有 Claude.app 安装探针与官方配置路径 |
| `cline-desktop` | Cline Desktop | Cline | 官方 desktop-v0.0.37 的 tauri.conf.json：productName = Cline |
| `codebuddy-ide` | CodeBuddy IDE | CodeBuddy | 现有 CodeBuddy.app 启动与安装定义 |
| `codex-desktop` | Codex Desktop | ChatGPT | 本机 ChatGPT.app / CFBundleIdentifier = com.openai.codex |
| `kimi-code-desktop` | Kimi Code Desktop | Kimi Code | 官方安装包核验记录：docs/kimi-code-desktop.md |
| `opencode-desktop` | OpenCode Desktop | OpenCode | 官方 OpenCode.app 接入记录：docs/opencode-desktop.md |
| `qoder` | Qoder IDE | Qoder | 只读挂载本机官方 Qoder 安装包：Qoder.app / com.qoder.ide |
| `qoder-desktop` | Qoder Desktop | Qoder | 官方 Qoder.app 包审计：docs/qoder-desktop-models.md / com.qoder.app |
| `qwenwork` | QwenWork 国际版 | QwenWork | 官方包核验记录：docs/qwenwork-integration.md / QwenWork.app |
| `qwenwork-cn` | QwenWork CN 国内版 | QwenWorkCN | 官方包核验记录：docs/qwenwork-integration.md / QwenWorkCN.app |
| `theiaai-theiaide` | Eclipse Theia IDE | TheiaIDE | 官方 electron-builder.yml：productName = TheiaIDE |
| `trae-ide` | TRAE IDE | TRAE | 现有国际版 TRAE.app 安装探针；本机 Trae CN 为另一发行版，不改指向 |
| `trae-cli` | TRAE CLI 2.0 | TRAE CLI | 去掉发布版本 2.0；保留官方 CLI 产品名 |
| `workbuddy` | 海外 WorkBuddy AI | WorkBuddy AI | 本机 WorkBuddy AI.app / Info.plist |
| `workbuddy-cn` | 中国 WorkBuddy | WorkBuddy | 本机 WorkBuddy.app / Info.plist |
| `zcode` | ZCode Desktop | ZCode | 现有 ZCode.app 安装探针与原生接入记录 |
| `zencoder` | Zencoder | Zenflow | 现有 Zenflow.app 安装探针与启动定义 |

## 一致性

- `data/agents.json` 为已核验 Agent 名称权威；目录名称在 `data/agent-catalog.json`。
- 前端只保留品牌颜色，从上述定义读取名称，覆盖选择器、资源消费者、提示与回退名称。
- Models 列表从同一 Agent 定义填充名称；Skills 已使用同一能力图。
- 目录生成时以已核验定义覆盖重合项的名称，防止外部目录重新引入说明性后缀。
- Qoder 的 IDE 与独立桌面应用都实际叫 Qoder；Cline 插件和 App 都叫 Cline；OpenCode CLI 和 App 都叫 OpenCode。保持独立 ID，沿用形态标记，不创造替代名称。

## 全量核对结果

| ID | 最终名称 | 形态 | 处理 |
|---|---|---|---|
| `5ire` | 5ire | client | 保留来源目录名称 |
| `agent-bridge` | Agent-Bridge | client | 保留来源目录名称 |
| `agent-cli` | Agent-cli | client | 保留来源目录名称 |
| `agent-one` | AgentOne | client | 保留来源目录名称 |
| `agentai` | AgentAI | client | 保留来源目录名称 |
| `agenticflow` | AgenticFlow | client | 保留来源目录名称 |
| `agentkube` | Agentkube | desktop | 沿用已核验产品名 |
| `agoragentic-acp` | Agoragentic | coding-agent | 保留来源目录名称 |
| `aiaw` | AIaW | client | 保留来源目录名称 |
| `aiql-tuui` | AIQL TUUI | client | 保留来源目录名称 |
| `amazon-q` | Amazon Q Developer IDE | plugin | 沿用已核验产品名 |
| `amazon-q-cli` | Amazon Q CLI | client | 保留来源目录名称 |
| `amp` | Amp | cli | 沿用已核验产品名 |
| `antigravity` | Antigravity | ide | 本轮修正 |
| `antigravity-cli` | Google Antigravity CLI | cli | 沿用已核验产品名 |
| `apidog` | Apidog | client | 保留来源目录名称 |
| `apigene-mcp-client` | Apigene MCP Client | client | 保留来源目录名称 |
| `archestra` | Archestra | client | 保留来源目录名称 |
| `argo-local-ai` | Argo-LocalAI | client | 保留来源目录名称 |
| `askit-mcp` | askit-mcp | client | 保留来源目录名称 |
| `astr-bot` | AstrBot | client | 保留来源目录名称 |
| `augment` | Augment Code | cli | 沿用已核验产品名 |
| `autohand` | Autohand Code | coding-agent | 保留来源目录名称 |
| `avatar-shell` | Avatar Shell | client | 保留来源目录名称 |
| `beeai-framework` | BeeAI Framework | client | 保留来源目录名称 |
| `blackbox-cli` | BLACKBOX CLI | cli | 保留来源目录名称 |
| `bob-shell` | Bob Shell | client | 保留来源目录名称 |
| `boltai` | BoltAI | desktop | 沿用已核验产品名 |
| `browse-wiz` | BrowseWiz | client | 保留来源目录名称 |
| `call-chirp` | Call Chirp | client | 保留来源目录名称 |
| `call-my-bot` | CallMyBot | client | 保留来源目录名称 |
| `chainlit` | Chainlit | client | 保留来源目录名称 |
| `chat-frame` | ChatFrame | client | 保留来源目录名称 |
| `chatbox` | Chatbox | client | 保留来源目录名称 |
| `chatgpt` | ChatGPT | web | 保留来源目录名称 |
| `chatmcp` | ChatMCP | desktop | 沿用已核验产品名 |
| `chatty` | Chatty | client | 保留来源目录名称 |
| `chatwise` | ChatWise | client | 保留来源目录名称 |
| `cherry-studio` | Cherry Studio | client | 保留来源目录名称 |
| `chorus` | Chorus | client | 保留来源目录名称 |
| `claude-ai` | Claude.ai | web | 保留来源目录名称 |
| `claude-code` | Claude Code | cli | 沿用已核验产品名 |
| `claude-desktop` | Claude | desktop | 本轮修正 |
| `claude-mind` | ClaudeMind | client | 保留来源目录名称 |
| `cline` | Cline | plugin | 沿用已核验产品名 |
| `cline-desktop` | Cline | desktop | 本轮修正 |
| `codebuddy-code` | CodeBuddy CLI | cli | 沿用已核验产品名 |
| `codebuddy-ide` | CodeBuddy | ide | 本轮修正 |
| `codegpt` | CodeGPT | client | 保留来源目录名称 |
| `codewhale` | CodeWhale | cli | 沿用已核验产品名 |
| `codex` | Codex CLI | cli | 沿用已核验产品名 |
| `codex-desktop` | ChatGPT | desktop | 本轮修正 |
| `cody` | Sourcegraph Cody | client | 保留来源目录名称 |
| `console-chat-gpt` | console-chat-gpt | client | 保留来源目录名称 |
| `continue` | Continue | plugin | 沿用已核验产品名 |
| `continue-cli` | Continue CLI | cli | 沿用已核验产品名 |
| `copilot-cli` | GitHub Copilot CLI | cli | 沿用已核验产品名 |
| `copilot-mcp` | Copilot-MCP | client | 保留来源目录名称 |
| `copilot-xcode` | GitHub Copilot for Xcode | ide | 保留来源目录名称 |
| `cortex-code` | Snowflake Cortex Code | cli | 沿用已核验产品名 |
| `corust-agent` | Corust Agent | coding-agent | 保留来源目录名称 |
| `crow-cli` | crow-cli | coding-agent | 保留来源目录名称 |
| `crush` | Crush | cli | 沿用已核验产品名 |
| `cursor` | Cursor | ide | 沿用已核验产品名 |
| `cursor-cli` | Cursor CLI | cli | 沿用已核验产品名 |
| `daydreams` | Daydreams | client | 保留来源目录名称 |
| `deepagents` | DeepAgents | coding-agent | 保留来源目录名称 |
| `deepchat` | DeepChat | client | 保留来源目录名称 |
| `deepgram-saga` | Deepgram Saga | client | 保留来源目录名称 |
| `devin` | Devin | coding-agent | 保留来源目录名称 |
| `dimcode` | DimCode | coding-agent | 保留来源目录名称 |
| `dirac` | Dirac | cli | 沿用已核验产品名 |
| `docker-agent` | Docker Agent | cli | 沿用已核验产品名 |
| `docker-gordon` | Docker Gordon | desktop | 保留来源目录名称 |
| `dolphin-mcp` | Dolphin-MCP | client | 保留来源目录名称 |
| `eca-editor-code-assistant` | ECA - Editor Code Assistant | client | 保留来源目录名称 |
| `emacs-mcp` | Emacs Mcp | client | 保留来源目录名称 |
| `factory-droid` | Factory Droid | cli | 沿用已核验产品名 |
| `fast-agent` | fast-agent | coding-agent | 保留来源目录名称 |
| `firebender` | Firebender | desktop | 沿用已核验产品名 |
| `flowdown` | FlowDown | client | 保留来源目录名称 |
| `flujo` | FLUJO | client | 保留来源目录名称 |
| `freebuff` | Freebuff | cli | 沿用已核验产品名 |
| `gemini` | Gemini CLI | cli | 沿用已核验产品名 |
| `genaiscript` | GenAIScript | client | 保留来源目录名称 |
| `genkit` | Genkit | client | 保留来源目录名称 |
| `github-copilot` | GitHub Copilot | coding-agent | 保留来源目录名称 |
| `github-copilot-coding-agent` | GitHub Copilot coding agent | client | 保留来源目录名称 |
| `glm-acp-agent` | GLM Agent | coding-agent | 保留来源目录名称 |
| `glue` | Glue | client | 保留来源目录名称 |
| `goose` | Goose | cli | 沿用已核验产品名 |
| `gptme` | gptme | client | 保留来源目录名称 |
| `grok-build` | Grok Build | cli | 沿用已核验产品名 |
| `harn` | Harn | coding-agent | 保留来源目录名称 |
| `hermes` | Hermes Agent | cli | 沿用已核验产品名 |
| `heym-mcp-client` | Heym MCP Client | client | 保留来源目录名称 |
| `highlight-ai` | Highlight AI | client | 保留来源目录名称 |
| `hyper-chat` | HyperChat | client | 保留来源目录名称 |
| `hyperagent` | HyperAgent | client | 保留来源目录名称 |
| `ibm-bob` | IBM Bob | client | 保留来源目录名称 |
| `inspector` | Inspector | client | 保留来源目录名称 |
| `jdbcx` | JDBCX | client | 保留来源目录名称 |
| `jenova` | Jenova | client | 保留来源目录名称 |
| `jetbrains-ai-assistant` | JetBrains AI Assistant | client | 保留来源目录名称 |
| `jetbrains-air` | JetBrains Air | ide | 保留来源目录名称 |
| `joey` | Joey | client | 保留来源目录名称 |
| `junie` | JetBrains Junie | plugin | 沿用已核验产品名 |
| `kibitz` | kibitz | client | 保留来源目录名称 |
| `kiln-ai` | Kiln AI | client | 保留来源目录名称 |
| `kilo-code` | Kilo Code CLI | cli | 沿用已核验产品名 |
| `kimi-code` | Kimi Code CLI | cli | 沿用已核验产品名 |
| `kimi-code-desktop` | Kimi Code | desktop | 本轮修正 |
| `kiro` | Kiro | ide | 沿用已核验产品名 |
| `klavis-ai-slack-discord-web` | Klavis AI Slack/Discord/Web | client | 保留来源目录名称 |
| `lang-bot` | LangBot | client | 保留来源目录名称 |
| `langdock` | Langdock | client | 保留来源目录名称 |
| `langflow` | Langflow | client | 保留来源目录名称 |
| `libre-chat` | LibreChat | client | 保留来源目录名称 |
| `lm-kit-net` | LM-Kit.NET | client | 保留来源目录名称 |
| `lmstudio` | LM Studio | desktop | 沿用已核验产品名 |
| `lovable` | Lovable | web | 保留来源目录名称 |
| `lutra` | Lutra | client | 保留来源目录名称 |
| `mcp-agent` | mcp-agent | client | 保留来源目录名称 |
| `mcp-assistant` | MCP Assistant | client | 保留来源目录名称 |
| `mcp-bundler-for-macos` | MCP Bundler for MacOS | client | 保留来源目录名称 |
| `mcp-chatbot` | MCP Chatbot | client | 保留来源目录名称 |
| `mcp-cli-client` | MCP CLI client | client | 保留来源目录名称 |
| `mcp-client-chatbot` | mcp-client-chatbot | client | 保留来源目录名称 |
| `mcp-client-go` | mcp-client-go | client | 保留来源目录名称 |
| `mcp-partner` | MCP Partner | client | 保留来源目录名称 |
| `mcp-simple-slackbot` | MCP Simple Slackbot | client | 保留来源目录名称 |
| `mcp-super-assistant` | MCP SuperAssistant | client | 保留来源目录名称 |
| `mcp-use` | mcp-use | client | 保留来源目录名称 |
| `mcpbundles` | MCPBundles | client | 保留来源目录名称 |
| `mcpc` | mcpc | client | 保留来源目录名称 |
| `mcphub` | MCPHub | client | 保留来源目录名称 |
| `mcpjam` | MCPJam | client | 保留来源目录名称 |
| `mcpomni-connect` | MCPOmni Connect | client | 保留来源目录名称 |
| `mcps-playground` | Mcps Playground | client | 保留来源目录名称 |
| `memex` | Memex | client | 保留来源目录名称 |
| `memgraph-lab` | Memgraph Lab | client | 保留来源目录名称 |
| `microsoft-365-copilot` | Microsoft 365 Copilot | web | 保留来源目录名称 |
| `microsoft-copilot-studio` | Microsoft Copilot Studio | client | 保留来源目录名称 |
| `mindpal` | MindPal | client | 保留来源目录名称 |
| `minimax-code` | MiniMax Code | cli | 沿用已核验产品名 |
| `minion-code` | Minion Code | cli | 沿用已核验产品名 |
| `mistral-ai-le-chat` | Mistral AI: Le Chat | client | 保留来源目录名称 |
| `mistral-vibe` | Mistral Vibe | cli | 沿用已核验产品名 |
| `modelcontextchat-com` | modelcontextchat.com | client | 保留来源目录名称 |
| `moopoint` | MooPoint | client | 保留来源目录名称 |
| `msty-studio` | Msty Studio | client | 保留来源目录名称 |
| `navigator` | Navigator | client | 保留来源目录名称 |
| `needle` | Needle | client | 保留来源目录名称 |
| `nerve` | Nerve | client | 保留来源目录名称 |
| `nextchat` | NextChat | client | 保留来源目录名称 |
| `nova` | Nova | coding-agent | 保留来源目录名称 |
| `nvidia-agent-intelligence-aiq-toolkit` | NVIDIA Agent Intelligence (AIQ) toolkit | client | 保留来源目录名称 |
| `ollamac-pro` | Ollamac Pro | client | 保留来源目录名称 |
| `open-webui` | Open WebUI | web | 保留来源目录名称 |
| `openclaw` | OpenClaw | cli | 沿用已核验产品名 |
| `opencode` | OpenCode | cli | 沿用已核验产品名 |
| `opencode-desktop` | OpenCode | desktop | 本轮修正 |
| `openhands` | OpenHands CLI | cli | 沿用已核验产品名 |
| `opensumi` | OpenSumi | client | 保留来源目录名称 |
| `oterm` | oterm | client | 保留来源目录名称 |
| `pi` | Pi Coding Agent | cli | 沿用已核验产品名 |
| `poolside` | Poolside | cli | 沿用已核验产品名 |
| `posthog-code` | PostHog Code | coding-agent | 保留来源目录名称 |
| `postman` | Postman | client | 保留来源目录名称 |
| `proxyman` | Proxyman | client | 保留来源目录名称 |
| `qoder` | Qoder | ide | 本轮修正 |
| `qoder-cli` | Qoder CLI | cli | 沿用已核验产品名 |
| `qoder-desktop` | Qoder | desktop | 本轮修正 |
| `qoderwork` | QoderWork | desktop | 沿用已核验产品名 |
| `qodo` | Qodo | coding-agent | 保留来源目录名称 |
| `qordinate` | Qordinate | client | 保留来源目录名称 |
| `qwen-code` | Qwen Code | cli | 沿用已核验产品名 |
| `qwenwork` | QwenWork | desktop | 本轮修正 |
| `qwenwork-cn` | QwenWorkCN | desktop | 本轮修正 |
| `ravenala` | Ravenala | client | 保留来源目录名称 |
| `raycast` | Raycast | desktop | 沿用已核验产品名 |
| `recurse-chat` | RecurseChat | client | 保留来源目录名称 |
| `replit` | Replit | client | 保留来源目录名称 |
| `replit-agent` | Replit Agent | web | 保留来源目录名称 |
| `roo-code` | Roo Code | plugin | 沿用已核验产品名 |
| `rovo-dev` | Atlassian Rovo Dev CLI | cli | 沿用已核验产品名 |
| `rtrvr-ai` | rtrvr.ai | client | 保留来源目录名称 |
| `runbear` | Runbear | client | 保留来源目录名称 |
| `seekchat` | SeekChat | client | 保留来源目录名称 |
| `sema4` | Sema4.ai | agent-platform | 保留来源目录名称 |
| `shelbula` | Shelbula | client | 保留来源目录名称 |
| `shortwave` | Shortwave | client | 保留来源目录名称 |
| `sigit` | siGit Code | coding-agent | 保留来源目录名称 |
| `simtheory` | Simtheory | client | 保留来源目录名称 |
| `slack-mcp-client` | Slack MCP Client | client | 保留来源目录名称 |
| `smithery-playground` | Smithery Playground | client | 保留来源目录名称 |
| `spinai` | SpinAI | client | 保留来源目录名称 |
| `stakpak` | Stakpak | cli | 沿用已核验产品名 |
| `step-code` | Step Code | cli | 沿用已核验产品名 |
| `superinterface` | Superinterface | client | 保留来源目录名称 |
| `superjoin` | Superjoin | client | 保留来源目录名称 |
| `swarms` | Swarms | client | 保留来源目录名称 |
| `systemprompt` | Systemprompt | client | 保留来源目录名称 |
| `tabnine` | Tabnine | plugin | 沿用已核验产品名 |
| `tambo` | Tambo | client | 保留来源目录名称 |
| `tencent-cloudbase-ai-devkit` | Tencent CloudBase AI DevKit | client | 保留来源目录名称 |
| `tester-mcp-client` | Tester MCP Client | client | 保留来源目录名称 |
| `theiaai-theiaide` | TheiaIDE | ide | 本轮修正 |
| `tiles-notebook` | Tiles Notebook | client | 保留来源目录名称 |
| `tome` | Tome | client | 保留来源目录名称 |
| `trae-agent` | TRAE Agent | cli | 保留来源目录名称 |
| `trae-cli` | TRAE CLI | cli | 本轮修正 |
| `trae-ide` | TRAE | ide | 本轮修正 |
| `typingmind-app` | TypingMind App | client | 保留来源目录名称 |
| `v0` | v0 | client | 保留来源目录名称 |
| `visual-studio` | Visual Studio | ide | 保留来源目录名称 |
| `vscode` | Visual Studio Code | ide | 沿用已核验产品名 |
| `vt-code` | VT Code | cli | 沿用已核验产品名 |
| `warp` | Warp | desktop | 沿用已核验产品名 |
| `whatsmcp` | WhatsMCP | client | 保留来源目录名称 |
| `windsurf` | Windsurf | ide | 沿用已核验产品名 |
| `witsy` | Witsy | client | 保留来源目录名称 |
| `workbuddy` | WorkBuddy AI | desktop | 本轮修正 |
| `workbuddy-cn` | WorkBuddy | desktop | 本轮修正 |
| `y-cli` | y-cli | client | 保留来源目录名称 |
| `zcode` | ZCode | desktop | 本轮修正 |
| `zed` | Zed | ide | 沿用已核验产品名 |
| `zencoder` | Zenflow | desktop | 本轮修正 |
| `zin-mcp-client` | zin-mcp-client | client | 保留来源目录名称 |

## 可复核来源

- [Cline 官方构建配置](https://github.com/cline/cline/blob/desktop-v0.0.37/apps/examples/desktop-app/src-tauri/tauri.conf.json)
- [Theia 官方构建配置](https://github.com/eclipse-theia/theia-ide/blob/master/applications/electron/electron-builder.yml)
- [QwenWork 安装包记录](qwenwork-integration.md)
- [Kimi Code 安装包记录](kimi-code-desktop.md)
- [Qoder 桌面包记录](qoder-desktop-models.md)
- [OpenCode 接入记录](opencode-desktop.md)
- [WorkBuddy 两版记录](workbuddy.md)
