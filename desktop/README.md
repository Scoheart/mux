# MUX Desktop

Tauri + React desktop app for MCP configuration management.

## Development

```bash
cd desktop
npm install
npm run tauri dev   # Opens the MUX window (requires a display)
```

## Features (Plan 2)

- Browse and search the built-in MCP registry (40+ servers)
- Install to global or project scope, multiple agents in one operation
- Per-agent env overrides
- Preview the exact JSON that will be written before applying
- Automatic backup to `~/.mux/backups/` before every write

## Data

- Server/agent definitions: repo root `data/registry.json` / `data/agents.json` (shared with CLI)
- Overrides: `~/.mux/overrides.json`

## Manual Smoke Test (requires display — cannot run in headless CI)

This step MUST be run by a human on a machine with a graphical display:

```bash
cd desktop
npm run tauri dev
```

Verify the following:

1. Window opens and shows the registry grid with 40+ server cards.
2. Typing "git" in the search box filters the cards correctly.
3. Clicking a card opens the install dialog.
4. Select scope "全局" (global), check "claude-code", click "预览改动" (Preview) — a preview panel appears showing the target file path and config JSON.
5. Click "应用" (Apply) — success message "✅ 已应用" appears.
6. Confirm with `cat ~/.claude.json` (or the agent's config file) that the server was added to `mcpServers`.
7. Confirm `ls ~/.mux/backups/` shows a backup file.
8. Close and reopen the app — the card for that server shows "已装 1 处".

Record results in the PR description or commit message.

## Build (headless / CI)

```bash
# TypeScript + Vite compile check (no display needed)
cd desktop && npm run build

# Rust compile check (no display needed)
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --manifest-path desktop/src-tauri/Cargo.toml

# Unit tests (no display needed)
cargo test --manifest-path desktop/src-tauri/Cargo.toml
```
