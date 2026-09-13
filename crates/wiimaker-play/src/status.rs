//! Discover / build a game play plugin (CLI `editor play-status`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::plugin::{dylib_path, LoadedPlugin};
use crate::PLAY_ABI_VERSION;

/// Set to `1` to force editor Play onto the WASD fallback.
pub const FORCE_FALLBACK_ENV: &str = "WIIMAKER_PLAY_FALLBACK";

#[derive(Clone, Debug)]
pub struct PlayStatus {
    pub package: String,
    pub game_dir: PathBuf,
    pub cdylib: bool,
    pub dylib: PathBuf,
    pub dylib_exists: bool,
    pub abi: Option<u32>,
    pub backend: &'static str,
    pub hint: Option<String>,
    pub force_fallback: bool,
}

impl PlayStatus {
    pub fn backend_is_plugin(&self) -> bool {
        self.backend == "plugin"
    }
}

/// `true` when the game `Cargo.toml` requests a `cdylib` (string match is enough).
pub fn package_has_cdylib(game_dir: &Path) -> bool {
    let toml = game_dir.join("Cargo.toml");
    std::fs::read_to_string(toml)
        .map(|s| s.contains("cdylib"))
        .unwrap_or(false)
}

pub fn inspect_play_plugin(workspace: &Path, package: &str, game_dir: &Path) -> PlayStatus {
    let force_fallback = std::env::var(FORCE_FALLBACK_ENV).ok().as_deref() == Some("1");
    let cdylib = package_has_cdylib(game_dir);
    let dylib = dylib_path(workspace, package);
    let dylib_exists = dylib.is_file();
    let abi = if dylib_exists {
        LoadedPlugin::abi_only(&dylib).ok()
    } else {
        None
    };
    let (backend, hint) = if force_fallback {
        (
            "fallback",
            Some(format!("{FORCE_FALLBACK_ENV}=1 forces WASD fallback")),
        )
    } else if cdylib && dylib_exists && abi == Some(PLAY_ABI_VERSION) {
        ("plugin", None)
    } else if cdylib && !dylib_exists {
        (
            "fallback",
            Some("cdylib listed but not built — `wiimaker editor play-status --build` or editor Play".into()),
        )
    } else if !cdylib {
        (
            "fallback",
            Some(
                "add src/lib.rs + `crate-type = [\"cdylib\", \"rlib\"]` (see templates/basic-game)"
                    .into(),
            ),
        )
    } else {
        (
            "fallback",
            Some("plugin dylib present but ABI mismatch or unloadable".into()),
        )
    };
    PlayStatus {
        package: package.to_string(),
        game_dir: game_dir.to_path_buf(),
        cdylib,
        dylib,
        dylib_exists,
        abi,
        backend,
        hint,
        force_fallback,
    }
}

/// `cargo build -p <package> --lib` in the workspace (debug cdylib).
pub fn build_play_plugin(workspace: &Path, package: &str) -> Result<String> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let status = Command::new(cargo)
        .args(["build", "-p", package, "--lib"])
        .current_dir(workspace)
        .status()
        .context("spawn cargo build --lib")?;
    if !status.success() {
        bail!("cargo build -p {package} --lib failed");
    }
    let path = dylib_path(workspace, package);
    if path.is_file() {
        Ok(format!("built play plugin {}", path.display()))
    } else {
        Ok(format!(
            "cargo build -p {package} --lib ok (dylib {})",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cdylib_in_toml() {
        let dir = tempfile_dir();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n",
        )
        .unwrap();
        assert!(package_has_cdylib(&dir));
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert!(!package_has_cdylib(&dir));
    }

    fn tempfile_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "wiimaker-play-status-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
