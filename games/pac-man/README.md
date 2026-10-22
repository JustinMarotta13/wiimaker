# pac-man (sample)

Committed sample under `games/pac-man` (workspace member). Not an engine feature — it exists to show what WiiMaker can author **today** vs what a maze game still has to fake. Board: `MEMORY/kanban.md`.

```bash
cd wiimaker                  # repo root; rustc 1.98+
cargo run -p wiimaker-cli -- cook pac-man
cargo run -p pac-man
# or: cargo run -p wiimaker-cli -- run pac-man
# edit maze: cargo run -p wiimaker-cli -- edit pac-man
```

**Player sprite:** the stand-in sheet `assets/basic_space_suit.png` (+ `.sprites.json`) is **not committed** (it comes from another project). Without it the Player sprite is skipped (missing-texture fallback) and the rest of the game runs as normal. Drop any 64×16 four-cell sheet with that name into `assets/` (or retarget the `Player` Sprite and the `*.anim.json` cells) to see it. Cooked `assets.wpack` / `scene.wscn` and `.wiimaker/` editor prefs are gitignored.

Menu plays a 3s **Timeline** intro on awake (`assets/intro.timeline.json`, entity `Director`, 2026-10-04). Tracks: Activation reveals `IntroGhost` (0.05–2.2s), Animation drives `Player` chomp, Audio oneshot `beep`, Transform pans `MainCamera` out and back to 320,240. `play_on_awake` true, `loop` false. `animate_world` calls `tick_timelines`; oneshots drain through `HostAudio`.

**2026-10-06 Signal + Control:** `intro.timeline.json` now has a **Control** track `Nest` (drives `IntroGhost` and its own `ghost-bob` director) and a **Signal** track `Cues` firing `IntroDone` (payload `ready`) at 2.5s. `src/main.rs` drains `World::take_timeline_signals()` after `animate_world`; the menu `Hint` (cyan press-Start disc) stays hidden until `IntroDone` fires. Check: `PACMAN_DUMP_FRAMES=170 PACMAN_DUMP_FRAME=/tmp/f.ppm cargo run -p pac-man`.

**2026-10-07 Timeline curves:** `CamSlide` keeps `from`/`to` fallbacks and adds optional `curves.x` / `curves.y` — ease on the outbound pan (0–1.5s), linear on the return (1.5–3s). New Float track `GhostPulse` binds `IntroGhost` `Transform.scale_x` with `curves.value` pulsing 1→1.25→1 over ~0.05–2.2s (ease keys). Nest / PlayerChomp / Stinger / Cues(`IntroDone`) unchanged. Sample: `wiimaker asset timeline-curve sample pac-man --timeline intro --track CamSlide --clip 0 --prop x --from 0 --to 1.5`.

**2026-10-07 (cont.):** `intro.timeline.json` also has `PacSwoop` (Transform on `TitlePac`, clip 0–1.5s; keyed `curves.x` -40 ease → 285 ease → 260 overshoot-settle, `curves.y` 200 ease → 150 ease → 200 constant hop) and `PlayerFade` (Float `Sprite.alpha` on `Player`, 0 ease → 1 over 1s). `ghost-bob.timeline.json` `Bob` now uses an eased Y curve 200 → 170 → 200 instead of the one-way lerp. All authored with `wiimaker asset timeline-curve add-key` (use `--v=-40` on builds before the negative-number fix). Check: `wiimaker entity timeline-status pac-man --scene scenes/menu.scene.json --name Director --steps 30 --json` (TitlePac ≈ 100,151 at 0.5s).

**2026-10-08 Blend trees (PR #55 probe):** `player.controller.json` `Chomp` is now a **1D blend tree** on Float `Speed` — `chomp`@0 → `dash`@1 (new `dash.anim.json`, cells 0–3 @16fps). Moving still flips Idle↔Chomp via Bool `Moving`. `src/main.rs` ramps `Speed` toward 1 while ghosts are frightened (player also moves 1.2× then), back to 0 otherwise. Optional **2D** variant `player-2d.controller.json`: `Chomp` blends `Speed` × `Heading` (0 horizontal, 1 vertical from the Mover dir) over `chomp`(0,0) / `chomp-v`(0,1) / `dash`(1,0) / `dash-v`(1,1); swap with `wiimaker entity add-component pac-man Animator --name Player --controller player-2d --scene scenes/main.scene.json` (and `--controller player` to go back). Authored with `wiimaker asset anim`, `asset controller`, `asset blend-tree`, `entity animator-set --float Speed=0 --float Heading=0`. Check: `PACMAN_START_MAZE=1 PACMAN_FRIGHT=1 PACMAN_DUMP_FRAMES=60 PACMAN_DUMP_FRAME=/tmp/f.ppm cargo run -p pac-man` prints `blend ... active=dash`; add `PACMAN_DUMP_TURN=20` (stick up after 20 frames) with the 2D controller to see `dash-v` / `chomp-v`. Known gap: `entity animator-status` / `animator-set --json` error on Player (`texture 'basic_space_suit_0' not found in wpack`) because the CLI hydrates with an empty TextureMap.

Player uses **Animator** Idle/Chomp on the Bool parameter `Moving` (engine 2026-10-03). Stand-in sprite is `basic_space_suit` until a Mii head. `Moving` is true while `dir != (0,0)` or mid-cell (`progress > 0`); `animate_world` ticks the controller each frame.

Controls: **Enter / Z / Space** start (and restart after a win). **Arrows / WASD** four-way. **Esc** quit. Score and lives print to stdout (no HUD).

`wiimaker new pac-man` created `games/pac-man`; it is a workspace member in the root `Cargo.toml` and committed as the sample game.

## Scene

- `scenes/main.scene.json` — 19×20px maze, Player, four ghosts, pellets, `MainCamera`
- `scenes/menu.scene.json` — title discs, press Start
- `scenes/win.scene.json` — trophy discs
- `assets/prefabs/pellet.prefab.json` — created via CLI from a maze dot (if present)

Tags: `1` wall · `2` pellet · `3` power pellet · `4` ghost · `5` player.

Hierarchy roots: `Maze` (walls as children) · `Pellets` · `Player` · `Ghost*` · `MainCamera`.

## What the engine actually did

| Piece | Engine support |
|---|---|
| Project / scenes / entities | `game.toml`, `.scene.json`, named entities, Transform |
| Player | Sprite + Animation + Animator (`player.controller.json`, Idle/Chomp) |
| Ghosts / pellets | `Disc` components |
| Hierarchy | `parent` on walls and pellets |
| Camera entity | `Camera { active }` hydrates; **not used by `render_world`** |
| Tags | `tag: u32` on entities, copied into `World` |
| Prefabs | `entity create-prefab` / instantiate (no variants) |
| Input | GCN-layout stick + D-pad from WASD/arrows |
| Draw | `render_world` → DrawList Disc/Sprite, sort by `z` |
| Cook | `wiimaker cook` writes an empty `.wpack` (no PNGs) |
| Editor | Hierarchy / Inspector / Scene / Project / Play toolbar |
| Undo, snap, Move/Scale/Rotate | editor only |
| Second scenes on disk | `scene list` sees them |

## What we faked (engine lacks it)

| Need | Fake in this game | Board item |
|---|---|---|
| Tilemap / grid | 19×19 ASCII baked to **one Disc per cell** | Now #1 |
| Solid tiles | `HashSet<(col,row)>` rebuilt from tag-1 discs | Now #1 / #2 |
| AABB collision | grid occupancy, not a Collider component | Now #2 |
| Triggers / collectibles | same-cell despawn of tag 2/3 | Now #3 |
| Sprite animation | Player Idle/Chomp via Animator (suit stand-in, not a Mii head) | Now #4 shipped for Player |
| Timeline / cutscene | Menu `intro` PlayableDirector (activation, chomp, beep, camera) | 2026-10-04 |
| Scene switching | game re-`hydrate`s menu/main/win JSON | Next #5 |
| 4-way discrete move | `Mover` with queued cardinals + cell progress | Next #6 |
| Audio | menu intro beep oneshot; maze still mostly silent | Next #7 partial |
| Camera follow | camera entity is a marker; view is identity 640×480 | Next #8 |
| Sorting layers | raw `z` (pellets 1, actors 2) | Later |
| Prefab variants | one pellet prefab; maze dots are not instances | Later |
| Play-in-editor | toolbar Play still WASD-moves `Player` and **ignores walls / this `App`** | Later |
| HUD / text | stdout score | Later |
| Wrap tunnels | closed maze | — |

Editor **Play** will let you drag the yellow Player through walls — that is the engine’s hardcoded preview, not this crate. Use **File → Run external…** or `cargo run -p pac-man` for the real loop.

## CLI used while scaffolding

```
wiimaker new pac-man
wiimaker scene set-clear pac-man --rgb 4,4,24
wiimaker entity create-prefab pac-man --name Dot_1_1 --as-name pellet
wiimaker cook pac-man
wiimaker doctor pac-man
```

**2026-10-09 Animator override layers (PR #56 probe):** `player.controller.json` gains override layer **`Power`** (file weight 0) on top of Base (Idle / Chomp blend). Power states: `Rest` (empty placeholder, default — never overrides) and `Powered` (new `power.anim.json`: cells 3→0 @20fps). Transitions Rest→Powered on `Moving=true`, back on `Moving=false`. `src/main.rs` `sync_player_animator` sets the Power weight via `World::set_animator_layer_weight` (only on change): 1 while ghosts are frightened, blinking 1/0 in step with the ghost flash in the last 2s so the Base `dash` blend shows through; 0 otherwise (Base wins). Authored with `wiimaker asset anim pac-man power …`, `asset controller-layer pac-man player add --name Power --weight 0`, `… state --name Power --state Powered --clip power`, `… state --name Power --state Rest --clip ""`, `… default --name Power --state Rest`, `… transition --name Power --from Rest --to Powered --condition Moving=true` (and the reverse). Check: `PACMAN_START_MAZE=1 PACMAN_FRIGHT=1 PACMAN_DUMP_FRAMES=60 PACMAN_DUMP_FRAME=/tmp/f.ppm cargo run -p pac-man` prints `layer Power weight=1.00 state=Powered` / `playback layer=Power`; live runs log each weight flip. New probe env `PACMAN_AUTOPILOT=1` steers the player when no input (for screenshots). Known gap (still): `entity animator-status` / `animator-set` error on Player (`texture 'basic_space_suit_0' not found in wpack`; CLI hydrates with an empty TextureMap) — and `animator-set` writes the scene before that error.

**2026-10-13 N7 pairing (animator CLI texture map):** Best probe entity is **`Player`** on `scenes/main.scene.json` — Sprite `basic_space_suit_0` + Animation `idle` + Animator controller `player` (`assets/player.controller.json`: Base Idle/Chomp 1D `Speed` blend + override layer `Power` / `power.anim.json`). Menu also has a Sprite+Animator `Player` (same controller, no scene params); prefer **main** so `--bool` / `--float` / `--layer Power --weight` all exercise something real. Host Play / Power layer / fright path in `src/main.rs` is already good — **do not retouch** for N7.

After N7 merges (pull master, rebuild CLI), from `/workspace/wiimaker`:

```bash
# 1) prove hydrate loads wpack textures (today fails: texture 'basic_space_suit_0' not found in wpack)
cargo run -p wiimaker-cli -- entity animator-status pac-man \
  --name Player --scene scenes/main.scene.json --json

# 2) set params + Power weight, then dump again (must exit 0; must NOT leave a half-written scene on failure)
cargo run -p wiimaker-cli -- entity animator-set pac-man \
  --name Player --scene scenes/main.scene.json \
  --bool Moving=true --float Speed=1 --layer Power --weight 1 --json
cargo run -p wiimaker-cli -- entity animator-status pac-man \
  --name Player --scene scenes/main.scene.json --json

# 3) restore authored defaults so host Play stays sane (Moving false, Speed 0, Power weight 0)
cargo run -p wiimaker-cli -- entity animator-set pac-man \
  --name Player --scene scenes/main.scene.json \
  --bool Moving=false --float Speed=0 --layer Power --weight 0 --json

# optional one-liner wrapper (same dir, gitignored):
#   bash games/pac-man/probe-n7-animator.sh
```

Expect JSON with `controller=player`, Base state flipping toward Chomp when `Moving=true`, `blend.active` toward `dash` at `Speed=1`, `layers[].name=Power` with `weight=1` / state `Powered` while moving, and `playback` naming the winning layer. Screenshots: (a) terminal JSON from steps 1–2, (b) optional host still with `PACMAN_START_MAZE=1 PACMAN_FRIGHT=1 PACMAN_DUMP_FRAMES=60 PACMAN_DUMP_FRAME=/tmp/f.ppm cargo run -p pac-man` (unchanged Power probe).

## 2026-10-14 — Animator window graph layout

Laid out `player.controller.json` with the new `asset controller-layer … move-state` CLI
(positions only, no behavior change). Base: Entry 0,40 · Idle 140,40 · Chomp 280,40 · Exit 400,40 · Any 0,120.
Power: Entry 0,40 · Rest 140,40 · Powered 280,40 · Exit 400,40 · Any 0,120. View in Window → Animator.

## 2026-10-15 — Animator layer fixes (#60) pairing

On main `493ad2a`: `entity animator-set --layer base|BASE|Base --weight 1` on Player stores exactly one
`Base` override. Seeded `Power=0.5`, then `asset controller-layer player rename --name Power --to ' Powered Up '`
→ controller and scene override both became `Powered Up` (trimmed); renamed back to `Power` and the override
followed. Scene restored to its original (no overrides); controller byte-identical. Behavior unchanged.

## 2026-10-16 — Animator layer-winner tests (#61) pairing

On main `eeaf445` (tests-only PR). Pac-Man tweak in `sync_player_animator`: in the last 2s of
fright the `Power` layer weight now toggles 1/0 every 0.5s instead of at the 8 Hz ghost-flash rate,
so each Base↔Power winner switch resets playback (Chomp ↔ Powered) cleanly and is visible in the
`animator layer Power weight=…` log. Ghost flashing is unchanged. Host check:
`PACMAN_AUTOPILOT=1 PACMAN_START_MAZE=1 PACMAN_FRIGHT=1 cargo run -p pac-man`.

## 2026-10-20 — Power pellets 1.25x + Game view Play chrome (#63) pairing

On main `e23a0e0`. Morning edit: the four power pellets (`Power0`–`Power3` under `Dots`) are scaled
1.25×1.25 in `scenes/main.scene.json` (Scene view prefs left at Scale tool · Center · Global).
Game view: `scene set-game-view pac-man --stats on --play-mode maximized` → `.wiimaker/prefs.toml`
`play_mode = "maximized"`, `mute_audio = false`, `stats = true`. Editor Play (WASD fallback, no cdylib)
fills the window with the Game tab; Stats reads Entities 309 · Draw cmds 742 (Sprites 1 · Text 0 ·
Quads 434 · Discs 306) · Render 640×480. Pause → Step advances one frame and stays paused; Stop restores
the docks. Known gaps: `editor play-step pac-man --frames 60 --json` errors
(`entity 'Player': texture 'basic_space_suit' not found in wpack` — CLI fallback hydrates strictly with an
empty TextureMap); Maximized + Free aspect draws the 640×480 frame enlarged and off-center (maze cropped).
Host check: `PACMAN_AUTOPILOT=1 PACMAN_START_MAZE=1 cargo run -p pac-man`.
Update (main `da86424`, #64): `editor play-step pac-man --frames 60 --json` now works → ok, 309 entities, 742 draw cmds (434 quads · 306 discs · 1 sprite · 0 text), Player at (30, 450), MainCamera follows to (30.02, 449.99); Maximized + Free now letterboxes a centered 640×480 frame (docked Free still fills the panel); workspace tests 468/468.

## 2026-10-21 — Console Collapse + Error Pause (#65) pairing

On main `e4bb114`. `editor set-console pac-man --collapse on --error-pause on` → `.wiimaker/prefs.toml`
`[console]` `clear_on_play = true`, `clear_on_build = false`, `collapse = true`, `error_pause = true`
(`editor prefs pac-man --json` confirms; other sections untouched). `editor play-step pac-man --frames 60 --json`
→ ok, `paused_on_error: false`, 309 entities, 742 draw cmds (434 quads · 306 discs · 1 sprite · 0 text).
In the editor, three Doctor runs collapse the 4 timeline-binding warnings to 4 rows with ×3 badges (12 messages);
Clear ▾ offers Clear on Play ✓ / Clear on Build; Play clears the Console (Clear on Play). Known gap: with Collapse on,
the level tag (`warn`) overlaps the start of the line text. Workspace tests 478/478.
Update (main `3721a39`, #66 → `17b2185`, #67): the Collapse tag/text overlap is fixed in #66, and #67 fixes the row alignment: the message now starts right after the `warn` tag on the same line (left- and top-aligned, not centered), with ×3 badges right-aligned, Collapse on or off. Tests 483/483.

## 2026-10-22 — Hierarchy eye / hand (Scene visibility + pickability, #68) pairing

On main `88398e7`. The walls are the single `Maze` Tilemap entity (no children), so
`editor set-visibility pac-man --entity Maze --pickable off --recursive on` locks them, and
`editor set-visibility pac-man --entity Ghost{Blinky,Pinky,Inky,Clyde} --hidden on` hides all four ghosts
in the Scene view only. `editor prefs pac-man --json` → `scene_visibility.enabled = true`, scene
`scenes/main.scene.json`, `hidden = [GhostBlinky, GhostClyde, GhostInky, GhostPinky]`, `unpickable = [Maze]`.
`editor play-step pac-man --frames 60 --json` is unchanged: ok, 309 entities, 742 draw cmds
(434 quads · 306 discs · 1 sprite · 0 text), so hiding doesn't leak into play. In the editor the Hierarchy
shows a slashed hand on Maze and slashed eyes (dimmed names) on the ghosts, the Scene view's ghost house is
empty, and the Game view (Stats: 309 entities / 742 draws) still draws all four ghosts. Clicking a wall in
the Scene view selects nothing; clicking Pac-Man selects Player; clicking where the hidden ghosts sit selects
nothing. Workspace tests 492/492.
