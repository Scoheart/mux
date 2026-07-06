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
cd desktop/src-tauri && cargo test               # Rust unit + integration tests
cargo test --test sources_flow                   # one integration test file
cd desktop && npm run tauri dev                  # run the app (needs a display)
cd desktop && npm run tauri build -- --bundles dmg
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

### Edit propagation
Editing a catalog entry re-stamps the new config into agents that installed it *clean* (on-disk config == the pre-edit registry config); hand-customized and project-scope installs are left untouched (`propagate_edit_to_installs` in `core/src/ops.rs`, run by `upsert_entry`/`remove_entry`). Because that auto-propagation is deliberately conservative (global-scope + clean only), an install that ever drifted stays stale. The explicit escape hatch is **`ops::resync_entry(name, transport, force)`** (`core/src/ops.rs`): it re-stamps the current config to all agents that have the entry actively installed at global scope — `force=false` skips customized installs and reports them in `skipped_customized`; `force=true` overwrites. Exposed as the desktop `resync_entry` command + the editor's「重新同步」button, and the TUI Registry `S` key (customized → Confirm → force).

### Deleting a catalog entry
**`ops::forget_entry(name, transport)`** (`core/src/ops.rs`) deletes a user-owned entry from the **manual and discovered** managed sources AND uninstalls it from every agent that has it (global; active or disabled-store). Only manual/discovered entries are deletable this way — remote/local source entries have nothing user-owned to remove (manage them via their source). Exposed as the desktop `forget_entry` command + the Registry card/detail 删除 button (shown only for manual/discovered), and the TUI Registry `d` key (→ Confirm; hint for remote/local).

### Shared defaults
`data/agents.json` (18 agents) and `data/registry.json` are the single source of truth, embedded by `mux-core` via `include_str!` (`core/src/agents.rs`, `registry.rs`) — so both front-ends share them. Edit those JSON files directly.

## Gotchas

- **Integration-test `$HOME` races**: `cargo test` runs a file's tests in parallel threads. Tests that `std::env::set_var("HOME", …)` to isolate `~/.mux` must be **one test per file** (or merged into one) — two in the same binary clobber each other's HOME.
- **CI**: pushing to `main` builds a macOS `.dmg` pre-release (`.github/workflows/build-desktop.yml`). Don't re-add `sccache` (its GHA-cache backend broke the build); changing `[profile.release]` invalidates the Rust cache and forces one cold rebuild.
- The live desktop nav is Registry / Sources / per-Agent + a full-page editor. (The old orphaned `MatrixView`/`ServerDetail`/`InstallDialog`/`RegistryGrid` components were deleted; `preview_install` and the `overrides` patch path in Rust are kept — test-covered request surface with no current UI caller.)
