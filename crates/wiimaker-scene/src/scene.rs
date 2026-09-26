//! JSON scene / prefab schema (agent-friendly).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use wiimaker_core::color::Rgba8;
use wiimaker_core::math::{Quat, Vec2, Vec3};
use wiimaker_core::world::Transform;
use wiimaker_core::{is_default_sorting_layer_name, DEFAULT_SORTING_LAYER};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    #[serde(default = "default_clear")]
    pub clear_color: [u8; 4],
    #[serde(default)]
    pub entities: Vec<EntityData>,
}

/// Default Scene/Game GX clear (navy). Editor Reset and `Scene::new` share this.
pub const DEFAULT_CLEAR_COLOR: [u8; 4] = [12, 18, 32, 255];

fn default_clear() -> [u8; 4] {
    DEFAULT_CLEAR_COLOR
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
    /// Source `*.prefab.json` (game-relative path or stem). Missing = not an instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab: Option<String>,
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

    /// Quaternion stored as scene `[x, y, z, w]`. Degenerate → identity.
    pub fn as_quat(&self) -> Quat {
        quat_from_xyzw(self.rotation)
    }

    /// Unity-like 2D TRS: scale multiplies component-wise (axis-aligned; non-uniform
    /// scale + rotation does **not** introduce shear), rotation is `parent * local`,
    /// and the local offset is scaled by the parent then rotated by the parent quat.
    pub fn compose_child(parent: &Self, local: &Self) -> Self {
        let parent_rot = parent.as_quat();
        let scaled_offset = Vec3::new(
            local.translation[0] * parent.scale[0],
            local.translation[1] * parent.scale[1],
            local.translation[2] * parent.scale[2],
        );
        let rotated = parent_rot * scaled_offset;
        Self {
            translation: [
                parent.translation[0] + rotated.x,
                parent.translation[1] + rotated.y,
                parent.translation[2] + rotated.z,
            ],
            rotation: quat_to_xyzw(parent_rot * local.as_quat()),
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
        let inv_rot = parent_world.as_quat().inverse();
        let delta = Vec3::new(
            world.translation[0] - parent_world.translation[0],
            world.translation[1] - parent_world.translation[1],
            world.translation[2] - parent_world.translation[2],
        );
        let unrotated = inv_rot * delta;
        Self {
            translation: [unrotated.x / sx, unrotated.y / sy, unrotated.z / sz],
            rotation: quat_to_xyzw(inv_rot * world.as_quat()),
            scale: [
                world.scale[0] / sx,
                world.scale[1] / sy,
                world.scale[2] / sz,
            ],
        }
    }
}

fn quat_from_xyzw(q: [f32; 4]) -> Quat {
    let q = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
    if q.length_squared() < 1e-12 {
        Quat::IDENTITY
    } else {
        q.normalize()
    }
}

fn quat_to_xyzw(q: Quat) -> [f32; 4] {
    let q = if q.length_squared() < 1e-12 {
        Quat::IDENTITY
    } else {
        q.normalize()
    };
    [q.x, q.y, q.z, q.w]
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
    #[serde(default, rename = "Animation", skip_serializing_if = "Option::is_none")]
    pub animation: Option<SceneAnimation>,
    #[serde(default, rename = "GridMover", skip_serializing_if = "Option::is_none")]
    pub grid_mover: Option<SceneGridMover>,
    #[serde(
        default,
        rename = "AudioSource",
        skip_serializing_if = "Option::is_none"
    )]
    pub audio_source: Option<SceneAudioSource>,
    #[serde(default, rename = "Text", skip_serializing_if = "Option::is_none")]
    pub text: Option<SceneText>,
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
    /// Unity Sorting Layer name. Empty / omitted / `"Default"` → Default.
    #[serde(default, skip_serializing_if = "is_default_sorting_layer_name")]
    pub sorting_layer: String,
    /// When false, skipped by hydrate / pick / bake (Unity component checkbox).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Optional normalized pivot override (`[0,0]` top-left, `[0.5,0.5]` center).
    /// Omitted / `None` → catalog cell pivot (unchanged default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<[f32; 2]>,
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

    /// Display name: empty → [`DEFAULT_SORTING_LAYER`].
    pub fn sorting_layer_name(&self) -> &str {
        display_sorting_layer(&self.sorting_layer)
    }

    /// Component override, else `fallback` (typically the catalog cell pivot).
    pub fn effective_pivot(&self, fallback: [f32; 2]) -> [f32; 2] {
        self.pivot.unwrap_or(fallback)
    }
}

/// Empty layer name means Unity Default.
pub fn display_sorting_layer(name: &str) -> &str {
    let t = name.trim();
    if t.is_empty() {
        DEFAULT_SORTING_LAYER
    } else {
        t
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneDisc {
    pub radius: f32,
    #[serde(default = "mintish")]
    pub color: [u8; 4],
    #[serde(default)]
    pub z: f32,
    /// Unity Sorting Layer name. Empty / omitted / `"Default"` → Default.
    #[serde(default, skip_serializing_if = "is_default_sorting_layer_name")]
    pub sorting_layer: String,
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

    pub fn sorting_layer_name(&self) -> &str {
        display_sorting_layer(&self.sorting_layer)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneCamera {
    #[serde(default = "default_true")]
    pub active: bool,
    /// Named entity to track (empty / omitted = no follow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<String>,
    /// Follow lerp factor (`0` frozen, `1` snap). Default `0.15`.
    #[serde(
        default = "default_follow_lerp",
        skip_serializing_if = "is_default_follow_lerp"
    )]
    pub lerp: f32,
}

fn default_follow_lerp() -> f32 {
    0.15
}

fn is_default_follow_lerp(v: &f32) -> bool {
    (*v - default_follow_lerp()).abs() < 1e-6
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
/// WSCN0003 `KIND_TILEMAP` bakes the grid plus a resolved palette for GX.
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
    /// Unity Sorting Layer name. Empty / omitted / `"Default"` → Default.
    #[serde(default, skip_serializing_if = "is_default_sorting_layer_name")]
    pub sorting_layer: String,
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
            palette: vec![SceneTilePalette::new(1)],
            z: -1.0,
            sorting_layer: String::new(),
            enabled: true,
        }
    }

    pub fn sorting_layer_name(&self) -> &str {
        display_sorting_layer(&self.sorting_layer)
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

    /// NESW bitmask for cell `(x, y)`. Uses the cell's palette `auto_tile` (default `id`).
    pub fn autotile_mask(&self, x: i32, y: i32) -> u8 {
        let (id, _) = self.get(x, y);
        let mode = self
            .palette
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.auto_tile)
            .unwrap_or(SceneAutoTile::Id);
        self.autotile_mask_mode(x, y, mode)
    }

    pub fn autotile_mask_mode(&self, x: i32, y: i32, mode: SceneAutoTile) -> u8 {
        let (id, _) = self.get(x, y);
        wiimaker_core::tilemap::autotile_bits(
            self.neighbor_match(x, y - 1, id, mode),
            self.neighbor_match(x + 1, y, id, mode),
            self.neighbor_match(x, y + 1, id, mode),
            self.neighbor_match(x - 1, y, id, mode),
        )
    }

    fn neighbor_match(&self, nx: i32, ny: i32, id: u16, mode: SceneAutoTile) -> bool {
        match mode {
            SceneAutoTile::Id => self.in_bounds(nx, ny) && id != 0 && self.get(nx, ny).0 == id,
            SceneAutoTile::Solid => {
                if self.in_bounds(nx, ny) {
                    self.get(nx, ny).1
                } else {
                    true
                }
            }
        }
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

/// 4-neighbor auto-tile rule stored on a palette entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SceneAutoTile {
    /// Same non-zero palette id (water / lava blobs).
    Id,
    /// Neighbor solid bits; out-of-bounds counts as solid (maze walls).
    Solid,
}

impl SceneAutoTile {
    pub fn as_str(self) -> &'static str {
        match self {
            SceneAutoTile::Id => "id",
            SceneAutoTile::Solid => "solid",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "id" | "same" | "same-id" | "same_id" => Some(SceneAutoTile::Id),
            "solid" | "wall" => Some(SceneAutoTile::Solid),
            _ => None,
        }
    }

    pub fn to_runtime(self) -> wiimaker_core::tilemap::AutoTileMatch {
        match self {
            SceneAutoTile::Id => wiimaker_core::tilemap::AutoTileMatch::Id,
            SceneAutoTile::Solid => wiimaker_core::tilemap::AutoTileMatch::Solid,
        }
    }
}

fn skip_empty_opt(v: &Option<String>) -> bool {
    v.as_ref().map(|s| s.is_empty()).unwrap_or(true)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneTilePalette {
    pub id: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    #[serde(default = "wall_color")]
    pub color: [u8; 4],
    /// Optional `assets/<clip>.anim.json` stem (Unity AnimatedTile).
    #[serde(default, skip_serializing_if = "skip_empty_opt")]
    pub anim: Option<String>,
    /// When set, overrides the clip file's fps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_fps: Option<f32>,
    /// 4-neighbor auto-tile (Unity RuleTile analogue). Omitted = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_tile: Option<SceneAutoTile>,
    /// Up to 16 sprite names indexed by NESW bitmask (N=1 E=2 S=4 W=8).
    /// Empty slots (and a short list) fall back to `{sprite}_{mask}` then `sprite`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_sprites: Vec<String>,
}

impl Default for SceneTilePalette {
    fn default() -> Self {
        Self::new(1)
    }
}

impl SceneTilePalette {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            sprite: None,
            color: wall_color(),
            anim: None,
            anim_fps: None,
            auto_tile: None,
            auto_sprites: Vec::new(),
        }
    }

    pub fn color_rgba(&self) -> Rgba8 {
        Rgba8::new(self.color[0], self.color[1], self.color[2], self.color[3])
    }

    pub fn anim_clip(&self) -> Option<&str> {
        self.anim
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// Sprite clip player (Unity Animator analogue). `clip` is `assets/<clip>.anim.json` stem.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneAnimation {
    pub clip: String,
    /// When set, overrides the clip file's fps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f32>,
    #[serde(
        default = "default_true",
        rename = "loop",
        skip_serializing_if = "is_true"
    )]
    pub loop_: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl Default for SceneAnimation {
    fn default() -> Self {
        Self {
            clip: String::new(),
            fps: None,
            loop_: true,
            enabled: true,
        }
    }
}

impl SceneAnimation {
    pub fn new(clip: impl Into<String>) -> Self {
        Self {
            clip: clip.into(),
            ..Default::default()
        }
    }
}

/// 4-way grid-snap mover (Pac-Man queued cardinals). Host-first; not in WSCN.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneGridMover {
    #[serde(default = "default_grid_cell")]
    pub cell: f32,
    /// World units per second.
    #[serde(default = "default_grid_speed")]
    pub speed: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_dir: Option<SceneDir>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

fn default_grid_cell() -> f32 {
    16.0
}
fn default_grid_speed() -> f32 {
    120.0
}

impl Default for SceneGridMover {
    fn default() -> Self {
        Self {
            cell: default_grid_cell(),
            speed: default_grid_speed(),
            queued_dir: None,
            enabled: true,
        }
    }
}

impl SceneGridMover {
    pub fn new(cell: f32, speed: f32) -> Self {
        Self {
            cell: if cell <= 0.0 {
                default_grid_cell()
            } else {
                cell
            },
            speed: speed.max(0.0),
            queued_dir: None,
            enabled: true,
        }
    }
}

/// Oneshot clip (Unity AudioSource analogue). Host plays `assets/*.wav`;
/// WSCN0003 bakes `KIND_AUDIO` and/or a trailing AudioSource table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneAudioSource {
    /// `assets/<clip>.wav` stem (or `name.wav`).
    #[serde(default)]
    pub clip: String,
    #[serde(
        default = "default_audio_volume",
        skip_serializing_if = "is_default_audio_volume"
    )]
    pub volume: f32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub play_on_awake: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

fn default_audio_volume() -> f32 {
    1.0
}

fn is_default_audio_volume(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-6
}

impl Default for SceneAudioSource {
    fn default() -> Self {
        Self {
            clip: String::new(),
            volume: default_audio_volume(),
            play_on_awake: false,
            enabled: true,
        }
    }
}

impl SceneAudioSource {
    pub fn new(clip: impl Into<String>, volume: f32, play_on_awake: bool) -> Self {
        Self {
            clip: clip.into(),
            volume: volume.clamp(0.0, 1.0),
            play_on_awake,
            enabled: true,
        }
    }
}

fn default_text_size() -> f32 {
    16.0
}

fn is_default_text_size(v: &f32) -> bool {
    (*v - default_text_size()).abs() < 1e-6
}

fn is_default_text_align(v: &SceneTextAlign) -> bool {
    *v == SceneTextAlign::Left
}

/// Host HUD string (Unity Text analogue). WSCN0003 `KIND_TEXT`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneText {
    #[serde(default)]
    pub text: String,
    #[serde(
        default = "default_text_size",
        skip_serializing_if = "is_default_text_size"
    )]
    pub size: f32,
    #[serde(default = "white4")]
    pub color: [u8; 4],
    #[serde(default, skip_serializing_if = "is_default_text_align")]
    pub align: SceneTextAlign,
    #[serde(default)]
    pub z: f32,
    /// Unity Sorting Layer name. Empty / omitted / `"Default"` → Default.
    #[serde(default, skip_serializing_if = "is_default_sorting_layer_name")]
    pub sorting_layer: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl Default for SceneText {
    fn default() -> Self {
        Self {
            text: String::new(),
            size: default_text_size(),
            color: white4(),
            align: SceneTextAlign::Left,
            z: 0.0,
            sorting_layer: String::new(),
            enabled: true,
        }
    }
}

impl SceneText {
    pub fn new(text: impl Into<String>, size: f32, color: [u8; 4]) -> Self {
        Self {
            text: text.into(),
            size: if size <= 0.0 {
                default_text_size()
            } else {
                size
            },
            color,
            align: SceneTextAlign::Left,
            z: 0.0,
            sorting_layer: String::new(),
            enabled: true,
        }
    }

    pub fn color_rgba(&self) -> Rgba8 {
        Rgba8::new(self.color[0], self.color[1], self.color[2], self.color[3])
    }

    pub fn sorting_layer_name(&self) -> &str {
        display_sorting_layer(&self.sorting_layer)
    }

    pub fn align_runtime(&self) -> wiimaker_core::TextAlign {
        self.align.to_runtime()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "PascalCase")]
pub enum SceneTextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl SceneTextAlign {
    pub fn to_runtime(self) -> wiimaker_core::TextAlign {
        match self {
            SceneTextAlign::Left => wiimaker_core::TextAlign::Left,
            SceneTextAlign::Center => wiimaker_core::TextAlign::Center,
            SceneTextAlign::Right => wiimaker_core::TextAlign::Right,
        }
    }

    pub fn from_runtime(a: wiimaker_core::TextAlign) -> Self {
        match a {
            wiimaker_core::TextAlign::Left => SceneTextAlign::Left,
            wiimaker_core::TextAlign::Center => SceneTextAlign::Center,
            wiimaker_core::TextAlign::Right => SceneTextAlign::Right,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        wiimaker_core::TextAlign::parse(s).map(Self::from_runtime)
    }

    /// WSCN `KIND_TEXT` align byte: 0 Left, 1 Center, 2 Right.
    pub fn to_wscn(self) -> u8 {
        match self {
            SceneTextAlign::Left => 0,
            SceneTextAlign::Center => 1,
            SceneTextAlign::Right => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum SceneDir {
    Up,
    Down,
    Left,
    Right,
}

impl SceneDir {
    pub fn to_runtime(self) -> wiimaker_core::Dir {
        match self {
            SceneDir::Up => wiimaker_core::Dir::Up,
            SceneDir::Down => wiimaker_core::Dir::Down,
            SceneDir::Left => wiimaker_core::Dir::Left,
            SceneDir::Right => wiimaker_core::Dir::Right,
        }
    }

    pub fn from_runtime(d: wiimaker_core::Dir) -> Self {
        match d {
            wiimaker_core::Dir::Up => SceneDir::Up,
            wiimaker_core::Dir::Down => SceneDir::Down,
            wiimaker_core::Dir::Left => SceneDir::Left,
            wiimaker_core::Dir::Right => SceneDir::Right,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "up" => Some(SceneDir::Up),
            "down" => Some(SceneDir::Down),
            "left" => Some(SceneDir::Left),
            "right" => Some(SceneDir::Right),
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rot_z_deg(deg: f32) -> [f32; 4] {
        let half = deg.to_radians() * 0.5;
        [0.0, 0.0, half.sin(), half.cos()]
    }

    fn xf(x: f32, y: f32, rot_deg: f32, sx: f32, sy: f32) -> SceneTransform {
        SceneTransform {
            translation: [x, y, 0.0],
            rotation: rot_z_deg(rot_deg),
            scale: [sx, sy, 1.0],
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn near3(a: [f32; 3], b: [f32; 3]) -> bool {
        near(a[0], b[0]) && near(a[1], b[1]) && near(a[2], b[2])
    }

    /// Quaternions q and −q are the same rotation.
    fn near_quat(a: [f32; 4], b: [f32; 4]) -> bool {
        let d =
            (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs() + (a[3] - b[3]).abs();
        let opp =
            (a[0] + b[0]).abs() + (a[1] + b[1]).abs() + (a[2] + b[2]).abs() + (a[3] + b[3]).abs();
        d < 1e-4 || opp < 1e-4
    }

    fn assert_xf_eq(got: &SceneTransform, want: &SceneTransform) {
        assert!(
            near3(got.translation, want.translation),
            "translation {:?} != {:?}",
            got.translation,
            want.translation
        );
        assert!(
            near_quat(got.rotation, want.rotation),
            "rotation {:?} != {:?}",
            got.rotation,
            want.rotation
        );
        assert!(
            near3(got.scale, want.scale),
            "scale {:?} != {:?}",
            got.scale,
            want.scale
        );
    }

    #[test]
    fn compose_identity_rotation_is_translate_times_scale() {
        let parent = xf(100.0, 50.0, 0.0, 2.0, 3.0);
        let local = xf(10.0, 20.0, 0.0, 0.5, 2.0);
        let world = SceneTransform::compose_child(&parent, &local);
        assert!(near3(world.translation, [120.0, 110.0, 0.0]));
        assert!(near_quat(world.rotation, rot_z_deg(0.0)));
        assert!(near3(world.scale, [1.0, 6.0, 1.0]));
    }

    #[test]
    fn compose_parent_45_child_plus_x() {
        let parent = xf(320.0, 240.0, 45.0, 1.0, 1.0);
        let local = xf(80.0, 0.0, 0.0, 1.0, 1.0);
        let world = SceneTransform::compose_child(&parent, &local);
        let s = 45f32.to_radians().sin();
        let c = 45f32.to_radians().cos();
        assert!(near3(
            world.translation,
            [320.0 + 80.0 * c, 240.0 + 80.0 * s, 0.0]
        ));
        assert!(near_quat(world.rotation, rot_z_deg(45.0)));
        assert!(near3(world.scale, [1.0, 1.0, 1.0]));
    }

    #[test]
    fn compose_parent_90_child_plus_x() {
        let parent = xf(0.0, 0.0, 90.0, 1.0, 1.0);
        let local = xf(80.0, 0.0, 0.0, 1.0, 1.0);
        let world = SceneTransform::compose_child(&parent, &local);
        assert!(near3(world.translation, [0.0, 80.0, 0.0]));
        assert!(near_quat(world.rotation, rot_z_deg(90.0)));
    }

    #[test]
    fn compose_nested_grandparent_rotation() {
        let mut scene = Scene::new("orbit");
        scene.entities.push(EntityData {
            name: "gp".into(),
            parent: None,
            transform: xf(10.0, 20.0, 90.0, 1.0, 1.0),
            components: SceneComponents::default(),
            tag: 0,
            prefab: None,
        });
        scene.entities.push(EntityData {
            name: "p".into(),
            parent: Some("gp".into()),
            transform: xf(10.0, 0.0, 90.0, 1.0, 1.0),
            components: SceneComponents::default(),
            tag: 0,
            prefab: None,
        });
        scene.entities.push(EntityData {
            name: "c".into(),
            parent: Some("p".into()),
            transform: xf(10.0, 0.0, 0.0, 1.0, 1.0),
            components: SceneComponents::default(),
            tag: 0,
            prefab: None,
        });
        // parent world: (10,20) + rot90(10,0) = (10,30), rot 180°
        // child world: (10,30) + rot180(10,0) = (0,30), rot 180°
        let child = scene.world_transform("c").unwrap();
        assert!(near3(child.translation, [0.0, 30.0, 0.0]));
        assert!(near_quat(child.rotation, rot_z_deg(180.0)));
        let parent = scene.world_transform("p").unwrap();
        assert!(near3(parent.translation, [10.0, 30.0, 0.0]));
        assert!(near_quat(parent.rotation, rot_z_deg(180.0)));
    }

    #[test]
    fn compose_child_to_local_roundtrip() {
        let cases = [
            xf(320.0, 240.0, 45.0, 1.0, 1.0),
            xf(100.0, 50.0, 90.0, 2.0, 0.5),
            xf(-8.0, 12.0, -30.0, 1.5, 1.5),
            xf(0.0, 0.0, 0.0, 1.0, 1.0),
        ];
        let locals = [
            xf(80.0, 0.0, 0.0, 1.0, 1.0),
            xf(10.0, 20.0, 15.0, 0.5, 2.0),
            xf(-4.0, 7.0, -90.0, 1.0, 1.0),
        ];
        for parent in &cases {
            for local in &locals {
                let world = SceneTransform::compose_child(parent, local);
                let back = SceneTransform::to_local(parent, &world);
                assert_xf_eq(&back, local);
                let again = SceneTransform::compose_child(parent, &back);
                assert_xf_eq(&again, &world);
            }
        }
    }

    #[test]
    fn world_transform_rotparent_hello_orb() {
        let mut scene = Scene::new("hello");
        scene.entities.push(EntityData {
            name: "RotParent".into(),
            parent: None,
            transform: xf(320.0, 240.0, 45.0, 1.0, 1.0),
            components: SceneComponents::default(),
            tag: 0,
            prefab: None,
        });
        scene.entities.push(EntityData {
            name: "RotChild".into(),
            parent: Some("RotParent".into()),
            transform: xf(80.0, 0.0, 0.0, 1.0, 1.0),
            components: SceneComponents::default(),
            tag: 0,
            prefab: None,
        });
        let child = scene.world_transform("RotChild").unwrap();
        let s = 45f32.to_radians().sin();
        let c = 45f32.to_radians().cos();
        assert!(near3(
            child.translation,
            [320.0 + 80.0 * c, 240.0 + 80.0 * s, 0.0]
        ));
    }
}
