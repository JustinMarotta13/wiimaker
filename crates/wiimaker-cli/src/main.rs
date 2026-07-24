//! `wiimaker` CLI — scaffold, cook, scene/entity edits, doctor, edit.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use wiimaker_assets::WPack;
use wiimaker_scene::{
    add_component_disc, add_component_sprite, add_entity, diagnose, find_game_dir, load_project,
    load_scene, remove_entity, save_scene, set_entity_transform, set_scene_clear, write_scene_wscn,
    GameProject, MutateOpts, Scene,
};

#[derive(Parser, Debug)]
#[command(name = "wiimaker", about = "Build Wii games with a host-first loop")]
struct Cli {
    /// Emit machine-readable JSON where applicable
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Scaffold a new game crate under games/
    New { name: String },
    /// Run a game on the host
    Run { name: String },
    /// Open the egui scene editor
    Edit { name: String },
    /// Cook assets for a game (from game.toml)
    Cook {
        name: String,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Bake scene.wscn for Wii embed (requires cooked wpack)
    BakeWii { name: String },
    /// Cross-build via Docker (runtime/wii)
    BuildWii { name: String },
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
}

#[derive(Subcommand, Debug)]
enum SceneCmd {
    List { game: String },
    Show {
        game: String,
        scene: Option<String>,
    },
    SetClear {
        game: String,
        #[arg(long, value_parser = parse_rgb)]
        rgb: [u8; 3],
    },
}

#[derive(Subcommand, Debug)]
enum EntityCmd {
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
        #[arg(long)]
        scene: Option<String>,
    },
    AddComponent {
        game: String,
        #[arg(long)]
        name: String,
        /// Component kind: Sprite or Disc
        kind: String,
        #[arg(long)]
        texture: Option<String>,
        #[arg(long, default_value_t = 32.0)]
        width: f32,
        #[arg(long, default_value_t = 32.0)]
        height: f32,
        #[arg(long, default_value_t = 36.0)]
        radius: f32,
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
}

#[derive(Subcommand, Debug)]
enum AssetCmd {
    List { game: String },
    Import {
        game: String,
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = find_root()?;
    match cli.cmd {
        Cmd::New { name } => new_game(&root, &name, cli.json),
        Cmd::Run { name } => {
            let status = Command::new("cargo")
                .args(["run", "-p", &name])
                .current_dir(&root)
                .status()?;
            if !status.success() {
                bail!("cargo run failed");
            }
            Ok(())
        }
        Cmd::Edit { name } => {
            let status = Command::new("cargo")
                .args(["run", "-p", "wiimaker-editor", "--", &name])
                .current_dir(&root)
                .status()?;
            if !status.success() {
                bail!("editor failed");
            }
            Ok(())
        }
        Cmd::Cook { name, input, output } => cook_game(&root, &name, input, output, cli.json),
        Cmd::BakeWii { name } => bake_wii_game(&root, &name, cli.json),
        Cmd::BuildWii { name } => {
            let script = root.join("tools/wii-build.sh");
            let status = Command::new(&script).arg(&name).current_dir(&root).status()?;
            if !status.success() {
                bail!("wii build failed");
            }
            Ok(())
        }
        Cmd::Doctor { name } => doctor_game(&root, &name, cli.json),
        Cmd::Scene { cmd } => scene_cmd(&root, cmd, cli.json),
        Cmd::Entity { cmd } => entity_cmd(&root, cmd, cli.json),
        Cmd::Asset { cmd } => asset_cmd(&root, cmd, cli.json),
    }
}

fn find_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("runtime/wii").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("not inside a wiimaker workspace");
        }
    }
}

fn new_game(root: &Path, name: &str, json: bool) -> Result<()> {
    let dest = root.join("games").join(name);
    if dest.exists() {
        bail!("{dest:?} already exists");
    }
    let template = root.join("templates/basic-game");
    copy_dir(&template, &dest)?;
    // Rewrite placeholders in text files
    for rel in [
        "Cargo.toml",
        "src/main.rs",
        "game.toml",
        "scenes/main.scene.json",
    ] {
        let path = dest.join(rel);
        if path.is_file() {
            let contents = fs::read_to_string(&path)?.replace("{{name}}", name);
            fs::write(path, contents)?;
        }
    }
    fs::create_dir_all(dest.join("assets"))?;

    let ws = root.join("Cargo.toml");
    let mut ws_toml = fs::read_to_string(&ws)?;
    let entry = format!("    \"games/{name}\",\n");
    if !ws_toml.contains(&format!("games/{name}")) {
        if let Some(idx) = ws_toml.find("games/hello-orb\",\n]") {
            let insert_at = idx + "games/hello-orb\",\n".len();
            ws_toml.insert_str(insert_at, &entry);
            fs::write(ws, ws_toml)?;
        }
    }

    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            created: &'a str,
            path: String,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                created: name,
                path: dest.display().to_string(),
            })?
        );
    } else {
        println!("created games/{name}");
        println!("  wiimaker cook {name}");
        println!("  wiimaker run {name}");
        println!("  wiimaker edit {name}");
    }
    Ok(())
}

fn cook_game(
    root: &Path,
    name: &str,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    // Legacy: --input/--output without a project
    if let (Some(input), Some(output)) = (&input, &output) {
        return cook_dir(input, output, json);
    }

    let game_dir = find_game_dir(root, name)?;
    let project = if game_dir.join("game.toml").is_file() {
        load_project(&game_dir)?
    } else {
        GameProject::new(name)
    };
    let assets = input.unwrap_or_else(|| project.assets_path(&game_dir));
    let out = output.unwrap_or_else(|| project.wpack_path(&game_dir));
    cook_dir(&assets, &out, json)
}

fn cook_dir(input: &Path, output: &Path, json: bool) -> Result<()> {
    let mut pack = WPack::new();
    let warnings = pack.cook_dir(input)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    pack.write_to(output)?;
    if json {
        #[derive(Serialize)]
        struct Out {
            output: String,
            textures: usize,
            warnings: Vec<String>,
        }
        let msgs: Vec<_> = warnings.iter().map(|w| w.message.clone()).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                output: output.display().to_string(),
                textures: pack.textures.len(),
                warnings: msgs,
            })?
        );
    } else {
        for w in &warnings {
            println!("warn {}: {}", w.texture, w.message);
        }
        println!(
            "wrote {} ({} textures)",
            output.display(),
            pack.textures.len()
        );
    }
    Ok(())
}

fn bake_wii_game(root: &Path, name: &str, json: bool) -> Result<()> {
    let game_dir = find_game_dir(root, name)?;
    let project = load_project(&game_dir)?;
    let wpack_path = project.wpack_path(&game_dir);
    if !wpack_path.is_file() {
        bail!(
            "missing {} — run `wiimaker cook {name}` first",
            wpack_path.display()
        );
    }
    let pack = WPack::read_from(&wpack_path)?;
    let scene = load_scene(&project.scene_path(&game_dir))?;
    let out = project.wscn_path(&game_dir);
    write_scene_wscn(&out, &scene, &pack)?;
    if json {
        #[derive(Serialize)]
        struct Out {
            output: String,
            entities: usize,
            textures: usize,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                output: out.display().to_string(),
                entities: scene.entities.len(),
                textures: pack.textures.len(),
            })?
        );
    } else {
        println!(
            "wrote {} ({} entities, {} textures)",
            out.display(),
            scene.entities.len(),
            pack.textures.len()
        );
    }
    Ok(())
}

fn doctor_game(root: &Path, name: &str, json: bool) -> Result<()> {
    let game_dir = find_game_dir(root, name)?;
    let project = if game_dir.join("game.toml").is_file() {
        load_project(&game_dir)?
    } else {
        bail!("missing game.toml in {}", game_dir.display());
    };
    let diag = diagnose(&game_dir, &project);
    if json {
        println!("{}", serde_json::to_string_pretty(&diag)?);
    } else {
        println!("doctor {} — {}", diag.game, if diag.ok { "ok" } else { "issues" });
        for issue in &diag.issues {
            println!("  [{:?}] {}", issue.severity, issue.message);
        }
    }
    if !diag.ok {
        bail!("doctor found errors");
    }
    Ok(())
}

fn open_scene(root: &Path, game: &str, scene_rel: Option<&str>) -> Result<(PathBuf, GameProject, PathBuf, Scene)> {
    let game_dir = find_game_dir(root, game)?;
    let project = load_project(&game_dir)?;
    let scene_path = match scene_rel {
        Some(rel) => game_dir.join(rel),
        None => project.scene_path(&game_dir),
    };
    let scene = load_scene(&scene_path)?;
    Ok((game_dir, project, scene_path, scene))
}

fn scene_cmd(root: &Path, cmd: SceneCmd, json: bool) -> Result<()> {
    match cmd {
        SceneCmd::List { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let scenes = game_dir.join("scenes");
            let mut names = Vec::new();
            if scenes.is_dir() {
                for entry in fs::read_dir(&scenes)? {
                    let path = entry?.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(n) = path.file_name().and_then(|s| s.to_str()) {
                            names.push(n.to_string());
                        }
                    }
                }
            }
            names.sort();
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        SceneCmd::Show { game, scene } => {
            let (_gd, _p, path, scene) = open_scene(root, &game, scene.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&scene)?);
            } else {
                println!("{} ({})", scene.name, path.display());
                println!("clear {:?}", scene.clear_color);
                for e in &scene.entities {
                    println!("  - {}", e.name);
                }
            }
            Ok(())
        }
        SceneCmd::SetClear { game, rgb } => {
            let (_gd, _p, path, mut scene) = open_scene(root, &game, None)?;
            set_scene_clear(&mut scene, rgb);
            save_scene(&path, &scene)?;
            emit_ok(json, "scene clear updated")
        }
    }
}

fn entity_cmd(root: &Path, cmd: EntityCmd, json: bool) -> Result<()> {
    match cmd {
        EntityCmd::List { game, scene } => {
            let (_gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let names: Vec<_> = sc.entities.iter().map(|e| e.name.clone()).collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&sc.entities)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        EntityCmd::Add {
            game,
            name,
            sprite,
            x,
            y,
            radius,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            add_entity(
                &mut sc,
                &name,
                &MutateOpts {
                    x,
                    y,
                    sprite,
                    radius,
                    ..Default::default()
                },
            )?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("added entity {name}"))
        }
        EntityCmd::Set {
            game,
            name,
            x,
            y,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            set_entity_transform(&mut sc, &name, x, y)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("updated entity {name}"))
        }
        EntityCmd::AddComponent {
            game,
            name,
            kind,
            texture,
            width,
            height,
            radius,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            match kind.to_ascii_lowercase().as_str() {
                "sprite" => {
                    let tex = texture.ok_or_else(|| anyhow::anyhow!("--texture required for Sprite"))?;
                    add_component_sprite(&mut sc, &name, &tex, [width, height])?;
                }
                "disc" => {
                    add_component_disc(&mut sc, &name, radius, [72, 210, 160, 255])?;
                }
                other => bail!("unknown component kind '{other}' (Sprite|Disc)"),
            }
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("added {kind} to {name}"))
        }
        EntityCmd::Remove { game, name, scene } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            remove_entity(&mut sc, &name)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("removed entity {name}"))
        }
    }
}

fn asset_cmd(root: &Path, cmd: AssetCmd, json: bool) -> Result<()> {
    match cmd {
        AssetCmd::List { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let mut names = Vec::new();
            if assets.is_dir() {
                for entry in fs::read_dir(&assets)? {
                    let path = entry?.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("png") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
            names.sort();
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        AssetCmd::Import { game, path, name } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            fs::create_dir_all(&assets)?;
            let stem = name.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("tex")
                    .to_string()
            });
            let dest = assets.join(format!("{stem}.png"));
            fs::copy(&path, &dest).with_context(|| format!("copy {} → {}", path.display(), dest.display()))?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    imported: String,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        imported: dest.display().to_string()
                    })?
                );
            } else {
                println!("imported {}", dest.display());
            }
            Ok(())
        }
    }
}

fn emit_ok(json: bool, msg: &str) -> Result<()> {
    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            message: &'a str,
        }
        println!("{}", serde_json::to_string(&Out { ok: true, message: msg })?);
    } else {
        println!("{msg}");
    }
    Ok(())
}

fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let parts: Vec<_> = s.split(',').collect();
    if parts.len() != 3 {
        return Err("expected R,G,B".into());
    }
    let parse = |p: &str| p.trim().parse::<u8>().map_err(|e| e.to_string());
    Ok([parse(parts[0])?, parse(parts[1])?, parse(parts[2])?])
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}
