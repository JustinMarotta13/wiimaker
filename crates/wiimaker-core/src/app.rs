//! App trait — implement this for your game.

use crate::draw::DrawList;
use crate::input::Input;
use crate::time::Clock;

/// Per-frame context handed to [`App::update`] / [`App::render`].
pub struct FrameCtx<'a> {
    pub input: &'a Input,
    pub clock: &'a Clock,
    pub framebuffer_w: u32,
    pub framebuffer_h: u32,
}

/// Game entrypoint. Keep it free of platform types.
pub trait App {
    fn title(&self) -> &str {
        "wiimaker"
    }

    fn update(&mut self, ctx: &FrameCtx<'_>);

    fn render(&mut self, ctx: &FrameCtx<'_>, draw: &mut DrawList);
}
