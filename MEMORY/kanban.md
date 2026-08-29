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

Runtime already: `World` (named entities, Transform, Sprite, Disc, Camera marker, Tilemap, Collider, `tag: u32`), `DrawList` IR, GCN-layout `Input` (WASD/arrows → stick + D-pad), 60 Hz `Clock`, `render_world` sorts by component `z` (tile cells as sprites/colored quads), parented local transforms, sprite UV/pivot, `.wpack` cook, WSCN0003 bake (UV + pivot + length-prefixed Tilemap), `wiimaker build` / `dolphin` / `play-wii`. Queries: `tile_solid` / `world_to_cell` / `tile_solid_world` · `overlaps` / `move_and_collide`.

**Not present:** triggers, sprite clips, audio playback, camera used at render time, named sorting layers, prefab variants, runtime scene API, text/UI, play-mode running the game crate, Wii GX draw of tilemaps (payload skipped).

---

## Now

### 2. Trigger / collectible — **GUI + CLI**
Unity: `isTrigger` + OnTriggerEnter. Dots, power pellets, fruit, ghost house door.

- `Collider.trigger = true` or `Trigger { filter_tag }`.
- `world.triggers_entered(id) -> [EntityId]` each tick (or callback list the game polls).
- **GUI:** Inspector checkbox + tag filter.
- **CLI:** `entity add-component … Trigger --filter 2` · `entity despawn`.
- **Test:** walk over Dot_* → despawn + score; power pellet tag distinct from pellet.

### 3. Sprite animation clips — **GUI + CLI**
Unity: Animator / Animation window (2D). Chomp + ghost legs. Sheets + cells already exist.

- `Animation { clip, fps, loop }` + `assets/<name>.anim.json` listing cell names.
- Tick in `render_world` or a `animate_world(world, dt)` the game/editor both call.
- **GUI:** Sprite Editor “clips” row, or Inspector clip combo + preview.
- **CLI:** `asset anim <game> chomp --cells player_0,player_1 --fps 10` · `entity set-anim`.
- **Test:** slice a 2-frame sheet, play clip on Player; doctor warns missing cells.

---

## Next

### 5. Runtime scene load / switch — **GUI + CLI**
Unity: LoadScene. Menu → maze → win. Today games hydrate once; `scene list/show` is authoring-only. Editor can *preview* another scene but does not rewrite `game.toml`.

- `load_scene_into(world, path, catalog)` already almost exists (`hydrate_into`). Add `App` helper + keep atlas.
- **GUI:** Build Settings–style default + additive list on Project/Inspector for `game.toml`.
- **CLI:** `scene new <game> --name win` · `scene set-default` · (runtime is game code, not CLI).
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
| `scene list` · `scene show` · `scene set-clear --rgb` | |
| `entity list` · `entity add` · `entity set` · `entity remove` | `--name --sprite --x --y --sx --sy --rotation-deg` |
| `entity add-component` · `entity remove-component` · `entity set-component-enabled` | kinds: `Sprite` \| `Disc` \| `Tilemap` (`--cols --rows --cell`) \| `Collider` (`--w --h` / `--shape Circle --radius`, `--solid`) |
| `entity overlaps` | `--name` [ `--other` ] · pairwise or list hits |
| `entity duplicate` · `entity rename` · `entity set-parent` | |
| `entity create-prefab` · `entity instantiate-prefab` · `entity apply-prefab` · `entity unpack-prefab` | |
| `asset list` · `asset import` · `asset slice --cols --rows` · `asset set-pivot --x --y` · `asset list-sprites` | |
| `tilemap set` · `tilemap fill` · `tilemap stamp` · `tilemap get` | `--name --x --y --id` · `--ascii` / `--cells --width` · `--json` |

### Editor chrome (exact control names)

File: Save scene · Doctor · Play · Stop Play · Run external… · Build · Play in Dolphin · Build & Run · Instantiate \<prefab\>
Edit: Undo · Redo · Duplicate · Copy · Paste
Toolbar: Save · Play / Pause / Stop · Build · Play in Dolphin · Build & Run · Prefab / Instantiate · ⋯ (Cook assets… · Doctor · Refresh assets)
Viewport: Move · Scale · Rotate · Paint · Erase · Pick · Snap · grid size
Inspector: Edit Sprites… · Save as Prefab… · component enable checkbox · Remove · Tilemap grid/palette/Brush · Collider kind/w/h/radius/solid/offset
Shortcuts: Cmd/Ctrl+S, Z/Y, D, C, V, I (instantiate)

---

## Recommended first morning

**Ship Trigger / collectible (Now #2).** Colliders overlap; games still poll `overlaps` themselves. Triggers unlock dots / pellets / fruit via `isTrigger` + a per-tick entered list. Test: walk over Dot_* → despawn + score.

