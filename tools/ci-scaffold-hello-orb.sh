#!/usr/bin/env bash
# Materialize games/hello-orb from templates/basic-game for CI / fresh clones.
# hello-orb is a workspace member but lives under gitignored games/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/games/hello-orb"
TEMPLATE="$ROOT/templates/basic-game"

if [[ -f "$DEST/Cargo.toml" ]]; then
  echo "ci-scaffold-hello-orb: already present at $DEST"
  exit 0
fi

mkdir -p "$ROOT/games"
cp -a "$TEMPLATE" "$DEST"
for rel in Cargo.toml src/main.rs src/lib.rs src/game.rs game.toml scenes/main.scene.json; do
  path="$DEST/$rel"
  if [[ -f "$path" ]]; then
    sed -i 's/{{name}}/hello-orb/g' "$path"
  fi
done
mkdir -p "$DEST/assets"
echo "ci-scaffold-hello-orb: created $DEST from template"
