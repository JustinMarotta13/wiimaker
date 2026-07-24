//! `.wpack` — packed asset archive for Wii + host.
//!
//! Design goals (from CavEX / wii-3d-engine lessons):
//! - Offline conversion only — no PNG decode on console
//! - 32-byte aligned blobs for GX
//! - TOC small enough to mmap from DVD / SD

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use image::{GenericImageView, ImageBuffer, Rgba, RgbaImage};

pub const MAGIC: &[u8; 8] = b"WPACK001";

#[derive(Clone, Debug)]
pub struct WPack {
    pub textures: Vec<PackedTexture>,
    pub meshes: Vec<PackedMesh>,
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
        }
    }

    pub fn texture_index(&self, name: &str) -> Option<usize> {
        self.textures.iter().position(|t| t.name == name)
    }

    /// Cook a PNG. Non-power-of-two images are padded up (original top-left).
    pub fn add_png(&mut self, name: impl Into<String>, path: impl AsRef<Path>) -> Result<Option<CookWarning>> {
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

    /// Cook every PNG in a directory into this pack.
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
        Ok(pack)
    }
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
}
