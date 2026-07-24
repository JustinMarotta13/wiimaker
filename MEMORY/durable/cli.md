# CLI memory

Operational notes for `wiimaker-cli` and agent-facing workflows.

Canonical rules: `.cursor/rules/wiimaker-cli.mdc`.

## Gotchas

- Prefer `--json` on mutate/query commands; cook after asset import before run.
- `bake-wii` needs a cooked `.wpack` first; `wii-build.sh` runs `cook` then `bake-wii` automatically.

## Decisions

- Wii embed uses `scene.wscn` (baked) rather than parsing JSON on console.

## Open follow-ups

<!-- Next concrete CLI tasks. -->
