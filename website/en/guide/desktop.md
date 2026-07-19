# Desktop app

The desktop app is MUX's visual front-end (macOS, Tauri + React) for maintaining central MCP, Model, and Skill assets and letting Agents consume them. Data lives under shared `~/.mux/`; Skills currently have a Desktop entry only.

> Not installed yet? Start with [Installation](/en/guide/install#desktop-app-macos).

## Interface overview

Opening the app lands you on **MCPs** by default. The main interface is made up of these areas:

![The MUX Registry interface](/img/registry-overview-current.jpg)

| Area | Purpose |
|---|---|
| **MCPs / Models (Beta) / Skills** | Switch among the MCP catalog, model endpoints, and user-level Skills management. |
| **Agent selector** | Search the verified writable targets; discovery-only records remain available for catalog validation without appearing as a separate tab. |
| **`+`** | Add a custom agent, next to the agent selector. |
| **Proxy / Theme / Rescan / Check for updates** | Configure MUX networking, switch appearance, re-read each agent's config, and manually check for stable updates. |
| **Source bar** | Filter the catalog by source; the top offers "Add subscription" and "Import config." |
| **Catalog toolbar** | Search, view shadowed entries, paste config, export the effective config, create a new MCP. |
| **MCP card grid** | Shows name, transport, source, endpoint, usage, and conflict status. |

## Catalog and shadowing

The Registry shows **every copy** from all enabled sources by default. When the same `name::transport` appears in several sources, the higher-precedence copy takes effect and the rest keep showing rather than being hidden:

- The effective copy keeps the normal card style, without a repeated "effective" label.
- A shadowed copy uses an indigo accent, is marked **shadowed**, and notes "superseded by <source>."
- A **Shadowed N** item appears to the right of the search box only when there's a conflict in the current scope; click it to see just the shadowed copies, click again to restore all.
- Clicking a source on the left shows all copies from that source; the "shadowed" filter still applies only to the current source.

What's available for Agent consumption and export is the highest-precedence copy per composite key. For the full rules, see [Precedence](/en/guide/concepts#precedence-dedup-rules).

## How to read a card

Each card contains:

| Element | Meaning |
|---|---|
| Colored avatar and name | The MCP's identity. |
| `STDIO` / `HTTP` | The normalized transport category; `sse` and `streamable-http` fall under the HTTP identity. |
| Source | An agent name, manual entry, local file, or subscription name. |
| Endpoint | The stdio launch command or HTTP URL, truncated when too long. |
| Usage status | A green dot means it's used by some agents; a gray dot means unused. |

Cards select assets and show read-only consumer impact; lifecycle actions live in the Inspector. Agent relationships are edited only from the relevant Agent page. Only user-owned MCP source copies can be edited or deleted directly; subscriptions and imported files are managed by their source. External configurations observed only in Agent files are not Registry entries.

## Let an Agent consume central assets

1. Choose an agent from the selector in the top bar.
2. Confirm the **Agent config file** and **MCP config file** in the agent configuration center. They may be the same file or two separate files; MUX labels the relationship explicitly.
3. In MCPs, Model, or Skills, click **Manage** and set the Agent's complete desired selection from the central picker. MCPs and Skills allow multiple selections; Model allows at most one.
4. Review relationship changes, target files, shared Skill targets, drift, and conflicts. MUX then backs up, writes in the Agent's native format, and rescans to verify the result.

Consumption relationships are edited only from Agent pages; central asset Inspectors do not configure Agents. MUX currently manages only user-level global configuration.

## Relationship state and removal

An Agent page shows desired central assets even if an observed target is missing or conflicted. Core reconciles each relationship as synced, pending, drifted, or conflicted; it never silently overwrites drift in the background. Removing use changes that Agent's relationship and managed target but does not delete the central asset.

Configurations found only in Agent files appear as read-only external state; scanning does not import them or create a relationship. Deleting a central asset first reviews all consumers, then atomically removes every managed target and relationship together with the central record.

## Edit, paste, and export

- **Edit**: modify a user-owned central entry. The plan retains relationships and includes every consumer, then commits the central asset and all targets together.
- **Drift override**: manually customized targets are shown during review and require an explicit confirmation bound to the current candidate hash. Conflicts or concurrent changes block the entire commit.
- **Paste config**: supports recognizable JSON, TOML, or YAML; once parsed it's added to "manual."
- **Export the effective config**: the download icon in the toolbar exports the full, deduplicated catalog — not just manual entries; the CLI equivalent is `mux export`.

## Source management

The top of the source bar has just two add actions:

- **Add subscription**: enter a remote config URL; the **Mux curated** button in the dialog fills in the official curated subscription.
- **Import config**: select a local JSON / TOML config file.

Remote subscriptions and local files can be refreshed; unmanaged sources can be deleted. Rescanning Agents only refreshes observed inventory and never creates managed `discovered` entries in the background. Source removal deletes its cache and catalog copies, while existing desired relationships must be handled through an impact plan.

The source model itself supports enable/disable, which the TUI's "Sources" screen does with `Space` / `Enter`. The desktop `v1.2.0` source bar does not yet offer an on/off toggle.

## Agent management

- The `+` to the right of the top agent selector adds a custom JSON, TOML, or YAML agent.
- An Agent page is that Agent's consumption center. MCPs, Model, and Skills tabs show only desired central assets, their reconciled status, configuration paths, and a central picker; they do not embed asset creation, source resolution, or Skill installation.
- When the paths match the page shows “Same file”; when model settings and MCP use different files it shows “Separate MCP file.” Paths identify configuration targets only; MUX never returns the complete config to the UI.
- Inside a built-in agent's page you may only override the global MCP path; the official format, config key, and codec are fixed, to avoid producing incompatible configs.
- Paths inside the home directory are saved as `~/…`; absolute paths outside it keep their original value.

For the full 42 verified targets, 41 writable targets, and 194 retained records, see [Supported agents](/en/guide/agents).

## Models (Beta)

The top-level **Models** workspace creates central reusable Profiles without touching an Agent. Each Agent page then shows its observed current state and compatible switch targets, with at most one Profile per Agent. Editing propagates through every consumer and deletion cascades through relationships and managed targets. API keys remain in macOS Keychain and never enter settings, persisted plans, previews, or backups.

Claude Code currently accepts Anthropic Messages profiles, Codex uses the Responses API, and Pi supports all three initial protocols. Qoder, Grok Build, and MiniMax Code expose their verified paths and setup entry; MUX neither writes Qoder's unpublished encrypted model store nor persists a MUX Keychain secret as plaintext in Grok Build or MiniMax Code model configuration.

## Skills

The top-level **Skills** workspace resolves candidates from public GitHub, a local folder, or a Skill archive, reviews files, local risk, and conflicts, then writes only one central copy. A separate consumer operation from an Agent page links that copy into verified Agent directories. Agents sharing one physical target are selected and reviewed as an inseparable group; Agent pages never resolve or install a Skill source.

Skills do not require system Git, Node.js, or `npx`. This version does not support project-level content, private repositories, or CLI/TUI Skills commands. See [User-level Skills](/en/guide/skills) for installation, shared aliases, high-risk second confirmation, backups, and recovery.

## Auto-update and the CLI

The network icon in the top bar configures one global proxy for MUX. It accepts `http://`, `socks4://`, `socks4a://`, and `socks5://` addresses. Later GitHub Skill, remote-source, CLI update, and Desktop update requests use the saved proxy; save an empty value to turn it off. HTTPS proxy endpoints, `socks5h://`, and proxy credentials are not supported, keeping usernames and passwords out of `~/.mux/settings.json`.

- About 2.5 seconds after launch, the app silently checks for the latest **stable** release; a failure won't interrupt you.
- **Check for updates vX.Y.Z** at the top lets you check manually anytime, and shows the error on failure.
- The download runs in the background; once done you can restart immediately or have it take effect on the next launch.
- The stable app bundles the CLI and maintains the `~/.local/bin/mux` symlink after launch. See [Installation](/en/guide/install#option-1-install-with-the-desktop-app-recommended).

## Write guarantees

- Before modifying an existing agent config, the original is backed up independently to `~/.mux/backups/`; if the backup fails, that target is stopped.
- Atomic replacement and concurrent-modification checks avoid half-written files or overwriting changes the agent made at the same time.
- When adding or removing a target server, only the MCP node is modified — the complete agent config is never read into the UI or returned.
- When updating an existing server, only managed connection fields change, preserving its permissions, OAuth, tool policies, and other specific fields.
- Other top-level keys, other servers, comments, indentation, and key order are preserved; writes are refused when the JSON, TOML, or YAML structure is invalid or ambiguous.
- `~/.mux/settings.json` uses atomic writes via temp file plus rename.

The command line manages MCPs, sources, and Agents; Skills currently have a Desktop entry only → [CLI / TUI](/en/guide/cli).
