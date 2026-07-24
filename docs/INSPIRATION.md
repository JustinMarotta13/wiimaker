# Inspiration notes

Research snapshot for the wiimaker redesign (Jul 2026).

## What failed last time

Pure `ogc-rs` ownership of `VIDEO_Configure` leaked a heap `GXRModeObj` into
the VI ISR path and crashed Dolphin at `PC=0x80030100`. Lesson: **C owns video
bring-up; Rust owns game logic.**

## What ships games today

- **HorrorDash** (itch.io, 2025) — C + GRRLIB, Blender/Inkscape art, Audacity
  audio. Pad mapping was the hard part; IR pointer abandoned.
- **CavEX** — Minecraft-like; same codebase builds for Wii *and* desktop OpenGL.
  This is the authoring-loop pattern we copy.
- **Texel** (2025–26) — C++ ECS, GLM math → GX at submit. Proves late-binding
  of matrices to GX is the right abstraction boundary.
- **wii-3d-engine** — custom `.wmesh` cooker; MEM1-aware asset pipeline.
- **GRRLIB 4.6.x** — still the friendly GX wrapper for C/C++ homebrew.

## Recomp scene (inspiration, not a feature)

- **NWiiRecomp** — DOL → C++ static recomp with HLE for VI/DVD/AX. Their
  service-oriented runtime model informs our `Platform` / ABI split.
- **Mario Kart Wiicompiled** (announced 2026) — first public Wii static recomp
  effort. Reminds us the Broadway ISA and GX semantics are well understood
  enough to model cleanly.

## libogc / luma

2025 provenance drama around libogc. We isolate all libogc calls in
`runtime/wii`. [rust-wii/luma](https://github.com/rust-wii/luma) is a Rust
rewrite we can experiment with later behind the same ABI.

## Design bets

1. Host-first software IR interpreter → finish games.
2. C bootstrap + stub game → prove Dolphin path before Rust cross.
3. `.wpack` cooker → no PNG on console.
4. GCN / Classic pads first → Wiimote IR later (or never).
