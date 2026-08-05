# CLI / TUI

`mux` is a native Rust CLI that shares `mux-core` and `~/.mux/` with the desktop app. MCPs, Models, and Skills all use the same model: central assets, Agent consumption relationships, and independently observed external state.

> Not installed yet? See [Installation · CLI](/en/guide/install#cli-tui-mux).

## Entry points

- `mux` opens the MCP terminal workspace for interactive catalog, source, and Agent relationship management.
- `mux <domain> <command>` uses the scriptable asset commands.

Set `MUX_NO_TUI=1` in scripts so an argument-free invocation prints help instead of entering the TUI.

## Unified command model

```text
mux mcp {list,show,status,assign,unassign,enable,disable,converge,add,delete,export}
mux model {list,show,status,assign,unassign,enable,disable,converge,use}
mux skill {list,show,status,assign,unassign,enable,disable,converge}
mux agent {list,enable,disable}
mux discover [mcp|model|skill]
mux workspace
mux upgrade
```

The three asset domains share these relationship semantics:

| Command | Meaning |
|---|---|
| `list` / `show` | Query central assets |
| `status` | Compare desired and observed state, including ownership, enabled/current state, status, and available actions |
| `assign` | Add only the specified desired relationships for one Agent |
| `unassign` | Remove only the specified relationships without deleting central assets |
| `enable` / `disable` | Keep the relationship and change its desired enabled state |
| `converge` | Adopt, restore, or detach one exact observation |

Domain-specific commands represent real differences: MCP supports manual catalog entries and full configuration export, while Model uses `use` for the current pointer. Full central Model, Provider, and Skill authoring remains in Desktop.

## External changes and convergence

Agent files and user-level Skill directories are legitimate input sources. External additions, deletions, toggles, current-model changes, and field edits do not put MUX into a migration conflict. CLI status queries rescan immediately; Desktop refreshes from file-system events.

```bash
mux mcp converge github::stdio --agent claude-code adopt
mux mcp converge github::stdio --agent claude-code restore
mux mcp converge github::stdio --agent claude-code detach

mux model converge external-<candidate-id> --agent codex adopt
mux model converge work --agent codex restore
mux model converge work --agent codex detach

mux skill converge review-changes --agent codex adopt
mux skill converge review-changes --agent codex restore
mux skill converge review-changes --agent codex detach
```

| Action | Meaning |
|---|---|
| `adopt` | Accept the Agent's exact observed state into the central asset or desired state while preserving the Agent bytes |
| `restore` | Restore only the selected relationship to MUX's desired state |
| `detach` | Release MUX ownership; drifted or external content remains and is projected as external afterward |

Every convergence request binds the inventory revision shown by `status`. Core rescans after planning and verifies the candidate hash and target snapshot again at commit. A changed observation returns `observation_stale` instead of applying an old review to new content.

The old `reapply`, top-level `adopt`, and `migration review/resolve` routes are removed. The explicit convergence action is the intent; there is no second drift-confirmation token.

## Statuses

| Status | Meaning |
|---|---|
| `synced` | desired and observed state agree |
| `external-added` | present in the Agent but not managed by MUX |
| `external-changed` | managed fields, enabled state, or current Model changed externally |
| `external-removed` | desired relationship remains but its Agent target was removed |
| `unparseable` | one observed target cannot be parsed |
| `ambiguous` | one observed target has ambiguous identity or conflicting values |
| `unsupported` | the observed state cannot be adopted or represented losslessly |

The last three statuses isolate the affected asset. Ordinary external drift also affects only its relationship. A single MCP, Model, or Skill problem does not lock unrelated domains; only damaged shared settings or an unrecoverable incomplete transaction can make the whole workspace read-only.

In JSON status output, `capability_errors` means one capability is locally unavailable. `recovery_error` is reserved for a shared transaction recovery boundary. The former never blocks queries or mutations in other domains.

## Stable IDs and Agent selection

Mutations use exact stable IDs and never fuzzy-match display names:

| Domain | ID | Example |
|---|---|---|
| MCP | `name::transport` | `github::stdio` |
| Model | Profile ID or the external ID returned by `status` | `work` |
| Skill | Central Skill name | `review-changes` |
| Agent | Agent ID | `claude-code`, `codex` |

Relationship commands require one explicit `--agent <id>`. `assign` and `unassign` can process several exact asset IDs; `enable`, `disable`, `use`, and `converge` process one at a time.
MCP additionally supports `unassign --all`: after review, it clears every managed, disabled, and external MCP entry from that one Agent while leaving central MCP assets and every other Agent unchanged.

```bash
mux mcp assign github::stdio filesystem::stdio --agent claude-code
mux mcp unassign --all --agent qoder
mux skill unassign source-explainer --agent codex
mux model assign work backup --agent pi
mux model use work --agent pi
```

`github::stdio` and `github::http` are separate assets and must be named separately.

## MCP

```bash
mux mcp list
mux mcp show github::stdio
mux mcp status --agent claude-code
mux mcp assign github::stdio --agent claude-code
mux mcp disable github::stdio --agent claude-code
```

MCP-specific central operations:

```bash
mux mcp add github::stdio --command npx --arg -y --arg @example/server
mux mcp add docs::http --url https://mcp.example.com --http-type streamable-http
mux mcp delete github::stdio
mux mcp export
mux mcp export --out mcp.json --yes
```

`export --out` creates a new `0600` file and refuses to overwrite an existing target. A stdout export contains the complete MCP configuration by definition; other JSON projections remain redacted.

## Model

```bash
mux model list
mux model show work
mux model status --agent pi
mux model assign work backup --agent pi
mux model assign work --agent claude-code --replace
mux model disable backup --agent pi
mux model use work --agent pi
```

A multi-model Agent may retain several assigned Profiles but has at most one current Model. Capability validation keeps single-model Agents within their native constraints. Repeating `assign`, `enable`, or `use` is a desired-state no-op and never overwrites drift as a side effect; use exact `converge` when observed state must change.

External Models are projected by candidate identity rather than collapsed into “the current or first candidate.” Candidates that require unsafe credential conversion or have ambiguous identity remain `unsupported` or `ambiguous`; MUX does not guess.

## Skill

```bash
mux skill list
mux skill show review-changes
mux skill status --agent codex
mux skill assign review-changes source-explainer --agent codex
mux skill disable review-changes --agent codex
```

A Skill relationship links the central copy under `~/.mux/skills/` into a verified user-level target. Several Agents may share one physical directory, so plans include the complete `affected_agent_ids`. `restore` rebuilds only a provably safe managed link; an external directory, regular file, or foreign symlink is never overwritten. `detach` preserves that external content.

## Read-only discovery

```bash
mux discover
mux discover mcp
mux discover model
mux discover skill
```

`discover` reports external observations and candidate details without creating ownership or editing Agent files. To change ownership, take the exact asset ID from `status` and run `converge`.

## Global options

| Option | Effect |
|---|---|
| `--json` | Emit a stable JSON envelope; mutations must also choose `--yes` or `--dry-run` |
| `--yes` | Skip the prompt without bypassing safety checks |
| `--dry-run` | Generate and display a plan, then cancel it without committing the requested mutation |
| `--no-color` | Disable ANSI colors |

`--yes` and `--dry-run` are mutually exclusive. Passing mutation-only options to a query is an error so automation cannot mistake an ignored flag for consent.

## Workspace and JSON

```bash
mux workspace
mux --json workspace
```

`workspace` returns one revision, Agent capabilities, central assets, desired relationships, observed inventory, and each row's `available_actions`. A success envelope looks like:

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "model.status",
  "changed": false,
  "data": {}
}
```

Failures go to stderr with a stable `error.code` and redacted details. API keys, tokens, raw configurations, and private absolute paths are not included in normal status JSON.

## TUI keys

The argument-free TUI currently focuses on MCP:

| Key | Action |
|---|---|
| `1` / `2` / `3` | Registry / Sources / Agents |
| `↑` `↓` or `j` `k` | Move |
| `/` | Search |
| `i` / `a` | Install or add an MCP to an Agent |
| `Space` | Toggle a source, Agent, or MCP |
| `d` | Delete a central MCP or detach an Agent relationship |
| `Ctrl-R` | Rescan |
| `?` | Help |
| `q` | Quit |

TUI, CLI, and Desktop all invoke the same core planner; they do not implement separate write semantics.

## Updates

```bash
mux upgrade
```

A standalone or `cargo install` CLI can follow Stable with `mux upgrade`. The Desktop-bundled CLI updates with the app. Set `MUX_NO_UPDATE_CHECK=1` to disable the daily version check after ordinary commands.

Next → [Supported agents](/en/guide/agents)
