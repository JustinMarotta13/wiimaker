//! Scene / project authoring types — source of truth for editor + CLI.

mod doctor;
mod hydrate;
mod mutate;
mod pick;
mod project;
mod render;
mod scene;
mod undo;
mod wscn;

pub use doctor::{diagnose, Diagnosis, Issue, Severity};
pub use hydrate::{
    hydrate, hydrate_into, hydrate_into_with_catalog, hydrate_lenient,
    hydrate_lenient_with_catalog, hydrate_with_catalog, TextureMap,
};
pub use mutate::{
    add_component_disc, add_component_sprite, add_entity, apply_prefab, duplicate_entity,
    entity_to_prefab, insert_entity_clone, instantiate_prefab, remove_component_disc,
    remove_component_sprite, remove_entity, rename_entity, set_component_enabled, set_entity_parent,
    set_entity_rotation_z, set_entity_scale, set_entity_transform, set_entity_world_xy,
    set_scene_clear, unique_entity_name, unpack_prefab_instance, MutateOpts,
};
pub use pick::{pick_entity_at, pick_entity_at_with_catalog, pointer_to_scene};
pub use project::{find_game_dir, list_scenes, load_project, save_project, GameProject};
pub use render::render_world;
pub use scene::{
    load_prefab, load_scene, save_prefab, save_scene, EntityData, Prefab, Scene, SceneComponents,
    SceneDisc, SceneSprite, SceneTransform,
};
pub use undo::UndoStack;
pub use wscn::{
    bake_scene_wscn, bake_scene_wscn_with_catalog, write_scene_wscn, write_scene_wscn_with_catalog,
    KIND_DISC, KIND_NONE, KIND_SPRITE, WSCN_MAGIC,
};
