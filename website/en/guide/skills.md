# User-level Skills

MUX Desktop manages user-level Skills that follow the Agent Skills format as central assets. Add a Skill to the central library first, then separately choose which Agents consume it; an Agent page never resolves a source or reinstalls the same Skill. This version manages only global Skills under the user's home directory and neither reads nor writes project directories such as `.agents/skills` or `.claude/skills`.

> Skills currently have a Desktop entry only. The CLI/TUI does not expose Skills commands yet.

## Add to the central library

Open **Skills** in the top bar and choose **Add Skill**. Central intake has three steps: choose a source, select discovered Skills, then review and confirm the plan. It only writes the central copy under `~/.mux/skills/`; it selects no Agent, creates no link, and establishes no consumption relationship. After intake, use **Manage Agents** in the Skill Inspector or **Manage Skills** on an Agent page to choose consumers.

| Source | Behavior |
|---|---|
| Public GitHub | Accepts `owner/repo`, repository URLs, and GitHub tree URLs for subdirectories. MUX resolves the source to an immutable commit over HTTPS and downloads an archive without invoking local Git. |
| Local folder | Must be selected with the native macOS folder picker. MUX copies a snapshot, never creates a live link to the original folder, and does not accept a typed path. |

A source may contain one or more Skills with a valid `SKILL.md`. Resolution, validation, diffs, and risk analysis run in MUX's bundled Rust core, so using this feature does not require Git, Node.js, or `npx`.

Private GitHub repositories, GitLab, SSH Git, and arbitrary archive URLs are not supported yet.

## One central copy, multiple links

After central intake is confirmed, MUX stores the single managed copy of each Skill at:

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

## Local risk review

Before writing, MUX performs deterministic local static analysis of candidate files. Escaping links are rejected during structural validation. For auditable content, MUX flags patterns such as executables, scripts, download-and-execute commands, privilege escalation, destructive file operations, credential reads, data upload, and obfuscated payloads, with the rule, file, line when applicable, and reason.

- Skill content, content hashes, file paths, and risk findings are never uploaded.
- MUX does not run candidate scripts, and “no high-risk pattern found” is not a security certification.
- A high-risk operation requires reviewing the displayed evidence, explicitly checking the override, and passing a separate second confirmation.
- `SKILL.md` is rendered as plain text; embedded HTML, scripts, and remote resources are not executed.

## Lifecycle operations

Every write starts with a plan and commits only after confirmation. When applicable, the plan shows file changes, risk, central-copy conflicts, target paths, shared impact, and the fact that a backup will be retained. If content or settings change after review, MUX rejects the stale plan and requires a new review.

| Operation | Result |
|---|---|
| Manage Agents | Choose consumers from a Skill Inspector or an Agent page and review a separate relationship plan. The central copy itself does not change; all Agents sharing one target are shown and changed together. |
| Check / update | Background and manual checks read only a GitHub revision or local hash and never change content. Choosing Update then downloads the candidate, shows the diff, reruns the audit, and confirms replacement. Local modifications to the central copy require “back up and replace.” |
| Import | An external copy in an Agent directory remains read-only first. After confirmation, MUX copies and validates it, backs up the original directory, and replaces it with a central link; the original is not moved before success. |
| Disable | Removes the managed target link while retaining the central copy and other assignments. Review lists every Agent that loses access through a shared directory. |
| Repair | Rebuilds a broken link that still matches the managed record. If central content is missing, MUX resolves the recorded source or read-only import backup again and presents the full diff and risk review. |
| Remove | Removes all managed links, moves the central copy into timestamped `~/.mux/backups/skills/`, then removes its managed record. This version has no permanent backup purge action. |

Candidates and reviewed plans live in `~/.mux/staging/skills/`; commit progress lives in `~/.mux/journals/skills/`. If a commit fails or the app crashes, the journal safely rolls back or finishes the commit according to the persisted phase. If recovery cannot complete, the Skills workspace becomes read-only and refuses new writes.

## Current boundaries

This version does not support:

- project-level Skills;
- private repositories or authenticated Git sources;
- creating or editing `SKILL.md` in MUX;
- CLI/TUI Skills commands.

Return to the [Desktop app guide](/en/guide/desktop#skills) or see [Supported agents](/en/guide/agents#skills-capabilities).
