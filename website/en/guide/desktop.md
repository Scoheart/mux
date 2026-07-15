# Desktop app

The desktop app is MUX's visual front-end (macOS, Tauri + React), used to browse the catalog, manage sources, install MCP servers, and edit agents. It shares `~/.mux/` with the command line.

> Not installed yet? Start with [Installation](/en/guide/install#desktop-app-macos).

## Interface overview

Opening the app lands you on the **Registry (catalog)** by default. In the current `v1.2.0`, the main interface is made up of these areas:

![The MUX Registry interface](/img/registry-overview-current.jpg)

| Area | Purpose |
|---|---|
| **Registry** | Return to the catalog from any agent page. |
| **Agent selector** | Search 192 clients; "Configurable" lists verified writable targets, "Client directory" shows discovery-only entries. |
| **`+`** | Add a custom agent, next to the agent selector. |
| **Theme / Rescan / Check for updates** | Switch appearance, re-read each agent's config, manually check for stable updates. |
| **Source bar** | Filter the catalog by source; the top offers "Add subscription" and "Import config." |
| **Catalog toolbar** | Search, view shadowed entries, paste config, export the effective config, create a new MCP. |
| **MCP card grid** | Shows name, transport, source, endpoint, usage, and conflict status. |

## Catalog and shadowing

The Registry shows **every copy** from all enabled sources by default. When the same `name::transport` appears in several sources, the higher-precedence copy takes effect and the rest keep showing rather than being hidden:

- The effective copy keeps the normal card style, without a repeated "effective" label.
- A shadowed copy uses an indigo accent, is marked **shadowed**, and notes "superseded by <source>."
- A **Shadowed N** item appears to the right of the search box only when there's a conflict in the current scope; click it to see just the shadowed copies, click again to restore all.
- Clicking a source on the left shows all copies from that source; the "shadowed" filter still applies only to the current source.

What's actually used for install and export is the highest-precedence copy per composite key. For the full rules, see [Precedence](/en/guide/concepts#precedence-dedup-rules).

## How to read a card

Each card contains:

| Element | Meaning |
|---|---|
| Colored avatar and name | The MCP's identity. |
| `STDIO` / `HTTP` | The normalized transport category; `sse` and `streamable-http` fall under the HTTP identity. |
| Source | An agent name, manual entry, local file, or subscription name. |
| Endpoint | The stdio launch command or HTTP URL, truncated when too long. |
| Usage status | A green dot means it's used by some agents; a gray dot means unused. |

Hovering a card reveals **Copy config / Edit / Delete** actions. Only "manual" and "discovered" entries can be edited or deleted entirely; entries from subscriptions and imported files are managed by their source. Click the card body to open its detail view.

## Install to an agent

1. Choose an agent from the selector in the top bar.
2. Confirm the **Agent config file** and **MCP config file** in the agent configuration center. They may be the same file or two separate files; MUX labels the relationship explicitly.
3. Click **Add MCP**, then search and select an entry that isn't installed yet.
4. MUX backs up the target file first, then writes it into the global config in that agent's format and config key.

MUX currently manages only agents' global installs.

## Toggle and delete

Every installed MCP on an agent page has toggle and delete actions:

- **Disable**: first save the target entry's complete semantic config in MUX, then remove it from the agent file; it can be restored later without overwriting a same-named entry rebuilt in the meantime.
- **Enable**: write the saved config back into the agent file.
- **Delete**: uninstall it from that agent.

Deleting a "manual / discovered" entry in the Registry removes it from both the catalog and all global agents at once, keeping backups.

## Edit, paste, and export

- **Edit**: modify a user-owned entry. Connection-config changes auto-sync to every global agent using the entry; description and tag changes do not trigger a sync.
- **Resync**: the bottom of the edit page lets you explicitly re-stamp global installs; by default it skips manually customized configs, and you can force overwrite after confirmation.
- **Paste config**: supports recognizable JSON, TOML, or YAML; once parsed it's added to "manual."
- **Export the effective config**: the download icon in the toolbar exports the full, deduplicated catalog — not just manual entries; the CLI equivalent is `mux export`.

## Source management

The top of the source bar has just two add actions:

- **Add subscription**: enter a remote config URL; the **Mux curated** button in the dialog fills in the official curated subscription.
- **Import config**: select a local JSON / TOML config file.

Hovering a source row lets you refresh remote subscriptions, local files, and discovery; unmanaged sources can be deleted. Deleting a source removes its cache and catalog entries, but does not uninstall configs already written to agents.

The source model itself supports enable/disable, which the TUI's "Sources" screen does with `Space` / `Enter`. The desktop `v1.2.0` source bar does not yet offer an on/off toggle.

## Agent management

- The `+` to the right of the top agent selector adds a custom JSON, TOML, or YAML agent.
- An agent page is that agent's configuration center: it shows the agent/model path, MCP path, model assignment, and installed MCPs together, so routine work no longer requires switching between Models and MCPs.
- When the paths match the page shows “Same file”; when model settings and MCP use different files it shows “Separate MCP file.” Paths identify configuration targets only; MUX never returns the complete config to the UI.
- Inside a built-in agent's page you may only override the global MCP path; the official format, config key, and codec are fixed, to avoid producing incompatible configs.
- Paths inside the home directory are saved as `~/…`; absolute paths outside it keep their original value.

For the full 40 verified targets and the 192-client directory scope, see [Supported agents](/en/guide/agents).

## Models (Beta)

The top-level **Models** workspace creates reusable endpoints and manages assignments across agents. The same compatible profiles are also available inside each supported agent's configuration center for direct application. A profile contains its protocol, Base URL, model ID, and optional token limits; API keys remain in macOS Keychain and are never included in settings, previews, or backups.

Claude Code currently accepts Anthropic Messages profiles, Codex supports the OpenAI protocols exposed by its provider configuration, and Pi supports all three initial protocols. Qoder shows its verified paths and official setup entry, but MUX does not write its unpublished encrypted model store.

## Auto-update and the CLI

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

The command line offers the same core capabilities → [CLI / TUI](/en/guide/cli).
