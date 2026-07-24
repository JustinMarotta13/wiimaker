# Architecture

## Goals

Ship original Wii games with a modern authoring loop:

- Edit → run on host in <1s
- Same codebase → `.dol` for Dolphin / USB Loader GX / HBC
- Stay inside Broadway limits: 24 MB MEM1, 64 MB MEM2, GX fixed-function

## Non-goals (for v0)

- Full GX feature parity on host (no TEV emulation science project)
- Wiimote IR pointer / sensor bar (use classic/GCN pads first — HorrorDash lesson)
- Online matchmaking
- Recompiling commercial games (we *learn* from recomp toolkits; we don't ship one)

## Layer cake

```
┌─────────────────────────────────────────────────────────┐
│  games/*          App + scenes (.scene.json) + gameplay │
├─────────────────────────────────────────────────────────┤
│  wiimaker-scene   Project / Scene / Prefab + mutate API │
│  wiimaker-editor  egui Hierarchy · Inspector · Viewport │
│  wiimaker-cli     Agent twin of every editor mutation   │
├─────────────────────────────────────────────────────────┤
│  wiimaker-core    World + components + DrawList IR      │
├──────────────────────────┬──────────────────────────────┤
│  wiimaker-host           │  runtime/wii (C) + rustlib   │
│  (minifb / egui viewport)│  VI · GX · PAD · ASND        │
└──────────────────────────┴──────────────────────────────┘
```

### Authoring track

Scenes are JSON on disk (`scenes/*.scene.json`). The egui editor and CLI share
mutation helpers in `wiimaker-scene` so agents and humans never diverge.

Unity mapping: Project → `game.toml`, Scene → `.scene.json`, GameObject → named
entity with `Transform` + `Sprite` / `Disc` / `Camera`, Prefab → `.prefab.json`.

### `wiimaker-core`

Platform-agnostic. Uses `glam` for math. Games never call GX or OpenGL directly.

Key types:

- `App` — implement `update` / `render`
- `World` — named entities with Transform + Sprite/Disc/Camera
- `DrawList` — ordered `DrawCmd` (Clear, SetCamera, DrawMesh, DrawSprite)
- `Input` — normalized buttons + sticks (GCN layout as the lingua franca)
- `Time` — fixed 60 Hz tick with accumulator (Wii VI is king)

### Display List IR

Inspired by how NWiiRecomp's runtime HLEs GX and how real homebrew builds display lists:

```rust
enum DrawCmd {
    Clear { color: [u8; 4] },
    SetCamera { view: Mat4, proj: Mat4 },
    SetTexture { id: TextureId },
    DrawMesh { mesh: MeshId, transform: Mat4, color: [u8; 4] },
    DrawSprite { texture: TextureId, dest: Rect, uv: Rect, color: [u8; 4] },
}
```

Host interprets this with a software rasterizer (v0) or GL (v1).
Wii maps each command onto GX immediate / display-list calls.

### Wii runtime (C)

The previous project's pure-Rust `Video::configure` path leaked a heap
`GXRModeObj` into the VI ISR and crashed at `PC=0x80030100`. We do not repeat that.

`runtime/wii/src/bootstrap.c`:

1. `VIDEO_Init` / preferred mode / double framebuffer
2. `GX_Init` FIFO in MEM1
3. `PAD_Init` (+ later `WPAD_Init`)
4. Call into Rust `wiimaker_game_frame(input, dt)` each VI

Rust builds as `staticlib`; the Makefile links it with `-lwiiuse -lbte -logc -lm`.

### Asset pipeline (`.wpack`)

Offline (`wiimaker-assets` + CLI):

| Source | Packed |
|---|---|
| PNG/TGA | RGB5A3 or CMPR tiles, 32-byte aligned |
| OBJ/glTF | Interleaved POS/NRM/UV as f32 or s16, 32-byte aligned |
| WAV | Mono/stereo PCM16 (ASND-friendly) |

A `.wpack` is a tiny TOC + blobs — no runtime parsing of PNG on console.

### Packaging targets

```
target/wii/<game>/
  boot.dol          # elf2dol output
  meta.xml          # HBC metadata
  icon.png
  ../<game>.iso     # optional wit/mkisofs disc for USB loaders
```

## Roadmap

### Milestone 0 — Scaffold (this commit)
- Workspace + host hello-orb
- Wii C bootstrap + Makefile stubs
- CLI skeleton, Docker file, docs

### Milestone 1 — Playable loop on host
- Sprites + mesh draw
- Fixed timestep, input map
- `.wpack` reader (host)
- **Scene file / prefab + egui editor + agent CLI** (pulled forward from M3)

### Milestone 2 — First Dolphin boot
- Rust staticlib linked through C bootstrap
- Clear screen + spinning cube via GX
- HBC pack script

### Milestone 3 — Real game shape
- Prefab instantiate polish + viewport gizmos
- Audio oneshots
- Wiimote classic / GCN pads polished
- Asset cooker CI in Docker

### Milestone 4 — Engine character
- Materials → TEV presets
- Simple collision
- Optional `luma` backend experiment

## Risk notes

- **libogc provenance drama (2025):** We isolate all libogc touchpoints in
  `runtime/wii`. Swapping to `luma` or a thinner HAL should not rewrite games.
- **Apple Silicon + powerpc-eabi:** Prefer Docker (`devkitpro/devkitppc`) for
  reproducible Wii builds; host path stays native arm64.
- **MEM1 pressure:** Keep GX FIFO + framebuffers accounted; stream level data
  from MEM2 / DVD.
