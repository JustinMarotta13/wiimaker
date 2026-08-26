use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{diagnose, find_game_dir, load_project};

use crate::pipeline;
use crate::util::copy_dir;

pub fn new_game(root: &Path, name: &str, json: bool) -> Result<()> {
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
        println!("  wiimaker build {name}");
        println!("  wiimaker run {name}");
        println!("  wiimaker edit {name}");
    }
    Ok(())
}

pub fn doctor_game(root: &Path, name: &str, json: bool) -> Result<()> {
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
        println!(
            "doctor {} — {}",
            diag.game,
            if diag.ok { "ok" } else { "issues" }
        );
        for issue in &diag.issues {
            println!("  [{:?}] {}", issue.severity, issue.message);
        }
    }
    if !diag.ok {
        bail!("doctor found errors");
    }
    Ok(())
}

pub fn run_game(root: &Path, name: &str) -> Result<()> {
    let status = Command::new("cargo")
        .args(["run", "-p", name])
        .current_dir(root)
        .status()?;
    if !status.success() {
        bail!("cargo run failed");
    }
    Ok(())
}

pub fn edit_game(root: &Path, name: &str) -> Result<()> {
    let status = Command::new("cargo")
        .args(["run", "-p", "wiimaker-editor", "--", name])
        .current_dir(root)
        .status()?;
    if !status.success() {
        bail!("editor failed");
    }
    Ok(())
}

pub fn cook(root: &Path, name: &str, input: Option<std::path::PathBuf>, output: Option<std::path::PathBuf>, json: bool) -> Result<()> {
    let out = pipeline::prepare_assets(root, name, input, output)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for w in &out.warnings {
            println!("warn {w}");
        }
        println!("wrote {} ({} textures)", out.output, out.textures);
    }
    Ok(())
}

pub fn bake_wii(root: &Path, name: &str, json: bool) -> Result<()> {
    let out = pipeline::bake_wii(root, name)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "wrote {} ({} entities, {} textures)",
            out.output, out.entities, out.textures
        );
    }
    Ok(())
}

pub fn build(root: &Path, name: &str, json: bool) -> Result<()> {
    let out = pipeline::build_dol(root, name)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("ready: {}", out.dol);
        println!("HBC pack: {}", out.hbc);
    }
    Ok(())
}

pub fn dolphin(root: &Path, name: &str, json: bool) -> Result<()> {
    let dol = pipeline::run_dolphin(root, name)?;
    if json {
        #[derive(Serialize)]
        struct Out {
            dol: String,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                dol: dol.display().to_string(),
            })?
        );
    } else {
        println!("launched {}", dol.display());
    }
    Ok(())
}

pub fn play_wii(root: &Path, name: &str, json: bool) -> Result<()> {
    let out = pipeline::play_wii(root, name)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("ready: {}", out.dol);
    }
    Ok(())
}
