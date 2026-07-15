# Installation

MUX has two entry points — the desktop app and the CLI / TUI — sharing `~/.mux/`. The desktop app already bundles the CLI, so you usually don't need to install it separately.

## Desktop app (macOS)

1. Open the [latest stable Release](https://github.com/Scoheart/mux/releases/latest) and pick the asset labeled **Desktop installer · Apple Silicon** (its filename looks like `MUX-Desktop-Installer-*-macOS-Apple-Silicon.dmg`).
2. Open the dmg and drag **MUX.app** into `/Applications`.
3. Launch it for the first time.

### Getting "MUX is damaged and can't be opened"?

The current release is not notarized with an Apple Developer ID, so macOS may block the first launch because of the quarantine attribute — it does not mean the file is damaged. Once you've confirmed the download came from this project's Release, you can clear the quarantine attribute:

```bash
xattr -dr com.apple.quarantine /Applications/MUX.app
```

Then open it normally. (Alternatively: right-click the app → Open → click "Open" again in the dialog.)

## CLI / TUI (`mux`)

`mux` is a native Rust binary that shares the same core as the desktop app.

### Option 1: install with the desktop app (recommended)

After the stable app launches, it symlinks the bundled CLI to `~/.local/bin/mux`. If your terminal can't find `mux`, add that directory to your `PATH`:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
mux --version
```

This symlink is auto-repaired by the app; when the app updates, the CLI updates with it.

### Option 2: download the prebuilt binary separately

In the stable Release, pick the asset labeled **Command-line tool · Apple Silicon**. For compatibility with older `mux upgrade`, the actual filename is still `mux_v<version>_aarch64-apple-darwin.tar.gz`:

```bash
# After downloading from Releases:
tar xzf mux_v*_aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin
mv mux ~/.local/bin/mux
mux --version
```

A standalone CLI can run `mux upgrade` to follow the latest stable release; the CLI bundled with the desktop app will instead tell you it updates with the app.

### Option 3: install from source (requires Rust)

```bash
git clone https://github.com/Scoheart/mux
cd mux
cargo install --path cli       # installs to ~/.cargo/bin/mux
```

### Usage

Run with no arguments to enter the **interactive TUI**:

```bash
mux
```

Or use subcommands for scripting (set `MUX_NO_TUI=1` to print help instead of entering the TUI when run with no arguments):

```bash
mux list            # list the MCP servers in the catalog
mux status          # show the MCP servers currently active per agent
mux apply <names…>  # non-interactive install to global config (--agent)
mux export --out mcp.json  # export the effective config
mux agents list     # list all agents
mux upgrade         # upgrade a standalone CLI install
```

See [CLI / TUI](/en/guide/cli) for details.

## Where data lives

All user data lives under `~/.mux/`:

```
~/.mux/
├── settings.json           # one document: agents · sources · disabled · state
├── sources/
│   ├── remote/<id>.json    # cache of subscribed URLs
│   └── local/<id>.(json|toml)  # imported local files + the managed manual/discovered sources
└── backups/                # timestamped backups made before writing to an existing agent file
```

Both the desktop app and the CLI read and write here, so the two stay in sync by design.

Next → [Core concepts](/en/guide/concepts)
