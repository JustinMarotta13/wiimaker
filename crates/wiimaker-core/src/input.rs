//! Normalized input — GameCube pad layout as the lingua franca.
//!
//! HorrorDash lesson: Wiimote IR/sensor bar is fiddly. Pads stay the map:
//! keyboard, Wiimote D-pad/A/B/1/2, Classic, and Nunchuk (stick + Z) all merge onto
//! these bits (see [`crate::wiimote_map`]).

/// Digital buttons shared across GCN / Classic / emulated keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Button {
    A = 0,
    B = 1,
    X = 2,
    Y = 3,
    Start = 4,
    Z = 5,
    L = 6,
    R = 7,
    DPadUp = 8,
    DPadDown = 9,
    DPadLeft = 10,
    DPadRight = 11,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stick {
    /// −1.0 ..= 1.0
    pub x: f32,
    /// −1.0 ..= 1.0
    pub y: f32,
}

impl Stick {
    pub fn deadzone(self, zone: f32) -> Self {
        let mag = crate::float::sqrt(self.x * self.x + self.y * self.y);
        if mag < zone {
            Self { x: 0.0, y: 0.0 }
        } else {
            self
        }
    }
}

/// Snapshot for one frame. Backends fill this; games only read it.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub main: Stick,
    pub c: Stick,
    pub l_analog: f32,
    pub r_analog: f32,
    down: u32,
    pressed: u32,
    released: u32,
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    fn mask(button: Button) -> u32 {
        1u32 << (button as u32)
    }

    pub fn set_down(&mut self, button: Button, is_down: bool) {
        let m = Self::mask(button);
        let was = self.down & m != 0;
        if is_down {
            self.down |= m;
            if !was {
                self.pressed |= m;
            }
        } else {
            self.down &= !m;
            if was {
                self.released |= m;
            }
        }
    }

    /// Call once at the start of each frame before applying fresh device state.
    pub fn begin_frame(&mut self) {
        self.pressed = 0;
        self.released = 0;
    }

    pub fn down(&self, button: Button) -> bool {
        self.down & Self::mask(button) != 0
    }

    pub fn pressed(&self, button: Button) -> bool {
        self.pressed & Self::mask(button) != 0
    }

    pub fn released(&self, button: Button) -> bool {
        self.released & Self::mask(button) != 0
    }

    /// Restore edge bits after reconstructing [`Input`] from a held-button snapshot.
    pub fn set_edge_bits(&mut self, pressed: u32, released: u32) {
        self.pressed = pressed;
        self.released = released;
    }

    pub fn down_bits(&self) -> u32 {
        self.down
    }

    /// Apply a GCN-layout held mask (`WIIMAKER_BTN_*` / [`crate::wiimote_map`]).
    pub fn apply_down_bits(&mut self, bits: u32) {
        const ALL: [Button; 12] = [
            Button::A,
            Button::B,
            Button::X,
            Button::Y,
            Button::Start,
            Button::Z,
            Button::L,
            Button::R,
            Button::DPadUp,
            Button::DPadDown,
            Button::DPadLeft,
            Button::DPadRight,
        ];
        for b in ALL {
            self.set_down(b, bits & Self::mask(b) != 0);
        }
    }

    pub fn pressed_bits(&self) -> u32 {
        self.pressed
    }

    pub fn released_bits(&self) -> u32 {
        self.released
    }
}
