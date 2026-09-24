#!/usr/bin/env bash
# Cross-build a Wii .dol via Docker + the C runtime (scene embed or Rust staticlib).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:-hello-orb}"
IMAGE="wiimaker-devkit"
OUT="$ROOT/target/wii/$GAME"

echo "==> cook + bake-wii for $GAME"
cargo run -q -p wiimaker-cli -- cook "$GAME"
cargo run -q -p wiimaker-cli -- bake-wii "$GAME"

echo "==> optional Rust staticlib (powerpc-unknown-eabi)"
# Best-effort: produces libwiimaker_wii.a when nightly/build-std work.
# Missing .a → Makefile USE_STUB=1 (C stub_game). Embeds always link either way.
"$ROOT/tools/wii-rustlib.sh" || true

if ! command -v docker >/dev/null 2>&1; then
  echo "wii-build: docker not found — cook/bake/rustlib done; skip .dol link" >&2
  echo "    Install Docker + run again, or make inside a devkitPPC container." >&2
  exit 0
fi

echo "==> building docker image $IMAGE"
docker build -t "$IMAGE" "$ROOT/docker"

echo "==> compiling runtime/wii for game=$GAME"
docker run --rm \
  -v "$ROOT":/src \
  -w /src/runtime/wii \
  "$IMAGE" \
  bash -lc "make clean && make GAME=$GAME"

mkdir -p "$OUT"
if [[ -f "$ROOT/runtime/wii/boot.dol" ]]; then
  cp "$ROOT/runtime/wii/boot.dol" "$OUT/boot.dol"
fi
cp "$ROOT/runtime/meta/meta.xml" "$OUT/meta.xml"
# HBC layout: apps/<game>/{boot.dol,meta.xml,icon.png}
HBC="$OUT/hbc/apps/$GAME"
mkdir -p "$HBC"
cp "$OUT/boot.dol" "$HBC/"
cp "$OUT/meta.xml" "$HBC/"
echo "==> ready: $OUT/boot.dol"
echo "    HBC pack: $HBC"
echo "    Try: ./tools/run-dolphin.sh $OUT/boot.dol"
