#!/usr/bin/env bash
# Cross-build crates/wiimaker-wii as powerpc-unknown-eabi staticlib for the Wii Makefile.
# Uses a custom target JSON (Rust has no built-in powerpc-unknown-eabi) + nightly build-std.
# Falls back with a clear message when the toolchain cannot finish — Makefile keeps stub_game.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_JSON="$ROOT/runtime/wii/targets/powerpc-unknown-eabi.json"
# Cargo names the dir after the JSON stem.
OUT_DIR="$ROOT/target/powerpc-unknown-eabi/release"
OUT="$OUT_DIR/libwiimaker_wii.a"

cd "$ROOT"

if ! command -v rustup >/dev/null 2>&1; then
  echo "wii-rustlib: rustup not found — skip (Makefile will use C stub_game)"
  exit 0
fi

if [[ ! -f "$TARGET_JSON" ]]; then
  echo "wii-rustlib: missing $TARGET_JSON" >&2
  exit 1
fi

echo "==> ensuring nightly + rust-src (build-std)"
rustup toolchain install nightly --profile minimal 2>/dev/null || true
rustup component add rust-src --toolchain nightly 2>/dev/null || true

echo "==> cargo +nightly build -Z build-std=core,alloc -Z json-target-spec --target $TARGET_JSON -p wiimaker-wii --release --no-default-features"
if cargo +nightly build -Z build-std=core,alloc -Z json-target-spec \
    --target "$TARGET_JSON" \
    -p wiimaker-wii \
    --release \
    --no-default-features; then
  if [[ -f "$OUT" ]]; then
    echo "==> rustlib ready: $OUT"
    exit 0
  fi
  echo "wii-rustlib: build reported ok but $OUT missing" >&2
  ls -la "$OUT_DIR" 2>/dev/null || true
  exit 1
fi

echo "wii-rustlib: PowerPC cross-compile failed — Makefile will use C stub_game when .a is absent" >&2
exit 0
