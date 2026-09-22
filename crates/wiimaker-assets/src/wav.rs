//! PCM16 WAV helpers for host oneshots and the `.wpack` audio TOC.
//!
//! Clips live as `assets/<stem>.wav` on disk (host `aplay` / editor preview)
//! and cook into `WPACK001` as LE PCM16 (name, rate, channels, blob) for Wii ASND.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

/// Header facts for a PCM16 WAV (host playback + doctor).
#[derive(Clone, Debug, PartialEq)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub frame_count: u32,
}

impl WavInfo {
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frame_count as f32 / self.sample_rate as f32
        }
    }
}

/// Stem names of every `*.wav` under `assets_dir` (non-recursive).
pub fn list_wav_clips(assets_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !assets_dir.is_dir() {
        return Ok(names);
    }
    for entry in fs::read_dir(assets_dir)? {
        let path = entry?.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("wav"))
            != Some(true)
        {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Resolve a clip name or path to an existing `.wav` under `assets_dir`.
///
/// Accepts `beep`, `beep.wav`, or `assets/beep.wav` (relative to the assets dir
/// or as a filename). Missing files return a clear error (caller may no-op).
pub fn resolve_wav(assets_dir: &Path, name: &str) -> Result<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        bail!("audio clip name is empty");
    }
    let as_path = Path::new(name);
    if as_path.is_absolute() {
        if as_path.is_file() {
            return Ok(as_path.to_path_buf());
        }
        bail!("audio clip not found: {}", as_path.display());
    }
    let candidates = [
        assets_dir.join(name),
        assets_dir.join(format!("{name}.wav")),
        assets_dir.join(as_path.file_name().unwrap_or_else(|| as_path.as_os_str())),
    ];
    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }
    let stem = as_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .trim_end_matches(".wav");
    bail!(
        "audio clip '{name}' missing (expected {})",
        assets_dir.join(format!("{stem}.wav")).display()
    );
}

/// Inspect a WAV; require PCM16 mono or stereo.
pub fn inspect_wav(path: impl AsRef<Path>) -> Result<WavInfo> {
    let path = path.as_ref();
    let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut riff = [0u8; 12];
    f.read_exact(&mut riff)
        .with_context(|| format!("read RIFF header {}", path.display()))?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        bail!("{}: not a RIFF/WAVE file", path.display());
    }
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // format, channels, rate, bits
    let mut data_bytes: Option<u32> = None;
    loop {
        let mut id = [0u8; 4];
        match f.read_exact(&mut id) {
            Ok(()) => {}
            Err(_) => break,
        }
        let size = f.read_u32::<LittleEndian>()?;
        if &id == b"fmt " {
            if size < 16 {
                bail!("{}: fmt chunk too small", path.display());
            }
            let format = f.read_u16::<LittleEndian>()?;
            let channels = f.read_u16::<LittleEndian>()?;
            let rate = f.read_u32::<LittleEndian>()?;
            let _byte_rate = f.read_u32::<LittleEndian>()?;
            let _block = f.read_u16::<LittleEndian>()?;
            let bits = f.read_u16::<LittleEndian>()?;
            let rest = size.saturating_sub(16);
            if rest > 0 {
                let mut skip = vec![0u8; rest as usize];
                f.read_exact(&mut skip)?;
            }
            fmt = Some((format, channels, rate, bits));
        } else if &id == b"data" {
            data_bytes = Some(size);
            break;
        } else {
            let mut skip = vec![0u8; size as usize];
            f.read_exact(&mut skip)?;
        }
        if size % 2 == 1 {
            let mut pad = [0u8; 1];
            let _ = f.read_exact(&mut pad);
        }
    }
    let Some((format, channels, rate, bits)) = fmt else {
        bail!("{}: missing fmt chunk", path.display());
    };
    if format != 1 {
        bail!(
            "{}: WAV must be PCM (format 1), got {format} — convert to PCM16 mono/stereo",
            path.display()
        );
    }
    if bits != 16 {
        bail!(
            "{}: WAV must be 16-bit PCM (ASND-friendly), got {bits}-bit",
            path.display()
        );
    }
    if channels != 1 && channels != 2 {
        bail!(
            "{}: WAV must be mono or stereo, got {channels} channels",
            path.display()
        );
    }
    let Some(nbytes) = data_bytes else {
        bail!("{}: missing data chunk", path.display());
    };
    let block = channels as u32 * 2;
    let frame_count = if block == 0 { 0 } else { nbytes / block };
    Ok(WavInfo {
        sample_rate: rate,
        channels,
        bits_per_sample: bits,
        frame_count,
    })
}

/// Load PCM16 samples (interleaved). Re-opens the file after [`inspect_wav`].
pub fn load_pcm16_wav(path: impl AsRef<Path>) -> Result<(WavInfo, Vec<i16>)> {
    let path = path.as_ref();
    let info = inspect_wav(path)?;
    let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header)?;
    loop {
        let mut id = [0u8; 4];
        f.read_exact(&mut id)?;
        let size = f.read_u32::<LittleEndian>()?;
        if &id == b"data" {
            let mut bytes = vec![0u8; size as usize];
            f.read_exact(&mut bytes)?;
            let mut samples = Vec::with_capacity(bytes.len() / 2);
            for chunk in bytes.chunks_exact(2) {
                samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
            }
            return Ok((info, samples));
        }
        let mut skip = vec![0u8; size as usize];
        f.read_exact(&mut skip)?;
        if size % 2 == 1 {
            let mut pad = [0u8; 1];
            let _ = f.read_exact(&mut pad);
        }
    }
}

/// Write a PCM16 WAV (little-endian). Used by tests / tiny fixtures.
pub fn write_pcm16_wav(
    path: impl AsRef<Path>,
    sample_rate: u32,
    channels: u16,
    samples: &[i16],
) -> Result<PathBuf> {
    if channels != 1 && channels != 2 {
        bail!("channels must be 1 or 2");
    }
    if sample_rate == 0 {
        bail!("sample_rate must be > 0");
    }
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let block_align = channels * 2;
    let byte_rate = sample_rate * u32::from(block_align);
    let data_len = (samples.len() * 2) as u32;
    let mut f = File::create(path).with_context(|| format!("create {}", path.display()))?;
    f.write_all(b"RIFF")?;
    f.write_u32::<LittleEndian>(36 + data_len)?;
    f.write_all(b"WAVE")?;
    f.write_all(b"fmt ")?;
    f.write_u32::<LittleEndian>(16)?;
    f.write_u16::<LittleEndian>(1)?; // PCM
    f.write_u16::<LittleEndian>(channels)?;
    f.write_u32::<LittleEndian>(sample_rate)?;
    f.write_u32::<LittleEndian>(byte_rate)?;
    f.write_u16::<LittleEndian>(block_align)?;
    f.write_u16::<LittleEndian>(16)?;
    f.write_all(b"data")?;
    f.write_u32::<LittleEndian>(data_len)?;
    for s in samples {
        f.write_i16::<LittleEndian>(*s)?;
    }
    Ok(path.to_path_buf())
}

/// Short 440 Hz PCM16 mono beep (for templates / tests).
pub fn write_beep_wav(path: impl AsRef<Path>, duration_secs: f32) -> Result<PathBuf> {
    let rate = 22050u32;
    let n = ((duration_secs.max(0.02) * rate as f32) as usize).max(16);
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let env = if i < n / 10 {
            i as f32 / (n / 10) as f32
        } else if i > n * 8 / 10 {
            (n - i) as f32 / (n / 10) as f32
        } else {
            1.0
        };
        let v = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * env * 0.35;
        samples.push((v * i16::MAX as f32) as i16);
    }
    write_pcm16_wav(path, rate, 1, &samples)
}

/// Launch a host WAV player (`aplay` / `paplay` / `pw-play` / `ffplay`).
pub fn spawn_wav_player(path: &Path) -> Result<Option<std::process::Child>> {
    inspect_wav(path)?;
    const CANDIDATES: &[(&str, &[&str])] = &[
        ("aplay", &["-q"]),
        ("paplay", &[]),
        ("pw-play", &[]),
        ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
    ];
    let path_env = std::env::var("PATH").unwrap_or_default();
    let mut chosen = None;
    for (bin, args) in CANDIDATES {
        let found = path_env
            .split(':')
            .any(|dir| Path::new(dir).join(bin).is_file());
        if found {
            chosen = Some((*bin, *args));
            break;
        }
    }
    let Some((bin, args)) = chosen else {
        return Ok(None);
    };
    let mut cmd = std::process::Command::new(bin);
    for a in args {
        cmd.arg(a);
    }
    match cmd
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => Ok(Some(child)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_inspect_roundtrip_pcm16_mono() {
        let dir = std::env::temp_dir().join(format!("wiimaker-wav-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("beep.wav");
        write_beep_wav(&path, 0.05).unwrap();
        let info = inspect_wav(&path).unwrap();
        assert_eq!(info.channels, 1);
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(info.sample_rate, 22050);
        assert!(info.duration_secs() > 0.04);
        let names = list_wav_clips(&dir).unwrap();
        assert!(names.iter().any(|n| n == "beep"));
        let resolved = resolve_wav(&dir, "beep").unwrap();
        assert_eq!(resolved, path);
        assert!(resolve_wav(&dir, "nope").is_err());
    }
}
