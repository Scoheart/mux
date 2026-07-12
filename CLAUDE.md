# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

MUX manages MCP (Model Context Protocol) servers across AI coding agents. It has **two front-ends that share one data directory** (`~/.mux/`), both built on **one shared Rust core crate, `mux-core` (`core/`)**:

- **CLI + TUI** — Rust, `cli/` (clap; bin `mux`). Subcommands for scripting; no-arg `mux` launches an interactive **ratatui TUI** (`cli/src/tui/`, The Elm Architecture over `mux-core` — see `docs/tui-architecture.md`). `MUX_NO_TUI=1` forces the help fallback.
- **Desktop app** — Tauri v2 (`desktop/src-tauri/src/`, depends on `mux-core`) + React 19 in `desktop/src/`.

Because both front-ends consume the same `mux-core`, **the data model lives in exactly one place (`core/src/`) — edit it once.** (Historically it had to be changed twice, once in Rust and once in a parallel TypeScript CLI; that TS CLI was removed and its logic folded into `mux-core`.) **Orchestration is shared too and lives in core, not in a front-end**: install/uninstall/import/clean, registry upsert/remove/paste (with edit-propagation), install-status scan, and enable/disable/delete all live in `core/src/ops.rs`; source management (subscribe/local/refresh/toggle/remove) in `core/src/sources.rs`; agent put/list in `core/src/agents.rs` — all tauri-free. The desktop Tauri commands (`commands.rs`) and the CLI are thin delegators over these. When adding an operation, put the logic in core and delegate, so the front-ends can't diverge.

The repo is a **Cargo workspace** (`core`, `cli`) with the **desktop `exclude`d** from it — so Tauri's bundle output stays at `desktop/src-tauri/target/` and CI's dmg path is unaffected.

## Commands

CLI / core (repo root):
```bash
cargo test                     # mux-core + mux-cli
cargo test -p mux-core         # core only
cargo build --release -p mux-cli   # → target/release/mux
cargo install --path cli       # install the `mux` binary
```

Desktop:
```bash
cd desktop && npm run build    # tsc + vite build (frontend)
bash desktop/scripts/prepare-sidecar.sh          # stage the CLI sidecar — required once before any src-tauri cargo build/test (externalBin is validated at compile time)
cd desktop/src-tauri && cargo test               # Rust unit + integration tests
cargo test --test sources_flow                   # one integration test file
cd desktop && npm run tauri dev                  # run the app (needs a display)
cd desktop && npm run tauri build -- --bundles dmg   # beforeBuildCommand stages the sidecar automatically
cd desktop && npm run tauri -- icon <1024.png>   # regenerate the icon set
```

There is a `run-desktop` skill for driving the GUI headlessly (Xvfb + screenshots); headless WebKit shows a black **first** frame — that's an environment artifact, not a bug.

## Architecture — the parts that span files

### Source-based catalog (no built-in base)
The catalog is **assembled from `settings.sources`**, not a hardcoded list. `read_registry()` (`core/src/registry.rs`) flattens every enabled source and dedupes by composite key, with precedence **low→high: external sources (remote/local) < discovered < manual** (the user's own edits win).

Source kinds live under `~/.mux/sources/`:
- `remote/<id>` — a subscribed URL, fetched (Rust uses `ureq`) and cached.
- `local/<id>` — a file imported from disk, **and** two *managed* sources: `manual.json` (手动添加) and `discovered.json` (自动探索).

**Manual and discovered entries are stored as files** (`write_manual_entry` / `write_discovered_entry`), **not** in `settings.registry`. A startup migration folds any legacy `settings.registry` into these files. `data/registry.json` is the bundled "official collection" — offered only as an opt-in remote subscription, not the base catalog.

### Identity, transport, provenance
- Composite key is **`name::transport`** (`transport ∈ {stdio, http}`; sse lives under http). Same-named stdio/http are distinct catalog items.
- Transport is auto-detected by the untagged `McpConfig` enum (`core/src/types.rs`): `command` ⇒ stdio, `url` ⇒ http. The http `type` field is a free-form string (`http`/`sse`/`streamable-http`/custom).
- `RegistryOrigin.kind` ∈ `discovered|manual|remote|local` (+ `source` id) drives the UI provenance tags.

### settings.json — single doc, cross-tool passthrough
All user data is one `~/.mux/settings.json`. **`mux-core` fully types only the sections it owns and passes the rest through opaquely** (via `#[serde(flatten)] extra`) so a future co-writer isn't clobbered. Every mutation is **read-whole → modify one section → write-whole atomically** (`mutate_settings` under a `static Mutex`, temp-file + rename).

### Agent config adapters — never rewrite the whole section
Agent files are edited through the `Adapter` trait (`core/src/adapter.rs`, `json_adapter.rs`, `toml_adapter.rs`): `read` / `upsert(one server)` / `remove(one server)`. **These operate per-server and preserve sibling servers' raw bytes** — a past data-loss bug came from rewriting the whole `mcpServers` map, so keep single-entry semantics. Installs also back up the target file first (`core/src/applier.rs`).

### Config-path portability (hard rule)
Stored agent paths must use `~/…`, **never** a hardcoded home like `/Users/name/…`. Use `collapse_home` on write and `expand_tilde` on read — both in `core/src/scanner.rs`.

### Edit propagation (auto-sync on save)
Saving a catalog entry whose **base config actually changed** auto-syncs the new config into **every** agent that has it actively installed at global scope — drifted and hand-customized copies included (each write is backed up first). `upsert_entry`/`remove_entry` (`core/src/ops.rs`) do this via `autosync_after_edit` → `resync_entry(force=true)`, and return the synced agent names so both front-ends can report「已同步 N 个 agent」. Description/tags-only edits, project-scope installs, and disabled-store entries are untouched. (This replaced an older conservative "clean installs only" propagation that left drifted installs permanently stale and forced a manual 重新同步 after every edit.) **`ops::resync_entry(name, transport, force)`** remains as the manual repair for installs that drifted *without* a registry edit: `force=false` skips customized installs and reports them in `skipped_customized`; `force=true` overwrites. Exposed as the desktop `resync_entry` command + the editor's「重新同步」button, and the TUI Registry `S` key (customized → Confirm → force).

### Deleting a catalog entry
**`ops::forget_entry(name, transport)`** (`core/src/ops.rs`) deletes a user-owned entry from the **manual and discovered** managed sources AND uninstalls it from every agent that has it (global; active or disabled-store). Only manual/discovered entries are deletable this way — remote/local source entries have nothing user-owned to remove (manage them via their source). Exposed as the desktop `forget_entry` command + the Registry card/detail 删除 button (shown only for manual/discovered), and the TUI Registry `d` key (→ Confirm; hint for remote/local).

### Self-update (stable channel only)
Both front-ends update from the **newest stable `vX.Y.Z` GitHub Release** — per-push `-build.N` pre-releases never reach users:
- **Desktop**: `tauri-plugin-updater` polls `releases/latest/download/latest.json` (endpoint + minisign pubkey in `tauri.conf.json`; `bundle.createUpdaterArtifacts` on). The UX lives in `desktop/src/hooks/useUpdater.ts` + `components/UpdateBanner.tsx` (silent startup check, non-blocking card, per-version "稍后" dismissal in localStorage); manual check = clicking the header version number. **Because `createUpdaterArtifacts` is on, every `tauri build` needs the signing key**: CI uses the `TAURI_SIGNING_PRIVATE_KEY`(+`_PASSWORD`) secrets; locally export `TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/mux_updater.key)` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""` first. On tag builds CI signs the `.app.tar.gz` and publishes it + `latest.json` with the Release.
- **CLI**: `mux upgrade` (logic in `core/src/update.rs`) replaces the running binary from the release `tar.gz`; other commands print a once-a-day passive "new version" notice (cache `~/.mux/update-check.json`, opt out with `MUX_NO_UPDATE_CHECK=1`). When the running `mux` resolves into a `.app` bundle (`update::managed_by_desktop_app`), `upgrade` and the passive notice both stand down — that copy updates with the desktop app.

### CLI ships inside the desktop app (sidecar)
The `mux` CLI is bundled into `MUX.app/Contents/MacOS/mux` via `bundle.externalBin` (staged by `desktop/scripts/prepare-sidecar.sh`, wired into `beforeBuildCommand`; the file must exist for **any** src-tauri cargo build/test). On startup the frontend (`useCliTool.ts`) silently symlinks it to `~/.local/bin/mux` (`cli_tool.rs` commands `cli_status`/`install_cli`) — no admin prompt, auto-repairs broken links, and because the link points into the bundle the CLI updates with the app. It refuses to overwrite a real file at that path (e.g. a `cargo install`ed mux). One-time toasts cover first install and a missing `~/.local/bin` in PATH.

### Shared defaults
`data/agents.json` (20 agents) and `data/registry.json` are the single source of truth, embedded by `mux-core` via `include_str!` (`core/src/agents.rs`, `registry.rs`) — so both front-ends share them. Edit those JSON files directly.

## Gotchas

- **Test env isolation**: any test that touches `~/.mux` or agent config paths must use `mux_core::testenv::TestHome` (RAII guard: process-wide mutex + fake `HOME`/`MUX_HOME` + restore-on-drop). Never hand-roll `set_var("HOME")`/`remove_var("HOME")` — `remove_var` is not a restore (`dirs::home_dir()` falls back to the real home) and parallel tests race on the process-global env; this corrupted the real `~/.mux/sources/remote/*` cache on 2026-07-08. With the guard, multiple tests per file are safe. `MUX_HOME` (the data dir itself, like `CARGO_HOME`) is also a user-facing override honored by `paths::mux_dir()`.
- **CI**: pushing to `main` builds a macOS `.dmg` pre-release (`.github/workflows/build-desktop.yml`). Don't re-add `sccache` (its GHA-cache backend broke the build); changing `[profile.release]` invalidates the Rust cache and forces one cold rebuild.
- The live desktop nav is Registry / Sources / per-Agent + a full-page editor. (The old orphaned `MatrixView`/`ServerDetail`/`InstallDialog`/`RegistryGrid` components were deleted; `preview_install` and the `overrides` patch path in Rust are kept — test-covered request surface with no current UI caller.)
