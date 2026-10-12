#!/usr/bin/env bash
# Builds the browser bundle: a .wasm module plus its JS loader.
#
# Run inside `nix develop` -- it needs wasm-bindgen-cli at the exact version
# pinned in Cargo.toml, plus lld for the wasm32 linker.
#
# Usage: scripts/build-wasm.sh [output-dir]
set -euo pipefail

out_dir="${1:-web/pkg}"
target_dir="target/wasm32-unknown-unknown/release"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "==> building for wasm32-unknown-unknown"
# --no-default-features drops the `native` feature, and with it ffmpeg, ratatui
# and clap. None of them can target the browser, and none of them are needed:
# the browser supplies the frames.
cargo build --target wasm32-unknown-unknown --release --no-default-features

echo "==> generating bindings into ${out_dir}"
rm -rf "$out_dir"
mkdir -p "$out_dir"
wasm-bindgen "${target_dir}/textura.wasm" \
    --target web \
    --out-dir "$out_dir" \
    --out-name textura \
    --no-typescript

echo
echo "==> output"
ls -lh "$out_dir"
