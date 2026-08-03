# CLI / TUI

`mux` is a native Rust binary built on the same `mux-core` as the desktop app. Both use the central assets, Agent relationships, and transaction state under `~/.mux/`.

> Not installed yet? See [Installation · CLI](/en/guide/install#cli-tui-mux).

It has two entry points:

- **No arguments**: enter the MCP-focused compatibility terminal manager (TUI).
- **With subcommands**: query or script MCP, Model, Skill, and Agent operations through one command model.

## Interactive TUI

```bash
mux
```

The no-argument TUI focuses on MCP compatibility management and has three screens:

| Key | Screen |
|---|---|
| `1` | Registry (MCP catalog) |
| `2` | Sources (MCP sources) |
| `3` | Agents (per-Agent MCP state) |

It can search and maintain the MCP catalog, sources, and Agent MCP installation state. Use the subcommands below for scriptable Model and Skill relationship management and explicit physical repair; the desktop app remains available for the full visual asset lifecycle.

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
| `i` | Installation wizard (multi-select Agents, `Space` to check, `Ctrl-S` to confirm) |
| `n` | Create an entry |
| `e` | Edit the selected entry |
| `p` | Paste an `mcpServers` config block |
| `S` | Resync the selected entry |
| `d` | Delete the selected entry and confirm the impact |

### Sources screen

| Key | Action |
|---|---|
| `Space`/`Enter` | Enable or disable the selected source |
| `r` | Refresh a source; external discovery only re-detects and never imports |
| `s` | Subscribe to a URL |
| `l` | Import a local file |
| `o` | Add the MUX curated collection |
| `d` | Delete a source and confirm the impact |

### Agents screen

| Key | Action |
|---|---|
| `Enter`/`→`/`l` | Enter an Agent and inspect its MCPs |
| `Space` | Toggle the Agent (list level) or an assigned MCP (detail level) |
| `a` | Add an MCP to the Agent |
| `e` | Edit the Agent's config path |
| `n` | Add a custom Agent |
| `d` | Unassign the selected MCP from the Agent |

## Subcommand overview

In scripts, set `MUX_NO_TUI=1` so running without arguments prints help instead of entering the TUI.

```text
mux mcp {list,show,status,assign,unassign,enable,disable,reapply,add,delete,export}
mux model {list,show,status,assign,unassign,enable,disable,reapply,use}
mux skill {list,show,status,assign,unassign,enable,disable,reapply}
mux agent {list,enable,disable}
mux discover [mcp|model|skill]
mux adopt {mcp,model,skill}
mux workspace
mux upgrade
```

MCPs, Models, and Skills share the same query and consumption-relationship verbs. Domain-specific operations remain under their own domain.

| Capability layer | MCP | Model | Skill |
|---|---|---|---|
| Shared queries | `list` / `show` / `status` | `list` / `show` / `status` | `list` / `show` / `status` |
| Shared relationships | `assign` / `unassign` / `enable` / `disable` | `assign` / `unassign` / `enable` / `disable` | `assign` / `unassign` / `enable` / `disable` |
| Explicit physical repair | `reapply --agent` (exact relationship; explicit `--all` also available) | `reapply --agent` (exact relationship) | `reapply --agent` (exact relationship / shared target) |
| Domain-specific | `add` / `delete` / `export` | `use` (select current) | — |

The command counts are not forced to match mechanically. What is unified is relationship semantics, review, and transactional guarantees. MCP catalog export and manual central-asset maintenance, and Model's current pointer, have no honest cross-domain equivalents. Rich central Model and Skill creation, editing, update, and removal remain in Desktop.

## Global options

Global options can appear before or after the domain command and can be combined with any applicable subcommand:

| Option | Effect |
|---|---|
| `--json` | Emit machine-readable JSON only, without tables, colors, or confirmation prompts; mutations must also include `--yes` or `--dry-run` |
| `--yes` | Accept a generated write plan; unresolved drift, conflicts, and concurrent changes still fail |
| `--dry-run` | Generate and print a plan without committing the requested domain mutation |
| `--no-color` | Disable ANSI colors |

For example:

```bash
mux --json skill status --agent codex
mux --dry-run mcp assign github::stdio --agent claude-code
mux --yes --no-color model use work --agent pi
```

`--yes` and `--dry-run` are mutually exclusive and apply only to mutations. Passing them to `list`, `show`, `status`, `discover`, `workspace`, or stdout-only `mcp export` is an error. Because `mcp export --out <path>` creates a file, it also requires interactive confirmation or an explicit `--yes` / `--dry-run`. `mux --json --help` and `mux --json --version` also return schema-v1 success envelopes; a literal `--json` used as an `mcp add --arg` value does not accidentally switch output mode.

## Stable IDs and Agent selection

Every mutation of an existing asset requires its exact stable ID; display names are never fuzzy-matched:

| Domain | Stable ID | Example |
|---|---|---|
| MCP | `name::transport` | `github::stdio`, `github::http` |
| Model | Model Profile ID | `work` |
| Skill | Central Skill name | `review-changes` |
| Agent | Agent ID | `claude-code`, `codex` |

Every Agent-relationship mutation requires exactly one explicit `--agent <id>`. `assign` and `unassign` may incrementally process several exact asset IDs in one command; `enable`, `disable`, and `use` process one at a time. Relationship commands never default to all Agents. `reapply` also defaults to one exact Agent in all three domains; only MCP provides an additional batch form that must be requested explicitly with `--all`.

```bash
mux mcp assign github::stdio filesystem::stdio --agent claude-code
mux skill unassign review-changes source-explainer --agent codex
```

If both `github::stdio` and `github::http` exist, they are separate assets. A command must name each exact ID; a shared name never selects both transports implicitly.

## Shared consumption semantics

| Verb | Semantics |
|---|---|
| `assign` | Incrementally add the specified assets to the Agent's desired relationships without removing other assignments |
| `unassign` | Remove only the specified relationships without deleting central assets |
| `enable` | Keep the relationship and make the specified assets active in the Agent |
| `disable` | Keep the relationship but make the specified assets inactive so they can be restored in place |
| `status` | Compare desired and observed state, including pending, synced, drifted, and conflicted results |

Relationship mutations all pass through `plan → review → commit` and update central relationships and Agent targets in one transaction. `--dry-run` stops after review. `--yes` skips only the interactive confirmation; it never bypasses safety checks.

## MCP

```bash
mux mcp list
mux mcp show github::stdio
mux mcp status
mux mcp status --agent claude-code

mux mcp assign github::stdio --agent claude-code
mux mcp unassign github::stdio --agent claude-code
mux mcp disable github::stdio --agent claude-code
mux mcp enable github::stdio --agent claude-code
```

MCP-specific operations:

```bash
mux mcp add github::stdio --command npx --arg -y --arg @example/server
mux mcp add docs::http --url https://mcp.example.com --http-type streamable-http
mux mcp delete github::stdio       # delete the central asset after reviewing every consumer
mux mcp export                     # write the effective catalog to stdout
mux mcp export --out mcp.json --yes # create a file through the write gate
mux mcp reapply github::stdio --agent claude-code
mux mcp reapply github::stdio --all # explicitly repair every out-of-sync desired consumer
```

`add` defines the MCP completely through arguments instead of prompting for connection fields, and it requires a complete stable ID; commit still follows the shared plan review. A stdio asset requires `--command`, accepts repeatable `--arg`, and optionally accepts `--cwd`. An HTTP asset requires `--url` and can set its native type with `--http-type`. Both accept an optional `--description` and repeatable `--tag`.

`export` contains the complete effective MCP catalog, keeping only the highest-precedence copy of each `name::transport`. Because it can contain credentials, `--out` follows the shared write-confirmation gate, creates only a new `0600` file, and refuses to overwrite an existing target. In JSON file mode it returns only the redacted path and permissions instead of copying the complete catalog to stdout. `reapply --agent` synchronizes only the named desired relationship. `reapply --all` is an explicit batch entry point and includes only currently out-of-sync desired consumers, so clean Agents are not rewritten. Both forms check drift, conflicts, Agent enabled state, and post-review concurrency.

## Model

```bash
mux model list
mux model show work
mux model status --agent pi

mux model assign work backup --agent pi
mux model assign work --agent claude-code --replace
mux model unassign backup --agent pi
mux model disable backup --agent pi
mux model enable backup --agent pi
mux model use work --agent pi
mux model reapply work --agent pi
```

`assign` adds Model Profile relationships by default. If a single-model target already has another Profile, an explicit `--replace` replaces its complete Model selection with the exact IDs in this command; without that option, nothing is removed implicitly. `use` separately sets the Agent's **current model**, so “assigned” and “current” are not collapsed into one action. A native multi-model Agent can have several assigned and enabled Profiles but at most one current Profile; single-model Agents remain constrained by their capabilities during planning.

Relationship verbs change desired state only. Repeating `assign`, `enable`, or `use` is an idempotent no-op even when the observed configuration has drifted, so those commands never overwrite drift as a side effect. `reapply` is the sole explicit physical-repair entry point. It synchronizes only the exact Profile on the named Agent and produces a candidate-hash-bound review when drift exists. If the requested Profile is observed as current but is not the desired current Profile, MUX asks you to reapply the desired current Profile instead of silently widening the repair. A disabled Profile is cleared only when its target is still provably MUX-managed; customized or ambiguous content is blocked.

## Skill

```bash
mux skill list
mux skill show review-changes
mux skill status --agent codex

mux skill assign review-changes source-explainer --agent codex
mux skill unassign source-explainer --agent codex
mux skill disable review-changes --agent codex
mux skill enable review-changes --agent codex
mux skill reapply review-changes --agent codex
```

A Skill assignment links the central copy in `~/.mux/skills/` into a verified user-level directory. Some Agents read the same physical directory, so assigning, toggling, or unassigning a Skill for one Agent may affect other installed Agents. The plan lists the complete physical target and every actually affected Agent; shared-directory effects cannot be hidden or split into contradictory relationships.

`reapply` repairs only an existing desired relationship. A missing or broken managed link can be rebuilt from the central copy; an external directory, regular file, or link to another location is never overwritten. For a shared directory, the review lists every Agent affected by the one physical repair.

## Agent

```bash
mux agent list
mux agent enable claude-code
mux agent disable cursor
```

These enable and disable commands control whether an Agent participates in MUX management. They are separate from enabling or disabling an individual asset relationship. The Agent ID must exactly match its stable catalog ID.

## External discovery and adoption

```bash
mux discover
mux discover mcp
mux discover model
mux discover skill
```

`discover` itself only refreshes or lists observed Agent configuration that MUX does not yet manage. It creates no ownership and does not edit Agent configuration. Adoption requires one exact candidate at a time:

```bash
mux adopt mcp github::stdio --agent claude-code
mux adopt model <candidate-id>
mux adopt skill <identity>
```

An MCP candidate uses its source Agent as an explicit anchor. If the same MCP identity appears in several Agents, MUX submits the complete current same-key observation set to core: exact copies and their original relationships are adopted atomically, while divergent copies remain conflicted. A Model or Skill candidate already identifies its source Agent or physical target and therefore does not accept `--agent`. Adoption first shows the central asset, original relationships, target files, and risks; each operation handles one logical asset rather than a cross-asset batch.

Every subcommand first runs the same safety bootstrap, which may finish a data migration, recover an incomplete transaction, or reconcile state. Regular non-JSON commands may also perform the once-daily Stable update check. “Read-only” and `--dry-run` therefore mean that the requested domain/ownership mutation is not committed; they do not promise a process with absolutely no maintenance filesystem or network effects. Set `MUX_NO_UPDATE_CHECK=1` when the version check must be disabled completely.

## JSON and automation

On success, `--json` writes one stable envelope to stdout:

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "skill.assign",
  "changed": true,
  "data": {}
}
```

Failures go to stderr with `ok: false`, a stable `error.code`, `error.message`, and safely projected `error.details` when applicable. Every non-export JSON response hides configuration values, credentials, raw parser diagnostics, and absolute paths; complete low-level diagnostics remain available only in human output. Only `mcp export` without `--out` emits the complete MCP configuration by design. A successful idempotent no-op remains successful and is distinguished by `changed: false`. `--json` never implies write consent; a mutation must explicitly choose `--yes` or `--dry-run`.

## Workspace and updates

```bash
mux workspace
mux --json workspace
mux upgrade
```

`workspace` shows the unified revision, central assets, desired relationships, observed inventory, and external discoveries. A standalone download or `cargo install` installation can use `mux upgrade` to follow the latest stable release. The CLI bundled with the desktop app is updated by the app.

Regular subcommands check for the latest stable release at most once a day after running. Set `MUX_NO_UPDATE_CHECK=1` to disable this check.

## Relationship with the desktop app

The CLI and desktop app read and write the same `~/.mux/` and invoke the same core planner. Relationships assigned from the CLI appear in Desktop, and central assets maintained in Desktop appear in the matching `list` and `status` output. The data model never forks.

Next → [Supported agents](/en/guide/agents)
