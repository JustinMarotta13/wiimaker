//! Normalized input — GameCube pad layout as the lingua franca.
//!
//! HorrorDash lesson: Wiimote IR/sensor bar is fiddly. Pads stay the map:
//! keyboard, Wiimote D-pad/A/B/1/2, Classic, and Nunchuk (stick + Z) all merge onto
//! these bits (see [`crate::wiimote_map`]). IR is **additive aiming** in 640×480
//! game space (`ir_x`/`ir_y`/`ir_valid`) — never a stick/D-pad replacement.
//! Motion (`accel_*` / `motion_valid` / [`Gesture`]) is likewise additive.

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

/// Frame-edge motion gestures (shake / swing). Bits mirror button edge style.
///
/// Detected after [`Input::set_accel`] when `motion_valid`. Thresholds live in
/// [`crate::wiimote_map`] (`SHAKE_DELTA_G`, `SWING_G`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Gesture {
    Shake = 0,
    SwingXPlus = 1,
    SwingXMinus = 2,
    SwingYPlus = 3,
    SwingYMinus = 4,
}

/// Dynamic accel magnitude (g) above gravity residual that counts as a shake.
/// `|accel − (0,0,1)| ≥ this` → [`Gesture::Shake`].
pub const SHAKE_DELTA_G: f32 = 1.5;

/// Per-axis dynamic component (g) for swing edges (`SwingX*` / `SwingY*`).
pub const SWING_G: f32 = 1.5;

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
    /// Aim X in 640×480 game space (+X right). Meaningful when [`Self::ir_valid`].
    pub ir_x: f32,
    /// Aim Y in 640×480 game space (+Y down). Meaningful when [`Self::ir_valid`].
    pub ir_y: f32,
    /// True when a pointing sample is live this frame (Wiimote IR / host mouse).
    pub ir_valid: bool,
    /// Acceleration X in **g** (Wiimote / libogc `gforce` style). Meaningful when
    /// [`Self::motion_valid`]. Rest ≈ `(0, 0, 1)` face-up (see wiimote_map).
    pub accel_x: f32,
    /// Acceleration Y in **g**.
    pub accel_y: f32,
    /// Acceleration Z in **g**.
    pub accel_z: f32,
    /// True when a live accel sample is present this frame (Wiimote / host sim).
    pub motion_valid: bool,
    down: u32,
    pressed: u32,
    released: u32,
    gesture_down: u8,
    gesture_pressed: u8,
    gesture_released: u8,
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    fn mask(button: Button) -> u32 {
        1u32 << (button as u32)
    }

    fn gesture_mask(g: Gesture) -> u8 {
        1u8 << (g as u8)
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
    ///
    /// Clears button/gesture edge bits, [`Self::ir_valid`], and [`Self::motion_valid`].
    /// Backends must re-set IR / accel each frame.
    pub fn begin_frame(&mut self) {
        self.pressed = 0;
        self.released = 0;
        self.gesture_pressed = 0;
        self.gesture_released = 0;
        self.ir_valid = false;
        self.motion_valid = false;
    }

    /// Set IR aim in 640×480 game space for this frame.
    pub fn set_ir(&mut self, x: f32, y: f32, valid: bool) {
        self.ir_x = x;
        self.ir_y = y;
        self.ir_valid = valid;
    }

    /// Set accelerometer sample in **g** for this frame.
    ///
    /// Convention (libogc `WPADData::gforce` / host Shift+mouse tilt): rest ≈
    /// `(0, 0, 1)` with the Wiimote face-up (buttons toward ceiling). When
    /// `valid`, runs [`Self::detect_gestures`]. When invalid, releases gesture
    /// holds without inventing accel.
    pub fn set_accel(&mut self, x: f32, y: f32, z: f32, valid: bool) {
        self.accel_x = x;
        self.accel_y = y;
        self.accel_z = z;
        self.motion_valid = valid;
        if valid {
            self.detect_gestures();
        } else {
            self.set_gesture(Gesture::Shake, false);
            self.set_gesture(Gesture::SwingXPlus, false);
            self.set_gesture(Gesture::SwingXMinus, false);
            self.set_gesture(Gesture::SwingYPlus, false);
            self.set_gesture(Gesture::SwingYMinus, false);
        }
    }

    /// Rising/falling edge for a motion gesture (same pattern as [`Self::set_down`]).
    pub fn set_gesture(&mut self, g: Gesture, is_down: bool) {
        let m = Self::gesture_mask(g);
        let was = self.gesture_down & m != 0;
        if is_down {
            self.gesture_down |= m;
            if !was {
                self.gesture_pressed |= m;
            }
        } else {
            self.gesture_down &= !m;
            if was {
                self.gesture_released |= m;
            }
        }
    }

    /// Derive shake / swing edges from the current accel sample (gravity residual).
    ///
    /// Shake: `|(ax, ay, az − 1)| ≥ SHAKE_DELTA_G` (~1.5 g dynamic). Swing:
    /// dynamic X/Y component beyond `SWING_G`. Call after filling accel, or via
    /// [`Self::set_accel`].
    pub fn detect_gestures(&mut self) {
        if !self.motion_valid {
            return;
        }
        let dx = self.accel_x;
        let dy = self.accel_y;
        let dz = self.accel_z - 1.0;
        let mag = crate::float::sqrt(dx * dx + dy * dy + dz * dz);
        self.set_gesture(Gesture::Shake, mag >= SHAKE_DELTA_G);
        self.set_gesture(Gesture::SwingXPlus, dx >= SWING_G);
        self.set_gesture(Gesture::SwingXMinus, dx <= -SWING_G);
        self.set_gesture(Gesture::SwingYPlus, dy >= SWING_G);
        self.set_gesture(Gesture::SwingYMinus, dy <= -SWING_G);
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

    /// Gesture held this frame (above threshold).
    pub fn gesture_down(&self, g: Gesture) -> bool {
        self.gesture_down & Self::gesture_mask(g) != 0
    }

    /// Gesture rising edge this frame.
    pub fn gesture_pressed(&self, g: Gesture) -> bool {
        self.gesture_pressed & Self::gesture_mask(g) != 0
    }

    /// Convenience: shake rising edge.
    pub fn shake(&self) -> bool {
        self.gesture_pressed(Gesture::Shake)
    }

    pub fn gesture_released(&self, g: Gesture) -> bool {
        self.gesture_released & Self::gesture_mask(g) != 0
    }

    /// Restore edge bits after reconstructing [`Input`] from a held-button snapshot.
    pub fn set_edge_bits(&mut self, pressed: u32, released: u32) {
        self.pressed = pressed;
        self.released = released;
    }

    /// Restore gesture edge bits (e.g. after [`PlayInputC`] round-trip).
    pub fn set_gesture_edge_bits(&mut self, pressed: u8, released: u8) {
        self.gesture_pressed = pressed;
        self.gesture_released = released;
    }

    /// Apply held gesture mask without inventing rising edges for already-held bits.
    pub fn apply_gesture_down_bits(&mut self, bits: u8) {
        const ALL: [Gesture; 5] = [
            Gesture::Shake,
            Gesture::SwingXPlus,
            Gesture::SwingXMinus,
            Gesture::SwingYPlus,
            Gesture::SwingYMinus,
        ];
        for g in ALL {
            self.set_gesture(g, bits & Self::gesture_mask(g) != 0);
        }
    }

    pub fn down_bits(&self) -> u32 {
        self.down
    }

    pub fn gesture_down_bits(&self) -> u8 {
        self.gesture_down
    }

    pub fn gesture_pressed_bits(&self) -> u8 {
        self.gesture_pressed
    }

    pub fn gesture_released_bits(&self) -> u8 {
        self.gesture_released
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_accel_and_begin_frame_clear_motion_valid() {
        let mut input = Input::new();
        input.set_accel(0.1, -0.2, 1.0, true);
        assert!(input.motion_valid);
        assert!((input.accel_x - 0.1).abs() < 1e-6);
        assert!((input.accel_y + 0.2).abs() < 1e-6);
        assert!((input.accel_z - 1.0).abs() < 1e-6);
        input.begin_frame();
        assert!(!input.motion_valid);
        // raw values retained; valid cleared like IR
        assert!((input.accel_x - 0.1).abs() < 1e-6);
    }

    #[test]
    fn shake_edge_fires_once_then_holds() {
        let mut input = Input::new();
        // Rest: no shake
        input.set_accel(0.0, 0.0, 1.0, true);
        assert!(!input.shake());
        assert!(!input.gesture_down(Gesture::Shake));
        input.begin_frame();
        // Big jab: rising edge
        input.set_accel(2.0, 0.0, 1.0, true);
        assert!(input.shake());
        assert!(input.gesture_down(Gesture::Shake));
        input.begin_frame();
        // Still above threshold: held, no new edge
        input.set_accel(2.0, 0.0, 1.0, true);
        assert!(!input.shake());
        assert!(input.gesture_down(Gesture::Shake));
        input.begin_frame();
        // Back to rest: released
        input.set_accel(0.0, 0.0, 1.0, true);
        assert!(!input.shake());
        assert!(!input.gesture_down(Gesture::Shake));
        assert!(input.gesture_released(Gesture::Shake));
    }

    #[test]
    fn swing_edges_from_axis_delta() {
        let mut input = Input::new();
        input.set_accel(2.0, 0.0, 1.0, true);
        assert!(input.gesture_pressed(Gesture::SwingXPlus));
        input.begin_frame();
        input.set_accel(-2.0, 0.0, 1.0, true);
        assert!(input.gesture_pressed(Gesture::SwingXMinus));
        assert!(!input.gesture_down(Gesture::SwingXPlus));
    }

    #[test]
    fn invalid_accel_clears_gestures() {
        let mut input = Input::new();
        input.set_accel(3.0, 0.0, 1.0, true);
        assert!(input.gesture_down(Gesture::Shake));
        input.begin_frame();
        input.set_accel(0.0, 0.0, 0.0, false);
        assert!(!input.motion_valid);
        assert!(!input.gesture_down(Gesture::Shake));
        assert!(input.gesture_released(Gesture::Shake));
    }
}
