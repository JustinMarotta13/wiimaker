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
- AudioSource: `entity add-component <game> --name Player AudioSource --clip beep --volume 1 --play-on-awake false`. `entity set --audio-clip --volume --play-on-awake`. `asset import` copies `.wav`; `asset play --name beep` (`--json` includes `skipped` when no device / `WIIMAKER_AUDIO=0`). `asset list-wavs`. `cook` / `prepare` pack WAV stems into the `.wpack` audio TOC (`--json` `audio` count). `bake-wii` emits `KIND_AUDIO` and/or a trailing AudioSource table.
- Sorting layers: `sorting-layer list|add|rename|move|remove` mutate `game.toml` (`--json`). `add --name [--index]`; `rename --from --to` remaps scenes/prefabs; `move --name --index` (0 = back); `remove --name` refuses Default and remaps assignments to Default. `entity set --sorting-layer --order-in-layer` (alias `--z`) on Sprite/Disc/Tilemap/Text. Empty `--sorting-layer ""` is Default. Unknown layer names error (CLI) / doctor warning (hydrate still draws as Default).
- Sprite pivot override: `entity set --name X --pivot-x --pivot-y` writes `SceneSprite.pivot` (error if no Sprite). `--clear-pivot` drops the override (catalog again). `entity add-component … Sprite --texture --pivot-x --pivot-y`. Mutate: `set_entity_sprite_pivot`.
- Text / HUD: `entity add-component <game> --name Hud Text --text "Score: 0" --size 16 --color 255,255,255 --align left`. `entity set --text --size --color --align` creates Text if missing. `--color` is `R,G,B` or `R,G,B,A`.
- `scene list` returns paths relative to the game dir (via `list_scenes`), e.g. `scenes/main.scene.json`.
- `scene set-game-view <game> [--width --height] [--aspect free|fixed] [--preset free|640x480|16:9|4:3|custom] [--scale]` writes `.wiimaker/prefs.toml` (same as Game tab).
- `editor prefs <game>` dumps Scene/Game/Project chrome; `editor set-scene-view` sets zoom/pan/grid/gizmos/snap; `editor set-project-view` collapse/expand/clear folder paths (`--json`). Move-tool axis handles are editor-only (CLI n/a). Filter text is session-only (not in prefs).
- `editor play-status <game> [--build] [--json]` reports whether in-editor Play will load the game `App` cdylib or the WASD fallback. No new prefs — CLI `run` remains the external host twin.
- `entity list` prints an indented tree (non-json); JSON still dumps flat entity array with `parent` fields.
- Tilemap: `tilemap set|fill|stamp|from-ascii|get|set-palette|mask`. `from-ascii FILE --name Maze` resolves FILE as cwd, then game dir, then `assets/`. Default `--resize true` (unlike inline `stamp --ascii`, which clips). Quote `--map '#=1,P=2:0'` because `#` is a shell comment.
- `input map` has no game argument (static table). `--json` includes `ok`, `legend`, `deadzone`, `buttons` bitmasks, `map[]`.

## Decisions

- CLI sources are modular: `main.rs` dispatch · `args.rs` (clap) · `cmds/{project,scene,entity,asset,editor,input}.rs` · `util.rs` · `pipeline.rs` (ship helpers).
- `input map` / `input map --json` prints the static GCN-layout table (Keyboard / Wiimote / Classic / Nunchuk stick + Z). No game name. Doctor text adds one `input:` legend line (JSON diagnose unchanged). Table rows live in `wiimaker-core::wiimote_map::MAP_ROWS`. Nunchuk C is unmapped (same WPAD bit as Classic LEFT).
- Wii embed uses `scene.wscn` **WSCN0003** (UV + pivot + length-prefixed Tilemap palette + `KIND_TEXT` + `KIND_AUDIO` / trailing AudioSource table) rather than parsing JSON on console. `.wpack` is `WPACK001` textures + meshes + optional audio TOC.
- Tilemap: `tilemap set|fill|stamp|from-ascii|get|set-palette|mask` (`--name --x --y --id`, `--ascii` or `--cells --width`, `from-ascii FILE` with optional `--map '#=1,P=2:0'` and `--resize` default true, palette `--sprite --color --anim --fps --auto-tile id|solid|off --auto-sprites`). Auto-creates a default Tilemap on the named entity if missing. `from-ascii` resizes the grid to the file (stamp `--ascii` stays inline and clips). `entity add-component … Tilemap --cols --rows --cell`. `get --json` includes NESW `mask`. WSCN bake packs resolved palette (tex/UV, auto-tile variants, anim frames) for GX.
- Collider / Trigger: `entity add-component … Collider|--trigger|--filter` or kind `Trigger`; `entity triggers <game> <name>`; `entity despawn <game> <name>`; `entity set --tag N`.
- Entity mutate twins stay in `wiimaker-scene` (`duplicate_entity`, `rename_entity`, `set_entity_parent`, `unique_entity_name`).
- Sheet grid math / catalog live in `wiimaker-assets` — CLI and editor must not fork slice logic.
- `cook` / `bake-wii` remain for agents; primary UX is `build` / `dolphin` / `play-wii`.

## Open follow-ups

- (none for P1 scale/rotate CLI — shipped)
