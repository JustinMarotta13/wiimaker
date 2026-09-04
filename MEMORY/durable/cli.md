# CLI memory

Operational notes for `wiimaker-cli` and agent-facing workflows.

Canonical rules: `.cursor/rules/wiimaker-cli.mdc`.

## Gotchas

- Prefer `--json` on mutate/query commands; prepare assets (`cook`) after asset import before run, or rely on `build` / host Play auto-prepare.
- `bake-wii` needs a prepared `.wpack` first; `wiimaker build` / `wii-build.sh` run prepare then bake automatically.
- Ship verbs: `build` (alias `build-wii`), `dolphin`, `play-wii`. Shared helpers in `pipeline.rs`.
- `entity duplicate <game> <name>` / `entity rename <game> <old> <new>` return new/renamed name in `--json`.
- `entity set-parent <game> --name Child [--parent Parent]` — omit `--parent` to unparent; preserves world pose.
- `entity remove-component` / `set-component-enabled --enabled true|false` (clap `ArgAction::Set`).
- Prefabs: `entity create-prefab` · `instantiate-prefab` · `apply-prefab` · `unpack-prefab` (files under `assets/prefabs/`).
- `entity set --name X [--x --y --sx --sy --rotation-deg]` — scale/rotate via `set_entity_scale` / `set_entity_rotation_z` (degrees → radians).
- `scene list` returns paths relative to the game dir (via `list_scenes`), e.g. `scenes/main.scene.json`.
- `scene build-list` / `build-add` / `build-remove` mutate `game.toml` `scenes` (Build Settings). Empty list is omitted; authoring `scene list` still walks `scenes/`.
- `entity list` prints an indented tree (non-json); JSON still dumps flat entity array with `parent` fields.
- Sprite sheets: `asset slice <game> <stem> --cols N --rows M`, `asset set-pivot <game> <cell> --x --y`, `asset list-sprites`.

## Decisions

- CLI sources are modular: `main.rs` dispatch · `args.rs` (clap) · `cmds/{project,scene,entity,asset}.rs` · `util.rs` · `pipeline.rs` (ship helpers).
- Wii embed uses `scene.wscn` **WSCN0003** (UV + pivot + length-prefixed Tilemap) rather than parsing JSON on console.
- Tilemap: `tilemap set|fill|stamp|get` (`--name --x --y --id`, `--ascii` or `--cells --width`). Auto-creates a default Tilemap on the named entity if missing. `entity add-component … Tilemap --cols --rows --cell`.
- Collider / Trigger: `entity add-component … Collider|--trigger|--filter` or kind `Trigger`; `entity triggers <game> <name>`; `entity despawn <game> <name>`; `entity set --tag N`.
- Entity mutate twins stay in `wiimaker-scene` (`duplicate_entity`, `rename_entity`, `set_entity_parent`, `unique_entity_name`).
- Sheet grid math / catalog live in `wiimaker-assets` — CLI and editor must not fork slice logic.
- `cook` / `bake-wii` remain for agents; primary UX is `build` / `dolphin` / `play-wii`.

## Open follow-ups

- (none for P1 scale/rotate CLI — shipped)
