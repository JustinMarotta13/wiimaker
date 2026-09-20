# Engine memory

Operational notes for `wiimaker-core`, `wiimaker-host`, `wiimaker-assets`, `runtime/wii`, and `games/*`.

Canonical rules: `.cursor/rules/wiimaker-engine.mdc` · architecture: `ARCHITECTURE.md`.

## Gotchas

- Non-PoT PNGs are padded at cook time; doctor warns (see `cyber_rover` in hello-orb).
- `.wpack` RGB5A3 is **GX 4×4 tiled** (cook tiles; host `to_rgba8` untiles). Re-cook after cooker changes.
- Wii C path embeds `assets.wpack` + `scene.wscn` (from prepare/`cook` + `bake-wii`). `wii-build.sh` / `wiimaker build` run both before Docker make. No Rust staticlib yet — `stub_game.c` is the scene player.
- Objcopy embed: copy bins into `runtime/wii/build/` first so symbols are `_binary_assets_wpack_*` / `_binary_scene_wscn_*` (path-mangled names break the C externs).
- Sprite sheets: sidecar `assets/<stem>.sprites.json`; cook still packs the whole PNG once. Catalog resolves cell name → sheet texture + UV (in **packed** PoT space) + pivot.
- Core `Sprite` has `pivot` (default `0.5,0.5`) and `uv`; optional `lock_pivot` keeps a scene component override across animation frame swaps. Render/pick/outline must share the same pivot math.
- WSCN magic is **`WSCN0003`**: sprite payload includes `u0,v0,u1,v1` + `pivot_x,pivot_y` (effective: SceneSprite override if set, else catalog); `KIND_TILEMAP=3` is length-prefixed (C skips; host hydrates JSON); `KIND_TEXT=4` is `u16` UTF-8 len + bytes, `f32` size, `u8` align (0/1/2), RGBA, `f32` z. Old `WSCN0002` still loads in C.

## Decisions

- `load_scene_into_world` (`wiimaker-scene`, thin `wiimaker-host` wrapper taking `&TextureAtlas`) resolves stem/path, `hydrate_into_with_catalogs` (clears World), returns clear color. Keep the atlas; do not recook on switch.
- Active Camera: dests in `render_world` are world − (cam − 320,240). Camera at default spawn is identity. Camera offset is applied in `render_world` dests; `SetCamera` is reserved for a future backend that transforms at flush time (do not emit both).
- Sorting layers: `game.toml` `sorting_layers = ["Background", "Default", "Foreground"]` (omitted → those defaults). Sprite/Disc/Tilemap/Text keep `z` as Order in Layer and optional `sorting_layer` name (empty/`Default` omitted on save). Runtime stores `sorting_layer: u16` index; unknown names hydrate to Default. `render_world` stable-sorts all drawable kinds by (layer index, then z). `load_scene_into_world` copies the project list onto `World` before hydrate. WSCN0003 unchanged (C still sorts by raw `z`).
- HUD text: `DrawCmd::DrawText` + `World`/`Scene` `Text`. Host samples a **built-in** 8×8 ASCII atlas (`wiimaker-assets::atlas_rgba8`, fixture `fixtures/hud_font.png`) — not cooked into game `.wpack`. Missing glyphs → `?`. Wii GX draws the same bits as untextured quads (`KIND_TEXT` under WSCN0003; `runtime/wii/include/font8x8.h` generated from `font.rs`). Sprite/Disc/Tilemap still win bake priority over Text.
- In-editor Play and host `run` share `wiimaker-play`: `step_app` + `apply_pad_keys` + optional game `cdylib` ABI (`export_play_app!`, ABI 1). Template/hello-orb export a plugin; crates without `cdylib` keep WASD/`Player` fallback. `WIIMAKER_PLAY_FALLBACK=1` forces fallback. Plugin hydrates the current scene JSON (unsaved edits).
- `World::follow_cameras` after play movement / game `update`. `World::step_grid_movers(&input, dt)` in the same tick (before follow). Diagonals: horizontal wins; reverse is immediate; 90° waits for cell center. Stick +Y (host Up) = `Dir::Up` = −Y.
- GridMover is host-first (JSON hydrate); WSCN bake skips it like Animation / AudioSource.
- Host audio: `HostAudio` plays validated PCM16 `assets/*.wav` via `aplay`/`paplay`/`pw-play` when present. `WIIMAKER_AUDIO=0` skips. In-crate `rodio`/`cpal` was not locked on Cargo 1.83 (bindgen → hashbrown 0.17 / edition 2024). Wii ASND is comment-only.
- Until Rust `staticlib` lands, Dolphin play uses the C scene player + GX textured quads; host keeps `wiimaker-scene` JSON hydrate + `SpriteCatalog`. WSCN/stub_game still skip cameras.
- `games/` is gitignored (local projects only); workspace still lists `games/hello-orb` for local cook/run.
- Optional `SceneSprite.pivot: [f32; 2]` overrides the catalog cell pivot per entity (omit = catalog). Hydrate writes runtime `Sprite.pivot` + `lock_pivot`; `animate_world` keeps the override when locked.
- Tilemap cells: row-major `u16` ids + 0/1 `solid` in JSON; packed bits at runtime. Cell `(0,0)` is top-left of `transform + origin`. Occupied (`id != 0`) draw as palette sprite or colored quad (`TextureId(u32::MAX)` → white sample × tint). `tile_solid` treats OOB as solid when any tilemap exists. ASCII load: `parse_ascii_tilemap` (`#`=1 solid, `.`/` `/`0` empty, `1`–`9` that id solid; optional `AsciiCharMap`). `tilemap_from_ascii` resizes to fit then stamps; `tilemap_stamp_ascii` is the no-resize clip path.
- Animated tiles / auto-tile: palette `anim` (clip stem) + optional `anim_fps`; hydrate resolves clip cells into `TileVisual.frames`. `animate_world` / `World::tick_tilemaps` advance the shared palette clock (all cells of that id stay in sync). `auto_tile`: `id` (same palette id) or `solid` (neighbor solid; OOB = solid). NESW bitmask N=1 E=2 S=4 W=8. Variant lookup: `auto_sprites[mask]` else catalog `{sprite}_{mask}` else base sprite. Hydrate must leave `auto_frames[mask] = None` when the variant name does not resolve — filling with `vis.texture` (frame 0) made `cell_texture` ignore the ticking clip, so `--anim` + `--auto-tile` drew neither. Resolved named variants stay static. Render uses `cell_texture`. Host-first; WSCN still stores painted ids only (C skips tilemap payload).
- Collider: AABB/Circle + `solid` / `trigger` / `filter_tag`. Triggers never block (`overlap_solid` / `move_and_collide`); poll `triggers_entered(world, id)` each tick. Filter 0 = any; else other entity `World::tag` must match.
- Primary ship verbs are Build / Play in Dolphin / Build & Run; `cook` is advanced/agent-only.

## Open follow-ups

- Rust staticlib for Wii sharing host `App` / `World` (replace C scene player).
- Sprite sheet offset/padding, Grid By Cell Size.
- Wii GX tilemaps (host animated / auto-tile exists; C skips WSCN Tilemap payload).
