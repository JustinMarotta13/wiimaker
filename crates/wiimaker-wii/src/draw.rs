//! Flush a core [`DrawList`] through GX (or the host test recorder).

use wiimaker_core::draw::{DrawCmd, DrawList};

use crate::ffi;

/// Issue GX draws for every IR command from [`wiimaker_core::render_world`].
pub fn flush_draw_list(draw: &DrawList) {
    for cmd in draw.cmds() {
        match cmd {
            DrawCmd::Clear { color } => {
                ffi::gx_set_clear(color.r, color.g, color.b, color.a);
            }
            DrawCmd::DrawSprite {
                texture,
                dest,
                uv,
                color,
                ..
            } => {
                let rgba = color.to_gx_rgba8();
                if texture.is_untextured() {
                    ffi::gx_draw_quad(dest.x, dest.y, dest.w, dest.h, rgba);
                } else {
                    ffi::gx_draw_sprite(
                        texture.0,
                        dest.x,
                        dest.y,
                        dest.w,
                        dest.h,
                        uv.x,
                        uv.y,
                        uv.x + uv.w,
                        uv.y + uv.h,
                        rgba,
                    );
                }
            }
            DrawCmd::DrawDisc {
                center,
                radius,
                color,
                ..
            } => {
                ffi::gx_draw_disc(center.x, center.y, *radius, color.to_gx_rgba8());
            }
            DrawCmd::DrawText {
                pos,
                text,
                color,
                size,
                align,
                ..
            } => {
                ffi::gx_draw_text(
                    pos.x,
                    pos.y,
                    text,
                    *size,
                    align.to_u8(),
                    color.to_gx_rgba8(),
                );
            }
            DrawCmd::SetCamera { .. }
            | DrawCmd::SetTexture { .. }
            | DrawCmd::DrawMesh { .. } => {}
        }
    }
}
