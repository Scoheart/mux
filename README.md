<img src="desktop/src-tauri/icons/icon.png" width="104" align="right" alt="MUX icon" />

# MUX — MCP Multiplexer

**One place to manage your MCP (Model Context Protocol) servers across every AI coding agent.**

MUX ships as **two front-ends that share the same data** (`~/.mux/`):

- 🖥️ a **macOS desktop app** (Tauri + React) — a visual manager, and
- ⌨️ a **CLI + TUI** (`mux`, a native Rust binary) — an interactive terminal UI
  plus scriptable subcommands.

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

A one-click **Mux 精选 (curated collection)** subscribes you to a curated set. Every source can be toggled on/off; the Registry shows the union of all enabled sources.

## Features

- **Aggregated catalog** with search, source filtering, and an explicit view of copies shadowed by precedence.
- **Per-agent** install / enable / disable / delete. *Disable* removes a server from the agent's file but remembers its exact config so you can flip it back.
- **Transport-aware** — `stdio` / `http` / `sse`, plus a **custom `type`** (e.g. `streamable-http`). Same-named stdio and http variants are tracked separately.
- **Paste a config** — drop a `{"mcpServers": {…}}` block and MUX recognizes the servers and adds them.
- **Edits propagate** — changing a catalog entry re-stamps it into agents that installed it *clean*, while leaving hand-customized per-agent configs untouched.
- **Safe, local writes** — MUX reads and edits only the configured MCP section on this machine. It never uploads the complete agent config. A timestamped **backup** is created before every change; unrelated keys, comments, formatting, servers, and unmodelled fields are preserved.
- **CLI ⇄ Desktop in sync** — both are built on one shared Rust core (`mux-core`) and read/write `~/.mux/`, so a change in one shows up in the other.
- **Dark mode** and a macOS "liquid glass" UI.

## Supported agents (21)

Claude Code · Claude Desktop · Cursor · VS Code · Codex · Zed · Windsurf · Roo Code · Gemini CLI · Qoder · Devin · Kiro · Junie · Amazon Q · OpenCode · Copilot CLI · Cline · Continue · Warp · Pi · QoderWork

Each agent's config path/format is editable in the app. Paths inside the home directory are normalized to the portable `~/…` form.

---

## Desktop app

Grab the **Desktop installer · Apple Silicon** asset from the latest stable [**Release**](../../releases/latest). The app checks that stable channel automatically and also exposes a manual **Check for updates** action. Installing the app makes its bundled `mux` CLI available through `~/.local/bin/mux` when that directory is on `PATH`.

Build from source:

```bash
cd desktop
npm install
npm run tauri build      # or: npm run tauri dev
```

## CLI

The `mux` CLI is a native Rust binary built on the same `mux-core` as the desktop app. It is bundled with the desktop app, can be downloaded separately from Releases, or built from source:

```bash
cargo install --path cli    # installs the `mux` binary onto your PATH
# or just build it:
cargo build --release -p mux-cli   # → target/release/mux
```

Everything runs against `~/.mux/`, shared with the desktop app.

Run `mux` with **no arguments** for the **interactive TUI** — a keyboard-driven
terminal manager with three screens (Registry / 来源 / Agents): browse and search
the catalog, install to agents (multi-select), enable / disable / delete, edit or
paste catalog entries, and manage sources and agents. Press `?` for the keymap,
`q` to quit. (Set `MUX_NO_TUI=1` to fall back to printing help in scripts.)

Or drive it non-interactively with subcommands:

```bash
mux import          # scan agents and import discovered servers
mux list            # list catalog entries
mux apply <names…>  # apply MCPs to global agent configs (--agent)
mux add <name>      # add a server to the manual source
mux remove <name>   # remove a manual entry
mux status          # show what's active across agents
mux export [--out <file>]  # export the effective catalog as JSON
mux clean [--agent <name>]   # clear MCPs from enabled agents
mux agents [list | enable <name> | disable <name>]
mux upgrade         # upgrade a standalone CLI from the latest stable Release
```

---

## Data layout

Everything lives under `~/.mux/`:

```
~/.mux/
├── settings.json           # one document: agents · sources · disabled · state
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

A Cargo workspace plus the Tauri desktop app:

```
core/           # mux-core — the shared Rust core (types, settings, sources, adapters, ops)
cli/            # mux-cli  — the clap-based `mux` binary, built on mux-core
desktop/        # Tauri v2 (Rust, depends on mux-core) + React 19 + Vite + Tailwind v4
data/           # shared agent defaults + the curated collection (embedded on both sides)
```

The desktop app is a separate build (`exclude`d from the workspace) so its Tauri bundle output path stays put.

```bash
cargo test                            # mux-core + mux-cli
cd desktop/src-tauri && cargo test    # Rust core + integration tests (desktop)
cd desktop && npm run build           # desktop frontend (tsc + vite)
```

## License

[MIT](LICENSE) © Scoheart
