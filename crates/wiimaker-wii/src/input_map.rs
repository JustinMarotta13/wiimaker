//! Map `WiimakerInput` (C ABI) → `wiimaker_core::Input`.

use wiimaker_core::{Input, Stick};

use crate::ffi::WiimakerInput;

/// Convert a Wii-frame `WiimakerInput` into core [`Input`].
///
/// Calls [`Input::begin_frame`] then applies sticks + `apply_down_bits`.
pub fn wiimaker_input_to_core(prev: &mut Input, raw: &WiimakerInput) {
    prev.begin_frame();
    prev.main = Stick {
        x: raw.main_x,
        y: raw.main_y,
    };
    prev.c = Stick {
        x: raw.c_x,
        y: raw.c_y,
    };
    prev.apply_down_bits(raw.buttons);
}

/// Snapshot without edge tracking (fresh Input each call).
pub fn wiimaker_input_snapshot(raw: &WiimakerInput) -> Input {
    let mut input = Input::new();
    wiimaker_input_to_core(&mut input, raw);
    input
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_core::{wiimote_map, Button};

    #[test]
    fn maps_buttons_and_sticks() {
        let raw = WiimakerInput {
            main_x: 0.5,
            main_y: -0.25,
            c_x: 0.1,
            c_y: 0.2,
            buttons: wiimote_map::BTN_A | wiimote_map::BTN_LEFT,
        };
        let input = wiimaker_input_snapshot(&raw);
        assert!((input.main.x - 0.5).abs() < 1e-6);
        assert!((input.main.y + 0.25).abs() < 1e-6);
        assert!(input.down(Button::A));
        assert!(input.down(Button::DPadLeft));
        assert!(!input.down(Button::B));
        assert!(input.pressed(Button::A));
    }

    #[test]
    fn edges_clear_across_frames() {
        let mut input = Input::new();
        let down = WiimakerInput {
            buttons: wiimote_map::BTN_A,
            ..Default::default()
        };
        wiimaker_input_to_core(&mut input, &down);
        assert!(input.pressed(Button::A));
        wiimaker_input_to_core(&mut input, &down);
        assert!(input.down(Button::A));
        assert!(!input.pressed(Button::A));
        let up = WiimakerInput::default();
        wiimaker_input_to_core(&mut input, &up);
        assert!(input.released(Button::A));
        assert!(!input.down(Button::A));
    }
}
