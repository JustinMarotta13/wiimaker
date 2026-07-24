# CLI memory

Operational notes for `wiimaker-cli` and agent-facing workflows.

Canonical rules: `.cursor/rules/wiimaker-cli.mdc`.

## Gotchas

- Prefer `--json` on mutate/query commands; cook after asset import before run.
- `bake-wii` needs a cooked `.wpack` first; `wii-build.sh` runs `cook` then `bake-wii` automatically.
- `scene list` returns paths relative to the game dir (via `list_scenes`), e.g. `scenes/main.scene.json`.

## Decisions

- Wii embed uses `scene.wscn` (baked) rather than parsing JSON on console.

## Open follow-ups

<!-- Next concrete CLI tasks. -->
