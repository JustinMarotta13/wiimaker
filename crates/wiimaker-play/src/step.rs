//! Editor Step transport and the headless `editor play-step` twin.
//!
//! Both call [`PlaySession::step_one`]: one 60 Hz `App::update` or the WASD
//! fallback. The transport plan is what the toolbar button does around that tick.

use std::path::Path;

use anyhow::{bail, Result};
use wiimaker_assets::{AnimClipCatalog, SpriteCatalog};
use wiimaker_core::draw::DrawStats;
use wiimaker_core::input::Input;
use wiimaker_core::world::World;
use wiimaker_scene::{load_project, load_scene, render_world, texture_map_from_wpack};

use crate::scene_tick::hydrate_play_world;
use crate::session::{PlayKind, PlaySession, PlayStart};

/// Toolbar transport. `Stopped` is editor Edit mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Stopped,
    Playing,
    Paused,
}

/// What the Step button does from `mode`.
///
/// Every arm advances exactly one 60 Hz frame and ends [`Transport::Paused`].
/// From Playing, the editor applies it on the next host frame so the in-flight
/// tick is the last full update and Step adds one more frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepPlan {
    pub enter_play: bool,
    pub defer_to_next_frame: bool,
    pub frames: u32,
    pub end: Transport,
}

pub fn plan_step(mode: Transport) -> StepPlan {
    StepPlan {
        enter_play: mode == Transport::Stopped,
        defer_to_next_frame: mode == Transport::Playing,
        frames: 1,
        end: Transport::Paused,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayStepEntity {
    pub name: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayStepReport {
    /// `plugin` or `fallback` — the same choice editor Play makes.
    pub backend: &'static str,
    pub frames: u32,
    pub entities: usize,
    pub draw: DrawStats,
    pub positions: Vec<PlayStepEntity>,
    /// Why this backend ran (plugin load log, or the fallback reason).
    pub note: String,
}

fn backend_name(kind: PlayKind) -> &'static str {
    match kind {
        PlayKind::Plugin => "plugin",
        PlayKind::Fallback => "fallback",
    }
}

fn positions_of(world: &World) -> Vec<PlayStepEntity> {
    let mut out = Vec::new();
    for id in world.iter_entities() {
        let Some(xf) = world.transform(id) else {
            continue;
        };
        out.push(PlayStepEntity {
            name: world.name(id).unwrap_or("").to_string(),
            x: xf.translation.x,
            y: xf.translation.y,
        });
    }
    out
}

/// Load the default scene and advance `frames` through [`PlaySession::step_one`].
///
/// `opts.build_if_needed` should stay false for the CLI so a headless step
/// does not recurse into `cargo`. A missing or unloadable `cdylib` uses the
/// same WASD fallback as editor Play; `backend` says which one ran.
pub fn headless_play_steps(
    opts: PlayStart<'_>,
    frames: u32,
    input: &Input,
) -> Result<PlayStepReport> {
    if frames == 0 {
        bail!("--frames must be > 0");
    }
    if frames > 10_000 {
        bail!("--frames must be <= 10000");
    }
    let scene_json = opts.scene_json;
    let mut session = PlaySession::start(opts);
    let note = session.take_logs().join(" · ");
    let project = load_project(opts.game_dir)?;
    let scene_path = project.scene_path(opts.game_dir);
    let scene = load_scene(&scene_path)?;
    let clear = scene.clear_rgba();

    let mut fallback_world = World::new();
    if !session.is_plugin() {
        let assets = project.assets_path(opts.game_dir);
        let catalog =
            SpriteCatalog::load_dir(&assets, |_| None).unwrap_or_else(|_| SpriteCatalog::empty());
        let anims = AnimClipCatalog::load_dir(&assets).unwrap_or_else(|_| AnimClipCatalog::empty());
        let textures = texture_map_from_wpack(opts.game_dir, &project)?;
        hydrate_play_world(
            &mut fallback_world,
            opts.game_dir,
            scene_json,
            &textures,
            &catalog,
            &anims,
        )?;
        fallback_world.queue_awake_audio();
        for _ in 0..frames {
            session.step_one(
                input,
                Some(&mut fallback_world),
                Some(&catalog),
                Some(&textures),
                640,
                480,
            )?;
        }
    } else {
        for _ in 0..frames {
            session.step_one(input, None, None, None, 640, 480)?;
        }
    }

    let world = session.world().unwrap_or(&fallback_world);
    let mut draw = wiimaker_core::draw::DrawList::new();
    render_world(world, &mut draw, clear);
    let positions = positions_of(world);
    let entities = positions.len();
    Ok(PlayStepReport {
        backend: backend_name(session.kind()),
        frames: session.stepped(),
        entities,
        draw: draw.stats(),
        positions,
        note,
    })
}

/// Workspace root is unused when `force_fallback` is set; tests pass `game_dir`.
pub fn headless_fallback_steps(
    game_dir: &Path,
    frames: u32,
    input: &Input,
) -> Result<PlayStepReport> {
    let project = load_project(game_dir)?;
    headless_play_steps(
        PlayStart {
            workspace: game_dir,
            package: &project.name,
            game_dir,
            scene_json: None,
            force_fallback: true,
            build_if_needed: false,
        },
        frames,
        input,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use wiimaker_core::input::Input;
    use wiimaker_scene::{save_project, save_scene, GameProject, Scene};

    fn apply(mode: &mut Transport, frames: &mut u32) {
        let plan = plan_step(*mode);
        assert_eq!(plan.frames, 1);
        assert_eq!(plan.end, Transport::Paused);
        *frames += plan.frames;
        *mode = plan.end;
    }

    #[test]
    fn step_state_machine_stopped_playing_paused_frame_counts() {
        let mut frames = 0u32;

        let mut mode = Transport::Stopped;
        let plan = plan_step(mode);
        assert!(plan.enter_play);
        assert!(!plan.defer_to_next_frame);
        apply(&mut mode, &mut frames);
        assert_eq!(mode, Transport::Paused);
        assert_eq!(frames, 1);

        let plan = plan_step(mode);
        assert!(!plan.enter_play);
        assert!(!plan.defer_to_next_frame);
        apply(&mut mode, &mut frames);
        assert_eq!(mode, Transport::Paused);
        assert_eq!(frames, 2);

        mode = Transport::Playing;
        let plan = plan_step(mode);
        assert!(!plan.enter_play);
        assert!(plan.defer_to_next_frame);
        assert_eq!(plan.frames, 1);
        apply(&mut mode, &mut frames);
        assert_eq!(mode, Transport::Paused);
        assert_eq!(frames, 3);
    }

    #[test]
    fn headless_fallback_steps_default_scene() {
        let dir = std::env::temp_dir().join(format!(
            "wiimaker-play-step-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        let mut project = GameProject::new("step-probe");
        project.default_scene = "scenes/main.scene.json".into();
        save_project(&dir, &project).unwrap();
        let mut scene = Scene::new("main");
        wiimaker_scene::add_entity(
            &mut scene,
            "Player",
            &wiimaker_scene::MutateOpts {
                x: Some(40.0),
                y: Some(80.0),
                ..Default::default()
            },
        )
        .unwrap();
        save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

        let mut input = Input::new();
        input.begin_frame();
        input.main.x = 1.0;
        let report = headless_fallback_steps(&dir, 3, &input).unwrap();
        assert_eq!(report.backend, "fallback");
        assert_eq!(report.frames, 3);
        assert_eq!(report.entities, 1);
        assert!(report.draw.commands >= 1);
        let player = report
            .positions
            .iter()
            .find(|e| e.name == "Player")
            .unwrap();
        let step = crate::PLAYER_WASD_SPEED / 60.0;
        assert!(
            (player.x - (40.0 + step * 3.0)).abs() < 1e-2,
            "x={}",
            player.x
        );
        assert!((player.y - 80.0).abs() < 1e-3);
        assert!(headless_fallback_steps(&dir, 0, &input).is_err());
    }

    #[test]
    fn headless_fallback_steps_sprite_missing_from_wpack() {
        let dir = std::env::temp_dir().join(format!(
            "wiimaker-play-step-sprite-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        let mut project = GameProject::new("step-sprite-miss");
        project.default_scene = "scenes/main.scene.json".into();
        save_project(&dir, &project).unwrap();
        let mut scene = Scene::new("main");
        wiimaker_scene::add_entity(
            &mut scene,
            "Player",
            &wiimaker_scene::MutateOpts {
                x: Some(12.0),
                y: Some(34.0),
                ..Default::default()
            },
        )
        .unwrap();
        wiimaker_scene::add_component_sprite(
            &mut scene,
            "Player",
            "basic_space_suit",
            [32.0, 32.0],
        )
        .unwrap();
        save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

        let input = Input::new();
        let report = headless_fallback_steps(&dir, 1, &input).unwrap();
        assert_eq!(report.backend, "fallback");
        assert_eq!(report.frames, 1);
        assert_eq!(report.entities, 1);
        assert!(report.draw.commands >= 1);
        assert_eq!(report.draw.sprites, 0);
    }

    #[test]
    fn headless_fallback_steps_loads_sprite_from_wpack() {
        use wiimaker_assets::WPack;

        let dir = std::env::temp_dir().join(format!(
            "wiimaker-play-step-wpack-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        let mut project = GameProject::new("step-wpack");
        project.default_scene = "scenes/main.scene.json".into();
        save_project(&dir, &project).unwrap();
        let mut scene = Scene::new("main");
        wiimaker_scene::add_entity(
            &mut scene,
            "Hero",
            &wiimaker_scene::MutateOpts {
                x: Some(50.0),
                y: Some(60.0),
                ..Default::default()
            },
        )
        .unwrap();
        wiimaker_scene::add_component_sprite(&mut scene, "Hero", "hero_tex", [16.0, 16.0])
            .unwrap();
        save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

        let mut pack = WPack::new();
        pack.textures.push(wiimaker_assets::PackedTexture {
            name: "hero_tex".into(),
            width: 8,
            height: 8,
            rgba16: vec![0; 128],
        });
        pack.write_to(&project.wpack_path(&dir)).unwrap();

        let input = Input::new();
        let report = headless_fallback_steps(&dir, 1, &input).unwrap();
        assert_eq!(report.backend, "fallback");
        assert_eq!(report.frames, 1);
        assert_eq!(report.entities, 1);
        assert!(report.draw.sprites >= 1, "sprites={}", report.draw.sprites);
    }
}
