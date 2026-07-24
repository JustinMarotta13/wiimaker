//! Host backend — software interpretation of the DisplayList IR via minifb.
//!
//! This is deliberately simple: enough to iterate gameplay. A GL/wgpu path
//! can replace `raster` later without touching games.

mod atlas;
mod raster;
mod window;

pub use atlas::{load_atlas, load_atlas_for_project, TextureAtlas};
pub use raster::{flush, flush_with_atlas, Framebuffer};
pub use window::{run, run_with_atlas};
