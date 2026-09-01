# Wiimaker feature board

Living engine board (not the local Pac-Man game). Ranked by what a maze-chomper actually needs next. Almost every item is **GUI + CLI** — files stay truth (`wiimaker-scene` mutate helpers, then twin the editor panel). Each Now/Next card is one morning.

Pac-Man probe (local, gitignored): `games/pac-man/` — maze is Disc entities + game-side grid. Use it as the acceptance test for Now items.

## Already have (do not rebuild)

Authoring loop is already Unity-shaped. Do not re-litigate these:

| Unity | Engine today |
|---|---|
| Project window | `game.toml` + `assets/` + `scenes/` + editor **Project** explorer |
| Hierarchy | editor Hierarchy (parent/unparent DnD, multi-select, duplicate) |
| Inspector | Transform + Sprite/Disc/Camera/Tilemap/Collider, enable checkbox, catalog combo, tile palette |
| Scene view | 640×480 viewport, pick/drag, **Move / Scale / Rotate / Paint / Erase / Pick**, Snap + nudge |
| Play | toolbar Play/Pause/Stop (hardcoded WASD on entity named `Player`; does **not** run game `App::update`) · File → Run external → `cargo run -p <game>` |
| Prefab | `.prefab.json` · Save as Prefab / Instantiate / Apply / Unpack (unpack is a no-op) |
| Sprite Editor | `assets/<stem>.sprites.json` · Grid By Cell Count + pivot |
| Undo | `UndoStack` in `wiimaker-scene` (depth 50) · Cmd/Ctrl+Z/Y |

Runtime already: `World` (named entities, Transform, Sprite, Disc, Camera marker, Tilemap, Collider, `tag: u32`), `DrawList` IR, GCN-layout `Input` (WASD/arrows → stick + D-pad), 60 Hz `Clock`, `render_world` sorts by component `z` (tile cells as sprites/colored quads), parented local transforms, sprite UV/pivot, `.wpack` cook, WSCN0003 bake (UV + pivot + length-prefixed Tilemap), `wiimaker build` / `dolphin` / `play-wii`. Queries: `tile_solid` / `world_to_cell` / `tile_solid_world` · `overlaps` / `move_and_collide` · `triggers_entered` · `animate_world` + `Animation` / `*.anim.json`.

**Not present:** audio playback, camera used at render time, named sorting layers, prefab variants, runtime scene API, text/UI, play-mode running the game crate, Wii GX draw of tilemaps (payload skipped).

---

## Now

**Recommended next morning:** Runtime scene load / switch (menu → maze → win for Pac-Man).

### 5. Runtime scene load / switch — **GUI + CLI**
Unity: LoadScene. Menu → maze → win. Today games hydrate once; `scene list/show` is authoring-only. Editor can *preview* another scene but does not rewrite `game.toml`.

- `load_scene_into(world, path, catalog)` already almost exists (`hydrate_into`). Add `App` helper + keep atlas.
- **GUI:** Build Settings–style default + additive list on Project/Inspector for `game.toml`.
- **CLI:** `scene new` / `scene set-default` already ship (chrome morning). Runtime load is game code.
- **Test:** pac-man Start on `menu.scene.json`, Enter → `main`, all dots → `win`.

### 6. 4-way discrete / grid-snap mover — **GUI + CLI**
Unity: nothing built-in; everyone writes it. Input already has D-pad + stick. Pac-Man needs queued cardinals + snap to cell centers.

- Optional `GridMover { cell, speed, queued_dir }` component **or** a core helper `cardinal(input) -> Option<Dir>` + `try_step(grid)`.
- **GUI:** Inspector on Player (cell size, speed).
- **CLI:** `entity add-component … GridMover --cell 20 --speed 6`.
- **Test:** hold Up+Right → only one axis, no diagonals; reverse allowed immediately.

### 7. Audio oneshots — **GUI + CLI**
ARCHITECTURE M3. Cook WAV → `.wpack` is sketched; **no playback** on host or Wii (ASND mentioned, unused).

- `AudioClip` asset + `world.play_oneshot("chomp")`. Host: cpal/rodio. Wii: ASND later is fine.
- **GUI:** Project on `.wav` → Inspector play; entity `AudioSource`.
- **CLI:** `asset import <game> chomp.wav` · `asset play` (host).
- **Test:** eat a dot → hear a tick; doctor lists clips.

### 8. Camera follow (use the Camera you already hydrate) — **GUI + CLI**
`SceneCamera` hydrates onto `World` but `render_world` never emits `SetCamera`. Maze-that-fits-640×480 hides this; bigger mazes cannot.

- If an active Camera exists, offset sprite/disc dest by `cam.translation` (ortho, screen-space).
- Optional `Follow { target: "Player", lerp }`.
- **GUI:** Inspector Follow target combo; viewport shows camera rect.
- **CLI:** `entity add-component … Camera` (exists via scene JSON today — add CLI kind) · `entity set --follow Player`.
- **Test:** 2× maze; camera keeps Player centered; no Camera → today's 640×480 identity.

---

## Later

- **Sorting layers** (GUI+CLI) — named layers + order, not only raw `z`. Unity Sorting Layer.
- **Prefab variants / overrides** (GUI+CLI) — unpack is a no-op; no orange-bold overrides. Dot/Ghost instances need this.
- **Play-in-editor runs `App`** (GUI) — today's Play is hello-orb WASD, not the game crate. Load game as dylib or interpret a tiny script graph. CLI already has `run`.
- **Gizmo handles** (GUI) — Move is drag-on-entity; add axis handles + collider/tilemap gizmos. CLI n/a except maybe `entity set`.
- **Text / HUD** (GUI+CLI) — DrawList has no glyphs; score lives in stdout. Bitmap font in `.wpack` + `DrawText`.
- **Animated tiles / auto-tile** (GUI+CLI) — after tilemap.
- **Tilemap CLI stamp from ASCII** (CLI, tiny GUI import) — `tilemap from-ascii maze.txt`.
- **Component-level pivot override** — durable engine follow-up.
- **Clear-color editor UI with undo** — editor follow-up; CLI `scene set-clear` exists.
- **Wii audio / Wiimote** — after host oneshots.

---

## Done

Shipped. Keep here so we do not rebuild them.

- **Unity Hierarchy + Inspector chrome** (2026-09-01) — Hierarchy search, + Create Empty, right-click Duplicate/Delete/Unparent, no per-row D/x; Inspector GameObject name+Tag, Transform Position/Rotation/Scale as XYZ DragValues, ⋮ Remove Component, full-width Add Component. Dark Pro only.

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
| `scene list` · `scene show` · `scene new --name` · `scene set-default --scene` · `scene set-clear --rgb` | |
| `entity list` · `entity add` · `entity set` · `entity remove` · `entity despawn` | `--name --sprite --x --y --sx --sy --rotation-deg --tag` |
| `entity add-component` · `entity remove-component` · `entity set-component-enabled` | kinds: `Sprite` \| `Disc` \| `Tilemap` (`--cols --rows --cell`) \| `Collider` (`--w --h` / `--shape Circle --radius`, `--solid` `--trigger` `--filter`) \| `Trigger` (collider with trigger=true) \| `Animation` (`--clip` `--fps` `--loop`) |
| `entity set-anim` | `--name --clip [--fps] [--loop]` |
| `entity overlaps` · `entity triggers` | `--name` [ `--other` ] · pairwise/list overlaps; `triggers <name>` lists entered triggers |
| `entity duplicate` · `entity rename` · `entity set-parent` | |
| `entity create-prefab` · `entity instantiate-prefab` · `entity apply-prefab` · `entity unpack-prefab` | |
| `asset list` · `asset import` · `asset slice --cols --rows` · `asset set-pivot --x --y` · `asset list-sprites` · `asset anim` · `asset list-anims` | `asset anim --name --cells a,b --fps --loop` |
| `tilemap set` · `tilemap fill` · `tilemap stamp` · `tilemap get` | `--name --x --y --id` · `--ascii` / `--cells --width` · `--json` |

### Editor chrome (exact control names)

File: Save scene · Doctor · Play · Stop Play · Run external… · Build · Play in Dolphin · Build & Run · Instantiate <prefab>
Edit: Undo · Redo · Duplicate · Copy · Paste
Window: Hierarchy · Inspector · Project · Console
Toolbar (left): Save · Build · Play in Dolphin · Build & Run · ⋯ (Cook assets… · Doctor · Refresh assets)
Toolbar (center): Play / Pause / Stop
Center tabs: Scene · Game
Scene view: Move · Scale · Rotate · Paint · Erase · Pick · Snap · grid size
Bottom tabs: Project · Console
Inspector: component foldout + enable + gear/Remove · Add Component · Edit Sprites… · Save as Prefab… · Tilemap grid/palette/Brush · Collider kind/w/h/radius/solid/Is Trigger/Filter Tag/offset · Animation clip combo + Override FPS + Loop
Shortcuts: Cmd/Ctrl+S, Z/Y, D, C, V, I (instantiate)

---

## Recommended next morning

**Ship runtime scene load / switch (Now #5).** Games hydrate once today; Pac-Man fakes menu → maze → win in game code. Engine should expose `load_scene` + Build Settings scene list.

