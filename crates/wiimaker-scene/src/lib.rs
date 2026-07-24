//! Scene / project authoring types — source of truth for editor + CLI.

mod doctor;
mod hydrate;
mod mutate;
mod project;
mod render;
mod scene;
mod undo;
mod wscn;

pub use doctor::{diagnose, Diagnosis, Issue, Severity};
pub use hydrate::{hydrate, hydrate_into, hydrate_lenient, TextureMap};
pub use mutate::{
    add_component_disc, add_component_sprite, add_entity, duplicate_entity, insert_entity_clone,
    remove_entity, rename_entity, set_entity_transform, set_scene_clear, unique_entity_name,
    MutateOpts,
};
pub use project::{find_game_dir, load_project, save_project, GameProject};
pub use render::render_world;
pub use scene::{
    load_prefab, load_scene, save_prefab, save_scene, EntityData, Prefab, Scene, SceneComponents,
    SceneDisc, SceneSprite, SceneTransform,
};
pub use undo::UndoStack;
pub use wscn::{bake_scene_wscn, write_scene_wscn, KIND_DISC, KIND_NONE, KIND_SPRITE, WSCN_MAGIC};
