#!/bin/bash
# Stage the mux CLI as a Tauri sidecar (bundle.externalBin), so installing the
# desktop app also ships the command-line tool inside MUX.app/Contents/MacOS/.
# Tauri resolves `binaries/mux` to `binaries/mux-<target-triple>` at build time.
set -euo pipefail
cd "$(dirname "$0")/.." # desktop/

TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
cargo build --release --locked -p mux-cli --manifest-path ../Cargo.toml
mkdir -p src-tauri/binaries
cp ../target/release/mux "src-tauri/binaries/mux-${TRIPLE}"
echo "sidecar staged: src-tauri/binaries/mux-${TRIPLE}"
