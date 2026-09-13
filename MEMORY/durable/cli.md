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
- Prefabs: `entity create-prefab` (writes asset + links the source entity) · `instantiate-prefab` (records `prefab` link) · `apply-prefab` (Unity Apply: push instance → asset; prefab arg optional when linked) · `revert-prefab` · `unpack-prefab` (clears link) · `prefab-status` (`--json`: `instance`, `prefab`, `overrides`). Files under `assets/prefabs/`.
- `entity set --name X [--x --y --sx --sy --rotation-deg --tag --follow --lerp]` — scale/rotate via `set_entity_scale` / `set_entity_rotation_z` (degrees → radians). `--follow` creates Camera if missing.
- Collider / Trigger: `entity add-component … Collider|--trigger|--filter` or kind `Trigger`; `entity triggers <game> <name>`; `entity despawn <game> <name>`; `entity set --tag N`.
- Camera / Follow: `entity add-component <game> --name Cam Camera` · `Follow --target Player --lerp 0.15`. Same mutate helpers as the editor.
- GridMover: `entity add-component <game> --name Player GridMover --cell 20 --speed 6 [--queued-dir Right]`. `entity set --name Player --cell --speed --queued-dir` creates/updates (empty `--queued-dir ""` clears queue). Diagonals use horizontal axis only.
- AudioSource: `entity add-component <game> --name Player AudioSource --clip beep --volume 1 --play-on-awake false`. `entity set --audio-clip --volume --play-on-awake`. `asset import` copies `.wav`; `asset play --name beep` (`--json` includes `skipped` when no device / `WIIMAKER_AUDIO=0`). `asset list-wavs`.
- Sorting layers: `sorting-layer list|add|rename|move|remove` mutate `game.toml` (`--json`). `add --name [--index]`; `rename --from --to` remaps scenes/prefabs; `move --name --index` (0 = back); `remove --name` refuses Default and remaps assignments to Default. `entity set --sorting-layer --order-in-layer` (alias `--z`) on Sprite/Disc/Tilemap. Empty `--sorting-layer ""` is Default. Unknown layer names error (CLI) / doctor warning (hydrate still draws as Default).
- `scene list` returns paths relative to the game dir (via `list_scenes`), e.g. `scenes/main.scene.json`.
- `scene set-game-view <game> [--width --height] [--aspect free|fixed] [--preset free|640x480|16:9|4:3|custom] [--scale]` writes `.wiimaker/prefs.toml` (same as Game tab).
- `editor prefs <game>` dumps Scene/Game chrome; `editor set-scene-view` sets zoom/pan/grid/gizmos/snap (`--json`). Move-tool axis handles are editor-only (CLI n/a).
- `editor play-status <game> [--build] [--json]` reports whether in-editor Play will load the game `App` cdylib or the WASD fallback. No new prefs — CLI `run` remains the external host twin.
- `entity list` prints an indented tree (non-json); JSON still dumps flat entity array with `parent` fields.
- Sprite sheets: `asset slice <game> <stem> --cols N --rows M`, `asset set-pivot <game> <cell> --x --y`, `asset list-sprites`.

## Decisions

- CLI sources are modular: `main.rs` dispatch · `args.rs` (clap) · `cmds/{project,scene,entity,asset,editor}.rs` · `util.rs` · `pipeline.rs` (ship helpers).
- Wii embed uses `scene.wscn` **WSCN0003** (UV + pivot + length-prefixed Tilemap) rather than parsing JSON on console.
- Tilemap: `tilemap set|fill|stamp|get` (`--name --x --y --id`, `--ascii` or `--cells --width`). Auto-creates a default Tilemap on the named entity if missing. `entity add-component … Tilemap --cols --rows --cell`.
- Collider / Trigger: `entity add-component … Collider|--trigger|--filter` or kind `Trigger`; `entity triggers <game> <name>`; `entity despawn <game> <name>`; `entity set --tag N`.
- Entity mutate twins stay in `wiimaker-scene` (`duplicate_entity`, `rename_entity`, `set_entity_parent`, `unique_entity_name`).
- Sheet grid math / catalog live in `wiimaker-assets` — CLI and editor must not fork slice logic.
- `cook` / `bake-wii` remain for agents; primary UX is `build` / `dolphin` / `play-wii`.

## Open follow-ups

- (none for P1 scale/rotate CLI — shipped)
