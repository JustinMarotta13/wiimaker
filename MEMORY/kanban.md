# Wiimaker feature board

Living engine board (not the local Pac-Man game). Ranked by what a maze-chomper actually needs next. Almost every item is **GUI + CLI** — files stay truth (`wiimaker-scene` mutate helpers, then twin the editor panel). Each Now/Next card is one morning.

Pac-Man probe (local, gitignored): `games/pac-man/` — maze is Disc entities + game-side grid. Use it as the acceptance test for Now items.

## Already have (do not rebuild)

Authoring loop is already Unity-shaped. Do not re-litigate these:

| Unity | Engine today |
|---|---|
| Project window | `game.toml` + `assets/` + `scenes/` + editor **Project** explorer |
| Hierarchy | editor Hierarchy (parent/unparent DnD, multi-select, duplicate) |
| Inspector | Transform + Sprite/Disc/Camera/Tilemap/Collider/Animation/GridMover/AudioSource/Text, enable checkbox, catalog combo, tile palette (anim + auto-tile), **Sorting Layer** + **Order in Layer**, Sprite **Pivot** X/Y override + Reset |
| Scene view | 640×480 viewport, pick/drag, **Move / Scale / Rotate / Hand / Paint / Erase / Pick**, 2D (always-on), zoom % + scroll, grid overlay, gizmos, **Move axis handles** (red X / green Y), Snap + nudge |
| Game view | aspect dropdown (Free / 640×480 / 16:9 / 4:3 / custom) + Scale + letterbox; prefs in `.wiimaker/prefs.toml` |
| Play | toolbar Play/Pause/Stop ticks the open game `App` (`wiimaker-play` cdylib) · WASD/`Player` fallback if no plugin · File → Run external → `cargo run -p <game>` |
| Prefab | `.prefab.json` · instance `prefab` link · Save as Prefab / Instantiate / Apply (push to asset) / Revert / Unpack Completely · orange-bold Inspector overrides |
| Sprite Editor | `assets/<stem>.sprites.json` · Grid By Cell Count + pivot |
| Undo | `UndoStack` in `wiimaker-scene` (depth 50) · Cmd/Ctrl+Z/Y |

Runtime already: `World` (named entities, Transform, Sprite, Disc, Camera + optional Follow, Tilemap, Collider, Animation, GridMover, AudioSource, Text, `tag: u32`), `DrawList` IR, GCN-layout `Input` (WASD/arrows → stick + D-pad), 60 Hz `Clock`, `render_world` sorts by Sorting Layer then order-in-layer `z` (tile cells as sprites/colored quads; HUD `DrawText` as bitmap glyphs), parented local transforms, sprite UV/pivot, `.wpack` cook, WSCN0003 bake (UV + pivot + length-prefixed Tilemap), `wiimaker build` / `dolphin` / `play-wii`. Queries: `tile_solid` / `world_to_cell` / `tile_solid_world` · `overlaps` / `move_and_collide` · `triggers_entered` · `animate_world` + `Animation` / `*.anim.json` · palette `anim` tiles + 4-neighbor `auto_tile` (NESW bitmask) · active Camera offsets dests (centered 640×480) + `World::follow_cameras` · `GridMover` + `cardinal` / `World::step_grid_movers` (horizontal wins on diagonals; reverse immediate; snap to cell centers) · `World::play_oneshot` / play-on-awake (host cpal). Project Sorting Layers in `game.toml` (`Background` / `Default` / `Foreground` when omitted).

**Not present:** nested prefabs / prefab variants, Wii GX draw of tilemaps (payload skipped — animated / auto-tile host-only), Wii GX text, Wii ASND.

---

## Now

**Recommended next morning (2026-09-16):** Clear-color editor UI with undo.

---

## Later

- **Clear-color editor UI with undo** — editor follow-up; CLI `scene set-clear` exists.
- **Wii audio / Wiimote** — after host oneshots.
- **Wii GX text** — host DrawText exists; C player still KIND_NONE.
- **Wii GX tilemaps** — host animated / auto-tile exists; C still skips the Tilemap payload.

---

## Done

Shipped. Keep here so we do not rebuild them.

- **Unity Hierarchy + Inspector chrome** (2026-09-01) · 1:1 follow-up: crops + rules in `MEMORY/durable/unity-chrome/` / `.cursor/rules/wiimaker-editor.mdc` — Hierarchy search, + Create Empty, right-click Duplicate/Delete/Unparent, no per-row D/x; Inspector GameObject name+Tag, Transform Position/Rotation/Scale as XYZ DragValues, ⋮ Remove Component, full-width Add Component. Dark Pro only.

- **Sprite animation clips** (2026-08-31) — `AnimClipMeta` / `assets/<name>.anim.json`, runtime `Animation`, `animate_world`, Inspector Animation foldout (clip combo + fps/loop), CLI `asset anim` / `asset list-anims` / `entity set-anim` / `add-component Animation`; doctor warns missing clip cells. Host-first (not in WSCN bake).

- Workspace + host hello-orb + Wii C bootstrap stubs
- Sprites + Disc + DrawList IR + host software raster + texture atlas
- Fixed 60 Hz tick, GCN-layout input (keyboard → stick/D-pad/A/B/Start)
- `.wpack` cook (PNG → tiled RGB5A3) · sprite sheet sidecar + catalog + pivot
- Scene JSON / prefab JSON · hydrate (strict + lenient) · WSCN0003 bake (UV + pivot + Tilemap payload)
- egui editor: Hierarchy, Inspector, Scene viewport, Project, Sprite Editor, theme
- Editor Play/Pause/Stop (game `App` plugin, WASD fallback) · Build · Play in Dolphin · Build & Run · Cook under ⋯
- Undo/redo, duplicate/copy/paste, parent/unparent, multi-select, snap, Move/Scale/Rotate
- Prefab create / instantiate / apply / unpack (link + overrides shipped 2026-09-12)
- Agent CLI twin of mutations (`--json`): see command list below
- Doctor (project/scene/assets)
- Parent local transforms (translate×scale; full rotation compose still open)
- Tilemap + solid cells (scene `Tilemap`, viewport Paint/Erase/Pick, Inspector grid+palette + **Import ASCII**, CLI `tilemap set|fill|stamp|from-ascii|get`, `tile_solid` / `world_to_cell`, WSCN0003 bake)
- AABB/Circle collider + overlap (scene `Collider`, Inspector kind/size/solid, viewport seafoam outline gizmo, CLI `entity add-component … Collider --w --h`, `entity overlaps`, `overlaps` / `move_and_collide`; host-first, WSCN0003 unchanged)
- Unity 6 editor chrome (dark Pro docks: Hierarchy left, Scene/Game center, Inspector right, Project/Console bottom; Play/Pause/Stop centered; component foldout cards). CLI `scene new` · `scene set-default`
- Trigger / collectible (GUI Is Trigger + Filter Tag; CLI Trigger/--trigger/--filter, entity triggers, entity despawn; `triggers_entered`; triggers skip `move_and_collide`)
- **Runtime scene load / switch** (2026-09-04) — `load_scene_into_world` (scene + host atlas helper) replaces World, keeps texture map; `game.toml` `scenes = [...]` Build Settings; doctor warns default missing from list / missing files; CLI `scene build-list` · `build-add` · `build-remove` (`--json`); editor File → Build Settings… + Inspector on `game.toml`. Pac-Man stays local (do not commit `games/`).

- **Camera follow** (2026-09-06) — active `Camera` offsets `render_world` dests (viewport center = camera translation; `(320,240)` = identity). Optional Follow on `SceneCamera` (`follow` name + `lerp`, default 0.15). `World::follow_cameras` each play/host tick. Inspector Camera foldout (target combo + lerp); Scene view 640×480 cyan rect gizmo; Game view applies offset. CLI `entity add-component … Camera` · `Follow --target --lerp` · `entity set --follow --lerp`. Host-first (WSCN unchanged). No Camera → identical dests.

- **4-way grid-snap mover** (2026-09-07) — optional `GridMover { cell, speed, queued_dir }` + `cardinal(input)` / `try_step` / `World::step_grid_movers`. Diagonals: **horizontal wins**. Reverse of current heading is immediate; 90° turns wait for cell center. Stick +Y = Up = −Y world. Inspector foldout (cell, speed, queued dir). CLI `entity add-component … GridMover --cell --speed --queued-dir` · `entity set --cell --speed --queued-dir` (empty `--queued-dir ""` clears). Host play + editor Play tick. Host-first (WSCN unchanged).

- **Scene viewer + Game view aspect** (2026-09-08) — Scene: geometric Move/Scale/Rotate/**Hand**/Paint/Erase/Pick, always-on **2D**, scroll + `%` zoom, pan (Hand or middle-drag), **Grid** overlay, **Gizmos** toggle, Snap. Game: Free Aspect / 640×480 / 16:9 / 4:3 / custom W×H, letterbox/pillarbox, Scale slider. Store: `<game>/.wiimaker/prefs.toml` (`EditorPrefs`; not `game.toml`). CLI `scene set-game-view` · `editor prefs` · `editor set-scene-view` (`--json`). Host-first.

- **Gizmo handles** (2026-09-09) — Scene Move tool: Unity-like 2D handles at the selected origin (Inspector red X / green Y, XY free square). Drag X locks Y; drag Y locks X; entity-body drag stays free. **Gizmos** on: collider AABB/circle fill + ticks (seafoam / amber triggers); tilemap bounds + cell grid. Hit-test in `wiimaker-scene` `gizmo.rs`. CLI n/a (no new prefs flag). Host-first.

- **Audio oneshots** (2026-09-10) — `AudioSource` (`clip`, `volume`, `play_on_awake`) on scene JSON; `World::play_oneshot` queue; host plays `assets/*.wav` (PCM16 mono/stereo) via `aplay`/`paplay` when present. Missing clip errors; no player / `WIIMAKER_AUDIO=0` skips. Inspector foldout + Project play / double-click; CLI `asset import` `.wav`, `asset play --name`, `asset list-wavs`, `entity add-component … AudioSource`, `entity set --audio-clip --volume --play-on-awake`. Cook still PNG-only (WAV stay on disk). WSCN unchanged. Wii ASND stub comments only. Fixture: `crates/wiimaker-assets/fixtures/beep.wav`.

- **Sorting layers** (2026-09-11) — Unity Sorting Layer + Order in Layer. `game.toml` `sorting_layers` (default Background / Default / Foreground). Sprite/Disc/Tilemap `sorting_layer` name + `z` as order-in-layer. `render_world` sorts (layer, then z) across kinds; missing/unknown → Default. Inspector combo + Order in Layer; Project `game.toml` list (↑↓ – Add Rename). CLI `sorting-layer list|add|rename|move|remove` (`--json`); `entity set --sorting-layer --order-in-layer` (alias `--z`). Doctor warns unknown names. Host-first; WSCN0003 unchanged (C still sorts by raw z).

- **Prefab instance links + overrides** (2026-09-12) — scene `EntityData.prefab` (relative `*.prefab.json` / stem; missing = not an instance). `instantiate_prefab` records the link; `unpack_prefab_instance` clears it (values stay). Override detect vs loaded asset (`transform.position`, `Disc.radius`, …). Inspector dark Pro: orange-bold labels + **Apply** / **Revert** / **Unpack Completely**. Apply writes the `.prefab.json` asset; Revert resets the instance. CLI twins: `create-prefab` (also links source) · `instantiate-prefab` · `apply-prefab` · `revert-prefab` · `unpack-prefab` · `prefab-status` (`--json`). One-entity prefabs only — no nested children / variants / per-property Apply. Host-first; WSCN unchanged.

- **Play-in-editor runs `App`** (2026-09-13) — `wiimaker-play` shared host/editor tick (`step_app`, `apply_pad_keys`, 60 Hz). Games export `cdylib` via `export_play_app!` (template + hello-orb). Editor Play loads the plugin and calls `App::update` / live World blit; Stop drops the plugin and rehydrates the authored scene; Pause skips ticks. No plugin → previous WASD/`Player` + OrbShadow fallback. No new prefs. CLI: `editor play-status [--build] [--json]`; `run` stays the external host twin. Tests: in-process `App` tick + `play_probe` cdylib (Marker moves; fallback would not). Host-first; WSCN unchanged.

- **Text / HUD** (2026-09-14) — `DrawCmd::DrawText` + scene `Text` (string, size, color, align, sorting layer / order-in-layer). Host raster samples a built-in 8×8 ASCII atlas (`wiimaker-assets` bits + `fixtures/hud_font.png`); **not** cooked into `.wpack`. `render_world` emits DrawText (camera offset + layer sort like Sprite/Disc). Inspector Text foldout + Add Component. CLI `entity add-component … Text --text --size --color --align` · `entity set --text --size --color --align` (`--json`). Missing glyphs → `?`. Wii GX skip; WSCN unchanged (text-only → KIND_NONE).

- **Animated tiles / auto-tile** (2026-09-14) — palette `anim` / `anim_fps` (reuses `*.anim.json`) + `auto_tile` `id`|`solid` NESW bitmask (N=1 E=2 S=4 W=8). Variants: `auto_sprites[mask]` or catalog `{sprite}_{mask}`. Runtime `TileVisual` frames tick in `animate_world` / `World::tick_tilemaps`; `render_world` picks auto-tile textures per cell. Inspector palette Anim + Auto Tile + painted NESW badge; Scene Edit ticks tile anims; Paint hover shows mask. CLI `tilemap set-palette` · `tilemap mask`; `tilemap get --json` includes `mask`. Mutate: `tilemap_set_palette` / `tilemap_autotile_mask`. Host-first; WSCN0003 still bakes static cell ids (C skips payload).

- **Tilemap CLI stamp from ASCII** (2026-09-15) — `tilemap from-ascii maze.txt --name Maze` reads UTF-8 (`#` wall, `.`/` `/`0` empty, `1`–`9` id; optional `--map '#=1,P=2:0'`). Default **resizes** the grid to the file (unlike inline `stamp --ascii`, which clips). Mutate: `parse_ascii_tilemap` / `tilemap_from_ascii` / `tilemap_from_ascii_path` in `wiimaker-scene`. Editor: Inspector Tilemap **Import ASCII**, Project `.txt` **Stamp into Tilemap**, drop TXT → `assets/`. `--json`. Host-first.

- **Component-level Sprite pivot override** (2026-09-16) — optional `SceneSprite.pivot: [f32; 2]` (omit = catalog cell). Hydrate / `animate_world` (preserves override on frame swap) / pick / Scene outline / `render_world` / WSCN0003 bake use the effective pivot. Inspector Sprite **Pivot** X/Y DragValues + catalog hint + **Reset**. CLI `entity set --pivot-x --pivot-y --clear-pivot`; `add-component Sprite --pivot-x --pivot-y`. Prefab `Sprite.pivot` in Apply/Revert/orange-bold. Host-first; WSCN0003 unchanged.

### CLI commands (exact names)

Global: `--json`

| Command | Notes |
|---|---|
| `new` | scaffold `games/<name>` from `templates/basic-game`, add workspace member |
| `run` | `cargo run -p <name>` |
| `edit` | `cargo run -p wiimaker-editor -- <name>` |
| `cook` | prepare `.wpack` (advanced) |
| `bake-wii` | bake `scene.wscn` |
| `build` (alias `build-wii`) | prepare + bake + Docker `.dol` |
| `dolphin` | launch existing `boot.dol` |
| `play-wii` | build then Dolphin |
| `doctor` | validate |
| `scene list` · `scene show` · `scene new --name` · `scene set-default --scene` · `scene set-clear --rgb` · `scene build-list` · `scene build-add --scene` · `scene build-remove --scene` · `scene set-game-view` | build-* mutate `game.toml` `scenes`; set-game-view writes `.wiimaker/prefs.toml` |
| `editor prefs` · `editor set-scene-view` · `editor play-status` | Scene zoom/pan/grid/gizmos/snap in prefs; play-status reports App plugin vs WASD fallback (no new prefs) |
| `entity list` · `entity add` · `entity set` · `entity remove` · `entity despawn` | `--name --sprite --x --y --sx --sy --rotation-deg --tag --follow --lerp --cell --speed --queued-dir --audio-clip --volume --play-on-awake --text --size --color --align --sorting-layer --order-in-layer` (`--z` alias) `--pivot-x --pivot-y --clear-pivot` |
| `entity add-component` · `entity remove-component` · `entity set-component-enabled` | kinds: `Sprite` \| `Disc` \| `Tilemap` (`--cols --rows --cell`) \| `Collider` (`--w --h` / `--shape Circle --radius`, `--solid` `--trigger` `--filter`) \| `Trigger` (collider with trigger=true) \| `Animation` (`--clip` `--fps` `--loop`) \| `Camera` \| `Follow` (`--target` `--lerp`) \| `GridMover` (`--cell` `--speed` `--queued-dir`) \| `AudioSource` (`--clip` `--volume` `--play-on-awake`) \| `Text` (`--text` `--size` `--color` `--align`) · Sprite `--pivot-x --pivot-y` |
| `entity set-anim` | `--name --clip [--fps] [--loop]` |
| `entity overlaps` · `entity triggers` | `--name` [ `--other` ] · pairwise/list overlaps; `triggers <name>` lists entered triggers |
| `entity duplicate` · `entity rename` · `entity set-parent` | |
| `entity create-prefab` · `entity instantiate-prefab` · `entity apply-prefab` · `entity revert-prefab` · `entity unpack-prefab` · `entity prefab-status` | apply writes asset; revert restores; status lists overrides |
| `asset list` · `asset import` · `asset slice --cols --rows` · `asset set-pivot --x --y` · `asset list-sprites` · `asset anim` · `asset list-anims` · `asset list-wavs` · `asset play --name` | `asset import` copies `.png` or `.wav`; `asset play` host oneshot (`--json` `skipped` if no device) |
| `tilemap set` · `tilemap fill` · `tilemap stamp` · `tilemap from-ascii` · `tilemap get` · `tilemap set-palette` · `tilemap mask` | `--name --x --y --id` · `--ascii` / `--cells --width` · `from-ascii FILE` (`--map` `--resize`) · palette `--anim --fps --auto-tile id|solid|off --auto-sprites` · mask NESW · `--json` |
| `sorting-layer list` · `sorting-layer add --name [--index]` · `sorting-layer rename --from --to` · `sorting-layer move --name --index` · `sorting-layer remove --name` | project Tags & Layers analogue on `game.toml`; rename/remove remap scenes + prefabs |

### Editor chrome (exact control names)

File: Save scene · Doctor · Build Settings… · Play · Stop Play · Run external… · Build · Play in Dolphin · Build & Run · Instantiate <prefab>
Edit: Undo · Redo · Duplicate · Copy · Paste
Window: Hierarchy · Inspector · Project · Console
Toolbar (left): Save · Build · Play in Dolphin · Build & Run · ⋯ (Cook assets… · Doctor · Refresh assets)
Toolbar (center): Play / Pause / Stop
Center tabs: Scene · Game
Scene view: Move · Scale · Rotate · Hand · Paint · Erase · Pick · 2D · Grid · Gizmos · Snap · grid size · zoom %
Game view: aspect preset (Free / 640×480 / 16:9 / 4:3 / custom W×H) · Scale
Bottom tabs: Project · Console
Inspector: component foldout + enable + gear/Remove · Add Component · Edit Sprites… · Save as Prefab… · Prefab instance **Apply** / **Revert** / **Unpack Completely** + orange-bold override labels · Tilemap grid/palette/Brush · palette **Anim** clip + Override FPS · **Auto Tile** Off/Same id/Solid + Variants · **Import ASCII** (project `.txt`) · Collider kind/w/h/radius/solid/Is Trigger/Filter Tag/offset · Animation clip combo + Override FPS + Loop · Camera Follow target combo + Lerp · GridMover cell/speed/queued dir · AudioSource clip combo + Volume + Play On Awake + Play · Text string + Size + Color + Align + Sorting Layer/Order in Layer · Sprite/Disc/Tilemap/Text **Sorting Layer** combo + **Order in Layer** · Sprite **Pivot** X/Y + catalog hint + **Reset** · `game.toml` Sorting Layers list (↑↓ – + Add / Rename) · Project `.txt` **Stamp into Tilemap**
Shortcuts: Cmd/Ctrl+S, Z/Y, D, C, V, I (instantiate)

---

## Recommended next morning

**Ship Clear-color editor UI with undo (Later).** Editor follow-up; CLI `scene set-clear` exists.

