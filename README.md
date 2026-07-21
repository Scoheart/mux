<img src="desktop/src-tauri/icons/icon.png" width="104" align="right" alt="MUX icon" />

# MUX — MCP Multiplexer

**Configure MCP servers, reusable model endpoints, and user-level Agent Skills once, then let each Agent consume those central assets.**

MUX is a central asset and Agent configuration manager for Claude Code, Codex,
Cursor, QoderWork, OpenCode, and many other AI agents. MCPs, Model Profiles, and
Skills are created and maintained in their top-level libraries; each Agent then
selects which compatible assets it should consume. MUX adapts that desired state
to the Agent's native format while preserving unrelated settings.

Model credentials managed by MUX stay in macOS Keychain. Agents without a safe
Keychain command receive only a non-secret environment-variable reference; MUX
never exports the secret or edits shell startup files. Primary model selection
is managed independently from fork, small, auxiliary, and other secondary model
slots. Skills are downloaded
from GitHub or imported from local folders and archives directly into one managed copy. MUX
still validates paths, archive structure, hashes, and concurrent changes in the
background. Assigning that copy to verified Agent directories remains a
separate operation, so an Agent page never asks you to reinstall the same Skill.

MUX ships as **two front-ends that share the same data** (`~/.mux/`):

- 🖥️ a **macOS desktop app** (Tauri + React) — a visual manager, and
- ⌨️ a **CLI + TUI** (`mux`, a native Rust binary) — an interactive terminal UI
  plus scriptable subcommands.

The Desktop app uses the central-asset consumption workflow. The existing CLI
and TUI continue to manage MCP targets through the same Rust core and `~/.mux/`
data while their UI migration remains out of scope for this release.

---

## Central sources and external observations

MUX doesn't bundle a fixed server list. Its central MCP catalog is assembled from sources you control, while Agent files remain an independent observed-state input:

| Source | What it is |
|--------|------------|
| **订阅 (Subscribe)** | A **URL** to an MCP config file. MUX fetches + caches it; refresh re-pulls upstream. |
| **本地 (Local)** | A config file **imported from disk** — copied into MUX; refresh re-reads the original. |
| **手动添加 (Manual)** | Servers you create by hand or **paste** in — stored as a managed local source. |
| **外部发现 (External)** | Servers already present in Agent files, scanned as read-only observed state. MUX detects them automatically and offers an explicit migration into central management. |

A one-click **Mux 精选 (curated collection)** subscribes you to a curated set. Managed sources can be toggled on/off; the Registry shows their effective union, while external observations remain read-only until an explicit import.

## Features

- **Aggregated catalog** with search, source filtering, and an explicit view of copies shadowed by precedence.
- **Central assets, explicit consumers** — create or import assets once, then manage each Agent's desired MCP/Skill/Model sets from that Agent's page. Asset Inspectors keep lifecycle and impact read-only.
- **Transport-aware** — `stdio` / `http` / `sse`, plus a **custom `type`** (e.g. `streamable-http`). Same-named stdio and http variants are tracked separately.
- **Paste a config** — drop a `{"mcpServers": {…}}` block and MUX recognizes the servers and adds them.
- **Desired vs. observed state** — Agent files and Skill links are scanned for `synced`, `pending`, `drifted`, `conflicted`, `unsupported`, and read-only `external` states; scans never silently create ownership.
- **Historical configuration migration** — newly detected global MCPs, Model Profiles, and user-level Skills appear in a non-blocking migration inbox. Exact copies are deduplicated, original Agent relationships and disabled MCP state are preserved, and divergent same-name copies remain blocked for review.
- **Reviewed propagation** — editing or deleting a central MCP or Model plans the central change together with every consumer. Drift replacement requires explicit confirmation, and unresolved conflicts prevent partial commits.
- **Safe, local writes** — MUX reads and edits only fields it owns. Existing files are backed up, prepared, and verified as one recoverable transaction; unrelated keys, comments, formatting, policy fields, permissions, and symlinks are preserved.
- **Unified Agent consumption center** — each Agent page shows only desired central assets under MCPs, Model, and Skills, with a central picker for relationship changes and a separate read-only external section.
- **Reusable model endpoints (preview)** — define a protocol, Base URL, model ID, and optional token limits once, then add compatible Profiles to any number of Agents. Native multi-model Agents can keep several Profiles installed, enable or disable each one, and choose exactly one current primary model; Claude Code and Codex retain their single-Profile contract.
- **User-level Skills in Desktop** — download a public GitHub repository or directly import a local folder or `.zip` / `.tar.gz` / `.tgz` / `.tar` archive without Git, Node.js, or `npx`; assign the central copy to Agents in a separate step.
- **One proxy for MUX networking** — configure HTTP, SOCKS4/SOCKS4A, or SOCKS5 once for GitHub Skills, remote sources, CLI updates, and signed Desktop update checks; credentials are never stored in `settings.json`.
- **CLI ⇄ Desktop in sync for MCP management** — both are built on one shared Rust core (`mux-core`) and read/write `~/.mux/`. Skills use the same core but intentionally have no CLI/TUI entry in this version.
- **Dark mode** and a compact, consistent resource workspace for MCPs, Models, and Skills, with shared cards, right-side Inspectors, and review dialogs only for consequential existing-asset changes.

## Screenshots

![MUX MCP catalog with source and conflict visibility](website/public/img/mcps-overview.png)

![MUX reusable model endpoints and agent assignments](website/public/img/model-endpoints.png)

![QoderWork MCP configuration managed by MUX](website/public/img/qoderwork-config.png)

See the [desktop app guide](website/guide/desktop.md) for Agent search, source
filtering, and shadowed-configuration screenshots.

## Supported agents

MUX tracks **211 unique Agent identities** across its reviewed sources: **56 deeply audited definitions** and **201 discovery-catalog entries**, with 46 IDs overlapping. Of the audited definitions, **46 have verified, writable global MCP targets** with native JSON, TOML, or YAML schemas; the remaining audited definitions are Skills-only targets or the explicit read-only Devin record. MUX never guesses a path or writes a generic schema into discovery-only records.

Audited targets include Claude Code/Desktop, Codex, Cursor, VS Code, Zed, Windsurf, Gemini CLI, Google Antigravity, Amazon Q, OpenCode, Grok Build, MiniMax Code, Copilot CLI, Cline, Continue, Goose, Hermes, Kimi Code, Qwen Code, Qoder Desktop, Qoder CLI, QoderWork, Mistral Vibe, Rovo Dev, Tabnine, LM Studio, and others. Claude Desktop and BoltAI local files accept stdio only. Pi is explicitly labeled as a community `pi-mcp-adapter` target because Pi core does not ship MCP support. Devin remains an audited read-only record because no stable user-level global config file is documented.

MUX exposes **14 Model targets**. Managed Model Profile configuration is available
for Claude Code, Codex, Grok Build, Pi, OpenCode, Kilo Code CLI, Qwen Code,
Crush, Mistral Vibe, Hermes Agent, Factory Droid, and Goose. MiniMax Code and
Qoder are the two guided targets because their available
configuration surfaces do not provide a safe equivalent writer for this flow.

See the [complete audited matrix](website/guide/agents.md) and [catalog methodology](docs/agent-catalog.md). Every writable target's global path remains editable; paths inside the home directory are normalized to the portable `~/…` form.

Skill consumption supports **45 separately verified user-level Agent capabilities** across CLI, IDE, and desktop products. Only capabilities detected on the current machine appear, and Agents sharing one physical compatibility directory are selected and reviewed as an inseparable impact group. Managed links expose one live central copy, so consumer-side edits are detected as central drift rather than isolated copies. See the [Skills guide](website/guide/skills.md).

---

## Desktop app

Grab the **Desktop installer · Apple Silicon** asset from the latest stable [**Release**](../../releases/latest). The app checks that stable channel automatically and also exposes a manual **Check for updates** action. Installing the app makes its bundled `mux` CLI available through `~/.local/bin/mux` when that directory is on `PATH`.

The normal delivery flow produces a versioned **Pre-release** for each ordinary `main` change, while one rolling Release PR proposes the next Stable version. Pre-releases never publish `latest.json` and are not offered by the in-app updater. During the audited 10-day Fast Lane recorded in `.github/fast-lane.json`, one current `main` push instead creates the next patch Stable release directly: Release Please and the redundant Pre-release are paused, and Quality remains asynchronous rather than blocking Stable publication. Immutable tags, release provenance, version checks, signing, App/DMG inspection, updater/CLI packaging, complete-asset validation, and semantic-version latest-channel ordering remain mandatory. The committed deadline cannot be extended by changing only the live repository variable, and the temporary bypass expires automatically with the Fast Lane.

Build from source:

```bash
cd desktop
npm ci
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
├── settings.json           # agents · sources · central metadata · desired consumption state
├── sources/
│   ├── remote/<id>.json    # cached copies of subscribed URLs
│   └── local/<id>.(json|toml)   # imported local files + the managed manual/discovered sources
├── skills/                 # one managed central copy per Skill
├── staging/skills/         # resolved Skill candidates and internal Skill operations
├── staging/consumption/    # reviewed cross-domain plans and durable rollback snapshots
├── backups/                # timestamped backups made before managed writes
│   └── skills/             # reversible Skill replacements, imports, and removals
└── journals/skills/        # crash-recovery progress for committed Skill operations
```

Skills-specific runtime paths:

```text
~/.mux/skills/                  managed Skill contents
~/.mux/staging/skills/          staged candidates and internal operation plans
~/.mux/backups/skills/          reversible replacements/removals
~/.mux/journals/skills/         crash recovery journals
```

Model API keys are not stored under `~/.mux/`; they remain in macOS Keychain.

## How it works

1. **Build the central libraries** — subscribe or import MCP sources, create Model Profiles, and directly download or import Skills. No Agent target is changed during central intake.
2. **Choose consumers** — from an Agent page, select compatible assets. MCPs and Skills are sets; supported multi-model Agents also keep a Model Profile set plus one independent current pointer.
3. **Review one impact plan** — MUX shows central changes, relationship changes, target files, shared Skill-directory impact, drift, and conflicts before commit.
4. **Commit and verify** — settings, Agent targets, and central lifecycle changes are applied as a recoverable transaction and rescanned before reporting success.
5. **Migrate historical state explicitly** — MUX detects unmanaged global MCPs, Model Profiles, and user-level Skills without taking ownership. After one confirmation, each selected asset is imported and its original Agent relationships are adopted in one recoverable per-asset transaction.
6. **Propagate central lifecycle changes** — updates reach every desired consumer; deletion clears all managed targets and relationships instead of leaving implicit orphan copies.

Skills in this version are user-level only. Project-level Skills, private repositories, Skill editing, and CLI/TUI Skills commands are not supported.

## Development

A Cargo workspace plus the Tauri desktop app:

```
core/           # mux-core — the shared Rust core (types, settings, sources, adapters, ops)
cli/            # mux-cli  — the clap-based `mux` binary, built on mux-core
desktop/        # Tauri v2 (Rust, depends on mux-core) + React 19 + Vite + Tailwind v4
data/           # audited agent definitions + discovery catalog + curated MCP collection
```

The desktop app is a separate build (`exclude`d from the workspace) so its Tauri bundle output path stays put.

```bash
cargo test                            # mux-core + mux-cli
cd desktop/src-tauri && cargo test    # Rust core + integration tests (desktop)
cd desktop && npm run build           # desktop frontend (tsc + vite)
node scripts/update-agent-catalog.mjs # refresh the public client discovery catalog
```

## License

[MIT](LICENSE) © Scoheart
