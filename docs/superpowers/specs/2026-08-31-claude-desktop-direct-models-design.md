# Claude Desktop Direct Models Design

## Goal

Add Claude Desktop as a managed MUX Model Agent on macOS. Applying an Anthropic Messages-compatible Model creates one MUX-owned third-party inference profile; non-Claude model routes automatically opt out of Claude Desktop's Claude-name verification.

This is a deliberate security exception to MUX's normal credential policy: Claude Desktop requires `inferenceGatewayApiKey` to be a literal string, so an explicitly reviewed apply exports the selected Provider credential from Keychain into Claude Desktop's private `0600` profile file.

## Product contract

- Claude Desktop appears in the Agent workspace with a managed, single-active-Model capability.
- The adapter accepts only `anthropic-messages` Profiles. It does not silently translate OpenAI or Gemini protocols.
- Applying a Profile creates or updates one deterministic `MUX` entry in Claude Desktop's `configLibrary` and makes that entry current.
- A route is Claude-compatible when its final model path segment starts with `claude-`. For example, both `claude-sonnet-4-6` and `anthropic/claude-sonnet-4-6` are Claude-compatible; `qwen3.7-max` is not.
- Non-Claude routes set `unstableDisableModelVerification: true`.
- Claude-compatible routes omit `unstableDisableModelVerification`; MUX never weakens verification when it is unnecessary.
- Every MUX profile disables Claude Desktop model discovery and writes one explicit `inferenceModels` entry. Provider model discovery remains a MUX Models-page concern and does not depend on the Provider returning Claude Desktop's expected `/v1/models` envelope.
- Applying or clearing reports that Claude Desktop must be restarted.

## Configuration targets

The macOS adapter owns narrowly scoped fields across these targets:

1. `~/Library/Application Support/Claude/claude_desktop_config.json`
   - Set only `deploymentMode` to `3p`.
   - Preserve MCP servers and every unrelated field.
2. `~/Library/Application Support/Claude-3p/claude_desktop_config.json`
   - Set only `deploymentMode` to `3p`.
   - Preserve MCP servers and every unrelated field.
3. `~/Library/Application Support/Claude-3p/configLibrary/_meta.json`
   - Add the deterministic MUX entry when absent.
   - Set `appliedId` to the MUX profile on apply.
   - Preserve every external entry and unknown field.
4. `~/Library/Application Support/Claude-3p/configLibrary/6d757800-0000-4000-8000-000000000001.json`
   - This is the only MUX-owned inference profile.
   - The entry name is `MUX`.

The fixed UUID makes ownership unambiguous across restarts without searching or taking over another tool's profile.

## MUX profile projection

The generated profile contains:

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "<Provider Anthropic base URL>",
  "inferenceGatewayApiKey": "<credential exported from Keychain>",
  "inferenceGatewayAuthScheme": "bearer",
  "modelDiscoveryEnabled": false,
  "inferenceModels": [
    {
      "name": "<Agent-native model route>",
      "labelOverride": "<MUX Model name>"
    }
  ],
  "coworkEgressAllowedHosts": ["*"],
  "disableDeploymentModeChooser": true
}
```

For non-Claude routes it additionally contains:

```json
{
  "unstableDisableModelVerification": true
}
```

The effective base URL uses the selected Provider's Anthropic Messages connection. MUX computes the Profile's full request URL and accepts it only when the terminal endpoint is `/v1/messages`; `inferenceGatewayBaseUrl` is the URL with that terminal suffix removed. For example, `https://max-ai.amap.com` plus `/v1/messages` becomes `https://max-ai.amap.com`, while `https://api.z.ai/api/anthropic` plus `/v1/messages` becomes `https://api.z.ai/api/anthropic`. Any other endpoint shape fails closed instead of producing a plausible but incorrect configuration.

## Credential handling

- The central Provider credential remains authoritative in macOS Keychain.
- Planning never reads or serializes the credential.
- Commit reads the credential only after the user confirms the operation.
- The operation review explicitly states that the credential will be exported to Claude Desktop's private configuration.
- The MUX profile is created with mode `0600`; an existing MUX profile with broader permissions is tightened before success is reported.
- Credential values never enter operation DTOs, logs, errors, screenshots, test snapshots, or repository fixtures.
- The MUX-owned secret-bearing profile is not copied into MUX's persistent backup directory. Transaction rollback retains prior bytes only in memory and zeroizes them after commit or rollback.

## Apply transaction

1. Planning reads and hashes all four physical targets, validates their JSON, confirms the Profile protocol, and records the exact write set without reading the credential.
2. Commit rechecks the candidate hash and target hashes, then reads the credential from Keychain.
3. Existing listened-to Claude configuration and `_meta.json` files are updated in place while preserving inode, permissions, unknown fields, and unrelated formatting where the existing safe-write layer supports it.
4. A new MUX profile is published atomically in the same directory with mode `0600`; an existing MUX profile is replaced only after CAS revalidation.
5. The transaction records the previously applied non-MUX profile ID in typed MUX operational state, then switches `_meta.json.appliedId` to the MUX profile.
6. Any failure rolls every changed target back. Cross-process changes fail closed rather than being overwritten.

Central desired-state persistence and Claude projection remain separate phases under the existing `Agent × capability × physical target` operation model.

## Clear and restore

- Clearing the MUX Model relationship never deletes CC Switch, Default, OpenRouter, or any other external Claude profile.
- If the MUX profile is active, clear restores the previously applied profile when that entry still exists.
- If the remembered profile was removed externally, clear chooses the existing entry named `Default`; if no safe restore target exists, planning fails with a repairable conflict.
- After `_meta.json` points away from MUX, the MUX entry and MUX-owned profile file are removed.
- Deployment mode remains `3p` because external profiles may depend on it.
- Clearing central Models or Providers remains outside this Agent-scoped operation.

## Observation and drift

- Synced means the MUX profile matches the active desired Profile and `_meta.json.appliedId` selects it.
- Missing means an assigned MUX Profile or meta entry is absent.
- Drifted means MUX-owned fields differ while the files remain parseable.
- Conflicted means JSON is invalid, the deterministic UUID belongs to an unexpected entry, the active selection is ambiguous, or a target changed during planning.
- External Claude profiles are preserved and are not silently adopted into the central Model library.

## UI surface

No Claude-specific page or dialog is added. The existing Agent Model workspace consumes the new core capability:

- Agent name: `Claude Desktop`
- Storage authority: `mux-mapping`
- Supports multiple Models: `false`
- Credential mode: `keychain-export`
- Supported protocols: `anthropic-messages`

The normal operation review gains a Claude Desktop credential-export warning. Existing add, switch, clear, success, error, and restart messaging remain shared.

MUX Desktop is React; no Vue component changes are involved.

## Alternatives rejected

### Only patch the currently active external profile

Rejected because it would make MUX overwrite CC Switch or another tool's ownership and could not reliably restore prior state.

### Local MUX gateway

Rejected for this release because the direct configuration already works, while a durable proxy would require a background service, request streaming, lifecycle management, and protocol routing.

### Write every MUX Model as a separate Claude profile

Rejected because MUX's current mapping contract needs only one active Profile. One deterministic entry is simpler to observe, update, clear, and restore safely.

## Verification contract

- Claude Desktop appears as a managed Model Agent only on macOS and reports the four exact targets.
- A failing test first proves `qwen3.7-max` needs `unstableDisableModelVerification`, while `anthropic/claude-sonnet-4-6` does not receive it.
- Apply preserves unrelated JSON and external config-library entries, writes the explicit model, selects the MUX profile, exports the Keychain credential only at commit, and enforces `0600`.
- Planning and serialized operation results contain no credential.
- Clear restores the previous external profile and removes only MUX-owned state.
- Invalid JSON, unsupported protocols, missing credentials, UUID collisions, and CAS changes fail closed.
- Focused Rust and React regression tests are retained. Per the repository's fast-delivery policy, broader test suites run only when explicitly requested; production Stable build and release verification remain mandatory.
