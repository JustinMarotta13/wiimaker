//! JSON scene / prefab schema (agent-friendly).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use wiimaker_core::color::Rgba8;
use wiimaker_core::math::{Quat, Vec2, Vec3};
use wiimaker_core::world::Transform;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    #[serde(default = "default_clear")]
    pub clear_color: [u8; 4],
    #[serde(default)]
    pub entities: Vec<EntityData>,
}

fn default_clear() -> [u8; 4] {
    [12, 18, 32, 255]
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            clear_color: default_clear(),
            entities: Vec::new(),
        }
    }

    pub fn clear_rgba(&self) -> Rgba8 {
        Rgba8::new(
            self.clear_color[0],
            self.clear_color[1],
            self.clear_color[2],
            self.clear_color[3],
        )
    }
}

/// Prefab = one entity blob (Unity prefab analogue).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefab {
    #[serde(flatten)]
    pub entity: EntityData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityData {
    pub name: String,
    #[serde(default)]
    pub transform: SceneTransform,
    #[serde(default)]
    pub components: SceneComponents,
    #[serde(default)]
    pub tag: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneTransform {
    #[serde(default = "zero3")]
    pub translation: [f32; 3],
    #[serde(default = "ident_quat")]
    pub rotation: [f32; 4],
    #[serde(default = "one3")]
    pub scale: [f32; 3],
}

fn zero3() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}
fn one3() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
fn ident_quat() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

impl Default for SceneTransform {
    fn default() -> Self {
        Self {
            translation: zero3(),
            rotation: ident_quat(),
            scale: one3(),
        }
    }
}

impl SceneTransform {
    pub fn from_xy(x: f32, y: f32) -> Self {
        Self {
            translation: [x, y, 0.0],
            ..Default::default()
        }
    }

    pub fn to_runtime(&self) -> Transform {
        Transform {
            translation: Vec3::from_array(self.translation),
            rotation: Quat::from_xyzw(
                self.rotation[0],
                self.rotation[1],
                self.rotation[2],
                self.rotation[3],
            ),
            scale: Vec3::from_array(self.scale),
        }
    }

    pub fn from_runtime(t: &Transform) -> Self {
        Self {
            translation: t.translation.to_array(),
            rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
            scale: t.scale.to_array(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SceneComponents {
    #[serde(default, rename = "Sprite", skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SceneSprite>,
    #[serde(default, rename = "Disc", skip_serializing_if = "Option::is_none")]
    pub disc: Option<SceneDisc>,
    #[serde(default, rename = "Camera", skip_serializing_if = "Option::is_none")]
    pub camera: Option<SceneCamera>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneSprite {
    pub texture: String,
    #[serde(default = "default_sprite_size")]
    pub size: [f32; 2],
    #[serde(default = "white4")]
    pub color: [u8; 4],
    #[serde(default)]
    pub z: f32,
}

fn default_sprite_size() -> [f32; 2] {
    [32.0, 32.0]
}
fn white4() -> [u8; 4] {
    [255, 255, 255, 255]
}

impl SceneSprite {
    pub fn size_vec(&self) -> Vec2 {
        Vec2::new(self.size[0], self.size[1])
    }

    pub fn color_rgba(&self) -> Rgba8 {
        Rgba8::new(self.color[0], self.color[1], self.color[2], self.color[3])
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneDisc {
    pub radius: f32,
    #[serde(default = "mintish")]
    pub color: [u8; 4],
    #[serde(default)]
    pub z: f32,
}

fn mintish() -> [u8; 4] {
    [72, 210, 160, 255]
}

impl SceneDisc {
    pub fn color_rgba(&self) -> Rgba8 {
        Rgba8::new(self.color[0], self.color[1], self.color[2], self.color[3])
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SceneCamera {
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}

pub fn load_scene(path: &Path) -> Result<Scene> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let scene: Scene = serde_json::from_str(&text).context("parse scene json")?;
    Ok(scene)
}

pub fn save_scene(path: &Path, scene: &Scene) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(scene)?;
    fs::write(path, text + "\n")?;
    Ok(())
}

pub fn load_prefab(path: &Path) -> Result<Prefab> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save_prefab(path: &Path, prefab: &Prefab) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(prefab)?;
    fs::write(path, text + "\n")?;
    Ok(())
}
