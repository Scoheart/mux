# MUX repository instructions

- MUX currently manages global Agent MCP configurations only. Do not reintroduce project-scope UI, CLI flags, or writes unless an issue explicitly changes that product boundary.
- Preserve unrelated top-level keys, sibling MCP servers, and unmodelled fields when editing Agent JSON or TOML files. Formatting and key order may change after serialization.
- Keep the stable CLI release filename `mux_v<version>_<target-triple>.tar.gz`; released `mux upgrade` clients resolve that exact pattern.
- Do not edit release-owned versions in feature PRs. `version.txt`, manifests, generated lock metadata, CHANGELOG, and the stable tag are advanced by the single Release Please PR.
- Keep npm lockfiles committed and use `npm ci`; outside the explicitly time-bounded Fast Lane bypass in `.github/fast-lane.json`, never weaken the stable `verify` check or replace full-SHA Action pins with mutable tags.
- Preserve semantic-version latest selection for Stable publication; a slower older build must never replace a newer updater channel.
- Do not edit generated `dist/`, `target/`, or `.vitepress/dist/` output.
- Run `cargo test -p mux-core -p mux-cli`, `npm --prefix desktop run build`, and `npm --prefix website run build` for relevant changes.
- Never merge a repair or Release PR, move a stable tag, publish a Draft Release, or replace the installed App. Leave the PR ready for owner review.
