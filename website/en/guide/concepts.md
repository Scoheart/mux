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
| **Legacy discovered** | Managed entries already written to `discovered.json` by older versions; they remain central assets after upgrade, but new scans no longer write here automatically. | `~/.mux/sources/local/discovered.json` |

There is also a one-click **Mux curated collection** — which is really just a subscription to a built-in, curated remote source. It's an **optional** subscription, not a default base.

A new item found in an Agent file is not a fifth source. It is independent, read-only **external observed state**. Only an explicit import makes it a central asset, and importing still does not establish a consumption relationship.

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

## Central assets and consumption relationships

Central assets define what an item is; a consumption relationship defines which Agent should use it. The Desktop Agent page and asset Inspector edit the same desired state:

- **MCP / Skills**: an Agent can consume `0..N` central assets.
- **Model**: an Agent can consume at most `0..1` Profile at a time.
- **Remove use**: delete the relationship and that Agent's managed target without deleting the central asset.
- **Observed state**: Agent files and Skill links are reconciliation evidence only; external content, drift, and conflicts never write themselves back into desired state.

## Edit propagation

When a central asset changes, MUX first enumerates all desired consumers and puts the central change, relationships, and target files into one impact plan. Clean targets propagate after one confirmation. Drift requires explicit overwrite confirmation; conflicts or concurrent changes block the whole commit, so there is no partial update.

## Forget a catalog entry

To **delete a user-owned central entry entirely**, first review every consumer, then remove the owned source copy together with every desired relationship and managed Agent target in one transaction.

Only **manual / discovered** entries can be deleted this way — entries from remote / local sources have no "user-owned" part to remove, so manage them through the source that provides them.

## Data layout

All user data lives under `~/.mux/`:

```
~/.mux/
├── settings.json           # one document: agents · sources · central metadata · desired state
├── update-check.json       # daily update-check cache for the standalone CLI (created on demand)
├── sources/
│   ├── remote/<id>.json    # cache of subscribed URLs
│   └── local/<id>.(json|toml)  # imported files + manual.json / discovered.json
├── staging/consumption/    # reviewed plans and durable transaction snapshots
└── backups/                # timestamped backups made before writing to an existing agent file
```

`settings.json` is **one document**: MUX modifies only the sections it owns and passes the rest through. Every change is **read the whole file → modify one section → write back atomically** (temp file + rename).

Agent JSON / TOML / YAML configs are updated by "locate the MCP node → locate the target entry → modify managed connection fields." Other top-level keys, other servers, unmodeled fields inside the target entry (permissions / OAuth / tool policies), comments, and original formatting are all preserved; existing files are backed up first, then atomically replaced via a temp file in the same directory. Writes are refused when a backup fails, on concurrent modification, or when the file or node structure is invalid — MUX will not try to repair or overwrite the whole config.

## Portable config paths

Agent config paths inside the home directory are collapsed to `~/…` for easy migration; custom absolute paths outside the home directory keep their original value. When syncing `~/.mux/` across machines, still double-check those external paths.

Next → [Desktop app](/en/guide/desktop) or [CLI / TUI](/en/guide/cli)
