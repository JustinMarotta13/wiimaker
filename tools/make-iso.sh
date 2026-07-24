#!/usr/bin/env bash
# Optional: wrap a .dol into a minimal playable disc image with wit.
set -euo pipefail

DOL="${1:?usage: make-iso.sh <boot.dol> [out.iso]}"
OUT="${2:-build/game.iso}"

if ! command -v wit >/dev/null 2>&1; then
  echo "wit (Wiimms ISO Tools) required: brew install wit"
  exit 1
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/files"
cp "$DOL" "$WORKDIR/files/main.dol"

# Create a bare ID6 disc; fine for Dolphin / USB Loader experiments.
wit cp --id=RWIIMK --name="wiimaker" --dest="$OUT" --overwrite "$WORKDIR"
echo "wrote $OUT"
