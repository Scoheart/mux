# FAQ

## Will MUX touch the other servers already in my agent config?

No. MUX only locates the target entry inside the MCP node and updates managed connection fields; other top-level keys, other servers, and the permissions / OAuth / tool policies, comments, and formatting inside the target entry are all preserved. It backs up before writing, then lands the change with atomic replacement; writes are refused when the backup fails, the config structure is invalid, or the file is modified by another process during the write.

## Why can't I see a remote HTTP MCP in Claude Desktop?

`claude_desktop_config.json` is a local MCP config that only accepts stdio servers. Remote MCP is managed by Claude Connectors, which is not the same local file interface; MUX hides and refuses to install HTTP entries into Claude Desktop.

## Are the desktop app and the CLI's data separate?

No. Both share the same data directory `~/.mux/` and are built on the same Rust core. A change on one side is visible on the other after a refresh. You can install just one, or both.

## What do I do about "MUX is damaged and can't be opened"?

The current release is not notarized with an Apple Developer ID, so macOS may block launch because of the quarantine attribute — it's not that the app content is damaged. Once you've confirmed the file came from this project's Release, run:

```bash
xattr -dr com.apple.quarantine /Applications/MUX.app
```

Or right-click the app → Open → click "Open" again in the dialog. See [Installation](/en/guide/install#getting-mux-is-damaged-and-can-t-be-opened) for details.

## Is there a Windows / Linux version?

For now, the desktop app is packaged and released as a **macOS (Apple Silicon)** `.dmg`. The CLI is native Rust and can in theory be compiled from source on other platforms (`cargo install --path cli`), but the released prebuilt binary is currently macOS aarch64.

## What's the difference between "disable" and "delete"?

- **Disable**: first saves the server's complete semantic config (including agent-specific policy), then removes it from the agent config; on restore it won't overwrite a same-named entry rebuilt in the meantime. Good for turning something off temporarily.
- **Delete**: uninstall it from an agent. For manual / discovered entries, you can also **Forget** it — delete it from the catalog entirely and uninstall it from all agents.

See [Core concepts](/en/guide/concepts#install-toggle-delete).

## I edited a catalog entry — why didn't a certain agent update?

Editing a catalog entry's connection config auto-re-stamps into every global agent using it, including hand-edited copies; each file is backed up first. Changing only the description or tags won't trigger a sync.

To force a push, use **Resync** — the button in the desktop editor, or the `S` key on the TUI Registry screen. Customized copies are skipped and reported, with an option to force overwrite. See [Edit propagation](/en/guide/concepts#edit-propagation).

## Will same-named stdio and http conflict?

No. MUX's identity is the **`name::transport`** composite key, and `sse` falls under `http`. A same-named stdio and http are **two independent entries**, installed, edited, and deleted independently.

## Where do the catalog entries come from? Can I keep only some?

The catalog is the union of all **enabled sources**; MUX doesn't ship a hardcoded MCP server list. The TUI Sources screen can enable/disable sources individually; the desktop `v1.2.0` currently offers only filtering, refresh, and delete by source. Disabling a source doesn't delete the underlying file. See [Sources](/en/guide/concepts#sources).

## Is the Mux curated collection required?

No. It's just an **optional** one-click subscription (a subscription to a built-in, curated remote source). Without it, the catalog still works from your own subscriptions / imports / manual / discovered sources.

## Can data sync across multiple machines?

MUX's catalog, sources, and state all live under `~/.mux/`. Agent paths inside the home directory are saved as `~/…`, but custom absolute paths outside the home directory keep their original value; after syncing across machines, still verify that agents are installed in the same place.

## How does MUX update?

The desktop app silently checks for the latest stable release after launch, and you can also click **Check for updates** at the top. A standalone CLI install uses `mux upgrade`; a CLI installed to `~/.local/bin/mux` alongside the desktop app updates with the app.

## Still have questions?

Ask or share feedback at [GitHub Issues](https://github.com/Scoheart/mux/issues).
