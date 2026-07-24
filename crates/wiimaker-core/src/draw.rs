//! Display-list style render IR.
//!
//! Games push commands; backends flush them to GX / host rasterizer.
//! Pattern inspired by Texel's late GX conversion and recomp HLE display lists.

use crate::color::Rgba8;
use crate::math::{Mat4, Vec2};

/// Handle into a packed mesh inside a `.wpack` (or host cache).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(pub u32);

/// Handle into a packed texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

/// Axis-aligned rect in pixel or UV space.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn unit() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }
}

/// One GPU-ish operation. Keep this small and backend-translatable.
#[derive(Clone, Debug)]
pub enum DrawCmd {
    Clear {
        color: Rgba8,
    },
    SetCamera {
        view: Mat4,
        proj: Mat4,
    },
    SetTexture {
        id: TextureId,
    },
    DrawMesh {
        mesh: MeshId,
        transform: Mat4,
        color: Rgba8,
    },
    DrawSprite {
        texture: TextureId,
        dest: Rect,
        uv: Rect,
        color: Rgba8,
        z: f32,
    },
    /// Filled disc — host approximates; Wii draws a triangle fan.
    DrawDisc {
        center: Vec2,
        radius: f32,
        color: Rgba8,
        z: f32,
    },
}

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Ordered command buffer for one frame.
#[derive(Default, Clone, Debug)]
pub struct DrawList {
    cmds: Vec<DrawCmd>,
}

impl DrawList {
    pub fn new() -> Self {
        Self { cmds: Vec::new() }
    }

    pub fn clear_buffer(&mut self) {
        self.cmds.clear();
    }

    pub fn push(&mut self, cmd: DrawCmd) {
        self.cmds.push(cmd);
    }

    pub fn clear(&mut self, color: Rgba8) {
        self.push(DrawCmd::Clear { color });
    }

    pub fn set_camera(&mut self, view: Mat4, proj: Mat4) {
        self.push(DrawCmd::SetCamera { view, proj });
    }

    pub fn disc(&mut self, center: Vec2, radius: f32, color: Rgba8, z: f32) {
        self.push(DrawCmd::DrawDisc {
            center,
            radius,
            color,
            z,
        });
    }

    pub fn sprite(&mut self, texture: TextureId, dest: Rect, color: Rgba8) {
        self.sprite_ex(texture, dest, Rect::unit(), color, 0.0);
    }

    pub fn sprite_ex(
        &mut self,
        texture: TextureId,
        dest: Rect,
        uv: Rect,
        color: Rgba8,
        z: f32,
    ) {
        self.push(DrawCmd::DrawSprite {
            texture,
            dest,
            uv,
            color,
            z,
        });
    }

    pub fn mesh(&mut self, mesh: MeshId, transform: Mat4, color: Rgba8) {
        self.push(DrawCmd::DrawMesh {
            mesh,
            transform,
            color,
        });
    }

    pub fn cmds(&self) -> &[DrawCmd] {
        &self.cmds
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;

    #[test]
    fn push_clear() {
        let mut dl = DrawList::new();
        dl.clear(Rgba8::BLACK);
        assert_eq!(dl.cmds().len(), 1);
    }
}
