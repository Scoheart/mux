# MUX repository instructions

- MUX currently manages global Agent MCP configurations only. Do not reintroduce project-scope UI, CLI flags, or writes unless an issue explicitly changes that product boundary.
- Preserve unrelated top-level keys, sibling MCP servers, and unmodelled fields when editing Agent JSON or TOML files. Formatting and key order may change after serialization.
- Keep the stable CLI release filename `mux_v<version>_<target-triple>.tar.gz`; released `mux upgrade` clients resolve that exact pattern.
- Do not edit generated `dist/`, `target/`, or `.vitepress/dist/` output.
- Run `cargo test -p mux-core -p mux-cli`, `npm --prefix desktop run build`, and `npm --prefix website run build` for relevant changes.
- Never merge a repair PR or publish a release. Leave the PR ready for owner review.
