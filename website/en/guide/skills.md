# User-level Skills

MUX Desktop manages user-level Skills that follow the Agent Skills format as central assets. Add a Skill to the central library first, then separately choose which Agents consume it; an Agent page never resolves a source or reinstalls the same Skill. This version manages only global Skills under the user's home directory and neither reads nor writes project directories such as `.agents/skills` or `.claude/skills`.

> Skills currently have a Desktop entry only. The CLI/TUI does not expose Skills commands yet.

## Add to the central library

Open **Skills** in the top bar and choose **Add Skill**. A GitHub source downloads directly; a local folder or archive imports directly. When a source contains multiple Skills, select only the ones you want. Central intake no longer opens review, risk-evidence, or file-diff screens. It only writes the central copy under `~/.mux/skills/`; it selects no Agent, creates no link, and establishes no consumption relationship.

MUX still validates source identity, directory boundaries, archive structure, content hashes, and concurrent changes, then writes through a recoverable atomic transaction. These checks add no interaction step. A same-name central asset is replaced only after choosing the explicit backup-and-continue action.

| Source | Behavior |
|---|---|
| Public GitHub | Accepts `owner/repo`, repository URLs, and GitHub tree URLs for subdirectories. MUX resolves the source to an immutable commit over HTTPS and downloads an archive without invoking local Git. |
| Local folder | Must be selected with the native macOS folder picker. MUX copies a snapshot, never creates a live link to the original folder, and does not accept a typed path. |
| Local archive | Imports `.zip`, `.tar.gz`, `.tgz`, or `.tar` through the native picker. MUX extracts it safely and records each Skill's path inside the archive for later checks, updates, and repair. |

A source may contain one or more Skills with a valid `SKILL.md`. Resolution and safety validation run in MUX's bundled Rust core, so using this feature does not require Git, Node.js, or `npx`.

Private GitHub repositories, GitLab, SSH Git, and remote archive URLs are not supported yet.

## One central copy, multiple links

After download or import completes, MUX stores the single managed copy of each Skill at:

```text
~/.mux/skills/<skill-name>/
```

When a consumption relationship is established, selected Agent directories contain only managed links to that central copy. Every consumer therefore sees one update, while removing one relationship only removes its link and does not delete the central content.

MUX normalizes consumption by physical directory. Some Agents also read a compatibility directory: Cursor, Gemini CLI, OpenCode, and GitHub Copilot CLI can all read `~/.agents/skills`. A link written to Codex's preferred directory may therefore grant access to those other installed Agents too. Agents sharing one physical target are selected as an inseparable group; the review lists every Agent actually affected and removes redundant links that would make one Skill appear twice.

## Verified Agent paths

The first release declares user-level Skills support for these six Agents. MUX shows an Agent only when an installation probe succeeds and its capability data is verified; the existence of a shared directory alone does not prove that Agent is installed.

| Agent | Preferred user-level directory | Compatibility directories |
|---|---|---|
| Claude Code | `~/.claude/skills` | — |
| Codex | `~/.agents/skills` | — |
| Cursor | `~/.cursor/skills` | `~/.agents/skills` |
| Gemini CLI | `~/.gemini/skills` | `~/.agents/skills` |
| OpenCode | `~/.config/opencode/skills` | `~/.claude/skills`, `~/.agents/skills` |
| GitHub Copilot CLI | `~/.copilot/skills` | `~/.agents/skills` |

An Agent's MCP config path and Skills path are separate contracts; MUX never infers one from the other. See [Supported agents](/en/guide/agents#skills-capabilities) for context.

## Background safety checks

Before writing, MUX validates candidate structure and content locally. Escaping links, path traversal, special files, oversized archives, and content changed before commit are rejected. Executable and script findings remain available in asset details but no longer add an approval step during download or import.

- Skill content, content hashes, file paths, and risk findings are never uploaded.
- MUX does not run candidate scripts, and “no high-risk pattern found” is not a security certification.
- `SKILL.md` is rendered as plain text; embedded HTML, scripts, and remote resources are not executed.

## Lifecycle operations

Download and import commit their internal plans directly from the user's action. Updates, removal, repair, and Agent assignment still show impact when applicable. If content or settings change after planning, MUX rejects the stale operation and asks the user to retry.

| Operation | Result |
|---|---|
| Assign to an Agent | Choose the Skill from the relevant Agent page and review a separate relationship plan. The central copy itself does not change; all Agents sharing one target are shown and changed together. |
| Check / update | Background and manual checks read only a GitHub revision, local-folder hash, or archive hash and never change content. Choosing Update then stages the candidate, shows the diff, reruns the audit, and confirms replacement. Local modifications to the central copy require “back up and replace.” |
| Import | An external copy in an Agent directory remains read-only first. Choosing Import directly copies and validates it, backs up the original directory, and replaces it with a central link; the original is not moved before success. |
| Disable | Removes the managed target link while retaining the central copy and other assignments. Review lists every Agent that loses access through a shared directory. |
| Repair | Rebuilds a broken link that still matches the managed record. If central content is missing, MUX resolves the recorded source or read-only import backup again and presents the full diff and risk review. |
| Remove | Removes all managed links, moves the central copy into timestamped `~/.mux/backups/skills/`, then removes its managed record. This version has no permanent backup purge action. |

Candidates and internal transaction plans live in `~/.mux/staging/skills/`; commit progress lives in `~/.mux/journals/skills/`. If a commit fails or the app crashes, the journal safely rolls back or finishes the commit according to the persisted phase. If recovery cannot complete, the Skills workspace becomes read-only and refuses new writes.

## Current boundaries

This version does not support:

- project-level Skills;
- private repositories or authenticated Git sources;
- creating or editing `SKILL.md` in MUX;
- CLI/TUI Skills commands.

Return to the [Desktop app guide](/en/guide/desktop#skills) or see [Supported agents](/en/guide/agents#skills-capabilities).
