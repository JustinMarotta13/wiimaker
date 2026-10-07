use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::util::{parse_rgb, parse_rgba};

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
    /// Tilemap paint / query (solid cells, palette anim / auto-tile)
    Tilemap {
        #[command(subcommand)]
        cmd: TilemapCmd,
    },
    /// Editor chrome prefs (`<game>/.wiimaker/prefs.toml`)
    Editor {
        #[command(subcommand)]
        cmd: EditorCmd,
    },
    /// Project Sorting Layers (Unity Tags & Layers analogue)
    SortingLayer {
        #[command(subcommand)]
        cmd: SortingLayerCmd,
    },
    /// GCN-layout pad map (keyboard / Wiimote / Classic / Nunchuk)
    Input {
        #[command(subcommand)]
        cmd: InputCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum InputCmd {
    /// Print the GCN-layout mapping table (`--json` for agents)
    Map,
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
    /// Whether in-editor Play can load the game `App` cdylib (no new prefs)
    PlayStatus {
        game: String,
        /// `cargo build -p <game> --lib` first
        #[arg(long)]
        build: bool,
    },
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
    /// Project explorer collapsed folders (writes `.wiimaker/prefs.toml`)
    SetProjectView {
        game: String,
        /// Collapse a relative folder (`assets`, `assets/fx`). Repeatable.
        #[arg(long)]
        collapse: Vec<String>,
        /// Expand a previously collapsed folder. Repeatable.
        #[arg(long)]
        expand: Vec<String>,
        /// Clear the collapsed list (all expanded). Applied before `--collapse`.
        #[arg(long)]
        clear_collapsed: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SortingLayerCmd {
    /// Print `game.toml` sorting_layers (defaults if omitted)
    List { game: String },
    /// Append or insert a named layer
    Add {
        game: String,
        #[arg(long)]
        name: String,
        /// Insert at this index (0 = back / drawn first). Omit to append.
        #[arg(long)]
        index: Option<usize>,
    },
    /// Rename a layer and remap scene/prefab assignments
    Rename {
        game: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
    },
    /// Reorder a layer (`--index 0` draws first)
    Move {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        index: usize,
    },
    /// Remove a layer (not Default); assignments remap to Default
    Remove {
        game: String,
        #[arg(long)]
        name: String,
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
        /// AudioSource clip stem (`assets/<clip>.wav`); creates AudioSource if missing
        #[arg(long)]
        audio_clip: Option<String>,
        /// AudioSource volume 0..1
        #[arg(long)]
        volume: Option<f32>,
        /// AudioSource play-on-awake
        #[arg(long, action = clap::ArgAction::Set)]
        play_on_awake: Option<bool>,
        /// HUD Text string (creates Text if missing)
        #[arg(long)]
        text: Option<String>,
        /// HUD Text glyph size in world pixels
        #[arg(long)]
        size: Option<f32>,
        /// HUD Text color `R,G,B` or `R,G,B,A`
        #[arg(long, value_parser = parse_rgba)]
        color: Option<[u8; 4]>,
        /// HUD Text align: Left, Center, Right
        #[arg(long)]
        align: Option<String>,
        /// Sorting Layer name (Sprite/Disc/Tilemap/Text). Empty string = Default.
        #[arg(long)]
        sorting_layer: Option<String>,
        /// Order in Layer (component `z` within the sorting layer)
        #[arg(long, visible_alias = "z")]
        order_in_layer: Option<f32>,
        /// Sprite component pivot X (normalized). Requires a Sprite.
        #[arg(long)]
        pivot_x: Option<f32>,
        /// Sprite component pivot Y (normalized). Requires a Sprite.
        #[arg(long)]
        pivot_y: Option<f32>,
        /// Clear Sprite pivot override (fall back to catalog).
        #[arg(long)]
        clear_pivot: bool,
        /// Animator controller stem (`assets/<stem>.controller.json`)
        #[arg(long)]
        controller: Option<String>,
        /// PlayableDirector timeline stem (`assets/<stem>.timeline.json`)
        #[arg(long)]
        timeline: Option<String>,
        /// PlayableDirector loop (creates the component if missing)
        #[arg(long = "loop", action = clap::ArgAction::Set)]
        timeline_loop: Option<bool>,
        #[arg(long)]
        scene: Option<String>,
    },
    AddComponent {
        game: String,
        #[arg(long)]
        name: String,
        /// Component kind: Sprite, Disc, Tilemap, Collider, Trigger, Animation, Animator, PlayableDirector, Camera, Follow, GridMover, AudioSource, or Text
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
        /// Animation clip stem (`assets/<clip>.anim.json`) or AudioSource clip (`assets/<clip>.wav`)
        #[arg(long)]
        clip: Option<String>,
        /// Animation fps override (omit to use clip file)
        #[arg(long)]
        fps: Option<f32>,
        /// Animation loop (default true). PlayableDirector loop (default false) shares this flag.
        #[arg(long, action = clap::ArgAction::Set)]
        r#loop: Option<bool>,
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
        /// AudioSource volume 0..1 (default 1)
        #[arg(long, default_value_t = 1.0)]
        volume: f32,
        /// AudioSource play-on-awake (default false). PlayableDirector play-on-awake (default true).
        #[arg(long, action = clap::ArgAction::Set)]
        play_on_awake: Option<bool>,
        /// HUD Text string (default "Text")
        #[arg(long)]
        text: Option<String>,
        /// HUD Text glyph size in world pixels (default 16)
        #[arg(long, default_value_t = 16.0)]
        size: f32,
        /// HUD Text color `R,G,B` or `R,G,B,A` (default 255,255,255,255)
        #[arg(long, value_parser = parse_rgba)]
        color: Option<[u8; 4]>,
        /// HUD Text align: Left, Center, Right (default Left)
        #[arg(long)]
        align: Option<String>,
        /// Sprite pivot X override (normalized)
        #[arg(long)]
        pivot_x: Option<f32>,
        /// Sprite pivot Y override (normalized)
        #[arg(long)]
        pivot_y: Option<f32>,
        /// Animator controller stem (`assets/<stem>.controller.json`)
        #[arg(long)]
        controller: Option<String>,
        /// PlayableDirector timeline stem (`assets/<stem>.timeline.json`)
        #[arg(long)]
        timeline: Option<String>,
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
        /// Component kind: Sprite, Disc, Tilemap, Collider, Animation, Animator, PlayableDirector, Camera, Follow, GridMover, AudioSource, or Text (Follow removal clears fields; use entity set --follow "" to clear)
        kind: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Enable or disable a component (Unity-style checkbox)
    SetComponentEnabled {
        game: String,
        #[arg(long)]
        name: String,
        /// Component kind: Sprite, Disc, Tilemap, Collider, Animation, Animator, PlayableDirector, Camera, GridMover, AudioSource, or Text
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
    /// Set Animator parameters (`--bool Moving=true`, `--float Speed=1`, `--trigger Jump`)
    AnimatorSet {
        game: String,
        #[arg(long)]
        name: String,
        /// Bool parameter `Name=true|false` (repeatable)
        #[arg(long = "bool")]
        bools: Vec<String>,
        /// Float parameter `Name=1.0` (repeatable)
        #[arg(long = "float")]
        floats: Vec<String>,
        /// Trigger parameter name (repeatable; stored as bool true)
        #[arg(long = "trigger")]
        triggers: Vec<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Report Animator controller, state, and parameters (`--json`)
    AnimatorStatus {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Start a PlayableDirector (hydrate, `playing: true`). `--json` reports transport.
    TimelinePlay {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Stop a PlayableDirector and snap time to 0. `--json` reports transport.
    TimelineStop {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Report PlayableDirector time, playing, finished, timeline stem, and bound XY (`--json`)
    TimelineStatus {
        game: String,
        #[arg(long)]
        name: String,
        /// Tick this many times before reporting (0 = current playhead, usually 0)
        #[arg(long, default_value_t = 0)]
        steps: u32,
        /// Seconds per step when `--steps` is set
        #[arg(long, default_value_t = 1.0 / 60.0)]
        dt: f32,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Tick a PlayableDirector and report signals fired along the way (`--json`)
    TimelineSignals {
        game: String,
        #[arg(long)]
        name: String,
        /// Seconds per tick (default 1/60)
        #[arg(long, default_value_t = 1.0 / 60.0)]
        dt: f32,
        /// How many ticks to accumulate (default 1)
        #[arg(long, default_value_t = 1)]
        steps: u32,
        /// Call play before ticking (default true). Restarts a finished director.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        play: bool,
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
    /// Save an entity (+ Hierarchy children) as `assets/prefabs/<name>.prefab.json`
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
    /// Create a prefab variant inheriting from a base (`--from`) or from an instance (`--name`)
    CreateVariant {
        game: String,
        /// Base prefab stem/path (create empty variant identical to base)
        #[arg(long)]
        from: Option<String>,
        /// Prefab instance entity — variant captures current overrides vs its link
        #[arg(long)]
        name: Option<String>,
        /// Variant file stem (required)
        #[arg(long)]
        as_name: String,
        /// Relink the instance to the new variant (only with `--name`)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        relink: bool,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Instantiate a prefab into the scene (root + nested children)
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
    /// Push instance overrides onto the prefab asset, or onto its base (`--to-base`)
    ApplyPrefab {
        game: String,
        /// Instance entity. Required unless `--to-base --prefab` targets a variant asset.
        #[arg(long)]
        name: Option<String>,
        /// Prefab stem or path (instance link when omitted; variant asset with `--to-base`)
        #[arg(long)]
        prefab: Option<String>,
        /// Write values into the base prefab and drop those paths from the variant overrides
        #[arg(long)]
        to_base: bool,
        /// With `--to-base`, write the root prefab instead of the immediate base
        #[arg(long)]
        to_root: bool,
        /// Override path (`Disc.color`, `Eye/Disc.radius`). Repeatable. Omit to apply all.
        #[arg(long = "field")]
        fields: Vec<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Report the base prefab an editor Open Base / Select Base would focus (`--json`)
    OpenBase {
        game: String,
        /// Prefab instance entity
        #[arg(long)]
        name: Option<String>,
        /// Variant asset stem or path (instead of `--name`)
        #[arg(long)]
        prefab: Option<String>,
        /// Target the root base instead of the immediate base
        #[arg(long)]
        root: bool,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Reset instance tree from the prefab asset (Unity Revert)
    RevertPrefab {
        game: String,
        #[arg(long)]
        name: String,
        /// Prefab stem or path (defaults to the entity's instance link)
        prefab: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Clear the prefab instance link; current values stay as a plain entity
    UnpackPrefab {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Report prefab link + overridden properties (`--json`)
    PrefabStatus {
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
    /// Create / overwrite `assets/<name>.controller.json`
    Controller {
        game: String,
        /// Controller stem (e.g. player)
        name: String,
        /// Default state name
        #[arg(long = "default")]
        default_state: Option<String>,
        /// States `Name:clip` comma-separated (e.g. Idle:idle,Walk:walk)
        #[arg(long)]
        states: Option<String>,
        /// Parameter `Name:Bool=false` or `Name:Float=0` (repeatable)
        #[arg(long = "param")]
        params: Vec<String>,
        /// Transition `From>To:Param=true` (repeatable). Prefix From with Any for any-state.
        #[arg(long)]
        transition: Vec<String>,
        /// Read a full controller JSON from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// List `*.controller.json` stems
    ListControllers {
        game: String,
    },
    /// Create / overwrite `assets/<name>.timeline.json`
    Timeline {
        game: String,
        /// Timeline stem (e.g. intro)
        name: String,
        #[arg(long, default_value_t = 4.0)]
        duration: f32,
        /// Track `Name:Kind:Binding:start-end:payload` (repeatable).
        /// Activation payload `true|false`. Animation payload is a clip stem.
        /// Audio payload `stem` or `stem:volume`. Transform payload `x,y>x,y`
        /// with optional `|x:t,v,interp;...|y:...`.
        /// Signal payload `SignalName` or `SignalName|text` (range is `t-t`).
        /// Control payload is empty; binding is the target entity.
        /// Float payload is `Transform.rotation|value:t,v,interp;...`
        /// (`Transform.scale_x`, `Transform.scale_y`, `Sprite.alpha` too).
        /// Binding `-` is unbound (Signal and Audio).
        /// Full JSON via `--stdin` may include `curves`.
        #[arg(long = "track")]
        tracks: Vec<String>,
        /// Read a full timeline JSON from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// List `*.timeline.json` stems
    ListTimelines {
        game: String,
    },
    /// Keyframed curves on a timeline clip
    TimelineCurve {
        #[command(subcommand)]
        cmd: TimelineCurveCmd,
    },
    /// List `*.wav` clip stems under assets/
    ListWavs {
        game: String,
    },
    /// Play a oneshot WAV on the host (`assets/<name>.wav`)
    Play {
        game: String,
        /// Clip stem or path (`beep` or `beep.wav`)
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 1.0)]
        volume: f32,
        /// Wait for the clip to finish when a device is present (default true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        wait: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum TimelineCurveCmd {
    /// Insert a key. Creates the curve when it is missing.
    AddKey {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        /// `x`, `y`, or `value`
        #[arg(long)]
        prop: String,
        /// Clip-local seconds
        #[arg(long, allow_negative_numbers = true)]
        t: f32,
        #[arg(long, allow_negative_numbers = true)]
        v: f32,
        /// `linear`, `constant`, or `ease` (default linear)
        #[arg(long, default_value = "linear")]
        interp: String,
    },
    /// Remove a key by `--index` or clip-local `--t`
    RemoveKey {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        #[arg(long)]
        prop: String,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long, allow_negative_numbers = true)]
        t: Option<f32>,
    },
    /// Move a key (time is clamped by the caller; stored as given)
    MoveKey {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        #[arg(long)]
        prop: String,
        #[arg(long)]
        index: usize,
        #[arg(long, allow_negative_numbers = true)]
        t: f32,
        #[arg(long, allow_negative_numbers = true)]
        v: f32,
    },
    /// Set interpolation on one key
    SetInterp {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        #[arg(long)]
        prop: String,
        #[arg(long)]
        index: usize,
        #[arg(long)]
        interp: String,
    },
    /// Seed an X or Y curve from the clip `from`→`to` (or `value` on a Float track)
    AddCurve {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        /// `x`, `y`, or `value`
        #[arg(long)]
        prop: String,
    },
    /// Drop one curve. The axis falls back to `from`→`to`.
    RemoveCurve {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        #[arg(long)]
        prop: String,
    },
    /// Sample a curve. `--from`/`--to` are clip-local seconds.
    Sample {
        game: String,
        #[arg(long)]
        timeline: String,
        #[arg(long)]
        track: String,
        #[arg(long, default_value_t = 0)]
        clip: usize,
        #[arg(long)]
        prop: String,
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        from: f32,
        #[arg(long, default_value_t = 1.0, allow_negative_numbers = true)]
        to: f32,
        /// Number of samples (inclusive endpoints when >= 2)
        #[arg(long, default_value_t = 5)]
        steps: usize,
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
    /// Load an ASCII maze file into a Tilemap (`#` wall, `.` empty; resizes to fit)
    FromAscii {
        game: String,
        /// UTF-8 maze file (cwd, game dir, or `assets/`)
        file: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 0)]
        x: i32,
        #[arg(long, default_value_t = 0)]
        y: i32,
        /// Grow/shrink the grid so the stamp fits (default true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        resize: bool,
        /// Glyph map, e.g. `#=1,.=0,P=2:0` (`id:solid`; solid defaults to id != 0)
        #[arg(long)]
        map: Option<String>,
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
    /// Create / update a palette entry (anim clip + auto-tile rule)
    SetPalette {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        id: u16,
        /// Sprite catalog name (empty string clears)
        #[arg(long)]
        sprite: Option<String>,
        #[arg(long, value_parser = parse_rgba)]
        color: Option<[u8; 4]>,
        /// `assets/<clip>.anim.json` stem (empty string clears)
        #[arg(long)]
        anim: Option<String>,
        /// Clip fps override (`0` clears)
        #[arg(long)]
        fps: Option<f32>,
        /// Auto-tile: `id`, `solid`, or `off`
        #[arg(long)]
        auto_tile: Option<String>,
        /// Comma-separated NESW variant sprites (index = bitmask)
        #[arg(long)]
        auto_sprites: Option<String>,
        #[arg(long)]
        scene: Option<String>,
    },
    /// NESW auto-tile bitmask for one cell (N=1 E=2 S=4 W=8)
    Mask {
        game: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        x: i32,
        #[arg(long)]
        y: i32,
        #[arg(long)]
        scene: Option<String>,
    },
}
