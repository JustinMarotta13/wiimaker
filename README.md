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
| Tilemap / TilemapCollider2D | `Tilemap` component (cell ids + solid bits) |
| AnimatedTile / RuleTile | Tilemap palette `anim` + `auto_tile` (`id` / `solid`, NESW bitmask) |
| BoxCollider2D / CircleCollider2D | `Collider` (`Aabb` / `Circle`, `solid`) |
| AudioSource | `AudioSource` (`clip`, `volume`, `play_on_awake`) + host / Wii ASND oneshots |
| Hierarchy / Inspector | `wiimaker edit` panels (or CLI) |
| Play | Editor Play ticks the game `App` (cdylib plugin) · `wiimaker run` / File → Run external |
| Scenes in Build / LoadScene | `game.toml` `scenes = [...]` · `load_scene_into_world` · File → Build Settings… |
| Build / Dolphin | `wiimaker build` · `wiimaker play-wii` |
| Sprite Editor | `assets/<name>.sprites.json` + editor / `wiimaker asset slice` |

Gameplay scripts stay as Rust `App` (like MonoBehaviour code): `load_scene_into_world` (keeps the atlas), then mutate entities in `update`.

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
wiimaker scene new my-game --name menu
wiimaker scene set-default my-game --scene menu
wiimaker scene build-add my-game --scene menu
wiimaker scene build-add my-game --scene main
wiimaker scene build-list my-game --json
wiimaker scene set-game-view my-game --preset 640x480 --scale 1
wiimaker editor set-scene-view my-game --zoom 1.25 --grid true --gizmos true
wiimaker editor set-project-view my-game --collapse assets --collapse assets/fx
wiimaker editor prefs my-game --json
wiimaker editor play-status my-game --json   # in-editor Play backend (plugin vs WASD fallback)
wiimaker asset import my-game ./beep.wav
wiimaker asset play my-game --name beep
wiimaker entity add-component my-game --name Player AudioSource --clip beep --volume 1 --play-on-awake false
# or: wiimaker entity set my-game --name Player --audio-clip beep --volume 0.8 --play-on-awake true
wiimaker entity add my-game --name Player --sprite hero_2 --x 320 --y 240
wiimaker entity add-component my-game --name Maze Tilemap --cols 28 --rows 31 --cell 16
wiimaker tilemap stamp my-game --name Maze --ascii $'###\n#.#\n###'
wiimaker tilemap from-ascii my-game maze.txt --name Maze --json
# optional: --map '#=1,.=0,P=2:0' --resize false
wiimaker tilemap set my-game --name Maze --x 1 --y 1 --id 0
wiimaker tilemap get my-game --name Maze --x 1 --y 1 --json
wiimaker tilemap set-palette my-game --name Maze --id 2 --sprite water --anim water --fps 8 --auto-tile id
wiimaker tilemap mask my-game --name Maze --x 1 --y 1 --json
wiimaker entity add-component my-game --name Wall Collider --w 32 --h 16
wiimaker entity add-component my-game --name MainCamera Camera
wiimaker entity add-component my-game --name MainCamera Follow --target Player --lerp 0.15
# or: wiimaker entity set my-game --name MainCamera --follow Player --lerp 0.15
wiimaker entity add-component my-game --name Player GridMover --cell 20 --speed 6
# or: wiimaker entity set my-game --name Player --cell 20 --speed 6
wiimaker entity add-component my-game --name Hud Text --text "Score: 0" --size 16 --color 255,255,255 --align left
# or: wiimaker entity set my-game --name Hud --text "Score: 1" --size 16 --align center
wiimaker entity overlaps my-game --name Player --other Wall
wiimaker cook my-game          # advanced / agents
wiimaker doctor my-game
wiimaker run my-game
wiimaker edit my-game
wiimaker build my-game         # .dol (alias: build-wii)
wiimaker dolphin my-game       # launch existing boot.dol
wiimaker play-wii my-game      # build then Dolphin
```

Scene / entity edits write `.scene.json` — the same files the egui editor saves.

Editor Scene/Game/Project chrome (zoom, grid, gizmos, Game aspect, Project collapsed folders) lives in `<game>/.wiimaker/prefs.toml`, not `game.toml`. `wiimaker scene set-game-view`, `wiimaker editor set-scene-view`, and `wiimaker editor set-project-view` mutate that file; the editor writes the same store. Project Search is session-only.

Sprite sheets keep one PNG; cells live in `assets/<stem>.sprites.json` (Grid By Cell Count + normalized pivot). Scenes reference cell names like `hero_2`.

A tiny PCM16 beep lives at `crates/wiimaker-assets/fixtures/beep.wav` (also copied into `templates/basic-game/assets/` for new games). Cook packs it into the `.wpack` audio TOC; host still previews the WAV on disk.

HUD text uses a built-in 8×8 bitmap font (`DrawCmd::DrawText` on host; WSCN0003 `KIND_TEXT` + GX quads on Wii). The visual fixture is `crates/wiimaker-assets/fixtures/hud_font.png`; it is **not** cooked into `.wpack`. Missing glyphs draw as `?`.

Tilemaps bake into the same `WSCN0003` blob (`KIND_TILEMAP=3`, length-prefixed grid + palette). The Wii C player draws occupied cells as GX textured quads (palette sprite / auto-tile variant / anim frame 0+) or untextured tinted quads when the palette has no texture — matching host `render_world`. Editor/CLI tilemap tools are unchanged.

AudioSource bakes under the same `WSCN0003` magic: `KIND_AUDIO=5` for audio-only entities, plus a trailing table (entity index, wpack clip, volume, play-on-awake) so a Sprite/Disc/Tilemap/Text can still fire a oneshot. Missing clips are `0xFFFF` and must not crash. The C player inits ASND, loads the TOC, and plays play-on-awake after scene load.

## Quick start (Wii)

Requires [devkitPro](https://devkitpro.org/) `wii-dev` **or** Docker:

```bash
wiimaker build hello-orb       # prepare + bake + Docker → target/wii/hello-orb/boot.dol
wiimaker dolphin hello-orb     # or: ./tools/run-dolphin.sh target/wii/hello-orb/boot.dol
# one shot:
wiimaker play-wii hello-orb
```

`build` prepares `.wpack` (PNG + PCM16 WAV TOC), bakes `scene.wscn` (WSCN0003 with UV + pivot + Tilemap palette + `KIND_TEXT` + `KIND_AUDIO` / audio table), and embeds both into the `.dol`. Editor toolbar: **Build** · **Play in Dolphin** · **Build & Run** (Cook is under ⋯).

**Dolphin check (tilemaps / audio):** `wiimaker build <game>` then `wiimaker dolphin <game>` (or File → Open `target/wii/<game>/boot.dol`). Occupied tile cells should match host `wiimaker run` / editor Game view (colored walls, sprite tiles, auto-tile variants). Animated palette clips tick on GX when the bake includes 2+ frames. AudioSource play-on-awake should fire once after load (PCM16 from the wpack TOC). This environment cannot run Dolphin; use that path on a machine with it.

## Workspace layout

```
wiimaker/
├── crates/
│   ├── wiimaker-core/     # engine: World, components, DrawList IR, input
│   ├── wiimaker-host/     # desktop backend (minifb + texture atlas)
│   ├── wiimaker-scene/    # game.toml / .scene.json + mutate helpers
│   ├── wiimaker-assets/   # PNG + PCM16 WAV → .wpack cooker
│   ├── wiimaker-play/     # host/editor App tick + optional game cdylib
│   ├── wiimaker-cli/      # `wiimaker` agent + human CLI
│   └── wiimaker-editor/   # egui Hierarchy / Inspector / Scene / Project
├── runtime/wii/           # C Broadway bootstrap (VI/GX/PAD/ASND)
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
