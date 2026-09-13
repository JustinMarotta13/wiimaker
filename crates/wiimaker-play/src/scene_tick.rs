//! Default scene-game tick (template `App` + editor WASD fallback).

use std::path::Path;

use anyhow::Result;
use wiimaker_assets::{AnimClipCatalog, SpriteCatalog};
use wiimaker_core::collider::move_and_collide;
use wiimaker_core::color::Rgba8;
use wiimaker_core::input::Input;
use wiimaker_core::math::Vec2;
use wiimaker_core::world::{World, SCREEN_H, SCREEN_W};
use wiimaker_scene::{
    animate_world, hydrate_lenient_with_sorting_layers, load_project, load_scene_into_world, Scene,
    TextureMap,
};

/// Pixels per second for free (non-GridMover) `Player` WASD.
pub const PLAYER_WASD_SPEED: f32 = 220.0;

#[derive(Clone, Copy, Debug)]
pub struct SceneTickOpts {
    /// Keep hello-orb `OrbShadow` glued to `Player` (harmless if missing).
    pub sync_orb_shadow: bool,
}

impl Default for SceneTickOpts {
    fn default() -> Self {
        Self {
            sync_orb_shadow: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FallbackTickResult {
    /// Solid collider hit by free WASD `Player` this tick (name).
    pub hit_name: Option<String>,
}

/// Animate, grid-move, optional free WASD on `Player`, cameras.
pub fn tick_scene_systems(
    world: &mut World,
    input: &Input,
    dt: f32,
    catalog: Option<&SpriteCatalog>,
    textures: Option<&TextureMap>,
    opts: SceneTickOpts,
) -> FallbackTickResult {
    if let (Some(catalog), Some(textures)) = (catalog, textures) {
        animate_world(world, catalog, textures, dt);
    }
    world.step_grid_movers(input, dt);

    let mut result = FallbackTickResult::default();
    let player_uses_grid = world
        .find_by_name("Player")
        .and_then(|id| world.grid_mover(id))
        .is_some();
    let world_dx = input.main.x;
    let world_dy = -input.main.y; // stick +Y = Up = −Y world
    if !player_uses_grid && (world_dx != 0.0 || world_dy != 0.0) {
        if let Some(id) = world.find_by_name("Player") {
            let speed = PLAYER_WASD_SPEED * dt;
            let hit = move_and_collide(world, id, Vec2::new(world_dx * speed, world_dy * speed));
            if let Some(hid) = hit.hit {
                result.hit_name = Some(world.name(hid).unwrap_or("?").to_string());
            }
            if world.active_camera().is_none() {
                let r = world.disc(id).map(|d| d.radius).unwrap_or(16.0);
                if let Some(xf) = world.transform_mut(id) {
                    xf.translation.x = xf.translation.x.clamp(r, SCREEN_W - r);
                    xf.translation.y = xf.translation.y.clamp(r, SCREEN_H - r);
                }
            }
        }
    }
    if opts.sync_orb_shadow {
        if let Some(id) = world.find_by_name("Player") {
            if let Some(shadow) = world.find_by_name("OrbShadow") {
                if let (Some(player), Some(sxf)) =
                    (world.transform(id).copied(), world.transform_mut(shadow))
                {
                    sxf.translation.x = player.translation.x + 4.0;
                    sxf.translation.y = player.translation.y + 6.0;
                }
            }
        }
    }
    world.follow_cameras();
    result
}

/// Hydrate `world` from unsaved scene JSON, or the project's default scene on disk.
pub fn hydrate_play_world(
    world: &mut World,
    game_dir: &Path,
    scene_json: Option<&str>,
    textures: &TextureMap,
    catalog: &SpriteCatalog,
    anims: &AnimClipCatalog,
) -> Result<Rgba8> {
    let project = load_project(game_dir)?;
    if let Some(json) = scene_json.map(str::trim).filter(|s| !s.is_empty()) {
        let scene: Scene = serde_json::from_str(json)?;
        let layers = project.effective_sorting_layers();
        *world = hydrate_lenient_with_sorting_layers(
            &scene,
            textures,
            Some(catalog),
            Some(anims),
            Some(&layers),
        );
        Ok(scene.clear_rgba())
    } else {
        Ok(load_scene_into_world(
            world,
            game_dir,
            &project,
            "",
            textures,
            Some(catalog),
            Some(anims),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_core::input::{Button, Input};
    use wiimaker_core::world::{Disc, Transform};
    use wiimaker_core::Rgba8;

    fn right_pad() -> Input {
        let mut i = Input::new();
        i.begin_frame();
        i.main.x = 1.0;
        i.set_down(Button::DPadRight, true);
        i
    }

    #[test]
    fn wasd_moves_player_not_unrelated_entity() {
        let mut world = World::new();
        let player = world.spawn_named("Player", Transform::from_xy(80.0, 80.0));
        world.set_disc(
            player,
            Some(Disc {
                radius: 8.0,
                color: Rgba8::WHITE,
                z: 0.0,
                sorting_layer: 0,
            }),
        );
        let marker = world.spawn_named("Marker", Transform::from_xy(10.0, 10.0));
        tick_scene_systems(
            &mut world,
            &right_pad(),
            0.05,
            None,
            None,
            SceneTickOpts {
                sync_orb_shadow: false,
            },
        );
        let px = world.transform(player).unwrap().translation.x;
        let mx = world.transform(marker).unwrap().translation.x;
        assert!(px > 80.0, "Player should move right, x={px}");
        assert!(
            (mx - 10.0).abs() < 1e-4,
            "Marker must stay put (fallback is Player-only)"
        );
    }

    #[test]
    fn orb_shadow_follows_player() {
        let mut world = World::new();
        world.spawn_named("Player", Transform::from_xy(100.0, 50.0));
        let sh = world.spawn_named("OrbShadow", Transform::from_xy(0.0, 0.0));
        tick_scene_systems(
            &mut world,
            &Input::new(),
            1.0 / 60.0,
            None,
            None,
            SceneTickOpts::default(),
        );
        let t = world.transform(sh).unwrap().translation;
        assert!((t.x - 104.0).abs() < 1e-3);
        assert!((t.y - 56.0).abs() < 1e-3);
    }
}
