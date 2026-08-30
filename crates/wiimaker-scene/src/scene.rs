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
    /// Parent entity name. `None` = scene root. Transform is local to parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
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

    /// Compose `local` under `parent` (translation × parent scale; scales multiply).
    pub fn compose_child(parent: &Self, local: &Self) -> Self {
        Self {
            translation: [
                parent.translation[0] + local.translation[0] * parent.scale[0],
                parent.translation[1] + local.translation[1] * parent.scale[1],
                parent.translation[2] + local.translation[2] * parent.scale[2],
            ],
            rotation: local.rotation,
            scale: [
                parent.scale[0] * local.scale[0],
                parent.scale[1] * local.scale[1],
                parent.scale[2] * local.scale[2],
            ],
        }
    }

    /// Inverse of [`compose_child`]: world pose → local under `parent_world`.
    pub fn to_local(parent_world: &Self, world: &Self) -> Self {
        let sx = safe_div_scale(parent_world.scale[0]);
        let sy = safe_div_scale(parent_world.scale[1]);
        let sz = safe_div_scale(parent_world.scale[2]);
        Self {
            translation: [
                (world.translation[0] - parent_world.translation[0]) / sx,
                (world.translation[1] - parent_world.translation[1]) / sy,
                (world.translation[2] - parent_world.translation[2]) / sz,
            ],
            rotation: world.rotation,
            scale: [
                world.scale[0] / sx,
                world.scale[1] / sy,
                world.scale[2] / sz,
            ],
        }
    }
}

fn safe_div_scale(s: f32) -> f32 {
    if s.abs() < 1e-8 {
        1.0
    } else {
        s
    }
}

impl Scene {
    pub fn find_entity(&self, name: &str) -> Option<&EntityData> {
        self.entities.iter().find(|e| e.name == name)
    }

    /// World-space transform (local composed through parents). `None` if missing or cyclic.
    pub fn world_transform(&self, name: &str) -> Option<SceneTransform> {
        let mut locals = Vec::new();
        let mut current = name.to_string();
        for _ in 0..=self.entities.len() {
            let ent = self.find_entity(&current)?;
            locals.push(ent.transform.clone());
            match &ent.parent {
                Some(p) => current = p.clone(),
                None => {
                    locals.reverse();
                    let mut world = locals[0].clone();
                    for local in locals.iter().skip(1) {
                        world = SceneTransform::compose_child(&world, local);
                    }
                    return Some(world);
                }
            }
        }
        None
    }

    /// Names of root entities, in scene order.
    pub fn root_names(&self) -> Vec<String> {
        self.entities
            .iter()
            .filter(|e| e.parent.is_none())
            .map(|e| e.name.clone())
            .collect()
    }

    /// Direct children of `parent`, in scene order.
    pub fn child_names(&self, parent: &str) -> Vec<String> {
        self.entities
            .iter()
            .filter(|e| e.parent.as_deref() == Some(parent))
            .map(|e| e.name.clone())
            .collect()
    }

    /// True if `maybe_desc` is `ancestor` or nested under it.
    pub fn is_descendant_of(&self, maybe_desc: &str, ancestor: &str) -> bool {
        if maybe_desc == ancestor {
            return true;
        }
        let mut current = maybe_desc.to_string();
        for _ in 0..=self.entities.len() {
            let Some(ent) = self.find_entity(&current) else {
                return false;
            };
            match &ent.parent {
                Some(p) if p == ancestor => return true,
                Some(p) => current = p.clone(),
                None => return false,
            }
        }
        false
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
    #[serde(default, rename = "Tilemap", skip_serializing_if = "Option::is_none")]
    pub tilemap: Option<SceneTilemap>,
    #[serde(default, rename = "Collider", skip_serializing_if = "Option::is_none")]
    pub collider: Option<SceneCollider>,
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
    /// When false, skipped by hydrate / pick / bake (Unity component checkbox).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
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
    /// When false, skipped by hydrate / pick / bake (Unity component checkbox).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
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

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn default_cell() -> f32 {
    16.0
}
fn default_tm_w() -> u32 {
    32
}
fn default_tm_h() -> u32 {
    18
}
fn wall_color() -> [u8; 4] {
    [48, 88, 176, 255]
}

/// Authoring tilemap (Unity Tilemap analogue). `cells` / `solid` are row-major.
/// `solid` is 0/1 per cell (JSON-friendly); packed to bits at hydrate time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneTilemap {
    #[serde(default = "default_cell")]
    pub cell: f32,
    #[serde(default)]
    pub origin: [f32; 2],
    #[serde(default = "default_tm_w")]
    pub width: u32,
    #[serde(default = "default_tm_h")]
    pub height: u32,
    #[serde(default)]
    pub cells: Vec<u16>,
    /// 0/1 per cell, same order as `cells`.
    #[serde(default)]
    pub solid: Vec<u8>,
    #[serde(default)]
    pub palette: Vec<SceneTilePalette>,
    #[serde(default)]
    pub z: f32,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl Default for SceneTilemap {
    fn default() -> Self {
        Self::new(default_tm_w(), default_tm_h(), default_cell())
    }
}

impl SceneTilemap {
    pub fn new(width: u32, height: u32, cell: f32) -> Self {
        let n = (width as usize).saturating_mul(height as usize);
        Self {
            cell: if cell <= 0.0 { default_cell() } else { cell },
            origin: [0.0, 0.0],
            width,
            height,
            cells: vec![0; n],
            solid: vec![0; n],
            palette: vec![SceneTilePalette {
                id: 1,
                sprite: None,
                color: wall_color(),
            }],
            z: -1.0,
            enabled: true,
        }
    }

    pub fn len(&self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }

    pub fn ensure_len(&mut self) {
        let n = self.len();
        if self.cells.len() < n {
            self.cells.resize(n, 0);
        } else if self.cells.len() > n {
            self.cells.truncate(n);
        }
        if self.solid.len() < n {
            self.solid.resize(n, 0);
        } else if self.solid.len() > n {
            self.solid.truncate(n);
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if self.in_bounds(x, y) {
            Some(y as usize * self.width as usize + x as usize)
        } else {
            None
        }
    }

    pub fn get(&self, x: i32, y: i32) -> (u16, bool) {
        match self.index(x, y) {
            Some(i) => (
                self.cells.get(i).copied().unwrap_or(0),
                self.solid.get(i).copied().unwrap_or(0) != 0,
            ),
            None => (0, false),
        }
    }

    pub fn set(&mut self, x: i32, y: i32, id: u16, solid: bool) -> bool {
        self.ensure_len();
        let Some(i) = self.index(x, y) else {
            return false;
        };
        self.cells[i] = id;
        self.solid[i] = if solid { 1 } else { 0 };
        true
    }

    pub fn world_to_cell(&self, world: &SceneTransform, wx: f32, wy: f32) -> (i32, i32) {
        let cell_x = (self.cell * world.scale[0]).abs().max(1e-6);
        let cell_y = (self.cell * world.scale[1]).abs().max(1e-6);
        let ox = world.translation[0] + self.origin[0] * world.scale[0];
        let oy = world.translation[1] + self.origin[1] * world.scale[1];
        (
            ((wx - ox) / cell_x).floor() as i32,
            ((wy - oy) / cell_y).floor() as i32,
        )
    }

    pub fn world_rect(&self, world: &SceneTransform) -> ([f32; 2], [f32; 2]) {
        let left = world.translation[0] + self.origin[0] * world.scale[0];
        let top = world.translation[1] + self.origin[1] * world.scale[1];
        let w = self.width as f32 * self.cell * world.scale[0];
        let h = self.height as f32 * self.cell * world.scale[1];
        ([left, top], [w, h])
    }

    /// Resize preserving the overlapping top-left region.
    pub fn resize(&mut self, width: u32, height: u32) {
        let old_w = self.width;
        let old_h = self.height;
        let old_cells = self.cells.clone();
        let old_solid = self.solid.clone();
        self.width = width.max(1);
        self.height = height.max(1);
        let n = self.len();
        self.cells = vec![0; n];
        self.solid = vec![0; n];
        let copy_w = old_w.min(self.width) as usize;
        let copy_h = old_h.min(self.height) as usize;
        for y in 0..copy_h {
            for x in 0..copy_w {
                let oi = y * old_w as usize + x;
                let ni = y * self.width as usize + x;
                if oi < old_cells.len() {
                    self.cells[ni] = old_cells[oi];
                }
                if oi < old_solid.len() {
                    self.solid[ni] = old_solid[oi];
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneTilePalette {
    pub id: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    #[serde(default = "wall_color")]
    pub color: [u8; 4],
}

impl SceneTilePalette {
    pub fn color_rgba(&self) -> Rgba8 {
        Rgba8::new(self.color[0], self.color[1], self.color[2], self.color[3])
    }
}

/// Unity BoxCollider2D / CircleCollider2D analogue.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum SceneColliderKind {
    #[default]
    Aabb,
    Circle,
}

fn default_collider_size() -> [f32; 2] {
    [32.0, 32.0]
}
fn default_collider_radius() -> f32 {
    16.0
}
fn is_zero2(v: &[f32; 2]) -> bool {
    v[0] == 0.0 && v[1] == 0.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneCollider {
    #[serde(default)]
    pub kind: SceneColliderKind,
    /// Full width/height when `kind` is Aabb.
    #[serde(default = "default_collider_size")]
    pub size: [f32; 2],
    /// Radius when `kind` is Circle.
    #[serde(default = "default_collider_radius")]
    pub radius: f32,
    /// Local offset from the entity transform.
    #[serde(default, skip_serializing_if = "is_zero2")]
    pub offset: [f32; 2],
    /// Blocks [`wiimaker_core::move_and_collide`] when true (ignored if `trigger`).
    #[serde(default = "default_true")]
    pub solid: bool,
    /// Unity `isTrigger`: never blocks; use `triggers_entered` / `entity_triggers_entered`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub trigger: bool,
    /// When non-zero on a trigger, the other entity's `tag` must match (0 = any).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub filter_tag: u32,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl Default for SceneCollider {
    fn default() -> Self {
        Self {
            kind: SceneColliderKind::Aabb,
            size: default_collider_size(),
            radius: default_collider_radius(),
            offset: [0.0, 0.0],
            solid: true,
            trigger: false,
            filter_tag: 0,
            enabled: true,
        }
    }
}

impl SceneCollider {
    pub fn aabb(width: f32, height: f32) -> Self {
        Self {
            kind: SceneColliderKind::Aabb,
            size: [width.max(0.0), height.max(0.0)],
            ..Default::default()
        }
    }

    pub fn circle(radius: f32) -> Self {
        Self {
            kind: SceneColliderKind::Circle,
            radius: radius.max(0.0),
            ..Default::default()
        }
    }

    /// World-space AABB (min xy, max xy) for gizmos / pick.
    pub fn world_aabb(&self, world: &SceneTransform) -> ([f32; 2], [f32; 2]) {
        let cx = world.translation[0] + self.offset[0] * world.scale[0];
        let cy = world.translation[1] + self.offset[1] * world.scale[1];
        let (hx, hy) = match self.kind {
            SceneColliderKind::Aabb => (
                (self.size[0] * 0.5 * world.scale[0]).abs(),
                (self.size[1] * 0.5 * world.scale[1]).abs(),
            ),
            SceneColliderKind::Circle => {
                let r = (self.radius * world.scale[0].abs().max(world.scale[1].abs())).abs();
                (r, r)
            }
        };
        ([cx - hx, cy - hy], [cx + hx, cy + hy])
    }

    pub fn world_center(&self, world: &SceneTransform) -> [f32; 2] {
        [
            world.translation[0] + self.offset[0] * world.scale[0],
            world.translation[1] + self.offset[1] * world.scale[1],
        ]
    }

    pub fn world_radius(&self, world: &SceneTransform) -> Option<f32> {
        match self.kind {
            SceneColliderKind::Circle => {
                Some((self.radius * world.scale[0].abs().max(world.scale[1].abs())).abs())
            }
            SceneColliderKind::Aabb => None,
        }
    }

    pub fn contains_point(&self, world: &SceneTransform, sx: f32, sy: f32) -> bool {
        match self.kind {
            SceneColliderKind::Aabb => {
                let (min, max) = self.world_aabb(world);
                sx >= min[0] && sx <= max[0] && sy >= min[1] && sy <= max[1]
            }
            SceneColliderKind::Circle => {
                let c = self.world_center(world);
                let r = self.world_radius(world).unwrap_or(0.0);
                let dx = sx - c[0];
                let dy = sy - c[1];
                dx * dx + dy * dy <= r * r
            }
        }
    }
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
