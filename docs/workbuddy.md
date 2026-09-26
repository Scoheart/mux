# WorkBuddy AI integration

MUX manages user-level custom MCP servers for the international WorkBuddy AI
desktop app (`WorkBuddy AI.app`) in `~/.workbuddy-ai/mcp.json` under `mcpServers`.
User-level Skills are linked into `~/.workbuddy-ai/skills`; Models remain native guided setup. The domestic `.workbuddy` directory
and CodeBuddy CLI configuration are separate targets and are not inferred.

## Evidence (2026-09-27)

- Official documentation: https://www.workbuddy.ai/docs/workbuddy/Overview
- Connector guide: https://www.workbuddy.ai/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Connector
- User-provided WorkBuddy AI 5.6.2 configuration editor shows the user-level path
  and an empty `mcpServers` object. The file may not exist until first save.
- Read-only inspection of the installed official app's `app.asar` confirms
  `UserMcpFile` in `main/server.js` joins the product config directory with
  `mcp.json`, and returns an empty object for a missing file.
- `resolveWorkbuddyConfigDir` in `main/code-cache.js` resolves the product's
  `dataFolderName`; the international app's `cli/product.json` declares
  `.workbuddy-ai`. Explicit custom MUX paths remain supported after initial
  promotion from the old read-only definition.
- `parseMcpServerEntry` / `toTransport` in `main/tar.js` accept `command`, `args`,
  `env`, and `cwd` for stdio; `url` and `headers` for HTTP; `type: sse` for SSE.
  `UserMcpFile.toEntry` omits type for stdio/HTTP and emits it for SSE.

## Write boundary

The WorkBuddy codec changes only connection fields. It preserves timeout,
description, deferred loading, disabled tools, unknown fields, and sibling
servers. Disabled entries are excluded from active observations and rejected
for update; MUX does not silently re-enable them. Invalid disabled flags,
malformed JSON, duplicate keys, and invalid MCP containers fail closed.

Existing files use the shared JSON adapter and in-place safe writer, preserving
the watched inode. Missing files use the existing create/backup/CAS path.
The fixture and round-trip coverage are in `core/tests/agent_formats.rs`.

## Skills

`createSkillsService` in the official app's `main/server.js` sets the user Skills
root to `join(configDir, "skills")`. `scanSkillsDirectory` in
`main/log-acl-guard.js` follows directory symlinks with `stat` and looks for
`SKILL.md`. This supports MUX's central-copy plus per-skill symlink contract.
MUX owns only its assigned links and leaves marketplace metadata, external
Skills, and project-level directories untouched.
