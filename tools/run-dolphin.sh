#!/usr/bin/env bash
# Launch a .dol / .elf / .iso in Dolphin if installed.
set -euo pipefail

TARGET="${1:?usage: run-dolphin.sh <boot.dol|game.iso>}"

candidates=(
  "dolphin-emu"
  "Dolphin"
  "/Applications/Dolphin.app/Contents/MacOS/Dolphin"
  "/opt/homebrew/bin/dolphin-emu"
)

for bin in "${candidates[@]}"; do
  if command -v "$bin" >/dev/null 2>&1 || [[ -x "$bin" ]]; then
    # No -b: keep the Dolphin window open for interactive testing.
    exec "$bin" -e "$TARGET"
  fi
done

echo "Dolphin not found. Install from https://dolphin-emu.org/ or brew install dolphin"
echo "Then open: $TARGET"
exit 1
