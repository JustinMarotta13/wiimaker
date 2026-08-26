# wiimaker

**Author Wii games on your Mac. Ship them to Dolphin or real hardware.**

Wiimaker is a dual-target game toolkit for Nintendo Wii homebrew. You write game
logic once against a platform-agnostic engine, iterate instantly on desktop, then
cross-compile the same project into a `.dol` / Homebrew Channel app / disc image.

```
  ┌──────────────┐     DisplayList IR      ┌────────────────────┐
  │  Your game   │ ──────────────────────► │  host (desktop)    │  ← day-to-day
  │  (Rust)      │                         │  soft/GL preview  │
  └──────┬───────┘                         └────────────────────┘
         │
         │ same IR                         ┌────────────────────┐
         └───────────────────────────────► │  wii (Broadway)    │  ← ship
                                           │  C runtime + GX    │
                                           └────────────────────┘
```

## Unity → WiiMaker cheat sheet

| Unity | WiiMaker |
|---|---|
| Project window | `game.toml` + `assets/` + `scenes/` |
| Scene | `.scene.json` |
| GameObject | Entity (`name` + Transform + components) |
| Transform | `transform.translation / rotation / scale` |
| SpriteRenderer | `Sprite` component |
| Prefab | `.prefab.json` |
| Hierarchy / Inspector | `wiimaker edit` panels (or CLI) |
| Play | `wiimaker run` / `cargo run -p <game>` |
| Build Settings | `wiimaker build` · `wiimaker play-wii` |
| Sprite Editor | `assets/<name>.sprites.json` + editor / `wiimaker asset slice` |

Gameplay scripts stay as Rust `App` (like MonoBehaviour code): load a scene, then mutate entities in `update`.

## Quick start (host)

```bash
cd /Users/justin/wiimaker
cargo run -p wiimaker-cli -- cook hello-orb   # advanced; Play/Build also prepare assets
cargo run -p hello-orb
# or open the editor:
cargo run -p wiimaker-cli -- edit hello-orb
```

Arrow keys / WASD move the orb. Esc quits.

## Agent-friendly CLI

Every command accepts `--json` for machine output.

```bash
wiimaker new my-game
wiimaker asset import my-game ./hero.png
wiimaker asset slice my-game hero --cols 4 --rows 1
wiimaker asset set-pivot my-game hero_2 --x 0.375 --y 0.375
wiimaker entity add my-game --name Player --sprite hero_2 --x 320 --y 240
wiimaker cook my-game          # advanced / agents
wiimaker doctor my-game
wiimaker run my-game
wiimaker edit my-game
wiimaker build my-game         # .dol (alias: build-wii)
wiimaker dolphin my-game       # launch existing boot.dol
wiimaker play-wii my-game      # build then Dolphin
```

Scene / entity edits write `.scene.json` — the same files the egui editor saves.

Sprite sheets keep one PNG; cells live in `assets/<stem>.sprites.json` (Grid By Cell Count + normalized pivot). Scenes reference cell names like `hero_2`.

## Quick start (Wii)

Requires [devkitPro](https://devkitpro.org/) `wii-dev` **or** Docker:

```bash
wiimaker build hello-orb       # prepare + bake + Docker → target/wii/hello-orb/boot.dol
wiimaker dolphin hello-orb     # or: ./tools/run-dolphin.sh target/wii/hello-orb/boot.dol
# one shot:
wiimaker play-wii hello-orb
```

`build` prepares `.wpack`, bakes `scene.wscn` (WSCN0002 with UV + pivot), and embeds both into the `.dol`. Editor toolbar: **Build** · **Play in Dolphin** · **Build & Run** (Cook is under ⋯).

## Workspace layout

```
wiimaker/
├── crates/
│   ├── wiimaker-core/     # engine: World, components, DrawList IR, input
│   ├── wiimaker-host/     # desktop backend (minifb + texture atlas)
│   ├── wiimaker-scene/    # game.toml / .scene.json + mutate helpers
│   ├── wiimaker-assets/   # PNG → .wpack cooker
│   ├── wiimaker-cli/      # `wiimaker` agent + human CLI
│   └── wiimaker-editor/   # egui Hierarchy / Inspector / Scene / Project
├── runtime/wii/           # C Broadway bootstrap (VI/GX/PAD)
├── games/hello-orb/       # reference game (scene + sample sprites)
├── templates/basic-game/  # scaffold for `wiimaker new`
├── docker/                # reproducible PowerPC builds
└── tools/                 # pack HBC, make ISO, launch Dolphin
```

## Design principles

1. **Host first.** If it doesn't run on your laptop in under a second, it won't get finished.
2. **C owns the metal.** Video init, GX FIFO, and PAD live in a tiny C runtime. Rust owns game logic.
3. **One IR.** Games emit `DrawCmd` lists — backends interpret them.
4. **Packed assets.** Source art converts offline into `.wpack` (GX-ready RGB5A3).
5. **Files are truth.** Editor and CLI mutate the same scene JSON agents can write.
6. **Swappable HAL.** `libogc` today; ready for `luma` later.

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full plan.
