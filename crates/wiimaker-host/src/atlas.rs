//! Host texture atlas loaded from `.wpack` for sprite sampling.

use wiimaker_assets::WPack;
use wiimaker_core::draw::TextureId;
use wiimaker_core::color::Rgba8;
use wiimaker_scene::TextureMap;

#[derive(Clone, Debug)]
pub struct HostTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// RGBA8 pixels.
    pub rgba8: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct TextureAtlas {
    textures: Vec<HostTexture>,
    map: TextureMap,
}

impl TextureAtlas {
    pub fn from_wpack(pack: &WPack) -> Self {
        let mut atlas = Self::default();
        for (i, tex) in pack.textures.iter().enumerate() {
            atlas.map.insert(tex.name.clone(), TextureId(i as u32));
            atlas.textures.push(HostTexture {
                name: tex.name.clone(),
                width: tex.width as u32,
                height: tex.height as u32,
                rgba8: tex.to_rgba8(),
            });
        }
        atlas
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn map(&self) -> &TextureMap {
        &self.map
    }

    pub fn get(&self, id: TextureId) -> Option<&HostTexture> {
        self.textures.get(id.0 as usize)
    }

    pub fn sample(&self, id: TextureId, u: f32, v: f32) -> Rgba8 {
        let Some(tex) = self.get(id) else {
            return Rgba8::WHITE;
        };
        if tex.width == 0 || tex.height == 0 || tex.rgba8.is_empty() {
            return Rgba8::WHITE;
        }
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let x = ((u * tex.width as f32) as u32).min(tex.width - 1);
        let y = ((v * tex.height as f32) as u32).min(tex.height - 1);
        let i = ((y * tex.width + x) * 4) as usize;
        if i + 3 >= tex.rgba8.len() {
            return Rgba8::WHITE;
        }
        Rgba8::new(
            tex.rgba8[i],
            tex.rgba8[i + 1],
            tex.rgba8[i + 2],
            tex.rgba8[i + 3],
        )
    }
}

/// Load a `.wpack` into a host atlas (or empty if missing).
pub fn load_atlas(path: impl AsRef<std::path::Path>) -> TextureAtlas {
    match WPack::read_from(path.as_ref()) {
        Ok(pack) => TextureAtlas::from_wpack(&pack),
        Err(_) => TextureAtlas::empty(),
    }
}

/// Load atlas for a game, cooking PNGs → `.wpack` if the pack is missing.
pub fn load_atlas_for_project(
    game_dir: &std::path::Path,
    project: &wiimaker_scene::GameProject,
) -> Result<TextureAtlas, Box<dyn std::error::Error>> {
    let wpack_path = project.wpack_path(game_dir);
    if wpack_path.is_file() {
        return Ok(load_atlas(&wpack_path));
    }
    let assets = project.assets_path(game_dir);
    let mut pack = WPack::new();
    if assets.is_dir() {
        let _ = pack.cook_dir(&assets)?;
        if let Some(parent) = wpack_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        pack.write_to(&wpack_path)?;
    }
    Ok(TextureAtlas::from_wpack(&pack))
}
