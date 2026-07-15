# CLI / TUI

`mux` is a native Rust binary, built on the same `mux-core` as the desktop app, with everything running against the shared `~/.mux/`.

> Not installed yet? See [Installation · CLI](/en/guide/install#cli-tui-mux).

It has two ways to use it:

- **No arguments** → enter the interactive **TUI** (a keyboard-driven terminal manager);
- **With subcommands** → non-interactive and scriptable.

## The interactive TUI

```bash
mux
```

The TUI has three screens, switched with number keys at the top:

| Key | Screen |
|---|---|
| `1` | Registry (catalog) |
| `2` | Sources |
| `3` | Agents |

### Common keys

| Key | Action |
|---|---|
| `↑`/`k`, `↓`/`j` | Move up / down |
| `Tab` / `Shift-Tab` | Move forward / backward across the three screens |
| `?` | Show help / keymap |
| `q` or `Ctrl-C` | Quit |
| `Ctrl-R` | Refresh |

### Registry screen

| Key | Action |
|---|---|
| `/` | Search |
| `[`/`]` or `←`/`→` | Switch filters |
| `i` | Install wizard (multi-select agents, `Space` to check, `Ctrl-S` to confirm) |
| `n` | Create a new entry |
| `e` | Edit the selected entry |
| `p` | Paste an `mcpServers` config block |
| `S` | Resync the selected entry (customized ones confirm first, can be forced) |
| `d` | Delete the selected entry (→ confirm; remote/local show a hint) |

### Sources screen

| Key | Action |
|---|---|
| `Space`/`Enter` | Enable/disable the selected source |
| `r` | Refresh a source |
| `s` | Subscribe to a URL |
| `l` | Import a local file |
| `o` | Add the Mux curated collection |
| `d` | Delete a source (→ confirm) |

### Agents screen

| Key | Action |
|---|---|
| `Enter`/`→`/`l` | Enter an agent to see which MCP servers it has |
| `Space` | Toggle the agent (list level) / toggle an installed MCP (detail level) |
| `a` | Add an MCP to the agent |
| `e` | Edit the agent's config path |
| `n` | Add a custom agent |
| `d` | Uninstall the selected MCP from the agent (detail level) |

## Subcommands (scripting)

When used in scripts, set `MUX_NO_TUI=1` so running with no arguments prints help instead of entering the TUI.

```bash
mux import                       # scan each agent and import detected servers
mux list                         # list the entries in the catalog
mux status                       # the MCP servers currently active per agent
mux add <name>                   # interactively add a server to the manual source
mux remove <name>                # remove an entry from the manual source
mux apply <names…>               # non-interactive install
mux export [--out <file>]        # export the deduplicated effective catalog; defaults to stdout
mux clean [--agent <name>]       # clear the MCP servers of (enabled) agents
mux agents                       # list all agents
mux agents enable <name>         # enable an agent
mux agents disable <name>        # disable an agent
mux upgrade                      # upgrade a standalone CLI install
```

### Arguments for `mux apply`

```bash
mux apply github filesystem \
  --agent all             # comma-separated agent names, or "all" (default all)
```

`mux apply` writes only to agents' global config.

Example: install `github` into just Claude Code and Cursor:

```bash
mux apply github --agent "claude-code,cursor"
```

If a same-named entry has both stdio and HTTP variants, `mux apply <name>` handles all transport variants under that name.

### Export

```bash
mux export                    # JSON to stdout
mux export --out mcp.json     # save to a file
```

The export is the complete **effective catalog**: only the highest-precedence copy is kept per `name::transport`.

### Updates

A CLI downloaded standalone or installed via `cargo install` can run:

```bash
mux upgrade
```

Regular subcommands check for the latest stable release at most once a day after running; set `MUX_NO_UPDATE_CHECK=1` to turn this off. The CLI bundled with the desktop app is updated by the app and won't replace the bundled binary itself.

## Relationship with the desktop app

Both read and write the same `~/.mux/`. An entry added via `mux add` in the CLI is visible in the desktop app after a refresh; an install done in the desktop app shows up in `mux status`. The data model exists in one place and never forks.

Next → [Supported agents](/en/guide/agents)
