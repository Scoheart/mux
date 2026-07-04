# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

MUX manages MCP (Model Context Protocol) servers across AI coding agents. It has **two front-ends that share one data directory** (`~/.mux/`):

- **CLI/TUI** — TypeScript, `src/` (published as `@scoheart/mux`, bin `mux`).
- **Desktop app** — Tauri v2 (Rust core in `desktop/src-tauri/src/`) + React 19 in `desktop/src/`.

Because both read/write the same `~/.mux/settings.json` and `~/.mux/sources/`, **most data-model changes must be made twice — once in Rust (`desktop/src-tauri/src/core/`) and once in TS (`src/core/`)** — or the two tools diverge.

## Commands

CLI (repo root):
```bash
npm test                       # vitest (watch)
npx vitest run                 # single pass
npx vitest run tests/core/sources.test.ts   # one file
npx tsc --noEmit               # typecheck
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
The catalog is **assembled from `settings.sources`**, not a hardcoded list. `read_registry()` (`core/registry.rs`) / `readRegistry()` (`src/core/registry.ts`) flatten every enabled source and dedupe by composite key, with precedence **low→high: external sources (remote/local) < discovered < manual** (the user's own edits win).

Source kinds live under `~/.mux/sources/`:
- `remote/<id>` — a subscribed URL, fetched (Rust uses `ureq`) and cached.
- `local/<id>` — a file imported from disk, **and** two *managed* sources: `manual.json` (手动添加) and `discovered.json` (自动探索).

**Manual and discovered entries are stored as files** (`write_manual_entry` / `write_discovered_entry`), **not** in `settings.registry`. A startup migration folds any legacy `settings.registry` into these files. `data/registry.json` is the bundled "official collection" — offered only as an opt-in remote subscription, not the base catalog.

### Identity, transport, provenance
- Composite key is **`name::transport`** (`transport ∈ {stdio, http}`; sse lives under http). Same-named stdio/http are distinct catalog items.
- Transport is auto-detected by the untagged `McpConfig` enum (`core/types.rs`): `command` ⇒ stdio, `url` ⇒ http. The http `type` field is a free-form string (`http`/`sse`/`streamable-http`/custom).
- `RegistryOrigin.kind` ∈ `discovered|manual|remote|local` (+ `source` id) drives the UI provenance tags.

### settings.json — single doc, cross-tool passthrough
All user data is one `~/.mux/settings.json`. **Each tool fully types only the sections it owns and passes the rest through opaquely** so neither clobbers the other: Rust uses `#[serde(flatten)] extra`; TS uses an index signature. Every mutation is **read-whole → modify one section → write-whole atomically** (`mutate_settings` under a `static Mutex`, temp-file + rename).

### Agent config adapters — never rewrite the whole section
Agent files are edited through the `Adapter` trait (`core/adapter.rs`, `json_adapter.rs`, `toml_adapter.rs`): `read` / `upsert(one server)` / `remove(one server)`. **These operate per-server and preserve sibling servers' raw bytes** — a past data-loss bug came from rewriting the whole `mcpServers` map, so keep single-entry semantics. Installs also back up the target file first (`core/applier.rs`).

### Config-path portability (hard rule)
Stored agent paths must use `~/…`, **never** a hardcoded home like `/Users/name/…`. Use `collapse_home` (commands.rs) on write and `expand_tilde` (scanner.rs) on read.

### Edit propagation
Editing a catalog entry re-stamps the new config into agents that installed it *clean* (on-disk config == the pre-edit registry config); hand-customized installs are left untouched (`propagate_edit_to_installs` in `commands.rs`).

### Shared defaults
`data/agents.json` (18 agents) and `data/registry.json` are the single source of truth, embedded on both sides (`include_str!` in Rust, JSON import in TS). Edit those JSON files directly.

## Gotchas

- **Integration-test `$HOME` races**: `cargo test` runs a file's tests in parallel threads. Tests that `std::env::set_var("HOME", …)` to isolate `~/.mux` must be **one test per file** (or merged into one) — two in the same binary clobber each other's HOME.
- **CI**: pushing to `main` builds a macOS `.dmg` pre-release (`.github/workflows/build-desktop.yml`). Don't re-add `sccache` (its GHA-cache backend broke the build); changing `[profile.release]` invalidates the Rust cache and forces one cold rebuild.
- The live desktop nav is Registry / Sources / per-Agent + a full-page editor. (The old orphaned `MatrixView`/`ServerDetail`/`InstallDialog`/`RegistryGrid` components were deleted; `preview_install` and the `overrides` patch path in Rust are kept — test-covered request surface with no current UI caller.)
