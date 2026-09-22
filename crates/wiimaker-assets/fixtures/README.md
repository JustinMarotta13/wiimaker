# Host-first fixtures

## `beep.wav`

PCM16 mono 22050 Hz beep (~80 ms). Copy into a game `assets/` folder; `cook` /
`prepare` packs it into the `.wpack` audio TOC for Wii ASND (host still plays
the WAV on disk):

```bash
wiimaker asset import <game> crates/wiimaker-assets/fixtures/beep.wav
WIIMAKER_AUDIO=0 wiimaker asset play <game> --name beep --json   # CI: skipped
wiimaker asset play <game> --name beep                           # hear it on desktop
```

Do not put clips under committed `games/`.

## `hud_font.png`

Built-in 8×8 HUD atlas (128×64, ASCII 32–126). **Not cooked into `.wpack`.**
The host rasterizer samples the same bits from `wiimaker_assets::atlas_rgba8()`;
this PNG is the visual fixture (must match the generated atlas).

```bash
# regenerate if you edit crates/wiimaker-assets/src/font.rs
# (test `fixture_png_matches_atlas` will fail until you rewrite this file)
```
