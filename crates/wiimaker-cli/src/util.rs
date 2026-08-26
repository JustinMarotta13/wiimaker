use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

pub fn find_root() -> Result<PathBuf> {
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

pub fn emit_ok(json: bool, msg: &str) -> Result<()> {
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

pub fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let parts: Vec<_> = s.split(',').collect();
    if parts.len() != 3 {
        return Err("expected R,G,B".into());
    }
    let parse = |p: &str| p.trim().parse::<u8>().map_err(|e| e.to_string());
    Ok([parse(parts[0])?, parse(parts[1])?, parse(parts[2])?])
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
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
