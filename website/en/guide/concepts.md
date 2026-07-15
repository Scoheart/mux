# Core concepts

Understand these few concepts and every MUX operation will make sense.

## The Registry (catalog)

The **catalog** is the heart of MUX — an aggregated view of all your MCP servers. It is **not** a list hardcoded into the program, but the **live union** of every [source](#sources) you have enabled.

In the catalog you can search, filter by source, inspect shadowed copies, and see each entry's transport, source, and usage.

## Sources

Every entry in the catalog comes from a source. MUX has four kinds:

| Source | What it is | Where it's stored |
|---|---|---|
| **Subscribe (remote)** | A **URL** pointing at an MCP config file; MUX fetches and caches it, and re-pulls upstream on refresh. | `~/.mux/sources/remote/<id>.json` |
| **Local** | A config file **imported** from disk and copied into MUX; refresh re-reads the original file. | `~/.mux/sources/local/<id>.(json\|toml)` |
| **Manual** | A server you write by hand or **paste** in, stored as a managed local source (`manual.json`). | `~/.mux/sources/local/manual.json` |
| **Discovered** | Servers **auto-detected** at startup from the config your agents already have (`discovered.json`). | `~/.mux/sources/local/discovered.json` |

There is also a one-click **Mux curated collection** — which is really just a subscription to a built-in, curated remote source. It's an **optional** subscription, not a default base.

Only **enabled** sources take part in assembling the catalog. The source model supports enable/disable, and the TUI's Sources screen can toggle them directly; the desktop `v1.2.0` currently offers filtering, refresh, and delete, but does not yet expose the on/off toggle.

### Precedence (dedup rules)

The same MCP may appear in several sources. The catalog dedupes by the **`name::transport`** composite key, with precedence **from low to high**:

```
external sources (remote / local) < discovered < manual
```

In other words, **your own manual edits always win** — even if a remote source also defines an entry with the same name.

### "All" and "shadowed" filters

The desktop Registry shows every copy from each source by default:

- The highest-precedence copy displays normally and takes part in install and export.
- The remaining copies are marked **shadowed**, showing "superseded by <source>".
- When there's a conflict in the current scope, a **Shadowed N** toolbar item appears, letting you view just those copies.
- "All" on the left means all sources; there is no longer a separate "effective" entry point. After clicking a specific source, the conflict filter applies only to that source.

## Identity, transport, and source tags

- **Composite key `name::transport`**. `transport ∈ {stdio, http}` (sse falls under http). A same-named stdio and http are **two independent entries**.
- **Transport is auto-detected**: MUX recognizes standard fields and agent-specific ones — e.g. OpenCode's command array, Gemini's `httpUrl`, Windsurf's `serverUrl`. The catalog normalizes everything to a stdio / http model, then converts to the target agent's fields and transport name on write.
- **Source tags**: each entry carries a source kind (`discovered` / `manual` / `remote` / `local`) plus a source id, which drives the source badges in the UI.

## Install, toggle, delete

Operated independently **per agent**:

- **Install**: write a catalog entry into that agent's real config file (backing it up first).
- **Disable**: first persist the server's complete semantic config (including agent-specific policy), then remove it from the agent config; it can be safely restored anytime.
- **Enable**: write a disabled server back in.
- **Delete**: uninstall from an agent (or delete it from the catalog entirely, see below).

## Edit propagation

When you change a catalog entry's connection config, MUX re-stamps the new config into every global agent still using that entry, including copies that were manually customized on disk. Each target file is backed up before writing; changes to description or tags do not trigger a sync.

The explicit remedy is **Resync**: re-stamp the current config into every global agent that has the entry actively installed.

- Not forced: skips customized installs and reports them.
- Forced: overwrites even the customized ones.

## Forget a catalog entry

To **delete a user-owned entry entirely**: remove it from both the manual and discovered managed sources, **and** uninstall it from every agent that has it (global, whether active or disabled).

Only **manual / discovered** entries can be deleted this way — entries from remote / local sources have no "user-owned" part to remove, so manage them through the source that provides them.

## Data layout

All user data lives under `~/.mux/`:

```
~/.mux/
├── settings.json           # one document: agents · sources · disabled · state
├── update-check.json       # daily update-check cache for the standalone CLI (created on demand)
├── sources/
│   ├── remote/<id>.json    # cache of subscribed URLs
│   └── local/<id>.(json|toml)  # imported files + manual.json / discovered.json
└── backups/                # timestamped backups made before writing to an existing agent file
```

`settings.json` is **one document**: MUX modifies only the sections it owns and passes the rest through. Every change is **read the whole file → modify one section → write back atomically** (temp file + rename).

Agent JSON / TOML / YAML configs are updated by "locate the MCP node → locate the target entry → modify managed connection fields." Other top-level keys, other servers, unmodeled fields inside the target entry (permissions / OAuth / tool policies), comments, and original formatting are all preserved; existing files are backed up first, then atomically replaced via a temp file in the same directory. Writes are refused when a backup fails, on concurrent modification, or when the file or node structure is invalid — MUX will not try to repair or overwrite the whole config.

## Portable config paths

Agent config paths inside the home directory are collapsed to `~/…` for easy migration; custom absolute paths outside the home directory keep their original value. When syncing `~/.mux/` across machines, still double-check those external paths.

Next → [Desktop app](/en/guide/desktop) or [CLI / TUI](/en/guide/cli)
