#!/bin/bash
# Installs the Rust toolchain required by the active Pyodide cross-build
# environment (see `pyodide xbuildenv install`).  Mirrors what pyodide-build
# does internally for in-tree recipes: older Pyodide versions need a nightly
# toolchain with a custom wasm-eh enabled sysroot for wasm32-unknown-emscripten.
#
# Prints the name of the toolchain to use on stdout.
set -euo pipefail

TOOLCHAIN="$(pyodide config get rust_toolchain)"
TARGET_URL="$(pyodide config get rust_emscripten_target_url 2>/dev/null || true)"

rustup toolchain install "$TOOLCHAIN" --profile minimal >&2

if [[ -z "$TARGET_URL" ]]; then
  rustup target add wasm32-unknown-emscripten --toolchain "$TOOLCHAIN" >&2
else
  RUSTLIB="$(dirname "$(dirname "$(rustup which --toolchain "$TOOLCHAIN" rustc)")")/lib/rustlib"
  TOKEN="$RUSTLIB/wasm32-unknown-emscripten_install-url.txt"
  if [[ ! -f "$TOKEN" || "$(cat "$TOKEN")" != "$TARGET_URL" ]]; then
    rm -rf "$RUSTLIB/wasm32-unknown-emscripten"
    curl -fsSL "$TARGET_URL" | tar -xj -C "$RUSTLIB"
    printf '%s' "$TARGET_URL" > "$TOKEN"
  fi
fi

echo "$TOOLCHAIN"
