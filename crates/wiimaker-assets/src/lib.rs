//! `.wpack` — packed asset archive for Wii + host.
//!
//! Design goals (from CavEX / wii-3d-engine lessons):
//! - Offline conversion only — no PNG decode on console
//! - 32-byte aligned blobs for GX
//! - TOC small enough to mmap from DVD / SD

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use image::{GenericImageView, ImageBuffer, Rgba, RgbaImage};

mod anim;
mod font;
mod sprites;
mod wav;

pub use anim::{list_anim_clips, write_anim_clip, AnimClipCatalog, AnimClipMeta};
pub use font::{
    atlas_image, atlas_rgba8, font8x8_c_header, glyph_bits, glyph_uv, map_char,
    write_font8x8_header, write_hud_font_png, FIRST_CHAR, FONT_ATLAS_H, FONT_ATLAS_W, FONT_CELL_PX,
    FONT_COLS, LAST_CHAR, MISSING_CHAR,
};
pub use sprites::{
    grid_by_cell_count, set_sprite_pivot, slice_sheet, Pivot, PixelRect, ResolvedSprite,
    SpriteCatalog, SpriteCell, SpriteSheetMeta,
};
pub use wav::{
    inspect_wav, list_wav_clips, load_pcm16_wav, resolve_wav, spawn_wav_player, write_beep_wav,
    write_pcm16_wav, WavInfo,
};

pub const MAGIC: &[u8; 8] = b"WPACK001";

#[derive(Clone, Debug)]
pub struct WPack {
    pub textures: Vec<PackedTexture>,
    pub meshes: Vec<PackedMesh>,
    /// PCM16 oneshots (LE interleaved). Absent on pre-audio packs (`audio_n` missing → empty).
    pub audio: Vec<PackedAudio>,
}

#[derive(Clone, Debug)]
pub struct PackedTexture {
    pub name: String,
    pub width: u16,
    pub height: u16,
    /// GX-tiled RGB5A3 (4×4 tiles, big-endian u16 pixels). Host untile on decode.
    pub rgba16: Vec<u8>,
}

impl PackedTexture {
    /// Decode tiled RGB5A3 → linear RGBA8 for host sampling.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let linear = untile_rgb5a3(self.width, self.height, &self.rgba16);
        let mut out = Vec::with_capacity(linear.len() * 2);
        for chunk in linear.chunks_exact(2) {
            let word = u16::from_be_bytes([chunk[0], chunk[1]]);
            let [r, g, b, a] = from_rgb5a3(word);
            out.extend_from_slice(&[r, g, b, a]);
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct PackedMesh {
    pub name: String,
    /// Interleaved f32 xyz + f32 uv (stride 20).
    pub interleaved: Vec<u8>,
    pub index_count: u32,
    pub indices: Vec<u16>,
}

/// PCM16 clip in the `WPACK001` audio TOC (after meshes).
///
/// On-disk: `u16` name + `u32` rate + `u16` channels + `u32` byte_len + LE i16 samples.
/// Wii C copies into a 32-byte-aligned buffer and byteswaps to BE for ASND.
#[derive(Clone, Debug)]
pub struct PackedAudio {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved little-endian PCM16 samples (mono or stereo).
    pub pcm: Vec<i16>,
}

impl PackedAudio {
    pub fn pcm_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pcm.len() * 2);
        for s in &self.pcm {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct CookWarning {
    pub texture: String,
    pub message: String,
}

impl WPack {
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            meshes: Vec::new(),
            audio: Vec::new(),
        }
    }

    pub fn texture_index(&self, name: &str) -> Option<usize> {
        self.textures.iter().position(|t| t.name == name)
    }

    /// Resolve a clip stem / `*.wav` path to a TOC index.
    pub fn audio_index(&self, name: &str) -> Option<usize> {
        let want = clip_stem(name);
        if want.is_empty() {
            return None;
        }
        self.audio.iter().position(|a| clip_stem(&a.name) == want)
    }

    /// Cook a validated PCM16 WAV into the audio TOC (name = stem).
    pub fn add_wav(&mut self, name: impl Into<String>, path: impl AsRef<Path>) -> Result<()> {
        let name = name.into();
        let path = path.as_ref();
        let (info, pcm) = load_pcm16_wav(path).with_context(|| format!("cook wav {path:?}"))?;
        self.audio.push(PackedAudio {
            name,
            sample_rate: info.sample_rate,
            channels: info.channels,
            pcm,
        });
        Ok(())
    }

    /// Cook a PNG. Non-power-of-two images are padded up (original top-left).
    pub fn add_png(
        &mut self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Option<CookWarning>> {
        let name = name.into();
        let path = path.as_ref();
        let img = image::open(path).with_context(|| format!("open {path:?}"))?;
        let (w, h) = img.dimensions();
        let rgba = img.to_rgba8();

        let mut warning = None;
        let (pw, ph, padded) = if w.is_power_of_two() && h.is_power_of_two() {
            (w, h, rgba)
        } else {
            let nw = w.next_power_of_two().max(1);
            let nh = h.next_power_of_two().max(1);
            warning = Some(CookWarning {
                texture: name.clone(),
                message: format!("padded {w}x{h} → {nw}x{nh}"),
            });
            let mut canvas: RgbaImage = ImageBuffer::from_pixel(nw, nh, Rgba([0, 0, 0, 0]));
            for y in 0..h {
                for x in 0..w {
                    canvas.put_pixel(x, y, *rgba.get_pixel(x, y));
                }
            }
            (nw, nh, canvas)
        };

        let mut linear = Vec::with_capacity((pw * ph * 2) as usize);
        for pixel in padded.pixels() {
            linear.extend_from_slice(&to_rgb5a3(pixel.0).to_be_bytes());
        }
        let rgba16 = tile_rgb5a3(pw as u16, ph as u16, &linear);
        self.textures.push(PackedTexture {
            name,
            width: pw as u16,
            height: ph as u16,
            rgba16,
        });
        Ok(warning)
    }

    /// Cook every PNG and PCM16 WAV in a directory into this pack.
    ///
    /// WAVs are packed into the `WPACK001` audio TOC (stem, rate, channels, LE PCM16).
    /// Host preview still plays `assets/*.wav` from disk. Invalid WAVs fail cook.
    pub fn cook_dir(&mut self, dir: &Path) -> Result<Vec<CookWarning>> {
        let mut warnings = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(dir)
            .with_context(|| format!("read {dir:?}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
            .collect();
        entries.sort();
        for path in entries {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("tex")
                .to_string();
            if let Some(w) = self.add_png(name, &path)? {
                warnings.push(w);
            }
        }
        let mut wavs = crate::list_wav_clips(dir).unwrap_or_default();
        wavs.sort();
        for stem in wavs {
            let path = crate::resolve_wav(dir, &stem)?;
            self.add_wav(stem, &path)?;
        }
        Ok(warnings)
    }

    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut f = File::create(path.as_ref())?;
        f.write_all(MAGIC)?;
        f.write_u32::<LittleEndian>(self.textures.len() as u32)?;
        f.write_u32::<LittleEndian>(self.meshes.len() as u32)?;

        for tex in &self.textures {
            write_str(&mut f, &tex.name)?;
            f.write_u16::<LittleEndian>(tex.width)?;
            f.write_u16::<LittleEndian>(tex.height)?;
            f.write_u32::<LittleEndian>(tex.rgba16.len() as u32)?;
            f.write_all(&tex.rgba16)?;
            pad32(&mut f)?;
        }
        for mesh in &self.meshes {
            write_str(&mut f, &mesh.name)?;
            f.write_u32::<LittleEndian>(mesh.interleaved.len() as u32)?;
            f.write_all(&mesh.interleaved)?;
            pad32(&mut f)?;
            f.write_u32::<LittleEndian>(mesh.index_count)?;
            for i in &mesh.indices {
                f.write_u16::<LittleEndian>(*i)?;
            }
            pad32(&mut f)?;
        }
        // Audio TOC (additive). Old readers stop after meshes; old packs omit this
        // u32 and `read_from` treats EOF as audio_n = 0.
        f.write_u32::<LittleEndian>(self.audio.len() as u32)?;
        for clip in &self.audio {
            write_str(&mut f, &clip.name)?;
            f.write_u32::<LittleEndian>(clip.sample_rate)?;
            f.write_u16::<LittleEndian>(clip.channels)?;
            let nbytes = (clip.pcm.len() * 2) as u32;
            f.write_u32::<LittleEndian>(nbytes)?;
            for s in &clip.pcm {
                f.write_i16::<LittleEndian>(*s)?;
            }
        }
        Ok(())
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let mut f = File::open(path.as_ref())?;
        let mut magic = [0u8; 8];
        f.read_exact(&mut magic)?;
        if &magic != MAGIC {
            bail!("bad wpack magic");
        }
        let tex_n = f.read_u32::<LittleEndian>()? as usize;
        let mesh_n = f.read_u32::<LittleEndian>()? as usize;
        let mut pack = WPack::new();
        for _ in 0..tex_n {
            let name = read_str(&mut f)?;
            let width = f.read_u16::<LittleEndian>()?;
            let height = f.read_u16::<LittleEndian>()?;
            let len = f.read_u32::<LittleEndian>()? as usize;
            let mut rgba16 = vec![0u8; len];
            f.read_exact(&mut rgba16)?;
            skip_pad32(&mut f, len)?;
            pack.textures.push(PackedTexture {
                name,
                width,
                height,
                rgba16,
            });
        }
        for _ in 0..mesh_n {
            let name = read_str(&mut f)?;
            let len = f.read_u32::<LittleEndian>()? as usize;
            let mut interleaved = vec![0u8; len];
            f.read_exact(&mut interleaved)?;
            skip_pad32(&mut f, len)?;
            let index_count = f.read_u32::<LittleEndian>()?;
            let mut indices = Vec::with_capacity(index_count as usize);
            for _ in 0..index_count {
                indices.push(f.read_u16::<LittleEndian>()?);
            }
            skip_pad32(&mut f, index_count as usize * 2)?;
            pack.meshes.push(PackedMesh {
                name,
                interleaved,
                index_count,
                indices,
            });
        }
        pack.audio = match read_audio_toc(&mut f) {
            Ok(clips) => clips,
            Err(e) => {
                let io_err = e.downcast_ref::<io::Error>();
                if io_err.is_some_and(|e| e.kind() == io::ErrorKind::UnexpectedEof) {
                    Vec::new()
                } else {
                    return Err(e);
                }
            }
        };
        Ok(pack)
    }
}

fn read_audio_toc(f: &mut impl Read) -> Result<Vec<PackedAudio>> {
    let audio_n = match f.read_u32::<LittleEndian>() {
        Ok(n) => n as usize,
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut clips = Vec::with_capacity(audio_n);
    for _ in 0..audio_n {
        let name = read_str(f)?;
        let sample_rate = f.read_u32::<LittleEndian>()?;
        let channels = f.read_u16::<LittleEndian>()?;
        let nbytes = f.read_u32::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; nbytes];
        f.read_exact(&mut bytes)?;
        let mut pcm = Vec::with_capacity(bytes.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            pcm.push(i16::from_le_bytes([chunk[0], chunk[1]]));
        }
        clips.push(PackedAudio {
            name,
            sample_rate,
            channels,
            pcm,
        });
    }
    Ok(clips)
}

/// Stem used for TOC lookup (`beep`, `beep.wav`, `assets/beep.wav` → `beep`).
pub fn clip_stem(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return String::new();
    }
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_string()
}

impl Default for WPack {
    fn default() -> Self {
        Self::new()
    }
}

/// Pack linear RGB5A3 (row-major BE u16) into GX 4×4 tiles.
pub fn tile_rgb5a3(width: u16, height: u16, linear: &[u8]) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    debug_assert_eq!(linear.len(), w * h * 2);
    let mut out = Vec::with_capacity(linear.len());
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            for ty in 0..4 {
                for tx in 0..4 {
                    let x = bx + tx;
                    let y = by + ty;
                    let i = (y * w + x) * 2;
                    out.push(linear[i]);
                    out.push(linear[i + 1]);
                }
            }
        }
    }
    out
}

/// Inverse of [`tile_rgb5a3`].
pub fn untile_rgb5a3(width: u16, height: u16, tiled: &[u8]) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    debug_assert_eq!(tiled.len(), w * h * 2);
    let mut out = vec![0u8; tiled.len()];
    let mut src = 0;
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            for ty in 0..4 {
                for tx in 0..4 {
                    let x = bx + tx;
                    let y = by + ty;
                    let i = (y * w + x) * 2;
                    out[i] = tiled[src];
                    out[i + 1] = tiled[src + 1];
                    src += 2;
                }
            }
        }
    }
    out
}

/// Nintendo RGB5A3: if a>=224 use RGB555, else RGB4A3.
fn to_rgb5a3(rgba: [u8; 4]) -> u16 {
    let [r, g, b, a] = rgba;
    if a >= 224 {
        let r5 = (r as u16) >> 3;
        let g5 = (g as u16) >> 3;
        let b5 = (b as u16) >> 3;
        (1 << 15) | (r5 << 10) | (g5 << 5) | b5
    } else {
        let a3 = (a as u16) >> 5;
        let r4 = (r as u16) >> 4;
        let g4 = (g as u16) >> 4;
        let b4 = (b as u16) >> 4;
        (a3 << 12) | (r4 << 8) | (g4 << 4) | b4
    }
}

fn from_rgb5a3(word: u16) -> [u8; 4] {
    if word & (1 << 15) != 0 {
        let r = (((word >> 10) & 0x1f) as u8) << 3;
        let g = (((word >> 5) & 0x1f) as u8) << 3;
        let b = ((word & 0x1f) as u8) << 3;
        [r, g, b, 255]
    } else {
        let a = ((((word >> 12) & 0x7) as u8) << 5) | (((word >> 12) & 0x7) as u8) << 2;
        let r = ((((word >> 8) & 0xf) as u8) << 4) | (((word >> 8) & 0xf) as u8);
        let g = ((((word >> 4) & 0xf) as u8) << 4) | (((word >> 4) & 0xf) as u8);
        let b = (((word & 0xf) as u8) << 4) | ((word & 0xf) as u8);
        [r, g, b, a]
    }
}

fn write_str(w: &mut impl Write, s: &str) -> Result<()> {
    let b = s.as_bytes();
    w.write_u16::<LittleEndian>(b.len() as u16)?;
    w.write_all(b)?;
    Ok(())
}

fn read_str(r: &mut impl Read) -> Result<String> {
    let len = r.read_u16::<LittleEndian>()? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}

fn pad32(w: &mut impl Write) -> Result<()> {
    let _ = w;
    Ok(())
}

fn skip_pad32(_r: &mut impl Read, _len: usize) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_roundtrip_preserves_pixels() {
        let w = 8u16;
        let h = 4u16;
        let mut linear = Vec::new();
        for i in 0..(w as u32 * h as u32) {
            linear.extend_from_slice(&(i as u16).to_be_bytes());
        }
        let tiled = tile_rgb5a3(w, h, &linear);
        assert_eq!(tiled.len(), linear.len());
        assert_eq!(untile_rgb5a3(w, h, &tiled), linear);
    }

    #[test]
    fn cook_wav_roundtrip_beep_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/beep.wav");
        let (info, samples) = load_pcm16_wav(&fixture).unwrap();
        assert_eq!(info.channels, 1);
        assert_eq!(info.bits_per_sample, 16);
        assert!(info.sample_rate > 0);
        assert!(!samples.is_empty());

        let dir = std::env::temp_dir().join(format!("wiimaker-wpack-wav-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::copy(&fixture, dir.join("beep.wav")).unwrap();

        let mut pack = WPack::new();
        let warnings = pack.cook_dir(&dir).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(pack.audio.len(), 1);
        assert_eq!(pack.audio_index("beep"), Some(0));
        assert_eq!(pack.audio_index("beep.wav"), Some(0));
        assert_eq!(pack.audio_index("assets/beep.wav"), Some(0));
        assert_eq!(pack.audio[0].sample_rate, info.sample_rate);
        assert_eq!(pack.audio[0].channels, info.channels);
        assert_eq!(pack.audio[0].pcm, samples);

        let out = dir.join("assets.wpack");
        pack.write_to(&out).unwrap();
        let loaded = WPack::read_from(&out).unwrap();
        assert_eq!(loaded.audio.len(), 1);
        assert_eq!(loaded.audio[0].name, "beep");
        assert_eq!(loaded.audio[0].sample_rate, info.sample_rate);
        assert_eq!(loaded.audio[0].channels, 1);
        assert_eq!(loaded.audio[0].pcm, samples);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_old_wpack_without_audio_toc() {
        // MAGIC + tex_n=0 + mesh_n=0 and no trailing audio_n (pre-TOC packs).
        let dir = std::env::temp_dir().join(format!("wiimaker-wpack-old-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.wpack");
        let mut bytes = Vec::from(*MAGIC);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        fs::write(&path, &bytes).unwrap();
        let pack = WPack::read_from(&path).unwrap();
        assert!(pack.textures.is_empty());
        assert!(pack.meshes.is_empty());
        assert!(pack.audio.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_empty_audio_toc_roundtrip() {
        let dir = std::env::temp_dir().join(format!("wiimaker-wpack-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.wpack");
        WPack::new().write_to(&path).unwrap();
        let pack = WPack::read_from(&path).unwrap();
        assert!(pack.audio.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
