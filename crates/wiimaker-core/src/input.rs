//! Normalized input — GameCube pad layout as the lingua franca.
//!
//! HorrorDash lesson: Wiimote IR/sensor bar is fiddly. Start with pads;
//! map Wiimote D-pad / classic controller onto the same buttons later.

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
        let mag = (self.x * self.x + self.y * self.y).sqrt();
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
}
