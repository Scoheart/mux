# MUX — MCP Multiplexer

Unified MCP Server configuration manager for AI coding agents.

Manage MCP configs across Claude Code, Codex, Cursor, Qoder, VS Code, Zed, Windsurf, and 10+ more tools from a single interactive TUI.

## Install

```bash
npm install -g @scoheart/mux
# or run directly
npx @scoheart/mux
```

## Quick Start

```bash
# Launch the interactive TUI
mux
```

First launch will scan your existing tool configs and offer to import discovered MCP servers into the registry.

## Features

- **40+ Built-in MCP Servers** — Playwright, GitHub, Figma, Supabase, Stripe and more, ready to apply
- **Interactive TUI** — Fuzzy search, keyboard navigation, shimmer animations
- **Multi-tool sync** — One config, applied to Claude Code, Cursor, Codex, VS Code, etc.
- **Scope control** — Apply MCPs globally or per-project
- **Auto-discovery** — Scans existing configs and imports them
- **Backup & restore** — Auto-backup before any changes

## TUI Controls

| Key | Action |
|-----|--------|
| `←/→` or `Tab` | Switch tabs (Project / Global / Registry) |
| `↑/↓` | Navigate list |
| `Space` | Toggle MCP on/off |
| `Enter` | Apply (Registry) / Open config |
| `/` or `s` | Focus search |
| `e` | Edit MCP config |
| `d` | Delete MCP |
| `n` | Add new MCP |
| `Ctrl+S` | Apply changes |
| `Esc` | Back / Cancel |
| `q` | Quit |

## CLI Commands

```bash
mux                              # Interactive TUI
mux agents                       # List all agents
mux agents enable <name>         # Enable an agent
mux agents disable <name>        # Disable an agent
```

## Built-in MCP Servers (40)

### Development & Code
filesystem, git, memory, sequential-thinking, github, gitlab, playwright, deepwiki

### Browser & Automation
chrome-devtools, puppeteer, browserbase

### Search & Web
fetch, brave-search, exa, tavily, firecrawl, context7

### Database
sqlite, postgres, mysql

### Design & Collaboration
figma (HTTP), figma-context (stdio), linear, slack, notion, obsidian

### Cloud & Infrastructure
aws, azure, cloudflare, supabase, vercel, google-workspace

### Monitoring & Testing
sentry, everything

### Other
e2b, pipedream, stripe, cal, screenpipe, aws-docs, screenpipe

## Supported Agents

| Agent | Format | Default |
|-------|--------|---------|
| Claude Code | JSON | enabled |
| Claude Desktop | JSON | enabled |
| Cursor | JSON | enabled |
| Codex | TOML | enabled |
| Qoder | JSON | enabled |
| VS Code | JSON | enabled |
| Zed | JSON | enabled |
| Windsurf | JSON | enabled |
| Roo Code | JSON | enabled |
| Gemini CLI | JSON | enabled |
| Devin | JSON | enabled |
| Kiro | JSON | enabled |
| Junie | JSON | enabled |
| Amazon Q | JSON | enabled |
| OpenCode | JSON | enabled |

## How It Works

```
~/.mux/
├── registry/          # One JSON file per MCP definition
├── agents.json        # Agent paths + format + enabled state
├── state.json         # Last applied state
├── .imported          # Import marker (skip scan after first import)
└── backups/           # Auto-backup before changes
```

1. **Registry** stores all known MCPs (built-in + custom) as individual JSON files
2. **TUI** reads current state from actual tool configs (bidirectional sync)
3. **Select** MCPs, configure scope (global/project) and target agents
4. **Apply** writes configs to selected tools; deselect removes them

## License

MIT
