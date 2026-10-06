//! Scene → World hydration with texture / sprite-cell name resolution.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use wiimaker_assets::{AnimClipCatalog, AnimatorControllerCatalog, SpriteCatalog, TimelineCatalog};
use wiimaker_core::animator::{
    Animator, AnimatorCondition, AnimatorParam, AnimatorParamKind, AnimatorState,
    AnimatorTransition,
};
use wiimaker_core::collider::{Collider, ColliderKind};
use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{Rect, TextureId};
use wiimaker_core::math::Vec2;
use wiimaker_core::tilemap::{TileVisual, Tilemap};
use wiimaker_core::timeline::{PlayableDirector, TimelineClipRuntime, TimelineTrackRuntime};
use wiimaker_core::world::{Animation, Camera, Disc, Follow, Sprite, World};
use wiimaker_core::AudioSource;
use wiimaker_core::GridMover;
use wiimaker_core::Text;

use crate::scene::{EntityData, Scene};

/// Maps texture asset names → [`TextureId`] (from a loaded `.wpack`).
#[derive(Clone, Debug, Default)]
pub struct TextureMap {
    by_name: HashMap<String, TextureId>,
}

impl TextureMap {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, id: TextureId) {
        self.by_name.insert(name.into(), id);
    }

    pub fn get(&self, name: &str) -> Option<TextureId> {
        self.by_name.get(name).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(|s| s.as_str())
    }

    pub fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut map = Self::new();
        for (i, name) in names.into_iter().enumerate() {
            map.insert(name.as_ref().to_string(), TextureId(i as u32));
        }
        map
    }
}

pub fn hydrate(scene: &Scene, textures: &TextureMap) -> Result<World> {
    hydrate_with_catalog(scene, textures, None)
}

pub fn hydrate_with_catalog(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> Result<World> {
    hydrate_with_catalogs(scene, textures, catalog, None)
}

pub fn hydrate_with_catalogs(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<World> {
    let mut world = World::new();
    hydrate_into_with_catalogs(&mut world, scene, textures, catalog, anims)?;
    Ok(world)
}

pub fn hydrate_with_all_catalogs(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
    controllers: Option<&AnimatorControllerCatalog>,
) -> Result<World> {
    let mut world = World::new();
    hydrate_into_with_all_catalogs(
        &mut world,
        scene,
        textures,
        catalog,
        anims,
        controllers,
        None,
    )?;
    Ok(world)
}

pub fn hydrate_into(world: &mut World, scene: &Scene, textures: &TextureMap) -> Result<()> {
    hydrate_into_with_catalog(world, scene, textures, None)
}

pub fn hydrate_into_with_catalog(
    world: &mut World,
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> Result<()> {
    hydrate_into_with_catalogs(world, scene, textures, catalog, None)
}

pub fn hydrate_into_with_catalogs(
    world: &mut World,
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    hydrate_into_with_all_catalogs(world, scene, textures, catalog, anims, None, None)
}

pub fn hydrate_into_with_all_catalogs(
    world: &mut World,
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
    controllers: Option<&AnimatorControllerCatalog>,
    timelines: Option<&TimelineCatalog>,
) -> Result<()> {
    world.clear();
    for ent in &scene.entities {
        spawn_entity(
            world,
            scene,
            ent,
            textures,
            catalog,
            anims,
            controllers,
            timelines,
        )?;
    }
    world.capture_timeline_baselines();
    Ok(())
}

/// Load a scene file into an existing [`World`], keeping the caller's texture map / catalogs.
///
/// Resolves `scene_rel` (stem or path relative to `game_dir`; empty uses `project.default_scene`),
/// then [`hydrate_into_with_catalogs`] (which clears `world`). Returns the scene clear color.
pub fn load_scene_into_world(
    world: &mut World,
    game_dir: &Path,
    project: &crate::project::GameProject,
    scene_rel: &str,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<Rgba8> {
    let key = if scene_rel.trim().is_empty() {
        project.default_scene.as_str()
    } else {
        scene_rel
    };
    let rel = crate::project::resolve_scene_rel(game_dir, key)?;
    let scene = crate::scene::load_scene(&game_dir.join(&rel))?;
    world.set_sorting_layers(project.effective_sorting_layers());
    let controllers =
        AnimatorControllerCatalog::load_dir(&project.assets_path(game_dir)).unwrap_or_default();
    let timelines = TimelineCatalog::load_dir(&project.assets_path(game_dir)).unwrap_or_default();
    hydrate_into_with_all_catalogs(
        world,
        &scene,
        textures,
        catalog,
        anims,
        Some(&controllers),
        Some(&timelines),
    )?;
    Ok(scene.clear_rgba())
}

fn spawn_entity(
    world: &mut World,
    scene: &Scene,
    ent: &EntityData,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
    controllers: Option<&AnimatorControllerCatalog>,
    timelines: Option<&TimelineCatalog>,
) -> Result<()> {
    let xf = scene
        .world_transform(&ent.name)
        .unwrap_or_else(|| ent.transform.clone());
    let id = world.spawn_named(ent.name.clone(), xf.to_runtime());
    world.set_tag(id, ent.tag);

    if let Some(sp) = &ent.components.sprite {
        if sp.enabled {
            let (tex_name, uv, pivot, size) = resolve_sprite(sp, catalog);
            let tex = textures.get(&tex_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "entity '{}': texture '{}' not found in wpack",
                    ent.name,
                    tex_name
                )
            })?;
            let mut sprite = Sprite::new(tex, size);
            sprite.uv = uv;
            sprite.pivot = pivot;
            sprite.lock_pivot = sp.pivot.is_some();
            sprite.color = sp.color_rgba();
            sprite.z = sp.z;
            sprite.sorting_layer = world.sorting_layer_index(&sp.sorting_layer);
            world.set_sprite(id, Some(sprite));
        }
    }

    if let Some(d) = &ent.components.disc {
        if d.enabled {
            let mut disc = Disc::new(d.radius, d.color_rgba());
            disc.z = d.z;
            disc.sorting_layer = world.sorting_layer_index(&d.sorting_layer);
            world.set_disc(id, Some(disc));
        }
    }

    if let Some(cam) = &ent.components.camera {
        apply_scene_camera(world, id, cam);
    }

    if let Some(tm) = &ent.components.tilemap {
        if tm.enabled {
            let layer = world.sorting_layer_index(&tm.sorting_layer);
            world.set_tilemap(
                id,
                Some(scene_tilemap_to_runtime(
                    tm, textures, catalog, anims, layer,
                )),
            );
        }
    }

    if let Some(c) = &ent.components.collider {
        if c.enabled {
            world.set_collider(id, Some(scene_collider_to_runtime(c)));
        }
    }

    if let Some(a) = &ent.components.animation {
        if a.enabled {
            let (cells, fps, loop_) = resolve_animation(a, anims);
            if !cells.is_empty() {
                let anim = Animation::new(a.clip.clone(), cells, fps, loop_);
                // Apply first frame onto sprite when possible.
                if let Some(cell) = anim.cell_name() {
                    apply_animation_cell(
                        world,
                        id,
                        cell,
                        textures,
                        catalog,
                        ent.components.sprite.as_ref().and_then(|s| s.pivot),
                    );
                }
                world.set_animation(id, Some(anim));
            } else {
                // Still attach stub so doctor/runtime see the clip name.
                world.set_animation(
                    id,
                    Some(Animation::new(a.clip.clone(), Vec::new(), fps, loop_)),
                );
            }
        }
    }

    if let Some(a) = &ent.components.animator {
        if a.enabled {
            apply_scene_animator(
                world,
                id,
                a,
                controllers,
                anims,
                textures,
                catalog,
                ent.components.sprite.as_ref().and_then(|s| s.pivot),
            );
        }
    }

    if let Some(d) = &ent.components.playable_director {
        if d.enabled {
            apply_scene_director(world, id, d, timelines, anims);
        }
    }

    if let Some(g) = &ent.components.grid_mover {
        if g.enabled {
            world.set_grid_mover(id, Some(scene_grid_mover_to_runtime(g)));
        }
    }

    if let Some(a) = &ent.components.audio_source {
        if a.enabled {
            world.set_audio_source(id, Some(scene_audio_source_to_runtime(a)));
        }
    }

    if let Some(t) = &ent.components.text {
        if t.enabled {
            let text = scene_text_to_runtime(world, t);
            world.set_text(id, Some(text));
        }
    }

    Ok(())
}

/// Catalog cell pivot unless the Sprite component overrides.
pub fn sprite_effective_pivot(
    sp: &crate::scene::SceneSprite,
    catalog: Option<&SpriteCatalog>,
) -> [f32; 2] {
    let fallback = catalog
        .and_then(|c| c.lookup(&sp.texture))
        .map(|r| r.pivot)
        .unwrap_or([0.5, 0.5]);
    sp.effective_pivot(fallback)
}

fn resolve_sprite(
    sp: &crate::scene::SceneSprite,
    catalog: Option<&SpriteCatalog>,
) -> (String, Rect, Vec2, Vec2) {
    let pivot = sprite_effective_pivot(sp, catalog);
    let pivot = Vec2::new(pivot[0], pivot[1]);
    if let Some(cat) = catalog {
        if let Some(r) = cat.lookup(&sp.texture) {
            let uv = Rect::new(r.uv[0], r.uv[1], r.uv[2], r.uv[3]);
            // Scene size wins when author set it; cells often keep authored size.
            let size = sp.size_vec();
            return (r.sheet_texture.clone(), uv, pivot, size);
        }
    }
    (sp.texture.clone(), Rect::unit(), pivot, sp.size_vec())
}

fn apply_animation_cell(
    world: &mut World,
    id: wiimaker_core::world::EntityId,
    cell: &str,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    pivot_override: Option<[f32; 2]>,
) {
    let Some(cat) = catalog else {
        return;
    };
    let Some(r) = cat.lookup(cell) else {
        return;
    };
    let Some(tex) = textures.get(&r.sheet_texture) else {
        return;
    };
    let uv = Rect::new(r.uv[0], r.uv[1], r.uv[2], r.uv[3]);
    let size = Vec2::new(r.pixel_size[0], r.pixel_size[1]);
    if let Some(sp) = world.sprite_mut(id) {
        sp.texture = tex;
        sp.uv = uv;
        if !sp.lock_pivot {
            if let Some(p) = pivot_override {
                sp.pivot = Vec2::new(p[0], p[1]);
                sp.lock_pivot = true;
            } else {
                sp.pivot = Vec2::new(r.pivot[0], r.pivot[1]);
            }
        }
        sp.size = size;
    } else {
        let mut sprite = Sprite::new(tex, size);
        sprite.uv = uv;
        if let Some(p) = pivot_override {
            sprite.pivot = Vec2::new(p[0], p[1]);
            sprite.lock_pivot = true;
        } else {
            sprite.pivot = Vec2::new(r.pivot[0], r.pivot[1]);
        }
        world.set_sprite(id, Some(sprite));
    }
}

/// Soft hydrate: missing textures skip the sprite instead of failing (editor preview).
pub fn hydrate_lenient(scene: &Scene, textures: &TextureMap) -> World {
    hydrate_lenient_with_catalog(scene, textures, None)
}

pub fn hydrate_lenient_with_catalog(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> World {
    hydrate_lenient_with_catalogs(scene, textures, catalog, None)
}

pub fn hydrate_lenient_with_catalogs(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> World {
    hydrate_lenient_with_all_catalogs(scene, textures, catalog, anims, None, None, None)
}

/// Like [`hydrate_lenient_with_catalogs`], but resolve Sorting Layers from `game.toml`.
pub fn hydrate_lenient_with_sorting_layers(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
    sorting_layers: Option<&[String]>,
) -> World {
    hydrate_lenient_with_all_catalogs(scene, textures, catalog, anims, None, sorting_layers, None)
}

/// Lenient hydrate with sprite clips, animator controllers, and sorting layers.
pub fn hydrate_lenient_with_all_catalogs(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
    controllers: Option<&AnimatorControllerCatalog>,
    sorting_layers: Option<&[String]>,
    timelines: Option<&TimelineCatalog>,
) -> World {
    let mut world = World::new();
    if let Some(layers) = sorting_layers {
        world.set_sorting_layers(layers.to_vec());
    }
    for ent in &scene.entities {
        let xf = scene
            .world_transform(&ent.name)
            .unwrap_or_else(|| ent.transform.clone());
        let id = world.spawn_named(ent.name.clone(), xf.to_runtime());
        world.set_tag(id, ent.tag);
        if let Some(sp) = &ent.components.sprite {
            if sp.enabled {
                let (tex_name, uv, pivot, size) = resolve_sprite(sp, catalog);
                if let Some(tex) = textures.get(&tex_name) {
                    let mut sprite = Sprite::new(tex, size);
                    sprite.uv = uv;
                    sprite.pivot = pivot;
                    sprite.lock_pivot = sp.pivot.is_some();
                    sprite.color = sp.color_rgba();
                    sprite.z = sp.z;
                    sprite.sorting_layer = world.sorting_layer_index(&sp.sorting_layer);
                    world.set_sprite(id, Some(sprite));
                }
            }
        }
        if let Some(d) = &ent.components.disc {
            if d.enabled {
                let mut disc = Disc::new(d.radius, d.color_rgba());
                disc.z = d.z;
                disc.sorting_layer = world.sorting_layer_index(&d.sorting_layer);
                world.set_disc(id, Some(disc));
            }
        }
        if let Some(cam) = &ent.components.camera {
            apply_scene_camera(&mut world, id, cam);
        }
        if let Some(tm) = &ent.components.tilemap {
            if tm.enabled {
                let layer = world.sorting_layer_index(&tm.sorting_layer);
                world.set_tilemap(
                    id,
                    Some(scene_tilemap_to_runtime(
                        tm, textures, catalog, anims, layer,
                    )),
                );
            }
        }
        if let Some(c) = &ent.components.collider {
            if c.enabled {
                world.set_collider(id, Some(scene_collider_to_runtime(c)));
            }
        }
        if let Some(a) = &ent.components.animation {
            if a.enabled {
                let (cells, fps, loop_) = resolve_animation(a, anims);
                let anim = Animation::new(a.clip.clone(), cells, fps, loop_);
                if let Some(cell) = anim.cell_name() {
                    apply_animation_cell(
                        &mut world,
                        id,
                        cell,
                        textures,
                        catalog,
                        ent.components.sprite.as_ref().and_then(|s| s.pivot),
                    );
                }
                world.set_animation(id, Some(anim));
            }
        }
        if let Some(a) = &ent.components.animator {
            if a.enabled {
                apply_scene_animator(
                    &mut world,
                    id,
                    a,
                    controllers,
                    anims,
                    textures,
                    catalog,
                    ent.components.sprite.as_ref().and_then(|s| s.pivot),
                );
            }
        }
        if let Some(d) = &ent.components.playable_director {
            if d.enabled {
                apply_scene_director(&mut world, id, d, timelines, anims);
            }
        }
        if let Some(g) = &ent.components.grid_mover {
            if g.enabled {
                world.set_grid_mover(id, Some(scene_grid_mover_to_runtime(g)));
            }
        }
        if let Some(a) = &ent.components.audio_source {
            if a.enabled {
                world.set_audio_source(id, Some(scene_audio_source_to_runtime(a)));
            }
        }
        if let Some(t) = &ent.components.text {
            if t.enabled {
                let text = scene_text_to_runtime(&world, t);
                world.set_text(id, Some(text));
            }
        }
    }
    world.capture_timeline_baselines();
    world
}

fn scene_tilemap_to_runtime(
    tm: &crate::scene::SceneTilemap,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
    sorting_layer: u16,
) -> Tilemap {
    let mut out = Tilemap::new(tm.width.max(1), tm.height.max(1), tm.cell);
    out.origin = Vec2::new(tm.origin[0], tm.origin[1]);
    out.z = tm.z;
    out.sorting_layer = sorting_layer;
    let n = out.len();
    out.cells = tm.cells.clone();
    if out.cells.len() < n {
        out.cells.resize(n, 0);
    } else if out.cells.len() > n {
        out.cells.truncate(n);
    }
    out.solid = vec![0; (n + 7) / 8];
    for (i, flag) in tm.solid.iter().take(n).enumerate() {
        if *flag != 0 {
            let byte = i / 8;
            let bit = i % 8;
            out.solid[byte] |= 1 << bit;
        }
    }
    out.palette = tm
        .palette
        .iter()
        .map(|p| scene_palette_to_runtime(p, textures, catalog, anims))
        .collect();
    if out.palette.is_empty() {
        out.palette
            .push(TileVisual::color_only(1, Rgba8::rgb(48, 88, 176)));
    }
    out
}

fn scene_palette_to_runtime(
    p: &crate::scene::SceneTilePalette,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> TileVisual {
    let mut vis = TileVisual::color_only(p.id, p.color_rgba());
    vis.texture = p.sprite.as_ref().and_then(|name| {
        let (tex_name, uv) = resolve_palette_sprite(name, catalog);
        textures.get(&tex_name).map(|tex| (tex, uv))
    });

    if let Some(clip) = p.anim_clip() {
        let meta = anims.and_then(|c| c.lookup(clip));
        vis.fps = p
            .anim_fps
            .filter(|f| *f > 0.0)
            .or_else(|| meta.map(|m| m.fps))
            .unwrap_or(10.0);
        vis.loop_ = true;
        if let Some(meta) = meta {
            for cell in &meta.cells {
                let (tex_name, uv) = resolve_palette_sprite(cell, catalog);
                if let Some(tex) = textures.get(&tex_name) {
                    vis.frames.push((tex, uv));
                }
            }
        }
        if vis.frames.is_empty() {
            if let Some(tex) = vis.texture {
                vis.frames.push(tex);
            }
        } else {
            vis.texture = Some(vis.frames[0]);
        }
    }

    if let Some(mode) = p.auto_tile {
        vis.auto_tile = Some(mode.to_runtime());
        let stem = p.sprite.as_deref().map(str::trim).filter(|s| !s.is_empty());
        for mask in 0..16 {
            let named = p
                .auto_sprites
                .get(mask)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| stem.map(|s| format!("{s}_{mask}")));
            // Leave unresolved masks None so `texture_for_mask` falls back to the
            // ticking `vis.texture`. Filling with frame 0 froze combined anim + auto-tile.
            vis.auto_frames[mask] = named.and_then(|name| {
                let (tex_name, uv) = resolve_palette_sprite(&name, catalog);
                textures.get(&tex_name).map(|tex| (tex, uv))
            });
        }
    }
    vis
}

fn resolve_palette_sprite(name: &str, catalog: Option<&SpriteCatalog>) -> (String, Rect) {
    if let Some(cat) = catalog {
        if let Some(r) = cat.lookup(name) {
            return (
                r.sheet_texture.clone(),
                Rect::new(r.uv[0], r.uv[1], r.uv[2], r.uv[3]),
            );
        }
    }
    (name.to_string(), Rect::unit())
}

fn resolve_animation(
    a: &crate::scene::SceneAnimation,
    anims: Option<&AnimClipCatalog>,
) -> (Vec<String>, f32, bool) {
    let meta = anims.and_then(|c| c.lookup(&a.clip));
    let cells = meta.map(|m| m.cells.clone()).unwrap_or_default();
    let fps = a
        .fps
        .filter(|f| *f > 0.0)
        .or_else(|| meta.map(|m| m.fps))
        .unwrap_or(10.0);
    let loop_ = a.loop_;
    (cells, fps, loop_)
}

fn apply_scene_animator(
    world: &mut World,
    id: wiimaker_core::world::EntityId,
    scene_a: &crate::scene::SceneAnimator,
    controllers: Option<&AnimatorControllerCatalog>,
    anims: Option<&AnimClipCatalog>,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
    lock_pivot: Option<[f32; 2]>,
) {
    let mut rt = Animator::new(scene_a.controller.clone());
    if let Some(meta) = controllers.and_then(|c| c.lookup(&scene_a.controller)) {
        for p in &meta.parameters {
            let mut rp = match p.kind {
                wiimaker_assets::ControllerParamType::Bool => {
                    AnimatorParam::bool_param(&p.name, p.default_bool())
                }
                wiimaker_assets::ControllerParamType::Float => {
                    AnimatorParam::float_param(&p.name, p.default_float())
                }
                wiimaker_assets::ControllerParamType::Trigger => {
                    AnimatorParam::trigger_param(&p.name)
                }
            };
            if let Some(ov) = scene_a.parameters.iter().find(|o| o.name == p.name) {
                if let Some(b) = ov.bool_value {
                    rp.bool_value = b;
                }
                if let Some(f) = ov.float_value {
                    rp.float_value = f;
                    if rp.kind == AnimatorParamKind::Bool {
                        rp.bool_value = f != 0.0;
                    }
                }
            }
            rt.parameters.push(rp);
        }
        for s in &meta.states {
            let clip = anims.and_then(|c| c.lookup(&s.clip));
            rt.states.push(AnimatorState {
                name: s.name.clone(),
                clip: s.clip.clone(),
                speed: if s.speed > 0.0 { s.speed } else { 1.0 },
                cells: clip.map(|m| m.cells.clone()).unwrap_or_default(),
                fps: clip.map(|m| m.fps).filter(|f| *f > 0.0).unwrap_or(10.0),
                loop_: clip.map(|m| m.loop_).unwrap_or(true),
            });
        }
        for t in &meta.transitions {
            rt.transitions.push(AnimatorTransition {
                from: t.from.clone(),
                from_any: wiimaker_assets::is_any_state(&t.from),
                to: t.to.clone(),
                conditions: t
                    .conditions
                    .iter()
                    .map(|c| runtime_condition(c, &rt.parameters))
                    .collect(),
                has_exit_time: t.has_exit_time,
            });
        }
        rt.state = meta.default_state.clone();
    }
    world.set_animator(id, Some(rt));
    let state = world
        .animator(id)
        .map(|a| a.state.clone())
        .unwrap_or_default();
    if !state.is_empty() {
        world.apply_animator_state(id, &state);
        if let Some(cell) = world
            .animation(id)
            .and_then(|a| a.cell_name().map(|s| s.to_string()))
        {
            apply_animation_cell(world, id, &cell, textures, catalog, lock_pivot);
        }
    }
}

fn apply_scene_director(
    world: &mut World,
    id: wiimaker_core::world::EntityId,
    scene_d: &crate::scene::ScenePlayableDirector,
    timelines: Option<&TimelineCatalog>,
    anims: Option<&AnimClipCatalog>,
) {
    let meta = timelines.and_then(|c| c.lookup(&scene_d.timeline));
    let duration = meta.map(|m| m.duration).unwrap_or(0.0);
    let mut rt = PlayableDirector::new(
        scene_d.timeline.clone(),
        duration,
        scene_d.play_on_awake,
        scene_d.loop_,
    );
    if let Some(meta) = meta {
        for track in &meta.tracks {
            let binding = track.binding.clone().unwrap_or_default();
            let kind = match track.kind {
                wiimaker_assets::TimelineTrackKind::Activation => {
                    wiimaker_core::TimelineTrackKind::Activation
                }
                wiimaker_assets::TimelineTrackKind::Animation => {
                    wiimaker_core::TimelineTrackKind::Animation
                }
                wiimaker_assets::TimelineTrackKind::Audio => {
                    wiimaker_core::TimelineTrackKind::Audio
                }
                wiimaker_assets::TimelineTrackKind::Transform => {
                    wiimaker_core::TimelineTrackKind::Transform
                }
                wiimaker_assets::TimelineTrackKind::Signal => {
                    wiimaker_core::TimelineTrackKind::Signal
                }
                wiimaker_assets::TimelineTrackKind::Control => {
                    wiimaker_core::TimelineTrackKind::Control
                }
            };
            let mut clips = Vec::new();
            for c in &track.clips {
                let anim = c
                    .clip
                    .as_deref()
                    .and_then(|stem| anims.and_then(|cat| cat.lookup(stem)));
                clips.push(TimelineClipRuntime {
                    start: c.start,
                    end: c.end,
                    active: c.active.unwrap_or(true),
                    clip: c.clip.clone().unwrap_or_default(),
                    cells: anim.map(|m| m.cells.clone()).unwrap_or_default(),
                    fps: anim.map(|m| m.fps).filter(|f| *f > 0.0).unwrap_or(10.0),
                    loop_clip: anim.map(|m| m.loop_).unwrap_or(true),
                    audio: c.audio.clone().unwrap_or_default(),
                    volume: c.volume.unwrap_or(1.0),
                    from: c.from.unwrap_or([0.0, 0.0]),
                    to: c.to.unwrap_or([0.0, 0.0]),
                    signal: c.signal.clone().unwrap_or_default(),
                    payload: c.payload.clone().unwrap_or_default(),
                });
            }
            rt.tracks.push(TimelineTrackRuntime {
                name: track.name.clone(),
                kind,
                binding,
                clips,
            });
        }
    }
    world.set_director(id, Some(rt));
}

fn runtime_condition(
    c: &wiimaker_assets::ControllerCondition,
    params: &[AnimatorParam],
) -> AnimatorCondition {
    let kind = params.iter().find(|p| p.name == c.param).map(|p| p.kind);
    if let Some(g) = c.greater {
        return AnimatorCondition::FloatGreater {
            param: c.param.clone(),
            value: g,
        };
    }
    if let Some(l) = c.less {
        return AnimatorCondition::FloatLess {
            param: c.param.clone(),
            value: l,
        };
    }
    if let Some(eq) = &c.equals {
        if let Some(b) = eq.as_bool() {
            if kind == Some(AnimatorParamKind::Trigger) && b {
                return AnimatorCondition::Trigger {
                    param: c.param.clone(),
                };
            }
            return AnimatorCondition::BoolEq {
                param: c.param.clone(),
                value: b,
            };
        }
        if let Some(n) = eq.as_f64() {
            return AnimatorCondition::FloatEq {
                param: c.param.clone(),
                value: n as f32,
            };
        }
    }
    if kind == Some(AnimatorParamKind::Trigger) {
        return AnimatorCondition::Trigger {
            param: c.param.clone(),
        };
    }
    AnimatorCondition::BoolEq {
        param: c.param.clone(),
        value: true,
    }
}

fn apply_scene_camera(
    world: &mut World,
    id: wiimaker_core::world::EntityId,
    cam: &crate::scene::SceneCamera,
) {
    world.set_camera(id, Some(Camera { active: cam.active }));
    match &cam.follow {
        Some(t) if !t.is_empty() => {
            world.set_follow(id, Some(Follow::new(t.clone(), cam.lerp)));
        }
        _ => world.set_follow(id, None),
    }
}

fn scene_collider_to_runtime(c: &crate::scene::SceneCollider) -> Collider {
    Collider {
        kind: match c.kind {
            crate::scene::SceneColliderKind::Aabb => ColliderKind::Aabb {
                size: Vec2::new(c.size[0].max(0.0), c.size[1].max(0.0)),
            },
            crate::scene::SceneColliderKind::Circle => ColliderKind::Circle {
                radius: c.radius.max(0.0),
            },
        },
        offset: Vec2::new(c.offset[0], c.offset[1]),
        solid: c.solid,
        trigger: c.trigger,
        filter_tag: c.filter_tag,
    }
}

fn scene_grid_mover_to_runtime(g: &crate::scene::SceneGridMover) -> GridMover {
    let mut gm = GridMover::new(g.cell, g.speed);
    gm.queued_dir = g.queued_dir.map(|d| d.to_runtime());
    gm
}

fn scene_audio_source_to_runtime(a: &crate::scene::SceneAudioSource) -> AudioSource {
    AudioSource::new(a.clip.clone(), a.volume, a.play_on_awake)
}

fn scene_text_to_runtime(world: &World, t: &crate::scene::SceneText) -> Text {
    let mut text = Text::new(t.text.clone(), t.size, t.color_rgba());
    text.align = t.align_runtime();
    text.z = t.z;
    text.sorting_layer = world.sorting_layer_index(&t.sorting_layer);
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutate::{add_entity, set_entity_parent, set_entity_rotation_z, MutateOpts};
    use crate::scene::Scene;

    fn rot_parent_scene() -> Scene {
        let mut scene = Scene::new("orbit");
        add_entity(
            &mut scene,
            "RotParent",
            &MutateOpts {
                x: Some(320.0),
                y: Some(240.0),
                radius: Some(28.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "RotParent", 45f32.to_radians()).unwrap();
        add_entity(
            &mut scene,
            "RotChild",
            &MutateOpts {
                x: Some(80.0),
                y: Some(0.0),
                radius: Some(16.0),
                ..Default::default()
            },
        )
        .unwrap();
        // add_entity writes world-ish defaults; force local (80,0) then parent without
        // preserving that authored local via set_entity_parent (which remaps pose).
        let child = scene
            .entities
            .iter_mut()
            .find(|e| e.name == "RotChild")
            .unwrap();
        child.transform.translation = [80.0, 0.0, 0.0];
        child.parent = Some("RotParent".into());
        scene
    }

    #[test]
    fn hydrate_flattens_rotated_parent_world_pose() {
        let scene = rot_parent_scene();
        let world = hydrate_lenient(&scene, &TextureMap::new());
        let id = world.find_by_name("RotChild").expect("child");
        let xf = world.transform(id).unwrap();
        let s = 45f32.to_radians().sin();
        let c = 45f32.to_radians().cos();
        assert!((xf.translation.x - (320.0 + 80.0 * c)).abs() < 1e-4);
        assert!((xf.translation.y - (240.0 + 80.0 * s)).abs() < 1e-4);
        // World has no parent links — pose is already composed.
        assert!(world.find_by_name("RotParent").is_some());
    }

    #[test]
    fn hydrate_identity_parent_still_translates() {
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "p",
            &MutateOpts {
                x: Some(100.0),
                y: Some(50.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "c",
            &MutateOpts {
                x: Some(130.0),
                y: Some(70.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_parent(&mut scene, "c", Some("p")).unwrap();
        let world = hydrate_lenient(&scene, &TextureMap::new());
        let xf = world.transform(world.find_by_name("c").unwrap()).unwrap();
        assert!((xf.translation.x - 130.0).abs() < 1e-4);
        assert!((xf.translation.y - 70.0).abs() < 1e-4);
    }
}
