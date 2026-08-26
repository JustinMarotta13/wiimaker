use std::path::PathBuf;

use anyhow::{bail, Result};

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
