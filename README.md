<img src="desktop/src-tauri/icons/icon.png" width="104" align="right" alt="MUX icon" />

# MUX — MCP Multiplexer

**One place to manage your MCP (Model Context Protocol) servers across every AI coding agent.**

MUX ships as **two front-ends that share the same data** (`~/.mux/`):

- 🖥️ a **macOS desktop app** (Tauri + React) — a visual manager, and
- ⌨️ a **CLI / TUI** (`@scoheart/mux`) — for the terminal.

Point either one at your tools — Claude Code, Codex, Cursor, VS Code, Zed, Windsurf, Gemini CLI, Qoder, and ~10 more — and install, toggle, or remove MCP servers per agent from one catalog.

---

## Sources, not a hardcoded list

MUX doesn't bundle a fixed server list. Your catalog is **assembled from sources** you control:

| Source | What it is |
|--------|------------|
| **订阅 (Subscribe)** | A **URL** to an MCP config file. MUX fetches + caches it; refresh re-pulls upstream. |
| **本地 (Local)** | A config file **imported from disk** — copied into MUX; refresh re-reads the original. |
| **手动添加 (Manual)** | Servers you create by hand or **paste** in — stored as a managed local source. |
| **自动探索 (Discovered)** | Servers already configured in your agents, **auto-detected** on launch. |

A one-click **官方精选合集 (official collection)** subscribes you to a curated set. Every source can be toggled on/off; the Registry shows the union of all enabled sources.

## Features

- **Aggregated catalog** with search and filters by **source** and **transport**.
- **Per-agent** install / enable / disable / delete. *Disable* removes a server from the agent's file but remembers its exact config so you can flip it back.
- **Transport-aware** — `stdio` / `http` / `sse`, plus a **custom `type`** (e.g. `streamable-http`). Same-named stdio and http variants are tracked separately.
- **Paste a config** — drop a `{"mcpServers": {…}}` block and MUX recognizes the servers and adds them.
- **Edits propagate** — changing a catalog entry re-stamps it into agents that installed it *clean*, while leaving hand-customized per-agent configs untouched.
- **Safe writes** — atomic file writes + a timestamped **backup** before touching any agent config.
- **CLI ⇄ Desktop in sync** — both read/write `~/.mux/`, so a change in one shows up in the other.
- **Dark mode** and a macOS "liquid glass" UI.

## Supported agents (18)

Claude Code · Claude Desktop · Cursor · VS Code · Codex · Zed · Windsurf · Roo Code · Gemini CLI · Qoder · Devin · Kiro · Junie · Amazon Q · OpenCode · Copilot CLI · Cline · Continue

Each agent's config path/format is editable in the app; paths are stored portably (`~/…`, never a hardcoded home).

---

## Desktop app

Grab the latest **`.dmg`** (Apple Silicon) from [**Releases**](../../releases) — every push to `main` publishes a versioned pre-release.

Build from source:

```bash
cd desktop
npm install
npm run tauri build      # or: npm run tauri dev
```

## CLI

```bash
npm install -g @scoheart/mux    # then run:
mux                             # interactive TUI
# or, without installing:
npx @scoheart/mux
```

First launch scans your existing tool configs and offers to import discovered MCP servers.

```bash
mux                 # interactive TUI
mux import          # scan agents and import discovered servers
mux list            # list catalog entries
mux apply <names…>  # apply MCPs non-interactively (--scope, --agent, --project)
mux add <name>      # add a server to the manual source
mux remove <name>   # remove a manual entry
mux status          # show what's active across agents
mux agents [enable|disable <name>]
```

---

## Data layout

Everything lives under `~/.mux/`:

```
~/.mux/
├── settings.json           # one document: agents · registry (manual/discovered) · sources · disabled · state
├── sources/
│   ├── remote/<id>.json    # cached copies of subscribed URLs
│   └── local/<id>.(json|toml)   # imported local files + the managed manual/discovered sources
└── backups/                # timestamped backups made before each write
```

## How it works

1. **Add sources** — subscribe to a URL, import a file, add the official collection, create/paste a server by hand, or let MUX auto-discover what's already in your agents.
2. **Browse the catalog** — the Registry aggregates every enabled source (precedence: external sources < discovered < your manual edits).
3. **Install to an agent** — pick an agent, toggle servers on/off; MUX writes that agent's real config file (backing it up first).
4. **Stay in sync** — enable/disable/edit/remove flow through `~/.mux/`, visible to both the CLI and the desktop app.

## Development

Monorepo:

```
src/            # TypeScript CLI + TUI (ink/react)
desktop/        # Tauri v2 (Rust core) + React 19 + Vite + Tailwind v4
data/           # shared agent defaults + the curated collection
tests/          # CLI tests (vitest)
```

```bash
npm install && npm test          # CLI: typecheck + vitest
cd desktop/src-tauri && cargo test   # Rust core + integration tests
```

## License

[MIT](LICENSE) © Scoheart
