//! Atomic file replace (write temp + rename).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Write `contents` to `path`, replacing any existing file atomically on Unix.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .context("atomic_write: path has no file name")?;
    let tmp = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{}.{}", file_name.to_string_lossy(), std::process::id()));
    fs::write(&tmp, contents).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "rename {} → {}",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}
