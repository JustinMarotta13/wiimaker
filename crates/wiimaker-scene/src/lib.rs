//! Scene / project authoring types — source of truth for editor + CLI.

mod animate;
mod collider;
mod doctor;
mod hydrate;
mod mutate;
mod pick;
mod project;
mod render;
mod scene;
mod tilemap;
mod undo;
mod wscn;

pub use collider::{
    add_component_collider, entities_overlap, entity_overlaps, entity_triggers_entered,
    remove_component_collider,
};
pub use animate::{animate_world, apply_animation_frame};
pub use doctor::{diagnose, Diagnosis, Issue, Severity};
pub use hydrate::{
    hydrate, hydrate_into, hydrate_into_with_catalog, hydrate_into_with_catalogs, hydrate_lenient,
    hydrate_lenient_with_catalog, hydrate_lenient_with_catalogs, hydrate_with_catalog,
    hydrate_with_catalogs, load_scene_into_world, TextureMap,
};
pub use mutate::{
    add_component_animation, add_component_disc, add_component_sprite, add_entity, apply_prefab,
    duplicate_entity, entity_to_prefab, insert_entity_clone, instantiate_prefab,
    remove_component_animation, remove_component_disc, remove_component_sprite, remove_entity,
    rename_entity, set_component_enabled, set_entity_anim, set_entity_parent,
    set_entity_rotation_z, set_entity_scale, set_entity_transform, set_entity_world_xy,
    set_scene_clear, unique_entity_name, unpack_prefab_instance, MutateOpts,
};
pub use pick::{pick_entity_at, pick_entity_at_with_catalog, pointer_to_scene};
pub use project::{
    add_build_scene, create_named_scene, find_game_dir, list_build_scenes, list_scenes,
    load_project, remove_build_scene, resolve_scene_rel, save_project, set_build_scenes,
    set_default_scene, GameProject,
};
pub use render::render_world;
pub use scene::{
    load_prefab, load_scene, save_prefab, save_scene, EntityData, Prefab, Scene, SceneAnimation,
    SceneCollider, SceneColliderKind, SceneComponents, SceneDisc, SceneSprite, SceneTilePalette,
    SceneTilemap, SceneTransform,
};
pub use tilemap::{
    add_component_tilemap, ensure_tilemap, remove_component_tilemap, tilemap_fill,
    tilemap_get_cell, tilemap_resize, tilemap_set_cell, tilemap_stamp, tilemap_stamp_ascii,
};
pub use undo::UndoStack;
pub use wscn::{
    bake_scene_wscn, bake_scene_wscn_with_catalog, write_scene_wscn, write_scene_wscn_with_catalog,
    KIND_DISC, KIND_NONE, KIND_SPRITE, KIND_TILEMAP, WSCN_MAGIC,
};
