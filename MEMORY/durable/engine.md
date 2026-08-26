# Engine memory

Operational notes for `wiimaker-core`, `wiimaker-host`, `wiimaker-assets`, `runtime/wii`, and `games/*`.

Canonical rules: `.cursor/rules/wiimaker-engine.mdc` · architecture: `ARCHITECTURE.md`.

## Gotchas

- Non-PoT PNGs are padded at cook time; doctor warns (see `cyber_rover` in hello-orb).
- `.wpack` RGB5A3 is **GX 4×4 tiled** (cook tiles; host `to_rgba8` untiles). Re-cook after cooker changes.
- Wii C path embeds `assets.wpack` + `scene.wscn` (from prepare/`cook` + `bake-wii`). `wii-build.sh` / `wiimaker build` run both before Docker make. No Rust staticlib yet — `stub_game.c` is the scene player.
- Objcopy embed: copy bins into `runtime/wii/build/` first so symbols are `_binary_assets_wpack_*` / `_binary_scene_wscn_*` (path-mangled names break the C externs).
- Sprite sheets: sidecar `assets/<stem>.sprites.json`; cook still packs the whole PNG once. Catalog resolves cell name → sheet texture + UV (in **packed** PoT space) + pivot.
- Core `Sprite` has `pivot` (default `0.5,0.5`) and `uv`; render/pick/outline must share the same pivot math.
- WSCN magic is **`WSCN0002`**: sprite payload includes `u0,v0,u1,v1` + `pivot_x,pivot_y`. Old `WSCN0001` embeds will fail loudly in C.

## Decisions

- Until Rust `staticlib` lands, Dolphin play uses the C scene player + GX textured quads; host keeps `wiimaker-scene` JSON hydrate + `SpriteCatalog`.
- `games/` is gitignored (local projects only); workspace still lists `games/hello-orb` for local cook/run.
- Pivot lives on sheet meta (not SceneSprite override) for v0.
- Primary ship verbs are Build / Play in Dolphin / Build & Run; `cook` is advanced/agent-only.

## Open follow-ups

- Rust staticlib for Wii sharing host `App` / `World` (replace C scene player).
- Sprite sheet offset/padding, Grid By Cell Size, component-level pivot override.
