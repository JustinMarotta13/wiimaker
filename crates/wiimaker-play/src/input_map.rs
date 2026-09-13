//! Keyboard → GCN-layout [`Input`] (shared by host minifb and editor egui).

use wiimaker_core::input::{Button, Input};

/// Digital keys the host / editor map onto the pad.
#[derive(Clone, Copy, Debug, Default)]
pub struct PadKeys {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
}

/// Fill `input` from keyboard (WASD / arrows / Z·Space / X / Enter).
///
/// Stick +Y is Up (host convention). Call [`Input::begin_frame`] first.
pub fn apply_pad_keys(input: &mut Input, keys: PadKeys) {
    input.main.x = (keys.right as i8 - keys.left as i8) as f32;
    input.main.y = (keys.up as i8 - keys.down as i8) as f32;
    let mag = (input.main.x * input.main.x + input.main.y * input.main.y).sqrt();
    if mag > 1.0 {
        input.main.x /= mag;
        input.main.y /= mag;
    }

    input.set_down(Button::A, keys.a);
    input.set_down(Button::B, keys.b);
    input.set_down(Button::Start, keys.start);
    input.set_down(Button::DPadUp, keys.up);
    input.set_down(Button::DPadDown, keys.down);
    input.set_down(Button::DPadLeft, keys.left);
    input.set_down(Button::DPadRight, keys.right);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_is_positive_stick_y() {
        let mut input = Input::new();
        input.begin_frame();
        apply_pad_keys(
            &mut input,
            PadKeys {
                up: true,
                ..Default::default()
            },
        );
        assert!((input.main.y - 1.0).abs() < 1e-6);
        assert!(input.down(Button::DPadUp));
    }
}
