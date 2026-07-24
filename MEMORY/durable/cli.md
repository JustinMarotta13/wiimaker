# CLI memory

Operational notes for `wiimaker-cli` and agent-facing workflows.

Canonical rules: `.cursor/rules/wiimaker-cli.mdc`.

## Gotchas

- Prefer `--json` on mutate/query commands; cook after asset import before run.
- `bake-wii` needs a cooked `.wpack` first; `wii-build.sh` runs `cook` then `bake-wii` automatically.
- `entity duplicate <game> <name>` / `entity rename <game> <old> <new>` return new/renamed name in `--json`.

## Decisions

- Wii embed uses `scene.wscn` (baked) rather than parsing JSON on console.
- Entity mutate twins stay in `wiimaker-scene` (`duplicate_entity`, `rename_entity`, `unique_entity_name`).

## Open follow-ups

<!-- Next concrete CLI tasks. -->
