# Agent Catalog Methodology

MUX deliberately separates client discovery from writable configuration support. A product appearing in an MCP directory proves that it is relevant to users; it does not prove a stable global file path or wire schema.

## Data sets

| File | Purpose | Current size |
|---|---|---:|
| `data/agents.json` | Deep-audited definitions with product-specific paths, layouts, codecs, transports, and evidence | 42 |
| `data/agent-catalog.json` | Discovery-only client records; unknown paths and formats always fail closed | 175 |

There are 23 overlapping IDs. Audited definitions override directory records, producing 194 distinct retained client records. The desktop picker intentionally shows only the 41 writable user-level targets among the 42 audited definitions; catalog-only records remain available to the core for future promotion. Devin remains read-only.

## Discovery sources

The broad catalog was assembled and cross-checked from:

1. [Glama MCP clients](https://glama.ai/mcp/clients), used for broad public discovery.
2. [MCP client feature matrix](https://modelcontextprotocol.io/extensions/client-matrix), used to catch major hosted and desktop clients.
3. [Docker MCP client support](https://docs.docker.com/reference/cli/docker/mcp/client/), used to cross-check installable desktop/CLI targets.
4. Official product documentation for supplemental clients absent from the public directory.

`node scripts/update-agent-catalog.mjs` fetches the Glama directory, normalizes known aliases, merges the reviewed official supplements encoded in the script, sorts IDs, and aborts if Glama unexpectedly falls below 150 entries. The MCP matrix and Docker documentation are cross-check sources, not additional runtime fetches. With a fixed `MUX_CATALOG_DATE`, the output is deterministic. Catalog records have no writable path, no transports, and `format: unknown`.

## Promotion criteria

A catalog client is promoted into `data/agents.json` only after all of these are known:

1. An official product page or official source confirms MCP client support.
2. A user-level global config path is stable for the target platform, or the record is explicitly marked read-only.
3. The top-level MCP key and map/list layout are known.
4. Local and remote transport fields are known independently.
5. Header, environment, working-directory, and command representations are known.
6. A codec test proves the exact on-disk shape.
7. Round-trip tests prove that unrelated settings and Agent-owned policy fields survive.

Community extensions are not presented as native support. The current example is Pi: Pi core does not ship MCP support, so the writable record is named `Pi Coding Agent (MCP Adapter)` and cites `pi-mcp-adapter`.

## Agent Skills capabilities

Skills support is audited separately from MCP configuration. Only definitions from `data/agents.json` may carry a trusted nested `skills` capability, and only when official product documentation confirms a user-level Skills directory and read-only installation evidence. Any such field from the discovery-only `data/agent-catalog.json` is discarded before the data sets are merged. Custom Agent definitions cannot create or override this metadata, and upgrades refresh it from audited data.

| Field | Contract |
|---|---|
| `target_id` | Stable identity for the preferred physical assignment target. |
| `global_dir` | Preferred user-level writable directory; it must be a safe `~/.../skills` path outside `~/.mux`. |
| `aliases` | Other verified target IDs and user-level directories the Agent reads. Aliases are compatibility targets, not the preferred write location. |
| `docs` | Official documentation that supports the Skills capability. |
| `evidence` | Verification level for the capability; the initial audited records require `official`. |
| `verified_at` | Date the Skills-specific evidence was last checked. |
| `probes` | Read-only command, path, or macOS bundle checks that indicate the Agent is installed. Probe paths are evidence only and may be absolute application paths. |

All preferred directories and aliases are validated before the built-in catalog is returned. The same `target_id` must always name the same path, and a physical path cannot be assigned contradictory target IDs. The audited capability-bearing ID set must be exactly Claude Code, Codex, Cursor, Gemini CLI, OpenCode, and GitHub Copilot CLI; missing or extra IDs, non-`official` evidence, blank documentation or verification dates, and empty probe lists reject the built-in catalog. Shared directories such as `~/.agents/skills` are never treated as installation evidence by themselves.

## Evidence levels

| Value | Meaning |
|---|---|
| `official` | Official product documentation describes the relevant path/schema. |
| `official-source` | Current official source code, including source shipped inside a signed official application bundle, resolves a platform path or field more precisely than the prose docs. |
| `community-extension` | MCP support belongs to a named third-party extension, not the core product. |
| `catalog` | Public discovery only; never writable. |

Every audited definition records `verified_at`, documentation URL, supported transports, codec, and layout. The existing set was rechecked through 2026-07-15; Grok Build was verified against its newly published official source and MiniMax Code against its signed `3.0.51` application bundle on 2026-07-16.

## Fail-closed rules

- Unknown/catalog-only formats never fall back to a generic JSON writer.
- Built-in schema metadata is refreshed on upgrade; user path and enabled-state overrides survive.
- Invalid or ambiguous JSON, TOML, or YAML is never repaired by overwriting it.
- Only the named MCP entry is exposed to scanners, previews, disable snapshots, and source caches.
- Full Agent files may exist only as local pre-write backups.

Run the validation suite after changing either data set:

```bash
MUX_CATALOG_DATE=2026-07-14 node scripts/update-agent-catalog.mjs
cargo test --workspace --locked
bash desktop/scripts/prepare-sidecar.sh
cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked
cd desktop && npm run build
```
