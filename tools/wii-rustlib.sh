#!/usr/bin/env bash
# Cross-build crates/wiimaker-wii as powerpc-unknown-eabi staticlib for the Wii Makefile.
# Uses a custom target JSON (Rust has no built-in powerpc-unknown-eabi) + nightly build-std.
#
# Default (local / wii-build.sh): best-effort — exits 0 on toolchain failure so the
# Makefile can keep C stub_game when libwiimaker_wii.a is absent.
#
# Strict (CI): set WIIMAKER_RUSTLIB_STRICT=1 or pass --strict so a failed cross-compile
# or missing .a exits non-zero. GitHub Actions uses this so PowerPC rustlib stays green.
set -euo pipefail

STRICT=0
if [[ "${WIIMAKER_RUSTLIB_STRICT:-}" == "1" ]] || [[ "${1:-}" == "--strict" ]]; then
  STRICT=1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_JSON="$ROOT/runtime/wii/targets/powerpc-unknown-eabi.json"
# Cargo names the dir after the JSON stem.
OUT_DIR="$ROOT/target/powerpc-unknown-eabi/release"
OUT="$OUT_DIR/libwiimaker_wii.a"

fail_or_skip() {
  local msg="$1"
  if [[ "$STRICT" -eq 1 ]]; then
    echo "wii-rustlib: STRICT: $msg" >&2
    exit 1
  fi
  echo "wii-rustlib: $msg" >&2
  exit 0
}

cd "$ROOT"

if ! command -v rustup >/dev/null 2>&1; then
  fail_or_skip "rustup not found — skip (Makefile will use C stub_game)"
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
    ls -la "$OUT"
    exit 0
  fi
  echo "wii-rustlib: build reported ok but $OUT missing" >&2
  ls -la "$OUT_DIR" 2>/dev/null || true
  exit 1
fi

fail_or_skip "PowerPC cross-compile failed — Makefile will use C stub_game when .a is absent"
