# Engine memory

Operational notes for `wiimaker-core`, `wiimaker-host`, `wiimaker-assets`, `runtime/wii`, and `games/*`.

Canonical rules: `.cursor/rules/wiimaker-engine.mdc` · architecture: `ARCHITECTURE.md`.

## Gotchas

- Non-PoT PNGs are padded at cook time; doctor warns (see `cyber_rover` in hello-orb).
- `.wpack` RGB5A3 is **GX 4×4 tiled** (cook tiles; host `to_rgba8` untiles). Re-cook after cooker changes.
- Wii C path embeds `assets.wpack` + `scene.wscn` (from `wiimaker cook` + `bake-wii`). `wii-build.sh` runs both before Docker make. No Rust staticlib yet — `stub_game.c` is the scene player.
- Objcopy embed: copy bins into `runtime/wii/build/` first so symbols are `_binary_assets_wpack_*` / `_binary_scene_wscn_*` (path-mangled names break the C externs).
- Wii sprites use content UV (`size / tex_size`) so PoT padding is not stretched into the dest rect.

## Decisions

- Until Rust `staticlib` lands, Dolphin play uses the C scene player + GX textured quads; host keeps `wiimaker-scene` JSON hydrate.

## Open follow-ups

- Rust staticlib for Wii sharing host `App` / `World` (replace C scene player).
- Align host sprite UV with content-size convention used on Wii.
