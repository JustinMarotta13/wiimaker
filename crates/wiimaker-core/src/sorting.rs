//! Named sorting layers (Unity Sorting Layer analogue).
//!
//! Drawables sort by layer list order, then order-in-layer (`z`) within a layer.

use core::cmp::Ordering;

#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Canonical default layer name (Unity `Default`).
pub const DEFAULT_SORTING_LAYER: &str = "Default";

/// Built-in project layers when `game.toml` omits `sorting_layers`.
pub const DEFAULT_SORTING_LAYERS: &[&str] = &["Background", "Default", "Foreground"];

/// Owned copy of [`DEFAULT_SORTING_LAYERS`].
pub fn default_sorting_layers() -> Vec<String> {
    DEFAULT_SORTING_LAYERS.iter().map(|s| (*s).into()).collect()
}

/// Index of [`DEFAULT_SORTING_LAYER`] in the built-in list (`1`).
pub fn default_sorting_layer_index() -> u16 {
    sorting_layer_index(DEFAULT_SORTING_LAYERS, "")
}

/// Resolve a layer name against an ordered list.
///
/// Empty / omitted → [`DEFAULT_SORTING_LAYER`]. Unknown names also fall back
/// to Default when present, otherwise index `0`.
pub fn sorting_layer_index<S: AsRef<str>>(layers: &[S], name: &str) -> u16 {
    let key = if name.trim().is_empty() {
        DEFAULT_SORTING_LAYER
    } else {
        name.trim()
    };
    if let Some(i) = layers.iter().position(|n| n.as_ref() == key) {
        return i as u16;
    }
    layers
        .iter()
        .position(|n| n.as_ref() == DEFAULT_SORTING_LAYER)
        .unwrap_or(0) as u16
}

/// Compare two drawables: layer order, then order-in-layer `z`.
pub fn cmp_sorting(layer_a: u16, z_a: f32, layer_b: u16, z_b: f32) -> Ordering {
    match layer_a.cmp(&layer_b) {
        Ordering::Equal => z_a.partial_cmp(&z_b).unwrap_or(Ordering::Equal),
        other => other,
    }
}

/// True when JSON/TOML should omit the field (empty or the Default name).
pub fn is_default_sorting_layer_name(name: &str) -> bool {
    let t = name.trim();
    t.is_empty() || t.eq_ignore_ascii_case(DEFAULT_SORTING_LAYER)
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;

    #[test]
    fn default_index_is_middle_builtin() {
        assert_eq!(default_sorting_layer_index(), 1);
        assert_eq!(sorting_layer_index(DEFAULT_SORTING_LAYERS, ""), 1);
        assert_eq!(sorting_layer_index(DEFAULT_SORTING_LAYERS, "Foreground"), 2);
        assert_eq!(sorting_layer_index(DEFAULT_SORTING_LAYERS, "nope"), 1);
    }

    #[test]
    fn unknown_without_default_is_zero() {
        let layers = ["A", "B"];
        assert_eq!(sorting_layer_index(&layers, "missing"), 0);
    }

    #[test]
    fn cmp_layer_beats_z() {
        assert_eq!(cmp_sorting(0, 99.0, 1, 0.0), Ordering::Less);
        assert_eq!(cmp_sorting(1, 0.0, 1, 1.0), Ordering::Less);
    }
}
