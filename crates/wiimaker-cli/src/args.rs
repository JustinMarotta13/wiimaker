use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::util::parse_rgb;

#[derive(Parser, Debug)]
#[command(name = "wiimaker", about = "Build Wii games with a host-first loop")]
pub struct Cli {
    /// Emit machine-readable JSON where applicable
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Scaffold a new game crate under games/
    New { name: String },
    /// Run a game on the host
    Run { name: String },
    /// Open the egui scene editor
    Edit { name: String },
    /// Prepare assets → `.wpack` (advanced / agents; prefer `build`)
    Cook {
        name: String,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Bake scene.wscn for Wii embed (advanced; requires prepared assets)
    BakeWii { name: String },
    /// Build Wii `.dol` (prepare + bake + Docker). Alias: `build-wii`
    #[command(alias = "build-wii")]
    Build { name: String },
    /// Launch existing `target/wii/<game>/boot.dol` in Dolphin
    Dolphin { name: String },
    /// Build then launch in Dolphin
    PlayWii { name: String },
    /// Validate project / scene / assets
    Doctor { name: String },
    /// Scene operations
    Scene {
        #[command(subcommand)]
        cmd: SceneCmd,
    },
    /// Entity operations
    Entity {
        #[command(subcommand)]
        cmd: EntityCmd,
    },
    /// Asset operations
    Asset {
        #[command(subcommand)]
        cmd: AssetCmd,
    },
    /// Tilemap paint / query (solid cells)
    Tilemap {
        #[command(subcommand)]
        cmd: TilemapCmd,
    },
    /// Editor chrome prefs (`<game>/.wiimaker/prefs.toml`)
    Editor {
        #[command(subcommand)]
        cmd: EditorCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum SceneCmd {
    List {
        game: String,
    },
    Show {
        game: String,
        scene: Option<String>,
    },
    /// Create `scenes/<name>.scene.json`
    New {
        game: String,
        #[arg(long)]
        name: String,
    },
    /// Persist `game.toml` default_scene (Build Settings analogue)
    SetDefault {
        game: String,
        #[arg(long)]
        scene: String,
    },
    /// Print `game.toml` Scenes in Build list
    BuildList {
        game: String,
    },
    /// Append a scene to `game.toml` scenes
    BuildAdd {
        game: String,
        #[arg(long)]
        scene: String,
    },
    /// Remove a scene from `game.toml` scenes
    BuildRemove {
        game: String,
        #[arg(long)]
        scene: String,
    },
    SetClear {
        game: String,
        #[arg(long, value_parser = parse_rgb)]
        rgb: [u8; 3],
    },
    /// Game view aspect / resolution (writes `.wiimaker/prefs.toml`)
    SetGameView {
        game: String,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        /// `free` (fill well) or `fixed` (letterbox to width×height / preset)
        #[arg(long)]
        aspect: Option<String>,
        /// `free` | `640x480` | `16:9` | `4:3` | `custom`
        #[arg(long)]
        preset: Option<String>,
        /// Scale slider (1 = contain-fit)
        #[arg(long)]
        scale: Option<f32>,
    },
}

#[derive(Subcommand, Debug)]
pub enum EditorCmd {
    /// Print `.wiimaker/prefs.toml` (defaults if missing)
    Prefs { game: String },
    /// Scene view zoom / pan / grid / gizmos / snap
    SetSceneView {
        game: String,
        #[arg(long)]
        zoom: Option<f32>,
        #[arg(long)]
        pan_x: Option<f32>,
        #[arg(long)]
        pan_y: Option<f32>,
        #[arg(long, action = clap::ArgAction::Set)]
        grid: Option<bool>,
        #[arg(long, action = clap::ArgAction::Set)]
        gizmos: Option<bool>,
        #[arg(long, action = clap::ArgAction::Set)]
        snap: Option<bool>,
        #[arg(long)]
        snap_size: Option<f32>,
        /// Must stay true (2D-only engine)
        #[arg(long, action = clap::ArgAction::Set)]
        mode_2d: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
pub enum EntityCmd {
    List {
        game: String,
        #[arg(long)]
        scene: Option<String>,
    },
    Add {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        sprite: Option<String>,
        #[arg(long)]
        x: Option<f32>,
        #[arg(long)]
        y: Option<f32>,
        #[arg(long)]
        radius: Option<f32>,
        #[arg(long)]
        scene: Option<String>,
    },
    Set {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        x: Option<f32>,
        #[arg(long)]
        y: Option<f32>,
        /// Local X scale
        #[arg(long)]
        sx: Option<f32>,
        /// Local Y scale
        #[arg(long)]
        sy: Option<f32>,
        /// Z rotation in degrees (2D)
        #[arg(long)]
        rotation_deg: Option<f32>,
        /// Entity gameplay tag (trigger filter counterpart)
        #[arg(long)]
        tag: Option<u32>,
        /// Camera follow target entity name (creates Camera if missing; empty string clears)
        #[arg(long)]
        follow: Option<String>,
        /// Camera follow lerp (`0` frozen, `1` snap). Use with `--follow`.
        #[arg(long)]
        lerp: Option<f32>,
        /// GridMover cell size (creates GridMover if missing)
        #[arg(long)]
        cell: Option<f32>,
        /// GridMover speed in world units per second
        #[arg(long)]
        speed: Option<f32>,
        /// GridMover queued cardinal: Up, Down, Left, Right (empty string clears)
        #[arg(long)]
        queued_dir: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    AddComponent {
        game: String,
        #[arg(long)]
        name: String,
        /// Component kind: Sprite, Disc, Tilemap, Collider, Trigger, Animation, Camera, Follow, or GridMover
        kind: String,
        #[arg(long)]
        texture: Option<String>,
        /// Sprite / AABB collider width (`--w`)
        #[arg(long, visible_alias = "w", default_value_t = 32.0)]
        width: f32,
        /// Sprite / AABB collider height (`--h`)
        #[arg(long, visible_alias = "h", default_value_t = 32.0)]
        height: f32,
        #[arg(long, default_value_t = 36.0)]
        radius: f32,
        /// Tilemap / GridMover cell size in world units
        #[arg(long, default_value_t = 16.0)]
        cell: f32,
        /// Tilemap grid width (cells)
        #[arg(long, default_value_t = 32)]
        cols: u32,
        /// Tilemap grid height (cells)
        #[arg(long, default_value_t = 18)]
        rows: u32,
        /// Collider shape: Aabb (default) or Circle
        #[arg(long, default_value = "Aabb")]
        shape: String,
        /// Collider is a wall (default true; ignored when trigger)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        solid: bool,
        /// Mark collider as Unity-style isTrigger
        #[arg(long, default_value_t = false)]
        trigger: bool,
        /// Trigger filter tag (0 = any)
        #[arg(long, default_value_t = 0)]
        filter: u32,
        /// Animation clip stem (`assets/<clip>.anim.json`)
        #[arg(long)]
        clip: Option<String>,
        /// Animation fps override (omit to use clip file)
        #[arg(long)]
        fps: Option<f32>,
        /// Animation loop (default true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        r#loop: bool,
        /// Follow target entity name (`Follow` / `Camera`)
        #[arg(long)]
        target: Option<String>,
        /// Follow lerp factor (`0` frozen, `1` snap). Default `0.15` for Follow.
        #[arg(long)]
        lerp: Option<f32>,
        /// GridMover speed in world units per second (default 120)
        #[arg(long, default_value_t = 120.0)]
        speed: f32,
        /// GridMover queued cardinal: Up, Down, Left, Right
        #[arg(long)]
        queued_dir: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    Remove {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Deep-clone an entity (unique name, +16,+16 offset)
    Duplicate {
        game: String,
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Rename an entity (fails if new name empty or taken)
    Rename {
        game: String,
        old: String,
        new: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Parent an entity under another (omit --parent to unparent / make root)
    SetParent {
        game: String,
        #[arg(long)]
        name: String,
        /// New parent entity name. Omit to move to scene root.
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Remove a component (Sprite or Disc) from an entity
    RemoveComponent {
        game: String,
        #[arg(long)]
        name: String,
        /// Component kind: Sprite, Disc, Tilemap, Collider, Animation, Camera, Follow, or GridMover (Follow removal clears fields; use entity set --follow "" to clear)
        kind: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Enable or disable a component (Unity-style checkbox)
    SetComponentEnabled {
        game: String,
        #[arg(long)]
        name: String,
        /// Component kind: Sprite, Disc, Tilemap, Collider, Animation, Camera, or GridMover
        kind: String,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Query collider overlap (`--name` vs `--other`, or list all hits)
    Overlaps {
        game: String,
        #[arg(long, visible_alias = "a")]
        name: String,
        #[arg(long, visible_alias = "b")]
        other: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// List trigger overlaps entered by an entity
    Triggers {
        game: String,
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Attach or update Animation clip on an entity
    SetAnim {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        clip: String,
        #[arg(long)]
        fps: Option<f32>,
        #[arg(long, action = clap::ArgAction::Set)]
        r#loop: Option<bool>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Remove an entity from the scene (Unity Destroy / despawn)
    Despawn {
        game: String,
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Save an entity as `assets/prefabs/<name>.prefab.json`
    CreatePrefab {
        game: String,
        #[arg(long)]
        name: String,
        /// Prefab file stem (defaults to entity name)
        #[arg(long)]
        as_name: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Instantiate a prefab into the scene
    InstantiatePrefab {
        game: String,
        /// Prefab stem or path relative to game (e.g. player or assets/prefabs/player.prefab.json)
        prefab: String,
        #[arg(long)]
        x: Option<f32>,
        #[arg(long)]
        y: Option<f32>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Overwrite entity transform/components from a prefab (keeps name/parent)
    ApplyPrefab {
        game: String,
        #[arg(long)]
        name: String,
        prefab: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Unpack prefab instance (v0: no-op beyond verifying entity exists)
    UnpackPrefab {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AssetCmd {
    List {
        game: String,
    },
    Import {
        game: String,
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Grid-slice a sheet PNG into `assets/<stem>.sprites.json`
    Slice {
        game: String,
        /// Sheet stem (e.g. basic_space_suit)
        sheet: String,
        #[arg(long)]
        cols: u32,
        #[arg(long)]
        rows: u32,
    },
    /// Set normalized pivot on a named cell
    SetPivot {
        game: String,
        /// Cell name (e.g. basic_space_suit_2)
        sprite: String,
        #[arg(long)]
        x: f32,
        #[arg(long)]
        y: f32,
    },
    /// List catalog sprite names (sheets + cells)
    ListSprites {
        game: String,
    },
    /// Create / overwrite `assets/<name>.anim.json`
    Anim {
        game: String,
        /// Clip stem (e.g. chomp)
        name: String,
        /// Comma-separated cell names
        #[arg(long)]
        cells: String,
        #[arg(long, default_value_t = 10.0)]
        fps: f32,
        /// Loop playback (default true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        r#loop: bool,
    },
    /// List `*.anim.json` clip stems
    ListAnims {
        game: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum TilemapCmd {
    /// Set one cell id + solid flag
    Set {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        x: i32,
        #[arg(long)]
        y: i32,
        #[arg(long)]
        id: u16,
        /// Override solid (default: true when id != 0)
        #[arg(long, action = clap::ArgAction::Set)]
        solid: Option<bool>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Fill a rectangle of cells
    Fill {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        x: i32,
        #[arg(long)]
        y: i32,
        #[arg(long)]
        w: i32,
        #[arg(long)]
        h: i32,
        #[arg(long)]
        id: u16,
        #[arg(long, action = clap::ArgAction::Set)]
        solid: Option<bool>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Stamp ASCII (`#` wall, `.` empty) or a flat `--cells` buffer
    Stamp {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 0)]
        x: i32,
        #[arg(long, default_value_t = 0)]
        y: i32,
        /// ASCII rows separated by newlines
        #[arg(long)]
        ascii: Option<String>,
        /// Comma-separated cell ids (row-major)
        #[arg(long)]
        cells: Option<String>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Read one cell or dump the whole tilemap
    Get {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        x: Option<i32>,
        #[arg(long)]
        y: Option<i32>,
        #[arg(long)]
        scene: Option<String>,
    },
}
