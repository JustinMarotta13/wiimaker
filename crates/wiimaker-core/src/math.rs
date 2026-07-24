//! Math re-exports. Games use glam; Wii backend converts to GX mtx at submit.

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

/// Build a perspective matrix matching GX's typical depth range conventions
/// as closely as host GL soft-renderers can. Exact GX projection is applied
/// in the Wii backend via `guPerspective` / `GX_LoadProjectionMtx`.
pub fn perspective(fovy_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    Mat4::perspective_rh(fovy_rad, aspect, near, far)
}

pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_rh(eye, target, up)
}

pub fn orthographic(w: f32, h: f32) -> Mat4 {
    Mat4::orthographic_rh(0.0, w, h, 0.0, -1.0, 1.0)
}
