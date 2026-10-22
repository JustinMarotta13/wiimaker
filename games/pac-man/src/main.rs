//! pac-man — sample of what wiimaker can author today.
//!
//! Host: `cargo run -p pac-man` or `wiimaker run pac-man`
//! Edit: `wiimaker edit pac-man` (this crate exports no cdylib, so editor Play uses the WASD / `Player` fallback, not this App)
//!
//! See README.md for what is real vs faked.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use wiimaker_assets::{AnimClipCatalog, AnimatorControllerCatalog, SpriteCatalog, TimelineCatalog};
use wiimaker_core::app::{App, FrameCtx};
use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::DrawList;
use wiimaker_core::input::{Button, Input};
use wiimaker_core::math::Vec3;
use wiimaker_core::tilemap::tile_solid;
use wiimaker_core::time::Clock;
use wiimaker_core::world::{EntityId, World};
use wiimaker_host::{
    flush_with_atlas, load_atlas_for_project, run_with_atlas, Framebuffer, HostAudio, TextureAtlas,
};
use wiimaker_scene::{
    animate_world, hydrate_lenient_with_all_catalogs, load_project, load_scene, render_world,
    GameProject, TextureMap,
};

const CELL: f32 = 20.0;
const COLS: i32 = 32;
const ROWS: i32 = 24;
const OX: f32 = 0.0;
const OY: f32 = 0.0;

const TAG_WALL: u32 = 1;
const TAG_DOT: u32 = 2;
const TAG_POWER: u32 = 3;
const TAG_GHOST: u32 = 4;

const PLAYER_SPEED: f32 = 6.0; // cells / sec
const GHOST_SPEED: f32 = 4.5;
const FRIGHT_SECS: f32 = 6.0;
/// Player speed multiplier while ghosts are frightened (drives blend `Speed` → 1).
const FRIGHT_BOOST: f32 = 1.2;
/// Blend param smoothing rate (per second) so `Speed` ramps instead of snapping.
const BLEND_RATE: f32 = 4.0;
/// Override layer (engine 2026-10-09, PR #56) in `player.controller.json`: `Power` plays the
/// `power` clip while moving and frightened; weight 0 falls back to the Base machine.
const POWER_LAYER: &str = "Power";
const GHOST_NAMES: [&str; 4] = ["GhostBlinky", "GhostPinky", "GhostInky", "GhostClyde"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Menu,
    Play,
    Win,
}

#[derive(Clone, Copy, Default)]
struct Mover {
    col: i32,
    row: i32,
    dir: (i32, i32),
    queued: (i32, i32),
    progress: f32,
}

struct PacMan {
    title: String,
    game_dir: PathBuf,
    project: GameProject,
    world: World,
    clear: wiimaker_core::Rgba8,
    phase: Phase,
    score: u32,
    lives: u32,
    remaining: u32,
    fright: f32,
    solid: HashSet<(i32, i32)>,
    player: Mover,
    ghosts: HashMap<String, Mover>,
    ghost_spawn: HashMap<String, (i32, i32)>,
    ghost_color: HashMap<String, Rgba8>,
    player_spawn: (i32, i32),
    /// Sprite cells for `animate_world` (Idle/Chomp UV swaps).
    sprites: SpriteCatalog,
    textures: TextureMap,
    /// Drains timeline (and other) oneshots such as the menu intro beep.
    audio: HostAudio,
    /// Menu `Hint` ("press Start") appears once the intro Signal track fires `IntroDone`.
    intro_ready: bool,
    /// Last weight written to the `Power` override layer (only re-sent on change).
    power_weight: f32,
    /// Smoothed blend param `Speed` (0 normal chomp → 1 fright dash).
    blend_speed: f32,
    /// Last non-zero heading for blend param `Heading` (0 horizontal, 1 vertical).
    blend_heading: f32,
}

impl PacMan {
    fn load() -> Result<(Self, TextureAtlas), Box<dyn std::error::Error>> {
        let game_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = load_project(&game_dir)?;
        let atlas = load_atlas_for_project(&game_dir, &project)?;
        let mut game = Self {
            title: project.title.clone(),
            game_dir,
            project,
            world: World::new(),
            clear: wiimaker_core::Rgba8::new(0, 0, 0, 255),
            phase: Phase::Menu,
            score: 0,
            lives: 3,
            remaining: 0,
            fright: 0.0,
            power_weight: 0.0,
            solid: HashSet::new(),
            player: Mover::default(),
            ghosts: HashMap::new(),
            ghost_spawn: HashMap::new(),
            ghost_color: HashMap::new(),
            player_spawn: (1, 22),
            sprites: SpriteCatalog::empty(),
            textures: TextureMap::new(),
            audio: HostAudio::new(),
            intro_ready: false,
            blend_speed: 0.0,
            blend_heading: 0.0,
        };
        game.load_named("scenes/menu.scene.json", &atlas)?;
        if std::env::var("PACMAN_START_MAZE").ok().as_deref() == Some("1") {
            game.start_maze(&atlas);
            // Probe: PACMAN_FRIGHT=1 starts frightened so the blend `Speed` ramps toward dash.
            if std::env::var("PACMAN_FRIGHT").ok().as_deref() == Some("1") {
                game.fright = FRIGHT_SECS;
            }
        }
        Ok((game, atlas))
    }

    fn load_named(
        &mut self,
        rel: &str,
        atlas: &TextureAtlas,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let scene = load_scene(&self.game_dir.join(rel))?;
        let assets = self.project.assets_path(&self.game_dir);
        let catalog = SpriteCatalog::load_dir(&assets, |stem| atlas.size_of(stem))?;
        let anims = AnimClipCatalog::load_dir(&assets).unwrap_or_else(|_| AnimClipCatalog::empty());
        let controllers = AnimatorControllerCatalog::load_dir(&assets)
            .unwrap_or_else(|_| AnimatorControllerCatalog::empty());
        let timelines =
            TimelineCatalog::load_dir(&assets).unwrap_or_else(|_| TimelineCatalog::empty());
        // `hydrate_with_all_catalogs` still passes timelines as None; directors need the catalog.
        // Lenient: a missing texture (e.g. the uncommitted `basic_space_suit` stand-in sheet)
        // skips that Sprite instead of failing the load, same as the editor preview.
        self.world = hydrate_lenient_with_all_catalogs(
            &scene,
            atlas.map(),
            Some(&catalog),
            Some(&anims),
            Some(&controllers),
            None,
            Some(&timelines),
        );
        self.sprites = catalog;
        self.textures = atlas.map().clone();
        self.clear = scene.clear_rgba();
        self.rebuild_from_world();
        println!("loaded {rel}  score={} lives={}", self.score, self.lives);
        Ok(())
    }

    fn rebuild_from_world(&mut self) {
        self.solid.clear();
        self.ghosts.clear();
        self.ghost_spawn.clear();
        self.ghost_color.clear();
        self.remaining = 0;
        let ids: Vec<EntityId> = self.world.iter_entities().collect();
        for id in ids {
            let tag = self.world.tag(id).unwrap_or(0);
            let Some(xf) = self.world.transform(id) else {
                continue;
            };
            let (c, r) = cell_of(xf.translation.x, xf.translation.y);
            match tag {
                TAG_WALL => {
                    self.solid.insert((c, r));
                }
                TAG_DOT | TAG_POWER => {
                    self.remaining += 1;
                }
                TAG_GHOST => {
                    if let Some(name) = self.world.name(id).map(|s| s.to_string()) {
                        let m = Mover {
                            col: c,
                            row: r,
                            dir: (0, -1),
                            queued: (0, 0),
                            progress: 0.0,
                        };
                        self.ghosts.insert(name.clone(), m);
                        self.ghost_spawn.insert(name.clone(), (c, r));
                        if let Some(d) = self.world.disc(id) {
                            self.ghost_color.insert(name, d.color);
                        }
                    }
                }
                _ => {}
            }
            if self.world.name(id) == Some("Player") {
                self.player = Mover {
                    col: c,
                    row: r,
                    dir: (0, 0),
                    queued: (0, 0),
                    progress: 0.0,
                };
                self.player_spawn = (c, r);
            }
        }
        for row in 0..ROWS {
            for col in 0..COLS {
                if tile_solid(&self.world, col, row) {
                    self.solid.insert((col, row));
                }
            }
        }
    }

    fn start_maze(&mut self, atlas: &TextureAtlas) {
        self.intro_ready = false;
        self.score = 0;
        self.lives = 3;
        self.fright = 0.0;
        self.power_weight = 0.0; // fresh scene → Power layer back to its file weight (0)
        if let Err(e) = self.load_named("scenes/main.scene.json", atlas) {
            eprintln!("failed to load maze: {e}");
        }
        self.phase = Phase::Play;
    }

    fn apply_mover(&mut self, id: EntityId, m: &Mover) {
        if let Some(xf) = self.world.transform_mut(id) {
            let (x, y) = mover_xy(m);
            xf.translation = Vec3::new(x, y, 0.0);
        }
    }
}

fn cell_center(col: i32, row: i32) -> (f32, f32) {
    (
        OX + col as f32 * CELL + CELL * 0.5,
        OY + row as f32 * CELL + CELL * 0.5,
    )
}

fn cell_of(x: f32, y: f32) -> (i32, i32) {
    let c = ((x - OX) / CELL).floor() as i32;
    let r = ((y - OY) / CELL).floor() as i32;
    (c, r)
}

fn mover_xy(m: &Mover) -> (f32, f32) {
    let (x0, y0) = cell_center(m.col, m.row);
    (
        x0 + m.dir.0 as f32 * m.progress * CELL,
        y0 + m.dir.1 as f32 * m.progress * CELL,
    )
}

fn in_bounds(c: i32, r: i32) -> bool {
    c >= 0 && r >= 0 && c < COLS && r < ROWS
}

fn legal(solid: &HashSet<(i32, i32)>, c: i32, r: i32) -> bool {
    in_bounds(c, r) && !solid.contains(&(c, r))
}

fn cardinal(ctx: &FrameCtx<'_>) -> (i32, i32) {
    let stick = ctx.input.main.deadzone(0.45);
    if stick.x.abs() > stick.y.abs() && stick.x.abs() > 0.0 {
        (if stick.x > 0.0 { 1 } else { -1 }, 0)
    } else if stick.y.abs() > 0.0 {
        // stick +y is up; screen +y is down
        (0, if stick.y > 0.0 { -1 } else { 1 })
    } else {
        (0, 0)
    }
}

fn tick_mover(m: &mut Mover, speed: f32, dt: f32, solid: &HashSet<(i32, i32)>) {
    if m.queued == (-m.dir.0, -m.dir.1) && (m.dir.0 != 0 || m.dir.1 != 0) {
        m.col += m.dir.0;
        m.row += m.dir.1;
        m.progress = (1.0 - m.progress).clamp(0.0, 1.0);
        m.dir = m.queued;
    }

    if m.progress <= 0.0001 {
        m.progress = 0.0;
        if m.queued != (0, 0) && legal(solid, m.col + m.queued.0, m.row + m.queued.1) {
            m.dir = m.queued;
        } else if !legal(solid, m.col + m.dir.0, m.row + m.dir.1) {
            m.dir = (0, 0);
        }
    }

    if m.dir == (0, 0) {
        return;
    }
    m.progress += speed * dt;
    if m.progress >= 1.0 {
        m.col += m.dir.0;
        m.row += m.dir.1;
        m.progress = 0.0;
        if m.queued != (0, 0) && legal(solid, m.col + m.queued.0, m.row + m.queued.1) {
            m.dir = m.queued;
        } else if !legal(solid, m.col + m.dir.0, m.row + m.dir.1) {
            m.dir = (0, 0);
        }
    }
}

fn ghost_think(m: &mut Mover, solid: &HashSet<(i32, i32)>, prefer_reverse: bool) {
    if m.progress > 0.0001 {
        return;
    }
    let options = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut legal_dirs: Vec<(i32, i32)> = options
        .into_iter()
        .filter(|d| legal(solid, m.col + d.0, m.row + d.1))
        .collect();
    if legal_dirs.is_empty() {
        m.dir = (0, 0);
        return;
    }
    let reverse = (-m.dir.0, -m.dir.1);
    if !prefer_reverse && legal_dirs.len() > 1 {
        legal_dirs.retain(|d| *d != reverse);
    }
    if prefer_reverse && legal_dirs.contains(&reverse) && legal_dirs.len() > 1 {
        // still allow random, but bias reverse away from player later
    }
    // Deterministic-ish pick: prefer current dir, else first legal.
    if legal_dirs.contains(&m.dir) && m.dir != (0, 0) && !prefer_reverse {
        m.queued = m.dir;
        return;
    }
    let idx = ((m.col * 13 + m.row * 7 + m.dir.0 * 3 + 5).unsigned_abs() as usize)
        % legal_dirs.len();
    m.queued = legal_dirs[idx];
}

impl App for PacMan {
    fn title(&self) -> &str {
        &self.title
    }

    fn update(&mut self, ctx: &FrameCtx<'_>) {
        match self.phase {
            Phase::Menu => {
                if ctx.input.pressed(Button::Start) || ctx.input.pressed(Button::A) {
                    // Atlas lives in host; re-cook from disk if needed.
                    let atlas = load_atlas_for_project(&self.game_dir, &self.project)
                        .unwrap_or_else(|_| TextureAtlas::empty());
                    self.start_maze(&atlas);
                }
            }
            Phase::Win => {
                if ctx.input.pressed(Button::Start) || ctx.input.pressed(Button::A) {
                    let atlas = load_atlas_for_project(&self.game_dir, &self.project)
                        .unwrap_or_else(|_| TextureAtlas::empty());
                    self.start_maze(&atlas);
                }
            }
            Phase::Play => {
                self.tick_play(ctx);
            }
        }
        self.sync_player_animator(ctx.clock.dt);
        // animate_world ticks animators, then PlayableDirectors (`tick_timelines`).
        animate_world(&mut self.world, &self.sprites, &self.textures, ctx.clock.dt);
        // Timeline Signal track (engine 2026-10-06): the menu intro fires `IntroDone`
        // at 2.5s; only then does the "press Start" Hint appear.
        for sig in self.world.take_timeline_signals() {
            println!(
                "timeline signal {} payload={:?} from {}/{}",
                sig.signal, sig.payload, sig.director, sig.timeline
            );
            if sig.signal == "IntroDone" {
                self.intro_ready = true;
            }
        }
        if self.phase == Phase::Menu {
            if let Some(id) = self.world.find_by_name("Hint") {
                self.world.set_active(id, self.intro_ready);
            }
        }
        let assets = self.project.assets_path(&self.game_dir);
        let _ = self.audio.play_world(&mut self.world, &assets);
    }

    fn render(&mut self, _ctx: &FrameCtx<'_>, draw: &mut DrawList) {
        render_world(&self.world, draw, self.clear);
        if self.phase == Phase::Play && self.fright > 0.0 {
            draw.disc(
                wiimaker_core::math::Vec2::new(24.0, 24.0),
                6.0,
                Rgba8::new(80, 80, 255, 255),
                3.0,
            );
        }
    }
}

impl PacMan {
    /// Bool `Moving` drives Idle→Chomp when the player has a direction or is mid-cell.
    /// Blend trees (engine 2026-10-08): `Chomp` is a 1D tree on Float `Speed` (chomp@0 → dash@1)
    /// in `player.controller.json`; `player-2d.controller.json` blends `Speed` × `Heading`
    /// (0 horizontal, 1 vertical) across chomp / chomp-v / dash / dash-v.
    fn sync_player_animator(&mut self, dt: f32) {
        let Some(id) = self.world.find_by_name("Player") else {
            return;
        };
        let moving = self.player.dir != (0, 0) || self.player.progress > 0.0;
        self.world.set_animator_bool(id, "Moving", moving);
        let target = if moving && self.fright > 0.0 { 1.0 } else { 0.0 };
        let k = (BLEND_RATE * dt).clamp(0.0, 1.0);
        self.blend_speed += (target - self.blend_speed) * k;
        if self.player.dir.1 != 0 {
            self.blend_heading = 1.0;
        } else if self.player.dir.0 != 0 {
            self.blend_heading = 0.0;
        }
        self.world.set_animator_float(id, "Speed", self.blend_speed);
        self.world.set_animator_float(id, "Heading", self.blend_heading);
        // Power override: weight 1 while frightened; in the last 2s it hands playback back to
        // Base Chomp in 0.5s beats (2026-10-16, was 8 Hz with the ghost flash) so each
        // Base<->Power winner switch restarts the clip cleanly instead of strobing mid-frame.
        let flash_off = self.fright < 2.0 && ((self.fright * 2.0) as i32) % 2 == 0;
        let power = if self.fright > 0.0 && !flash_off { 1.0 } else { 0.0 };
        if power != self.power_weight {
            self.power_weight = power;
            self.world.set_animator_layer_weight(id, POWER_LAYER, power);
            if let Some(a) = self.world.animator(id) {
                let win = a.winning_layer();
                println!(
                    "animator layer {POWER_LAYER} weight={power:.0} -> playback {}:{} clip={} fright={:.2}",
                    a.layer_name(win).unwrap_or("?"),
                    a.layer_state_name(win).unwrap_or("?"),
                    self.world.animation(id).map(|c| c.clip.as_str()).unwrap_or("?"),
                    self.fright
                );
            }
        }
    }

    fn tick_play(&mut self, ctx: &FrameCtx<'_>) {
        let card = cardinal(ctx);
        if card != (0, 0) {
            self.player.queued = card;
        } else if std::env::var("PACMAN_AUTOPILOT").ok().as_deref() == Some("1")
            && self.player.dir == (0, 0)
            && self.player.progress == 0.0
        {
            // Probe (screenshots without keyboard): when stopped, try the next open direction.
            for d in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                if d != self.player.queued
                    && !self.solid.contains(&(self.player.col + d.0, self.player.row + d.1))
                {
                    self.player.queued = d;
                    break;
                }
            }
        }

        let player_speed = if self.fright > 0.0 {
            PLAYER_SPEED * FRIGHT_BOOST
        } else {
            PLAYER_SPEED
        };
        tick_mover(&mut self.player, player_speed, ctx.clock.dt, &self.solid);
        if let Some(id) = self.world.find_by_name("Player") {
            let m = self.player;
            self.apply_mover(id, &m);
        }

        self.eat_at_player();

        self.fright = (self.fright - ctx.clock.dt).max(0.0);
        let frightened = self.fright > 0.0;
        let solid = self.solid.clone();
        let names: Vec<String> = GHOST_NAMES.iter().map(|s| s.to_string()).collect();
        for name in names {
            let Some(mut m) = self.ghosts.get(&name).copied() else {
                continue;
            };
            ghost_think(&mut m, &solid, frightened);
            let speed = if frightened {
                GHOST_SPEED * 0.6
            } else {
                GHOST_SPEED
            };
            tick_mover(&mut m, speed, ctx.clock.dt, &solid);
            if let Some(id) = self.world.find_by_name(&name) {
                self.apply_mover(id, &m);
                if let Some(d) = self.world.disc_mut(id) {
                    if frightened {
                        let flash = ((self.fright * 8.0) as i32) % 2 == 0 && self.fright < 2.0;
                        d.color = if flash {
                            Rgba8::new(200, 200, 255, 255)
                        } else {
                            Rgba8::new(48, 48, 200, 255)
                        };
                    } else if let Some(c) = self.ghost_color.get(&name) {
                        d.color = *c;
                    }
                }
            }
            self.ghosts.insert(name, m);
        }

        self.resolve_ghost_touch();

        if self.remaining == 0 && self.phase == Phase::Play {
            let atlas = load_atlas_for_project(&self.game_dir, &self.project)
                .unwrap_or_else(|_| TextureAtlas::empty());
            self.phase = Phase::Win;
            let _ = self.load_named("scenes/win.scene.json", &atlas);
            println!("YOU WIN  score={}", self.score);
        }
    }

    fn eat_at_player(&mut self) {
        if self.player.progress > 0.35 {
            return;
        }
        let (pc, pr) = (self.player.col, self.player.row);
        let ids: Vec<EntityId> = self.world.iter_entities().collect();
        for id in ids {
            let tag = self.world.tag(id).unwrap_or(0);
            if tag != TAG_DOT && tag != TAG_POWER {
                continue;
            }
            let Some(xf) = self.world.transform(id) else {
                continue;
            };
            let (c, r) = cell_of(xf.translation.x, xf.translation.y);
            if c == pc && r == pr {
                if tag == TAG_POWER {
                    self.score += 50;
                    self.fright = FRIGHT_SECS;
                    println!("power pellet  fright on  score={}", self.score);
                } else {
                    self.score += 10;
                }
                self.world.despawn(id);
                self.remaining = self.remaining.saturating_sub(1);
            }
        }
    }

    fn resolve_ghost_touch(&mut self) {
        if self.player.progress > 0.4 {
            return;
        }
        let (pc, pr) = (self.player.col, self.player.row);
        let names: Vec<String> = GHOST_NAMES.iter().map(|s| s.to_string()).collect();
        for name in names {
            let Some(m) = self.ghosts.get(&name).copied() else {
                continue;
            };
            if m.progress > 0.4 {
                continue;
            }
            if m.col != pc || m.row != pr {
                continue;
            }
            if self.fright > 0.0 {
                self.score += 200;
                println!("ate {name}  score={}", self.score);
                if let Some(spawn) = self.ghost_spawn.get(&name).copied() {
                    let nm = Mover {
                        col: spawn.0,
                        row: spawn.1,
                        dir: (0, -1),
                        queued: (0, 0),
                        progress: 0.0,
                    };
                    if let Some(id) = self.world.find_by_name(&name) {
                        self.apply_mover(id, &nm);
                    }
                    self.ghosts.insert(name, nm);
                }
            } else {
                self.hit_player();
                break;
            }
        }
    }

    fn hit_player(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        println!("ouch  lives={}", self.lives);
        let spawn = self.player_spawn;
        self.player = Mover {
            col: spawn.0,
            row: spawn.1,
            dir: (0, 0),
            queued: (0, 0),
            progress: 0.0,
        };
        if let Some(id) = self.world.find_by_name("Player") {
            let m = self.player;
            self.apply_mover(id, &m);
        }
        if self.lives == 0 {
            println!("game over  score={}", self.score);
            let atlas = load_atlas_for_project(&self.game_dir, &self.project)
                .unwrap_or_else(|_| TextureAtlas::empty());
            self.phase = Phase::Menu;
            let _ = self.load_named("scenes/menu.scene.json", &atlas);
        }
    }
}

fn write_ppm(path: &str, fb: &Framebuffer) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    write!(f, "P6\n{} {}\n255\n", fb.width, fb.height)?;
    let mut rgb = Vec::with_capacity(fb.width * fb.height * 3);
    for p in &fb.pixels {
        rgb.push(((p >> 16) & 0xff) as u8);
        rgb.push(((p >> 8) & 0xff) as u8);
        rgb.push((p & 0xff) as u8);
    }
    f.write_all(&rgb)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut game, atlas) = PacMan::load()?;
    if let Ok(path) = std::env::var("PACMAN_DUMP_FRAME") {
        let mut input = Input::new();
        input.main.x = 1.0;
        let clock = Clock::new(60.0);
        let ctx = FrameCtx {
            input: &input,
            clock: &clock,
            framebuffer_w: 640,
            framebuffer_h: 480,
        };
        // Frames into the menu intro (default ~0.33s; PACMAN_DUMP_FRAMES=170 is past IntroDone).
        let frames: usize = std::env::var("PACMAN_DUMP_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        // Probe: PACMAN_DUMP_TURN=N steers vertically (PACMAN_DUMP_Y, default 1 = up) after
        // N frames so the 2D blend `Heading` flips to 1.
        let turn: Option<usize> = std::env::var("PACMAN_DUMP_TURN").ok().and_then(|v| v.parse().ok());
        let mut turned = Input::new();
        turned.main.y = std::env::var("PACMAN_DUMP_Y")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        let turned_ctx = FrameCtx {
            input: &turned,
            clock: &clock,
            framebuffer_w: 640,
            framebuffer_h: 480,
        };
        for i in 0..frames {
            if turn.is_some_and(|t| i >= t) {
                game.update(&turned_ctx);
            } else {
                game.update(&ctx);
            }
        }
        let mut draw = DrawList::new();
        game.render(&ctx, &mut draw);
        let mut fb = Framebuffer::new(640, 480);
        flush_with_atlas(&draw, &mut fb, Some(&atlas));
        write_ppm(&path, &fb)?;
        if let Some(id) = game.world.find_by_name("Player") {
            let state = game.world.animator(id).map(|a| a.state.as_str()).unwrap_or("?");
            let clip = game.world.animation(id).map(|a| a.clip.as_str()).unwrap_or("?");
            println!("dump {path} animator={state} clip={clip}");
            if let Some(a) = game.world.animator(id) {
                for i in 0..a.layer_count() {
                    println!(
                        "layer {} weight={:.2} state={}",
                        a.layer_name(i).unwrap_or("?"),
                        a.layer_weight(i).unwrap_or(0.0),
                        a.layer_state_name(i).unwrap_or("?")
                    );
                }
                println!(
                    "playback layer={} fright={:.2}",
                    a.layer_name(a.winning_layer()).unwrap_or("?"),
                    game.fright
                );
            }
            if let Some(b) = game.world.animator(id).and_then(|a| a.current_blend()) {
                let active = b.active_motion().map(|m| m.clip.as_str()).unwrap_or("?");
                println!(
                    "blend params={:?} active={active} weights={:?} speed={:.2} heading={:.0}",
                    b.params, b.weights, game.blend_speed, game.blend_heading
                );
            }
        }
        if let Some(id) = game.world.find_by_name("Director") {
            if let Some(d) = game.world.director(id) {
                println!(
                    "intro timeline={} t={:.3} playing={} finished={}",
                    d.timeline, d.time, d.playing, d.finished
                );
            }
        }
        if let Some(id) = game.world.find_by_name("IntroGhost") {
            println!("intro ghost active={}", game.world.is_active(id));
        }
        if let Some(id) = game.world.find_by_name("Hint") {
            println!("hint active={} intro_ready={}", game.world.is_active(id), game.intro_ready);
        }
        if let Some(id) = game.world.find_by_name("MainCamera") {
            if let Some(xf) = game.world.transform(id) {
                println!(
                    "camera xy={:.1},{:.1}",
                    xf.translation.x, xf.translation.y
                );
            }
        }
        return Ok(());
    }
    println!("pac-man  arrows/WASD move · Enter/Z/Space start · Esc quit");
    run_with_atlas(game, atlas)
}
