# Wiimaker feature board

Living engine board (not the local Pac-Man game). Ranked by what a maze-chomper actually needs next. Almost every item is **GUI + CLI** — files stay truth (`wiimaker-scene` mutate helpers, then twin the editor panel). Each Now/Next card is one morning.

Pac-Man probe (local, gitignored): `games/pac-man/` — maze is Disc entities + game-side grid. Use it as the acceptance test for Now items.

## Already have (do not rebuild)

Authoring loop is already Unity-shaped. Do not re-litigate these:

| Unity | Engine today |
|---|---|
| Project window | `game.toml` + `assets/` + `scenes/` + editor **Project** explorer |
| Hierarchy | editor Hierarchy (parent/unparent DnD, multi-select, duplicate) |
| Inspector | Transform + Sprite/Disc/Camera/Tilemap/Collider/Animation/GridMover, enable checkbox, catalog combo, tile palette |
| Scene view | 640×480 viewport, pick/drag, **Move / Scale / Rotate / Hand / Paint / Erase / Pick**, 2D (always-on), zoom % + scroll, grid overlay, gizmos, Snap + nudge |
| Game view | aspect dropdown (Free / 640×480 / 16:9 / 4:3 / custom) + Scale + letterbox; prefs in `.wiimaker/prefs.toml` |
| Play | toolbar Play/Pause/Stop (hardcoded WASD on entity named `Player`; does **not** run game `App::update`) · File → Run external → `cargo run -p <game>` |
| Prefab | `.prefab.json` · Save as Prefab / Instantiate / Apply / Unpack (unpack is a no-op) |
| Sprite Editor | `assets/<stem>.sprites.json` · Grid By Cell Count + pivot |
| Undo | `UndoStack` in `wiimaker-scene` (depth 50) · Cmd/Ctrl+Z/Y |

Runtime already: `World` (named entities, Transform, Sprite, Disc, Camera + optional Follow, Tilemap, Collider, Animation, GridMover, `tag: u32`), `DrawList` IR, GCN-layout `Input` (WASD/arrows → stick + D-pad), 60 Hz `Clock`, `render_world` sorts by component `z` (tile cells as sprites/colored quads), parented local transforms, sprite UV/pivot, `.wpack` cook, WSCN0003 bake (UV + pivot + length-prefixed Tilemap), `wiimaker build` / `dolphin` / `play-wii`. Queries: `tile_solid` / `world_to_cell` / `tile_solid_world` · `overlaps` / `move_and_collide` · `triggers_entered` · `animate_world` + `Animation` / `*.anim.json` · active Camera offsets dests (centered 640×480) + `World::follow_cameras` · `GridMover` + `cardinal` / `World::step_grid_movers` (horizontal wins on diagonals; reverse immediate; snap to cell centers).

**Not present:** audio playback, named sorting layers, prefab variants, text/UI, play-mode running the game crate, Wii GX draw of tilemaps (payload skipped).

---

## Now

**Recommended next morning (2026-09-08):** Gizmo handles (Unity Scene chrome — axis handles + collider/tilemap gizmos). Audio oneshots stays in Later.

---

## Later

- **Audio oneshots** (GUI+CLI) — ARCHITECTURE M3. Cook WAV → `.wpack` sketched; no host/Wii playback yet. `world.play_oneshot`, cpal/rodio host, ASND later; Project `.wav` play + `AudioSource`; CLI `asset import` / `asset play`.
- **Sorting layers** (GUI+CLI) — named layers + order, not only raw `z`. Unity Sorting Layer.
- **Prefab variants / overrides** (GUI+CLI) — unpack is a no-op; no orange-bold overrides. Dot/Ghost instances need this.
- **Play-in-editor runs `App`** (GUI) — today's Play is hello-orb WASD, not the game crate. Load game as dylib or interpret a tiny script graph. CLI already has `run`.
- **Gizmo handles** (GUI) — Move is drag-on-entity; add axis handles + collider/tilemap gizmos. CLI n/a except maybe `entity set`. **Recommended next.**
- **Text / HUD** (GUI+CLI) — DrawList has no glyphs; score lives in stdout. Bitmap font in `.wpack` + `DrawText`.
- **Animated tiles / auto-tile** (GUI+CLI) — after tilemap.
- **Tilemap CLI stamp from ASCII** (CLI, tiny GUI import) — `tilemap from-ascii maze.txt`.
- **Component-level pivot override** — durable engine follow-up.
- **Clear-color editor UI with undo** — editor follow-up; CLI `scene set-clear` exists.
- **Wii audio / Wiimote** — after host oneshots.

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
- Editor Play/Pause/Stop (Player WASD only) · Build · Play in Dolphin · Build & Run · Cook under ⋯
- Undo/redo, duplicate/copy/paste, parent/unparent, multi-select, snap, Move/Scale/Rotate
- Prefab create / instantiate / apply / unpack(no-op)
- Agent CLI twin of mutations (`--json`): see command list below
- Doctor (project/scene/assets)
- Parent local transforms (translate×scale; full rotation compose still open)
- Tilemap + solid cells (scene `Tilemap`, viewport Paint/Erase/Pick, Inspector grid+palette, CLI `tilemap set|fill|stamp|get`, `tile_solid` / `world_to_cell`, WSCN0003 bake)
- AABB/Circle collider + overlap (scene `Collider`, Inspector kind/size/solid, viewport seafoam outline gizmo, CLI `entity add-component … Collider --w --h`, `entity overlaps`, `overlaps` / `move_and_collide`; host-first, WSCN0003 unchanged)
- Unity 6 editor chrome (dark Pro docks: Hierarchy left, Scene/Game center, Inspector right, Project/Console bottom; Play/Pause/Stop centered; component foldout cards). CLI `scene new` · `scene set-default`
- Trigger / collectible (GUI Is Trigger + Filter Tag; CLI Trigger/--trigger/--filter, entity triggers, entity despawn; `triggers_entered`; triggers skip `move_and_collide`)
- **Runtime scene load / switch** (2026-09-04) — `load_scene_into_world` (scene + host atlas helper) replaces World, keeps texture map; `game.toml` `scenes = [...]` Build Settings; doctor warns default missing from list / missing files; CLI `scene build-list` · `build-add` · `build-remove` (`--json`); editor File → Build Settings… + Inspector on `game.toml`. Pac-Man stays local (do not commit `games/`).

- **Camera follow** (2026-09-06) — active `Camera` offsets `render_world` dests (viewport center = camera translation; `(320,240)` = identity). Optional Follow on `SceneCamera` (`follow` name + `lerp`, default 0.15). `World::follow_cameras` each play/host tick. Inspector Camera foldout (target combo + lerp); Scene view 640×480 cyan rect gizmo; Game view applies offset. CLI `entity add-component … Camera` · `Follow --target --lerp` · `entity set --follow --lerp`. Host-first (WSCN unchanged). No Camera → identical dests.

- **4-way grid-snap mover** (2026-09-07) — optional `GridMover { cell, speed, queued_dir }` + `cardinal(input)` / `try_step` / `World::step_grid_movers`. Diagonals: **horizontal wins**. Reverse of current heading is immediate; 90° turns wait for cell center. Stick +Y = Up = −Y world. Inspector foldout (cell, speed, queued dir). CLI `entity add-component … GridMover --cell --speed --queued-dir` · `entity set --cell --speed --queued-dir` (empty `--queued-dir ""` clears). Host play + editor Play tick. Host-first (WSCN unchanged).

- **Scene viewer + Game view aspect** (2026-09-08) — Scene: geometric Move/Scale/Rotate/**Hand**/Paint/Erase/Pick, always-on **2D**, scroll + `%` zoom, pan (Hand or middle-drag), **Grid** overlay, **Gizmos** toggle, Snap. Game: Free Aspect / 640×480 / 16:9 / 4:3 / custom W×H, letterbox/pillarbox, Scale slider. Store: `<game>/.wiimaker/prefs.toml` (`EditorPrefs`; not `game.toml`). CLI `scene set-game-view` · `editor prefs` · `editor set-scene-view` (`--json`). Host-first.

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
| `editor prefs` · `editor set-scene-view` | Scene zoom/pan/grid/gizmos/snap in the same prefs file |
| `entity list` · `entity add` · `entity set` · `entity remove` · `entity despawn` | `--name --sprite --x --y --sx --sy --rotation-deg --tag --follow --lerp --cell --speed --queued-dir` |
| `entity add-component` · `entity remove-component` · `entity set-component-enabled` | kinds: `Sprite` \| `Disc` \| `Tilemap` (`--cols --rows --cell`) \| `Collider` (`--w --h` / `--shape Circle --radius`, `--solid` `--trigger` `--filter`) \| `Trigger` (collider with trigger=true) \| `Animation` (`--clip` `--fps` `--loop`) \| `Camera` \| `Follow` (`--target` `--lerp`) \| `GridMover` (`--cell` `--speed` `--queued-dir`) |
| `entity set-anim` | `--name --clip [--fps] [--loop]` |
| `entity overlaps` · `entity triggers` | `--name` [ `--other` ] · pairwise/list overlaps; `triggers <name>` lists entered triggers |
| `entity duplicate` · `entity rename` · `entity set-parent` | |
| `entity create-prefab` · `entity instantiate-prefab` · `entity apply-prefab` · `entity unpack-prefab` | |
| `asset list` · `asset import` · `asset slice --cols --rows` · `asset set-pivot --x --y` · `asset list-sprites` · `asset anim` · `asset list-anims` | `asset anim --name --cells a,b --fps --loop` |
| `tilemap set` · `tilemap fill` · `tilemap stamp` · `tilemap get` | `--name --x --y --id` · `--ascii` / `--cells --width` · `--json` |

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
Inspector: component foldout + enable + gear/Remove · Add Component · Edit Sprites… · Save as Prefab… · Tilemap grid/palette/Brush · Collider kind/w/h/radius/solid/Is Trigger/Filter Tag/offset · Animation clip combo + Override FPS + Loop · Camera Follow target combo + Lerp · GridMover cell/speed/queued dir
Shortcuts: Cmd/Ctrl+S, Z/Y, D, C, V, I (instantiate)

---

## Recommended next morning

**Ship Gizmo handles (Later — Unity Scene chrome).** Axis handles + richer collider/tilemap gizmos. Audio oneshots remains the next runtime gap after that.

