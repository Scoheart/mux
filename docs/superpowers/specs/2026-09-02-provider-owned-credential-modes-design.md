# Provider-owned credential modes

## Decision

Credential configuration belongs exclusively to a Provider. Agent screens must not expose credential-source or delivery choices. For every Model consumption, core derives the safest compatible Agent projection from the Provider source and the verified Agent capability matrix.

## Provider form

The source dropdown contains exactly four user-facing modes:

| UI label | Persisted source | Behavior |
|---|---|---|
| 明文 | `mux-store` | The user types the API Key; MUX stores its bytes in system Keychain. “明文” describes the input form, not MUX JSON or Agent output. |
| 环境变量 | `env` | MUX stores only the variable name. |
| 文件 | `file` | MUX stores only an absolute path and validates an owned, regular, non-symlink `0600` file. |
| 命令 | `helper` | MUX stores structured `command`, `args`, and optional TTL; no shell expression is accepted. |

The adaptive fields and file/command validation remain in the Provider editor. Existing `mux-store`, `env`, `file`, and `helper` wire values stay compatible.

## Agent behavior

- Remove the Agent-row “自动 / Agent 凭据存储 / 明文配置” dropdown.
- Remove the global plaintext-danger dialog and its public Tauri/API mutation command.
- Ignore any historical per-consumption source override or delivery value and always use the Provider source with `auto` routing.
- Keep internal legacy fields readable so settings written by `v1.8.162` do not fail to deserialize; new UI cannot create non-auto Agent policy.
- `auto` may use a native env/file/helper reference, a MUX Keychain helper, or a verified Agent-owned credential store. It must never write literal credentials into an Agent config.
- OpenCode automatically uses its verified `auth.json` store when the Provider mode is 明文 or 命令 and uses native env/file references for those Provider modes.

## Security and compatibility

- API Key bytes remain out of Provider JSON, consumption JSON, plans, logs, backups, tests, screenshots, and Git.
- Changing a Provider away from 明文 removes its now-unused Provider Keychain value.
- Required Providers without a resolvable source remain blocked before Agent configuration is written.
- Existing Agents and Provider records continue to deserialize; only the mistaken per-Agent control surface and mutation path are removed.

## Acceptance

- Provider source UI shows exactly 明文、环境变量、文件、命令.
- Agent Model rows contain no credential delivery selector and no plaintext dialog.
- Core routing uses `ApiKeyDelivery::Auto` regardless of stored legacy per-Agent policy.
- OpenCode明文 mode resolves through `auth.json`, not literal `options.apiKey`.
- Focused Rust and React regression tests plus production builds pass.
