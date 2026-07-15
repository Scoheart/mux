# What is MUX

**MUX (MCP Multiplexer)** is a tool for managing **MCP (Model Context Protocol) servers** across all your AI coding agents from one place.

![Overview of the MUX desktop app](/img/registry-overview-current.jpg)

> For a breakdown of each area above, see the [desktop app walkthrough](/en/guide/desktop#interface-overview).

## What problem it solves

If you use several AI coding tools at once (Claude Code, Cursor, VS Code, Codex, Zed, …), each one keeps its own MCP config file, with a different format, path, and set of fields. To give them all the same MCP (say `filesystem`, `github`, or `context7`), you have to:

- find each tool's config file path;
- write it out in that tool's own format (JSON / TOML / YAML), with its own key names and map/list layout;
- and edit them all again just to change one server's parameters.

MUX collects those MCP servers into **one catalog (the Registry)** so you can **install / toggle / edit / delete** any MCP into any agent from a single place — MUX handles what format each agent needs and which file it goes into.

## Two front-ends, one set of data

MUX has two interfaces, and they **share the same data directory, `~/.mux/`**:

| | Description |
|---|---|
| **Desktop app** | A macOS application (Tauri + React). Visual management, best for the mouse. |
| **CLI / TUI** | The native Rust binary `mux`. Its subcommands are scriptable; with no arguments it drops into an interactive terminal UI (TUI). |

Because both are built on the **same Rust core crate (`mux-core`)**, the data model exists in only one place. A change you make in the desktop app shows up in the CLI after a refresh, and vice versa.

## The core idea: a source-driven catalog

MUX does **not** ship a hardcoded MCP list. Your catalog is assembled from **sources**:

- **Subscribe** to a remote URL (pointing at an MCP config file); MUX fetches and caches it;
- **Import** a local config file;
- **Manually add** / paste a server;
- **Auto-discover** the MCP servers already configured in your agents.

The catalog = the union of all enabled sources. Delete or disable a source and its entries disappear from the catalog. See [Core concepts](/en/guide/concepts) for details.

## What it can do (feature overview)

- **Browse the catalog**: search, filter by source, and see each MCP's transport, source, which agents use it, and its GitHub repo.
- **Install to an agent**: pick an MCP, check which agents to install into, and write it in one click (the original file is backed up first).
- **Enable / disable**: temporarily disable an MCP (removed from the agent config, but its configuration is remembered and can be restored anytime).
- **Delete**: uninstall from an agent, or delete it from the catalog entirely.
- **Edit / paste**: edit an MCP config visually, or paste an `mcpServers` JSON/TOML block for automatic recognition.
- **Re-sync**: explicitly push an edited config to every agent that has the MCP installed.
- **Export the effective catalog**: export the deduplicated, complete catalog as standard MCP JSON.
- **Source management**: subscribe, import, refresh, enable/disable, and delete sources.
- **Agent management**: add a custom agent and edit its config file path.
- **Auto-update**: both the desktop app and the standalone CLI follow the latest stable channel.

Next → [Installation](/en/guide/install)
