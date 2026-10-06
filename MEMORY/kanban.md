# Wiimaker feature board

Living engine board (not the local Pac-Man game). Ranked by what a maze-chomper actually needs next. Almost every item is **GUI + CLI** — files stay truth (`wiimaker-scene` mutate helpers, then twin the editor panel). Each Now/Next card is one morning.

Pac-Man probe (local, gitignored): `games/pac-man/` — maze is Disc entities + game-side grid. Use it as the acceptance test for Now items.

## Already have (do not rebuild)

Authoring loop is already Unity-shaped. Do not re-litigate these:

| Unity | Engine today |
|---|---|
| Project window | `game.toml` + `assets/` + `scenes/` + editor **Project** explorer |
| Hierarchy | editor Hierarchy (parent/unparent DnD, multi-select, duplicate) |
| Inspector | Transform + Sprite/Disc/Camera/Tilemap/Collider/Animation/Animator/PlayableDirector/GridMover/AudioSource/Text, enable checkbox, catalog combo, tile palette (anim + auto-tile), **Sorting Layer** + **Order in Layer**, Sprite **Pivot** X/Y override + Reset, scene **Environment** **Clear Color** (empty selection + open `.scene.json`), **Input** card (live GCN-layout stick / D-pad / face / IR / Accel + Keyboard · Wiimote · IR · motion · Classic · GCN · mouse aim · Shift+mouse tilt) |
| Scene view | 640×480 viewport, pick/drag, **Move / Scale / Rotate / Hand / Paint / Erase / Pick**, 2D (always-on), zoom % + scroll, grid overlay, gizmos, **Move axis handles** (red X / green Y), Snap + nudge |
| Game view | aspect dropdown (Free / 640×480 / 16:9 / 4:3 / custom) + Scale + letterbox; prefs in `.wiimaker/prefs.toml` |
| Play | toolbar Play/Pause/Stop ticks the open game `App` (`wiimaker-play` cdylib) · WASD/`Player` fallback if no plugin · File → Run external → `cargo run -p <game>` |
| Console | editor Console: Search (session-only, Hierarchy chrome) + Info/Warn/Error toggles · Clear · Doctor |
| Prefab | `.prefab.json` · root + optional `children[]` (nested) · optional `base` + `overrides[]` (**variants**) · instance `prefab` link on **root only** · Save as Prefab / Create Prefab Variant / Instantiate / Apply / Revert / Unpack Completely · orange-bold overrides (`Child/field`) |
| Sprite Editor | `assets/<stem>.sprites.json` · Grid By Cell Count + pivot |
| Undo | `UndoStack` in `wiimaker-scene` (depth 50) · Cmd/Ctrl+Z/Y |

Runtime already: `World` (named entities, Transform, Sprite, Disc, Camera + optional Follow, Tilemap, Collider, Animation, Animator, PlayableDirector, GridMover, AudioSource, Text, `tag: u32`), `DrawList` IR, GCN-layout `Input` (WASD/arrows → stick + D-pad; Wii GCN + Wiimote 1/2/Minus + Classic + Nunchuk stick, D-pad synthesizes `main` when analog idle; **IR aim** `ir_x`/`ir_y`/`ir_valid` in 640×480; **motion** `accel_*`/`motion_valid` + shake/swing edges), 60 Hz `Clock`, `render_world` (in `wiimaker-core`, re-exported from `wiimaker-scene`) sorts by Sorting Layer then order-in-layer `z` (tile cells as sprites/colored quads; HUD `DrawText` as bitmap glyphs), parented local TRS compose (scale component-wise × parent×local rotation × scaled-then-rotated offset), sprite UV/pivot, `.wpack` cook (PNG + PCM16 audio TOC), WSCN0003 bake (UV + pivot + length-prefixed Tilemap palette + `KIND_TEXT` + `KIND_AUDIO` / audio table), GX C stub_game **or** Rust `wiimaker-wii` staticlib (WSCN→World → `render_world` → DrawList → GX + ASND oneshots), `wiimaker build` / `dolphin` / `play-wii`. Queries: `tile_solid` / `world_to_cell` / `tile_solid_world` · `overlaps` / `move_and_collide` · `triggers_entered` · `animate_world` + `Animation` / `*.anim.json` · `Animator` / `*.controller.json` (Bool/Float/Trigger SM → clip cells) · palette `anim` tiles + 4-neighbor `auto_tile` (NESW bitmask) · active Camera offsets dests (centered 640×480) + `World::follow_cameras` · `GridMover` + `cardinal` / `World::step_grid_movers` (horizontal wins on diagonals; reverse immediate; snap to cell centers) · `World::play_oneshot` / play-on-awake (host + Wii ASND). Project Sorting Layers in `game.toml` (`Background` / `Default` / `Foreground` when omitted).

**Not present:** blend trees / layers / Mecanim curves; timeline curve editor; WSCN bake of Animator or PlayableDirector. Signal / Control tracks (nested director via Control) shipped 2026-10-06.

---

## Now

**Recommended next morning (2026-10-07):** Timeline curves — edit Transform (and float) curves on timeline clips. Signal / Control shipped 2026-10-06. Apply to Base / Open Base shipped 2026-10-05.

---

## Later

- Timeline curves (Transform / float) on `*.timeline.json` clips. Signal / Control and nested directors via Control shipped 2026-10-06.
- Blend trees / Animator layers. WSCN bake of Animator or PlayableDirector (host-first until then).

---

## Done

Shipped. Keep here so we do not rebuild them.

- **Timeline Signal / Control tracks** (2026-10-06) — `assets/<name>.timeline.json` still loads older Activation / Animation / Audio / Transform files. New kinds: **Signal** (marker at clip `start`, `signal` name, optional `payload` string; `end` usually equals `start`) and **Control** (clip bound to a target entity). Signal fires once per playthrough when the playhead crosses the marker (same edge as Audio). Forward scrub past an already-fired marker does not fire again. Scrub back before the marker disarms without firing. A backward jump does not fire. Loop wrap and stop/restart clear the fired set; `play_timeline` seeks without firing so the next tick crosses markers (including t = 0). `World::timeline_signals` peeks; `World::take_timeline_signals` drains. `tick_timelines` clears the queue at the start of the tick. Control uses Activation-appear on the target (on inside, off outside / on Stop). If the target has a PlayableDirector, clip-local time drives it (0 at clip start) and stop/leave snaps it off (time 0). Self-control and cycles are dropped (lower entity id stays the root; the back-edge does not re-enter). Editor: Timeline dock diamond markers + Control clip rects labeled with the target; Project `.timeline.json` Inspector lists **Signal** / **Control**; **Add Signal Marker** and **Add Control Clip** (dock + Inspector) write the asset (not scene undo). Console Info during editor Play and Timeline preview: `signal <name> @ t (binding)`. CLI: `asset timeline --track Name:Signal:Binding:t-t:SignalName|payload` and `Name:Control:Target:start-end:` (or `--stdin` JSON) · `entity timeline-signals --name --dt --steps [--play]` · `entity timeline-status --json` includes `signals`. Host + editor Play. WSCN bake unchanged.

- **Apply to Base / Open Base** (2026-10-05) — Variant instance Inspector shows **Base: <name>** with **Open Base** / **Select Base** (both focus the base `.prefab.json` in Project + Inspector; no prefab stage). **Overrides** menu: per path **Apply to Prefab Variant '<v>'**, **Apply to Base '<b>'**, **Apply to Root '<r>'** (chain longer than one base), **Revert**; plus **Apply All to Prefab Variant '<v>'**, **Apply All to Base**, **Apply All to Root '<r>'**, **Revert All**. Variant asset Inspector: same Base row, per-override **Apply to Base** / **Revert to Base** (right-click the path too) and **Apply All to Base**. Mutate: `apply_instance_to_base`, `apply_variant_override_to_base`, `apply_instance_fields_to_prefab`, `revert_variant_to_base`, `revert_prefab_instance_fields` (scene undo). CLI: `entity apply-prefab --name <entity> --to-base [--field <path>]... [--to-root]` · `entity apply-prefab --prefab <variant> --to-base [--field <path>]... [--to-root]` · `entity apply-prefab --name <entity> --field <path>` (variant asset, not base) · `entity open-base (--name <entity> | --prefab <asset>) [--root]` · `entity prefab-status --json` adds `chain`, `root`, `variant_overrides`, `base_overrides` (instance `overrides` unchanged). Empty `overrides` on a sibling is still a full snapshot diff, so Apply to Base copies the new value onto other variants that do not list the path. `--to-root` keeps an intermediate variant's own different value. After Apply, open-scene instances inherit un-overridden paths (`scene_synced`; CLI saves the scene; editor dirty + one undo). Host-first. Deferred: per-field right-click on every orange DragValue outside the Overrides menu; asset writes are not on the scene UndoStack (same as Apply).

- **Timeline / cutscene authoring** (2026-10-04) — `assets/<name>.timeline.json` (`TimelineMeta` / catalog). Scene/runtime `PlayableDirector` (`timeline`, `play_on_awake`, `loop`, live `time` / `playing` / `finished`). Tracks: **Activation** (`active: true` appear = on inside clip, off outside / on Stop; `active: false` hide = off inside, restore capture flag outside), **Animation** (drive sibling `Animation` from `*.anim.json` for the clip; restore authored outside), **Audio** (oneshot once per playthrough when the playhead crosses `start`), **Transform** (lerp local XY; hold `to` after the clip; stop/scrub re-evaluates). `World::play_timeline` / `stop_timeline` / `set_timeline_time` / `tick_timelines` (from `animate_world`, host + editor Play). Inspector **Playable Director** foldout + bottom **Timeline** dock (dark Pro). CLI `asset timeline` / `list-timelines`; `entity add-component PlayableDirector --timeline`; `entity set --timeline --play-on-awake --loop`; `timeline-play` / `timeline-stop` / `timeline-status` (`--json`). Host-first (WSCN unchanged). Out of scope: signals, curves, nested timelines, WSCN bake.

- **Animator / animation state machine** (2026-10-03) — `assets/<name>.controller.json` (`AnimatorControllerMeta` / catalog). Distinct scene/runtime `Animator` feeds sibling `Animation` cells. Bool/Float/Trigger params, `Any` from-state, optional `has_exit_time`. `World::set_animator_bool/float/trigger` + `tick_animators` inside `animate_world` (host + editor Play). Inspector Animator foldout (controller combo, live state, params). CLI `asset controller` / `list-controllers`; `entity add-component Animator --controller`; `entity set --controller`; `animator-set` / `animator-status` (`--json`). Single-clip `Animation` still valid. Host-first (WSCN unchanged). Out of scope: blend trees, layers, WSCN bake.

- **Prefab variants** (2026-10-02) — `Prefab.base` + `overrides: Vec<String>` (old assets without fields still load). `create_prefab_variant` / `variant_from_instance`; `resolve_prefab` applies base + overrides (live inherit for non-overridden fields). Instantiate resolves first; Apply/Revert write the **variant** asset (not the base) and refresh overrides. Inspector **Prefab Variant** chrome (`of <base>`) + **Create Prefab Variant…**; orange-bold instance overrides. CLI `entity create-variant --from|--name --as-name` (`--json` base/overrides/variant); `instantiate-prefab` / `apply-prefab` / `prefab-status` twin. Host-first. Deferred: Apply to Base / Open Base.

- **Nested prefabs** (2026-10-01) — `Prefab` root + `children: Vec<EntityData>` (old one-entity JSON still loads). Save as Prefab / `entity create-prefab` captures Hierarchy subtree. Instantiate expands tree (unique names; only root gets `prefab` link). Apply/Revert/Unpack operate on whole tree; override paths `ChildName/field`. Inspector Prefab chrome on root; orange-bold for nested. CLI twins `--json` include `children` count. Host-first (WSCN hydrate still flattens). Variants shipped 2026-10-02.

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
- Parent local transforms (translate×scale; **full rotation compose 2026-09-26**) — `compose_child` / `to_local`: `world.scale = parent.scale * local.scale` (axis-aligned), `world.rotation = parent.rotation * local.rotation`, `world.t = parent.t + rotate(parent.r, local.t * parent.scale)`. Hydrate / pick / gizmos / WSCN bake / `set_entity_parent` / `set_entity_world_xy` all use `world_transform`. Non-uniform scale + rotation does not shear (Unity 2D-ish). Host-first.
- Tilemap + solid cells (scene `Tilemap`, viewport Paint/Erase/Pick, Inspector grid+palette + **Import ASCII**, CLI `tilemap set|fill|stamp|from-ascii|get`, `tile_solid` / `world_to_cell`, WSCN0003 bake)
- AABB/Circle collider + overlap (scene `Collider`, Inspector kind/size/solid, viewport seafoam outline gizmo, CLI `entity add-component … Collider --w --h`, `entity overlaps`, `overlaps` / `move_and_collide`; host-first, WSCN0003 unchanged)
- Unity 6 editor chrome (dark Pro docks: Hierarchy left, Scene/Game center, Inspector right, Project/Console bottom; Play/Pause/Stop centered; component foldout cards). CLI `scene new` · `scene set-default`
- Trigger / collectible (GUI Is Trigger + Filter Tag; CLI Trigger/--trigger/--filter, entity triggers, entity despawn; `triggers_entered`; triggers skip `move_and_collide`)
- **Runtime scene load / switch** (2026-09-04) — `load_scene_into_world` (scene + host atlas helper) replaces World, keeps texture map; `game.toml` `scenes = [...]` Build Settings; doctor warns default missing from list / missing files; CLI `scene build-list` · `build-add` · `build-remove` (`--json`); editor File → Build Settings… + Inspector on `game.toml`. Pac-Man stays local (do not commit `games/`).

- **Camera follow** (2026-09-06) — active `Camera` offsets `render_world` dests (viewport center = camera translation; `(320,240)` = identity). Optional Follow on `SceneCamera` (`follow` name + `lerp`, default 0.15). `World::follow_cameras` each play/host tick. Inspector Camera foldout (target combo + lerp); Scene view 640×480 cyan rect gizmo; Game view applies offset. CLI `entity add-component … Camera` · `Follow --target --lerp` · `entity set --follow --lerp`. Host-first (WSCN unchanged). No Camera → identical dests.

- **4-way grid-snap mover** (2026-09-07) — optional `GridMover { cell, speed, queued_dir }` + `cardinal(input)` / `try_step` / `World::step_grid_movers`. Diagonals: **horizontal wins**. Reverse of current heading is immediate; 90° turns wait for cell center. Stick +Y = Up = −Y world. Inspector foldout (cell, speed, queued dir). CLI `entity add-component … GridMover --cell --speed --queued-dir` · `entity set --cell --speed --queued-dir` (empty `--queued-dir ""` clears). Host play + editor Play tick. Host-first (WSCN unchanged).

- **Scene viewer + Game view aspect** (2026-09-08) — Scene: geometric Move/Scale/Rotate/**Hand**/Paint/Erase/Pick, always-on **2D**, scroll + `%` zoom, pan (Hand or middle-drag), **Grid** overlay, **Gizmos** toggle, Snap. Game: Free Aspect / 640×480 / 16:9 / 4:3 / custom W×H, letterbox/pillarbox, Scale slider. Store: `<game>/.wiimaker/prefs.toml` (`EditorPrefs`; not `game.toml`). CLI `scene set-game-view` · `editor prefs` · `editor set-scene-view` (`--json`). Host-first.

- **Gizmo handles** (2026-09-09) — Scene Move tool: Unity-like 2D handles at the selected origin (Inspector red X / green Y, XY free square). Drag X locks Y; drag Y locks X; entity-body drag stays free. **Gizmos** on: collider AABB/circle fill + ticks (seafoam / amber triggers); tilemap bounds + cell grid. Hit-test in `wiimaker-scene` `gizmo.rs`. CLI n/a (no new prefs flag). Host-first.

- **Audio oneshots** (2026-09-10) — `AudioSource` (`clip`, `volume`, `play_on_awake`) on scene JSON; `World::play_oneshot` queue; host plays `assets/*.wav` (PCM16 mono/stereo) via `aplay`/`paplay` when present. Missing clip errors; no player / `WIIMAKER_AUDIO=0` skips. Inspector foldout + Project play / double-click; CLI `asset import` `.wav`, `asset play --name`, `asset list-wavs`, `entity add-component … AudioSource`, `entity set --audio-clip --volume --play-on-awake`. Fixture: `crates/wiimaker-assets/fixtures/beep.wav`.

- **Wii ASND oneshots** (2026-09-22) — `WPACK001` audio TOC after meshes (`u32` count; old packs omit → 0): stem, rate, channels, LE PCM16. WSCN0003 `KIND_AUDIO=5` for audio-only entities; additive trailing table (entity index + clip + volume + play_on_awake) for AudioSource on Sprite/Disc/Tilemap/Text. C: `ASND_Init`, load TOC, 32-byte-aligned BE buffers, `ASND_SetVoice` at clip rate, volume 0..1, play-on-awake after load. Missing clip / empty TOC must not crash. Inspector subtitle no longer says ASND is unwired.

- **Wiimote / Classic / Nunchuk input** (2026-09-23) — GCN-layout `Input` is still the lingua franca. Wii `fill_input`: GCN stick+C-stick+buttons, core Wiimote A/B/Plus/Home/D-pad + **1→X 2→Y Minus→Z**, Classic digital **only when `WPAD_EXP_CLASSIC`** (Nunchuk Z/C share Classic UP/LEFT bits), Nunchuk Z→GCN Z (C unmapped), Classic left/right sticks + Nunchuk stick when GCN idle, D-pad synthesizes `main` when analog still idle. `WPAD_SetDataFormat` + `WPAD_Probe` so `EXP_NONE` is safe. Rust `wiimaker-core::wiimote_map` (unit tests) documents the same bits as `wiimaker_abi.h` / libogc `wpad.h`. CLI `input map` / `--json`. Editor: Inspector **Input** card (empty selection + open scene) + Game-view overlay while Playing/Paused. Host WASD unchanged. No IR / motion. WSCN unchanged.




- **Motion gestures** (2026-09-30) — Additive `Input::{accel_x, accel_y, accel_z, motion_valid}` in **g** (rest ≈ `(0,0,1)` face-up, libogc `gforce`) + `Gesture` edges (`Shake`, `SwingX±`, `SwingY±`). Pads/IR stay lingua franca; motion never replaces them. Wii: `wd->gforce` → ABI when Wiimote live. Host/editor: **Shift + mouse** offset from center → tilt; valid only while Shift. `WiimakerInput` + `PlayInputC` (ABI **3**). Editor: Inspector Accel + Motion Valid + Gesture badges; Game overlay XYZ bars + status. CLI `input map` rows for Wiimote accel + Host Shift+mouse. Helpers: `scale_wpad_accel_raw_to_g` / `host_mouse_tilt_to_accel` / `apply_accel` / `Input::detect_gestures`.

- **IR pointer / sensor-bar aiming** (2026-09-29) — Additive `Input::{ir_x, ir_y, ir_valid}` in 640×480 (+X right, +Y down). HorrorDash lesson: pads stay lingua franca; IR never replaces stick/D-pad. Wii: `WPAD_SetVRes(…, 640, 480)` + `wd->ir.valid` → ABI. Host mouse over 640×480 window / editor Game view letterbox → same fields. `WiimakerInput` + `PlayInputC` (ABI **2**, now **3** with motion) carry IR. Editor: Inspector Input IR + Valid badge; Game overlay crosshair when valid. CLI `input map` rows for Wiimote IR + Host mouse. Helpers: `map_ir_raw_to_640` / `apply_ir_aim`.

- **Wii `World` + `render_world`** (2026-09-28) — `render_world` / `render_world_ex` moved to `wiimaker-core` (`no_std` + alloc); `wiimaker-scene` re-exports. `wiimaker-wii` parses WSCN0002/0003 into a core `World` (named entities, Transform, Sprite/Disc/Tilemap/Text/AudioSource; Default sorting layer + baked `z`). Frame: `render_world` → `DrawList` → GX. Player/OrbShadow via `find_by_name`. Play-on-awake still ASND. C `stub_game` fallback unchanged. Do not bump WSCN magic.

- **Green PowerPC CI** (2026-09-27) — `.github/workflows/ci.yml`: host `cargo test --workspace` (scaffolds `games/hello-orb` via `tools/ci-scaffold-hello-orb.sh`) + strict PowerPC rustlib (`WIIMAKER_RUSTLIB_STRICT=1` / `./tools/wii-rustlib.sh --strict`, asserts `libwiimaker_wii.a`). Local rustlib stays best-effort for stub_game fallback. Unblocked Wii World + render_world.

- **Rust staticlib (WSCN player)** (2026-09-24) — `crates/wiimaker-wii` (`rlib` + `staticlib`): `wiimaker_game_init/frame/shutdown`, WSCN0002/0003 parse, GX/ASND FFI, `WiimakerInput` → core `Input`, hello-orb Player/OrbShadow tick. Superseded for draw by **Wii World + render_world** (2026-09-28). Makefile always links `EMBED_OBJ`; `USE_STUB=0` when `libwiimaker_wii.a` present. Stub_game remains fallback.

- **Sorting layers** (2026-09-11) — Unity Sorting Layer + Order in Layer. `game.toml` `sorting_layers` (default Background / Default / Foreground). Sprite/Disc/Tilemap `sorting_layer` name + `z` as order-in-layer. `render_world` sorts (layer, then z) across kinds; missing/unknown → Default. Inspector combo + Order in Layer; Project `game.toml` list (↑↓ – Add Rename). CLI `sorting-layer list|add|rename|move|remove` (`--json`); `entity set --sorting-layer --order-in-layer` (alias `--z`). Doctor warns unknown names. Host-first; WSCN0003 unchanged (C still sorts by raw z).

- **Prefab instance links + overrides** (2026-09-12) — scene `EntityData.prefab` (relative `*.prefab.json` / stem; missing = not an instance). `instantiate_prefab` records the link; `unpack_prefab_instance` clears it (values stay). Override detect vs loaded asset (`transform.position`, `Disc.radius`, …). Inspector dark Pro: orange-bold labels + **Apply** / **Revert** / **Unpack Completely**. Apply writes the `.prefab.json` asset; Revert resets the instance. CLI twins: `create-prefab` (also links source) · `instantiate-prefab` · `apply-prefab` · `revert-prefab` · `unpack-prefab` · `prefab-status` (`--json`). One-entity prefabs only — no nested children / variants / per-property Apply. Host-first; WSCN unchanged.

- **Play-in-editor runs `App`** (2026-09-13) — `wiimaker-play` shared host/editor tick (`step_app`, `apply_pad_keys`, 60 Hz). Games export `cdylib` via `export_play_app!` (template + hello-orb). Editor Play loads the plugin and calls `App::update` / live World blit; Stop drops the plugin and rehydrates the authored scene; Pause skips ticks. No plugin → previous WASD/`Player` + OrbShadow fallback. No new prefs. CLI: `editor play-status [--build] [--json]`; `run` stays the external host twin. Tests: in-process `App` tick + `play_probe` cdylib (Marker moves; fallback would not). Host-first; WSCN unchanged.

- **Text / HUD** (2026-09-14) — `DrawCmd::DrawText` + scene `Text` (string, size, color, align, sorting layer / order-in-layer). Host raster samples a built-in 8×8 ASCII atlas (`wiimaker-assets` bits + `fixtures/hud_font.png`); **not** cooked into `.wpack`. `render_world` emits DrawText (camera offset + layer sort like Sprite/Disc). Inspector Text foldout + Add Component. CLI `entity add-component … Text --text --size --color --align` · `entity set --text --size --color --align` (`--json`). Missing glyphs → `?`.

- **Wii GX text** (2026-09-20) — WSCN0003 `KIND_TEXT=4` (do not bump magic). Payload: `u16` UTF-8 len + bytes, `f32` size, `u8` align (0 Left / 1 Center / 2 Right), RGBA, `f32` z. Sprite/Disc/Tilemap still win bake priority. C player loads onto `Entity` and draws via `wiimaker_gx_draw_text` (untextured ink-pixel quads; bits from `wiimaker-assets` `font.rs` → `runtime/wii/include/font8x8.h`).

- **Wii GX tilemaps** (2026-09-21) — WSCN0003 `KIND_TILEMAP=3` stays length-prefixed (do not bump magic). Bake appends a resolved palette after cells/solid: id, RGBA, wpack tex+UV or `WSCN_TILE_NO_TEX` (untextured tint), `auto_tile` + NESW variants, optional anim frames (cap 16). C player draws occupied cells via `wiimaker_gx_draw_sprite` / `wiimaker_gx_draw_quad`; ticks palette clips. Host editor/CLI unchanged. Older bakes without a palette tail still draw the default wall tint.

- **Animated tiles / auto-tile** (2026-09-14) — palette `anim` / `anim_fps` (reuses `*.anim.json`) + `auto_tile` `id`|`solid` NESW bitmask (N=1 E=2 S=4 W=8). Variants: `auto_sprites[mask]` or catalog `{sprite}_{mask}`. Runtime `TileVisual` frames tick in `animate_world` / `World::tick_tilemaps`; `render_world` picks auto-tile textures per cell. Inspector palette Anim + Auto Tile + painted NESW badge; Scene Edit ticks tile anims; Paint hover shows mask. CLI `tilemap set-palette` · `tilemap mask`; `tilemap get --json` includes `mask`. Mutate: `tilemap_set_palette` / `tilemap_autotile_mask`. WSCN bake packs the resolved palette for GX.

- **Tilemap CLI stamp from ASCII** (2026-09-15) — `tilemap from-ascii maze.txt --name Maze` reads UTF-8 (`#` wall, `.`/` `/`0` empty, `1`–`9` id; optional `--map '#=1,P=2:0'`). Default **resizes** the grid to the file (unlike inline `stamp --ascii`, which clips). Mutate: `parse_ascii_tilemap` / `tilemap_from_ascii` / `tilemap_from_ascii_path` in `wiimaker-scene`. Editor: Inspector Tilemap **Import ASCII**, Project `.txt` **Stamp into Tilemap**, drop TXT → `assets/`. `--json`. Host-first.

- **Component-level Sprite pivot override** (2026-09-16) — optional `SceneSprite.pivot: [f32; 2]` (omit = catalog cell). Hydrate / `animate_world` (preserves override on frame swap) / pick / Scene outline / `render_world` / WSCN0003 bake use the effective pivot. Inspector Sprite **Pivot** X/Y DragValues + catalog hint + **Reset**. CLI `entity set --pivot-x --pivot-y --clear-pivot`; `add-component Sprite --pivot-x --pivot-y`. Prefab `Sprite.pivot` in Apply/Revert/orange-bold. Host-first; WSCN0003 unchanged.

- **Clear-color editor UI with undo** (2026-09-17) — Inspector **Environment** card with **Clear Color** picker + **Reset** (empty selection and open `.scene.json` in Project). Drag uses `begin_inspector_gesture`; Reset is discrete `push_undo`. Mutate via `set_scene_clear` (RGB, A=255). Viewport already blits `scene.clear_rgba()`. CLI twin unchanged: `scene set-clear --rgb`. Tests: mutate + UndoStack + WSCN bake of clear bytes. Host-first; WSCN0003 already stored clear_color.

- **Project explorer collapsible folders / filter** (2026-09-18) — Project Search (Hierarchy chrome: search icon + hint "Search"; session-only `project_filter`). Folders use geometric `foldout_button` + `#`; triangle toggles collapse without selecting; row click selects for Inspector; double-click folder toggles. Collapsed relative paths in `.wiimaker/prefs.toml` `project_view.collapsed` (empty = all expanded). Filter auto-reveals ancestors of matches. CLI: `editor set-project-view --collapse/--expand/--clear-collapsed` · `editor prefs --json` includes `project_view`. Host-first.

- **Console Search / filter** (2026-09-19) — Unity Console chrome: search icon + hint "Search" (session-only `console_filter`, like Hierarchy/Project). Case-insensitive match on line text **or** level tag (`info`/`warn`/`error`). Session-only Info/Warn/Error toggles. Count is `N messages` or `k / N messages` while filtering. Clear + Doctor stay. No CLI / prefs (session-only). Tests: `console_line_matches` / count label in `ui_console.rs`. Host-first.

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
| `editor prefs` · `editor set-scene-view` · `editor set-project-view` · `editor play-status` | Scene zoom/pan/grid/gizmos/snap + Project collapsed folders in prefs; play-status reports App plugin vs WASD fallback (no new prefs). Filter text is session-only. |
| `input map` | GCN-layout table: Keyboard / Wiimote / Classic / Nunchuk → Button/stick (`--json`) |
| `entity list` · `entity add` · `entity set` · `entity remove` · `entity despawn` | `--name --sprite --x --y --sx --sy --rotation-deg --tag --follow --lerp --cell --speed --queued-dir --audio-clip --volume --play-on-awake --text --size --color --align --sorting-layer --order-in-layer` (`--z` alias) `--pivot-x --pivot-y --clear-pivot` `--controller` `--timeline` `--loop` |
| `entity add-component` · `entity remove-component` · `entity set-component-enabled` | kinds: `Sprite` \| `Disc` \| `Tilemap` (`--cols --rows --cell`) \| `Collider` (`--w --h` / `--shape Circle --radius`, `--solid` `--trigger` `--filter`) \| `Trigger` (collider with trigger=true) \| `Animation` (`--clip` `--fps` `--loop`) \| `Animator` (`--controller`) \| `PlayableDirector` (`--timeline` `--play-on-awake` `--loop`) \| `Camera` \| `Follow` (`--target --lerp`) \| `GridMover` (`--cell --speed --queued-dir`) \| `AudioSource` (`--clip --volume --play-on-awake`) \| `Text` (`--text --size --color --align`) · Sprite `--pivot-x --pivot-y` |
| `entity set-anim` | `--name --clip [--fps] [--loop]` |
| `entity animator-set` · `entity animator-status` | `--name --bool Moving=true --float Speed=1 --trigger Jump` · status `--json` controller/state/params |
| `entity timeline-play` · `entity timeline-stop` · `entity timeline-status` · `entity timeline-signals` | `--name` · status `--json` timeline/time/playing/finished/`signals` · `timeline-signals --dt --steps [--play]` accumulates fired signals (`signal`, `payload`, `binding`, `time`, `director`, `timeline`) |
| `entity overlaps` · `entity triggers` | `--name` [ `--other` ] · pairwise/list overlaps; `triggers <name>` lists entered triggers |
| `entity duplicate` · `entity rename` · `entity set-parent` | |
| `entity create-prefab` · `entity create-variant` · `entity instantiate-prefab` · `entity apply-prefab` · `entity revert-prefab` · `entity unpack-prefab` · `entity prefab-status` · `entity open-base` | create-variant `--from` base or `--name` instance + `--as-name`; `apply-prefab --to-base [--field <path>]... [--to-root]` writes the base and drops the path from the variant (`--prefab` for the asset, `--name` for an instance); `--field` without `--to-base` writes that path onto the variant; `open-base (--name \| --prefab) [--root]` prints the base to focus; `prefab-status --json` adds `chain`, `root`, `variant_overrides`, `base_overrides` |
| `asset list` · `asset import` · `asset slice --cols --rows` · `asset set-pivot --x --y` · `asset list-sprites` · `asset anim` · `asset list-anims` · `asset controller` · `asset list-controllers` · `asset timeline` · `asset list-timelines` · `asset list-wavs` · `asset play --name` | `asset import` copies `.png` or `.wav`; `asset play` host oneshot (`--json` `skipped` if no device); `asset controller` writes `*.controller.json`; `asset timeline` writes `*.timeline.json` (`--duration` `--track Name:Kind:Binding:start-end:payload` or `--stdin`). Kinds: Activation `true\|false`, Animation clip stem, Audio `stem` or `stem:volume`, Transform `x,y>x,y`, Signal `SignalName` or `SignalName\|text` (range `t-t`), Control empty payload (binding = target). Binding `-` unbound for Signal and Audio. |
| `tilemap set` · `tilemap fill` · `tilemap stamp` · `tilemap from-ascii` · `tilemap get` · `tilemap set-palette` · `tilemap mask` | `--name --x --y --id` · `--ascii` / `--cells --width` · `from-ascii FILE` (`--map` `--resize`) · palette `--anim --fps --auto-tile id|solid|off --auto-sprites` · mask NESW · `--json` |
| `sorting-layer list` · `sorting-layer add --name [--index]` · `sorting-layer rename --from --to` · `sorting-layer move --name --index` · `sorting-layer remove --name` | project Tags & Layers analogue on `game.toml`; rename/remove remap scenes + prefabs |

### Editor chrome (exact control names)

File: Save scene · Doctor · Build Settings… · Play · Stop Play · Run external… · Build · Play in Dolphin · Build & Run · Instantiate <prefab>
Edit: Undo · Redo · Duplicate · Copy · Paste
Window: Hierarchy · Inspector · Project · Console · Timeline
Toolbar (left): Save · Build · Play in Dolphin · Build & Run · ⋯ (Cook assets… · Doctor · Refresh assets)
Toolbar (center): Play / Pause / Stop
Center tabs: Scene · Game
Scene view: Move · Scale · Rotate · Hand · Paint · Erase · Pick · 2D · Grid · Gizmos · Snap · grid size · zoom %
Game view: aspect preset (Free / 640×480 / 16:9 / 4:3 / custom W×H) · Scale · Play overlay: live stick / D-pad / face / IR crosshair + mapping legend
Bottom tabs: Project · Console · Timeline
Project: Search (filename / relative path) · collapsible folders (foldout + `#`) · Refresh · New scene · Set default · Build Settings…
Console: Search (text + info/warn/error tags) · Info / Warn / Error toggles · Clear · Doctor · count `N messages` / `k / N messages`
Inspector: component foldout + enable + gear/Remove · Add Component · Edit Sprites… · Save as Prefab… · Prefab / **Prefab Variant** instance **Apply** / **Revert** / **Unpack Completely** / **Create Prefab Variant…** + orange-bold override labels · variant **Base: <name>** · **Open Base** · **Select Base** · **Overrides** (**Apply to Prefab Variant '<v>'** · **Apply to Base '<b>'** · **Apply to Root '<r>'** · **Revert** · **Apply All to Prefab Variant '<v>'** · **Apply All to Base** · **Apply All to Root '<r>'** · **Revert All**) · variant asset **Apply to Base** / **Revert to Base** · Tilemap grid/palette/Brush · palette **Anim** clip + Override FPS · **Auto Tile** Off/Same id/Solid + Variants · **Import ASCII** (project `.txt`) · Collider kind/w/h/radius/solid/Is Trigger/Filter Tag/offset · Animation clip combo + Override FPS + Loop · Animator controller combo + current state + parameter toggles · **Playable Director** timeline combo + Play On Awake + Loop + time/state + Play/Stop/scrub · Camera Follow target combo + Lerp · GridMover cell/speed/queued dir · AudioSource clip combo + Volume + Play On Awake + Play · Text string + Size + Color + Align + Sorting Layer/Order in Layer · Sprite/Disc/Tilemap/Text **Sorting Layer** combo + **Order in Layer** · Sprite **Pivot** X/Y + catalog hint + **Reset** · scene **Environment** **Clear Color** picker + **Reset** (empty Inspector + open `.scene.json`) · **Input** card (live stick / D-pad / A/B/Start / IR + Keyboard · Wiimote · IR · Classic · GCN · mouse aim; empty Inspector + open `.scene.json`) · `game.toml` Sorting Layers list (↑↓ – + Add / Rename) · Project `.txt` **Stamp into Tilemap** · Project `.timeline.json` duration + track list (**Signal** / **Control**) · **Add Signal Marker** · **Add Control Clip**
Timeline window: track rows + clip rects + ruler + playhead · Signal diamond markers · Control clip rects labeled with the target · **Add Signal Marker** · **Add Control Clip** · Play / Stop (dark Pro)
Shortcuts: Cmd/Ctrl+S, Z/Y, D, C, V, I (instantiate)

---

## Recommended next morning

**Timeline curves** (Transform / float clips). Signal / Control tracks shipped 2026-10-06. Apply to Base / Open Base shipped 2026-10-05. Timeline / PlayableDirector shipped 2026-10-04. Animator shipped 2026-10-03. Prefab variants shipped 2026-10-02. Nested prefabs shipped 2026-10-01. Motion gestures shipped 2026-09-30. IR pointer / sensor-bar aiming shipped 2026-09-29.

