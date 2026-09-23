//! Host-testable GCN-layout merge used by Wii `fill_input` and the CLI/editor.
//!
//! Button bits match `runtime/wii/include/wiimaker_abi.h`. WPAD constants match
//! libogc `wiiuse/wpad.h` (`WPAD_BUTTON_*`, `WPAD_CLASSIC_BUTTON_*`).
//! Merges are additive (`OR`); analog sources never clear an already-live GCN stick.

use crate::input::{Button, Input, Stick};

/// Idle analog threshold. GCN stick above this is left alone; D-pad fills main
/// only when `|main|` is still inside this zone after Classic/Nunchuk.
pub const STICK_IDLE_DEADZONE: f32 = 0.20;

/// One-line legend for Game view / Inspector / `wiimaker input map`.
pub const INPUT_LEGEND: &str = "Keyboard · Wiimote D-pad/A/B/1/2 · Classic · GCN";

/// `WIIMAKER_BTN_A`
pub const BTN_A: u32 = 1 << 0;
/// `WIIMAKER_BTN_B`
pub const BTN_B: u32 = 1 << 1;
/// `WIIMAKER_BTN_X`
pub const BTN_X: u32 = 1 << 2;
/// `WIIMAKER_BTN_Y`
pub const BTN_Y: u32 = 1 << 3;
/// `WIIMAKER_BTN_START`
pub const BTN_START: u32 = 1 << 4;
/// `WIIMAKER_BTN_Z`
pub const BTN_Z: u32 = 1 << 5;
/// `WIIMAKER_BTN_L`
pub const BTN_L: u32 = 1 << 6;
/// `WIIMAKER_BTN_R`
pub const BTN_R: u32 = 1 << 7;
/// `WIIMAKER_BTN_UP`
pub const BTN_UP: u32 = 1 << 8;
/// `WIIMAKER_BTN_DOWN`
pub const BTN_DOWN: u32 = 1 << 9;
/// `WIIMAKER_BTN_LEFT`
pub const BTN_LEFT: u32 = 1 << 10;
/// `WIIMAKER_BTN_RIGHT`
pub const BTN_RIGHT: u32 = 1 << 11;

/// Core Wiimote bits (`WPAD_BUTTON_*` / `WIIMOTE_BUTTON_*`).
pub const WPAD_BUTTON_2: u32 = 0x0001;
pub const WPAD_BUTTON_1: u32 = 0x0002;
pub const WPAD_BUTTON_B: u32 = 0x0004;
pub const WPAD_BUTTON_A: u32 = 0x0008;
pub const WPAD_BUTTON_MINUS: u32 = 0x0010;
pub const WPAD_BUTTON_HOME: u32 = 0x0080;
pub const WPAD_BUTTON_LEFT: u32 = 0x0100;
pub const WPAD_BUTTON_RIGHT: u32 = 0x0200;
pub const WPAD_BUTTON_DOWN: u32 = 0x0400;
pub const WPAD_BUTTON_UP: u32 = 0x0800;
pub const WPAD_BUTTON_PLUS: u32 = 0x1000;

/// Classic Controller bits in `WPAD_ButtonsHeld` (high word).
pub const WPAD_CLASSIC_BUTTON_UP: u32 = 0x0001 << 16;
pub const WPAD_CLASSIC_BUTTON_LEFT: u32 = 0x0002 << 16;
pub const WPAD_CLASSIC_BUTTON_ZR: u32 = 0x0004 << 16;
pub const WPAD_CLASSIC_BUTTON_X: u32 = 0x0008 << 16;
pub const WPAD_CLASSIC_BUTTON_A: u32 = 0x0010 << 16;
pub const WPAD_CLASSIC_BUTTON_Y: u32 = 0x0020 << 16;
pub const WPAD_CLASSIC_BUTTON_B: u32 = 0x0040 << 16;
pub const WPAD_CLASSIC_BUTTON_ZL: u32 = 0x0080 << 16;
pub const WPAD_CLASSIC_BUTTON_FULL_R: u32 = 0x0200 << 16;
pub const WPAD_CLASSIC_BUTTON_PLUS: u32 = 0x0400 << 16;
pub const WPAD_CLASSIC_BUTTON_HOME: u32 = 0x0800 << 16;
pub const WPAD_CLASSIC_BUTTON_MINUS: u32 = 0x1000 << 16;
pub const WPAD_CLASSIC_BUTTON_FULL_L: u32 = 0x2000 << 16;
pub const WPAD_CLASSIC_BUTTON_DOWN: u32 = 0x4000 << 16;
pub const WPAD_CLASSIC_BUTTON_RIGHT: u32 = 0x8000 << 16;

/// Static mapping row for `wiimaker input map` / Inspector copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapRow {
    pub source: &'static str,
    pub control: &'static str,
    pub target: &'static str,
}

/// Keyboard, Wiimote, Classic, Nunchuk, GCN → GCN-layout `Input`.
pub const MAP_ROWS: &[MapRow] = &[
    MapRow {
        source: "Keyboard",
        control: "W / Arrow Up",
        target: "D-pad Up + main +Y",
    },
    MapRow {
        source: "Keyboard",
        control: "S / Arrow Down",
        target: "D-pad Down + main −Y",
    },
    MapRow {
        source: "Keyboard",
        control: "A / Arrow Left",
        target: "D-pad Left + main −X",
    },
    MapRow {
        source: "Keyboard",
        control: "D / Arrow Right",
        target: "D-pad Right + main +X",
    },
    MapRow {
        source: "Keyboard",
        control: "Z / Space",
        target: "A",
    },
    MapRow {
        source: "Keyboard",
        control: "X",
        target: "B",
    },
    MapRow {
        source: "Keyboard",
        control: "Enter",
        target: "Start",
    },
    MapRow {
        source: "GCN",
        control: "A / B / X / Y / Start / Z / D-pad",
        target: "same bits",
    },
    MapRow {
        source: "GCN",
        control: "Main stick",
        target: "main",
    },
    MapRow {
        source: "GCN",
        control: "C-stick",
        target: "c",
    },
    MapRow {
        source: "Wiimote",
        control: "A",
        target: "A",
    },
    MapRow {
        source: "Wiimote",
        control: "B",
        target: "B",
    },
    MapRow {
        source: "Wiimote",
        control: "Plus",
        target: "Start",
    },
    MapRow {
        source: "Wiimote",
        control: "Minus",
        target: "Z",
    },
    MapRow {
        source: "Wiimote",
        control: "1",
        target: "X",
    },
    MapRow {
        source: "Wiimote",
        control: "2",
        target: "Y",
    },
    MapRow {
        source: "Wiimote",
        control: "D-pad",
        target: "D-pad (synthesizes main if analog idle)",
    },
    MapRow {
        source: "Nunchuk",
        control: "Stick",
        target: "main (if GCN idle)",
    },
    MapRow {
        source: "Classic",
        control: "Left stick",
        target: "main (if GCN idle)",
    },
    MapRow {
        source: "Classic",
        control: "Right stick",
        target: "c (if GCN C idle)",
    },
    MapRow {
        source: "Classic",
        control: "A / B / X / Y",
        target: "A / B / X / Y",
    },
    MapRow {
        source: "Classic",
        control: "Plus / Home",
        target: "Start",
    },
    MapRow {
        source: "Classic",
        control: "Minus",
        target: "Z",
    },
    MapRow {
        source: "Classic",
        control: "L / ZL",
        target: "L",
    },
    MapRow {
        source: "Classic",
        control: "R / ZR",
        target: "R",
    },
    MapRow {
        source: "Classic",
        control: "D-pad",
        target: "D-pad",
    },
];

/// Analog sources the Wii bootstrap merges after GCN.
#[derive(Clone, Copy, Debug, Default)]
pub struct PadSources {
    pub gcn_main: Stick,
    pub gcn_c: Stick,
    pub gcn_buttons: u32,
    /// `WPAD_ButtonsHeld` (core + Classic high word when attached).
    pub wpad_held: u32,
    pub nunchuk: Option<Stick>,
    pub classic_l: Option<Stick>,
    pub classic_r: Option<Stick>,
}

/// Result of [`merge_pad`] — same fields as `WiimakerInput`.
#[derive(Clone, Copy, Debug, Default)]
pub struct MergedPad {
    pub main: Stick,
    pub c: Stick,
    pub buttons: u32,
}

/// `|stick|` squared vs `zone` — no sqrt.
pub fn stick_is_idle(stick: Stick, zone: f32) -> bool {
    stick.x * stick.x + stick.y * stick.y < zone * zone
}

/// Keep `base` when it is outside the deadzone; otherwise take `fallback`.
pub fn take_stick_if_idle(base: Stick, fallback: Stick, zone: f32) -> Stick {
    if stick_is_idle(base, zone) {
        fallback
    } else {
        base
    }
}

/// Core Wiimote held bits → GCN-layout (`1→X`, `2→Y`, Minus→Z, Home/Plus→Start).
pub fn wiimote_core_to_gcn(held: u32) -> u32 {
    let mut out = 0u32;
    if held & WPAD_BUTTON_A != 0 {
        out |= BTN_A;
    }
    if held & WPAD_BUTTON_B != 0 {
        out |= BTN_B;
    }
    if held & WPAD_BUTTON_1 != 0 {
        out |= BTN_X;
    }
    if held & WPAD_BUTTON_2 != 0 {
        out |= BTN_Y;
    }
    if held & (WPAD_BUTTON_PLUS | WPAD_BUTTON_HOME) != 0 {
        out |= BTN_START;
    }
    if held & WPAD_BUTTON_MINUS != 0 {
        out |= BTN_Z;
    }
    if held & WPAD_BUTTON_UP != 0 {
        out |= BTN_UP;
    }
    if held & WPAD_BUTTON_DOWN != 0 {
        out |= BTN_DOWN;
    }
    if held & WPAD_BUTTON_LEFT != 0 {
        out |= BTN_LEFT;
    }
    if held & WPAD_BUTTON_RIGHT != 0 {
        out |= BTN_RIGHT;
    }
    out
}

/// Classic Controller bits in `WPAD_ButtonsHeld` → GCN-layout (OR-only).
pub fn classic_to_gcn(held: u32) -> u32 {
    let mut out = 0u32;
    if held & WPAD_CLASSIC_BUTTON_A != 0 {
        out |= BTN_A;
    }
    if held & WPAD_CLASSIC_BUTTON_B != 0 {
        out |= BTN_B;
    }
    if held & WPAD_CLASSIC_BUTTON_X != 0 {
        out |= BTN_X;
    }
    if held & WPAD_CLASSIC_BUTTON_Y != 0 {
        out |= BTN_Y;
    }
    if held & (WPAD_CLASSIC_BUTTON_PLUS | WPAD_CLASSIC_BUTTON_HOME) != 0 {
        out |= BTN_START;
    }
    if held & WPAD_CLASSIC_BUTTON_MINUS != 0 {
        out |= BTN_Z;
    }
    if held & (WPAD_CLASSIC_BUTTON_FULL_L | WPAD_CLASSIC_BUTTON_ZL) != 0 {
        out |= BTN_L;
    }
    if held & (WPAD_CLASSIC_BUTTON_FULL_R | WPAD_CLASSIC_BUTTON_ZR) != 0 {
        out |= BTN_R;
    }
    if held & WPAD_CLASSIC_BUTTON_UP != 0 {
        out |= BTN_UP;
    }
    if held & WPAD_CLASSIC_BUTTON_DOWN != 0 {
        out |= BTN_DOWN;
    }
    if held & WPAD_CLASSIC_BUTTON_LEFT != 0 {
        out |= BTN_LEFT;
    }
    if held & WPAD_CLASSIC_BUTTON_RIGHT != 0 {
        out |= BTN_RIGHT;
    }
    out
}

/// OR two GCN-layout masks (GCN + Wiimote + Classic coexist).
pub fn or_buttons(a: u32, b: u32) -> u32 {
    a | b
}

/// Synthesize main stick from D-pad bits when analog is still idle.
pub fn synthesize_main_from_dpad(main: Stick, buttons: u32, zone: f32) -> Stick {
    if !stick_is_idle(main, zone) {
        return main;
    }
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    if buttons & BTN_LEFT != 0 {
        x -= 1.0;
    }
    if buttons & BTN_RIGHT != 0 {
        x += 1.0;
    }
    if buttons & BTN_DOWN != 0 {
        y -= 1.0;
    }
    if buttons & BTN_UP != 0 {
        y += 1.0;
    }
    let mag2 = x * x + y * y;
    if mag2 > 1.0 {
        let inv = 1.0 / mag2.sqrt();
        x *= inv;
        y *= inv;
    }
    Stick { x, y }
}

/// Same merge order as `runtime/wii/src/bootstrap.c` `fill_input`.
pub fn merge_pad(src: &PadSources) -> MergedPad {
    let buttons = or_buttons(
        src.gcn_buttons,
        or_buttons(
            wiimote_core_to_gcn(src.wpad_held),
            classic_to_gcn(src.wpad_held),
        ),
    );
    let mut main = src.gcn_main;
    if let Some(l) = src.classic_l {
        main = take_stick_if_idle(main, l, STICK_IDLE_DEADZONE);
    }
    if let Some(n) = src.nunchuk {
        main = take_stick_if_idle(main, n, STICK_IDLE_DEADZONE);
    }
    let mut c = src.gcn_c;
    if let Some(r) = src.classic_r {
        c = take_stick_if_idle(c, r, STICK_IDLE_DEADZONE);
    }
    main = synthesize_main_from_dpad(main, buttons, STICK_IDLE_DEADZONE);
    MergedPad { main, c, buttons }
}

/// Compact status for Game overlay / Inspector (host keyboard or last Play).
#[cfg(feature = "std")]
pub fn format_input_status(input: &Input) -> String {
    let mut dpad = String::new();
    if input.down(Button::DPadUp) {
        dpad.push('U');
    }
    if input.down(Button::DPadDown) {
        dpad.push('D');
    }
    if input.down(Button::DPadLeft) {
        dpad.push('L');
    }
    if input.down(Button::DPadRight) {
        dpad.push('R');
    }
    if dpad.is_empty() {
        dpad.push('·');
    }
    let mut face = String::new();
    if input.down(Button::A) {
        face.push_str("A ");
    }
    if input.down(Button::B) {
        face.push_str("B ");
    }
    if input.down(Button::X) {
        face.push_str("X ");
    }
    if input.down(Button::Y) {
        face.push_str("Y ");
    }
    if input.down(Button::Start) {
        face.push_str("Start ");
    }
    if input.down(Button::Z) {
        face.push_str("Z ");
    }
    if input.down(Button::L) {
        face.push_str("L ");
    }
    if input.down(Button::R) {
        face.push_str("R ");
    }
    let face = face.trim();
    let face = if face.is_empty() { "·" } else { face };
    format!(
        "stick {:+.2},{:+.2}  dpad {dpad}  {face}",
        input.main.x, input.main.y
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_or_keeps_gcn_bits() {
        let gcn = BTN_A | BTN_START;
        let held = WPAD_CLASSIC_BUTTON_B | WPAD_CLASSIC_BUTTON_X;
        let merged = or_buttons(gcn, classic_to_gcn(held));
        assert_ne!(merged & BTN_A, 0);
        assert_ne!(merged & BTN_START, 0);
        assert_ne!(merged & BTN_B, 0);
        assert_ne!(merged & BTN_X, 0);
    }

    #[test]
    fn wiimote_1_2_minus_home() {
        let bits = wiimote_core_to_gcn(
            WPAD_BUTTON_1 | WPAD_BUTTON_2 | WPAD_BUTTON_MINUS | WPAD_BUTTON_HOME,
        );
        assert_eq!(bits, BTN_X | BTN_Y | BTN_Z | BTN_START);
    }

    #[test]
    fn dpad_synthesizes_when_stick_idle() {
        let stick =
            synthesize_main_from_dpad(Stick { x: 0.0, y: 0.0 }, BTN_RIGHT, STICK_IDLE_DEADZONE);
        assert!((stick.x - 1.0).abs() < 1e-5);
        assert!(stick.y.abs() < 1e-5);
        let up = synthesize_main_from_dpad(Stick { x: 0.05, y: 0.02 }, BTN_UP, STICK_IDLE_DEADZONE);
        assert!((up.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn dpad_does_not_override_live_stick() {
        let stick =
            synthesize_main_from_dpad(Stick { x: 0.8, y: 0.1 }, BTN_LEFT, STICK_IDLE_DEADZONE);
        assert!((stick.x - 0.8).abs() < 1e-5);
        assert!((stick.y - 0.1).abs() < 1e-5);
    }

    #[test]
    fn nunchuk_leaves_gcn_alone() {
        let gcn = Stick { x: 0.6, y: 0.0 };
        let n = Stick { x: -1.0, y: 0.2 };
        let out = take_stick_if_idle(gcn, n, STICK_IDLE_DEADZONE);
        assert!((out.x - 0.6).abs() < 1e-5);
        assert!(out.y.abs() < 1e-5);
    }

    #[test]
    fn nunchuk_fills_when_gcn_idle() {
        let gcn = Stick { x: 0.05, y: 0.02 };
        let n = Stick { x: 0.9, y: -0.1 };
        let out = take_stick_if_idle(gcn, n, STICK_IDLE_DEADZONE);
        assert!((out.x - 0.9).abs() < 1e-5);
        assert!((out.y + 0.1).abs() < 1e-5);
    }

    #[test]
    fn merge_pad_classic_or_and_idle_nunchuk() {
        let merged = merge_pad(&PadSources {
            gcn_main: Stick { x: 0.7, y: 0.0 },
            gcn_c: Stick::default(),
            gcn_buttons: BTN_A,
            wpad_held: WPAD_CLASSIC_BUTTON_B | WPAD_BUTTON_UP,
            nunchuk: Some(Stick { x: -1.0, y: 0.0 }),
            classic_l: Some(Stick { x: 0.0, y: 1.0 }),
            classic_r: Some(Stick { x: 0.5, y: 0.0 }),
        });
        assert!((merged.main.x - 0.7).abs() < 1e-5, "GCN stick wins");
        assert!((merged.c.x - 0.5).abs() < 1e-5, "Classic right → c");
        assert_ne!(merged.buttons & BTN_A, 0);
        assert_ne!(merged.buttons & BTN_B, 0);
        assert_ne!(merged.buttons & BTN_UP, 0);
    }

    #[test]
    fn merge_pad_dpad_fills_wiimote_only() {
        let merged = merge_pad(&PadSources {
            wpad_held: WPAD_BUTTON_LEFT | WPAD_BUTTON_A,
            ..Default::default()
        });
        assert!((merged.main.x + 1.0).abs() < 1e-5);
        assert_ne!(merged.buttons & BTN_A, 0);
        assert_ne!(merged.buttons & BTN_LEFT, 0);
    }

    #[test]
    fn map_rows_cover_sources() {
        let sources: Vec<_> = MAP_ROWS.iter().map(|r| r.source).collect();
        for need in ["Keyboard", "GCN", "Wiimote", "Classic", "Nunchuk"] {
            assert!(sources.contains(&need), "missing {need}");
        }
    }

    #[test]
    fn format_status_mentions_stick() {
        let mut input = Input::new();
        input.main = Stick { x: 1.0, y: 0.0 };
        input.set_down(Button::A, true);
        input.set_down(Button::DPadRight, true);
        let s = format_input_status(&input);
        assert!(s.contains("stick"));
        assert!(s.contains("A"));
        assert!(s.contains('R'));
    }

    #[test]
    fn legend_is_compact() {
        assert!(INPUT_LEGEND.contains("Keyboard"));
        assert!(INPUT_LEGEND.contains("Wiimote"));
        assert!(INPUT_LEGEND.contains("Classic"));
        assert!(INPUT_LEGEND.contains("GCN"));
    }
}
