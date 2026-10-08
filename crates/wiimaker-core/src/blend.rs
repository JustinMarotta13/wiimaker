//! Blend-tree weights for Animator states (1D thresholds, 2D positions).
//!
//! Pure math shared by host and Wii builds (`no_std` + alloc). Every weight
//! vector is non-negative and sums to 1 when it is non-empty. Coincident
//! motions split their point's weight equally, so a stacked pair behaves like
//! one motion with a shared clip choice.

#[cfg(feature = "std")]
mod alloc_types {
    pub use std::vec::Vec;
}

#[cfg(not(feature = "std"))]
mod alloc_types {
    extern crate alloc;
    pub use alloc::vec::Vec;
}

use alloc_types::Vec;

const EPS: f32 = 1e-6;
const EPS2: f32 = EPS * EPS;

fn zeros(n: usize) -> Vec<f32> {
    let mut v = Vec::new();
    v.resize(n, 0.0);
    v
}

fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

fn sanitize_input(x: f32, lo: f32, hi: f32) -> f32 {
    (if x.is_nan() { 0.0 } else { x }).clamp(lo, hi)
}

/// Piecewise-linear weights over sorted-by-value thresholds.
///
/// The input is NaN-safe (NaN reads as 0) and then clamped to `[min, max]` of the thresholds.
/// Equal thresholds split their share evenly.
pub fn weights_1d(thresholds: &[f32], x: f32) -> Vec<f32> {
    let mut out = zeros(thresholds.len());
    if thresholds.is_empty() {
        return out;
    }
    let mut min_t = f32::INFINITY;
    let mut max_t = f32::NEG_INFINITY;
    for &t in thresholds {
        min_t = min_t.min(t);
        max_t = max_t.max(t);
    }
    let x = sanitize_input(x, min_t, max_t);

    let hits = thresholds.iter().filter(|&&t| (t - x).abs() <= EPS).count();
    if hits > 0 {
        for (i, &t) in thresholds.iter().enumerate() {
            if (t - x).abs() <= EPS {
                out[i] = 1.0 / hits as f32;
            }
        }
        return out;
    }

    let mut lo = f32::NEG_INFINITY;
    let mut hi = f32::INFINITY;
    for &t in thresholds {
        if t < x && t > lo {
            lo = t;
        }
        if t > x && t < hi {
            hi = t;
        }
    }
    if !(lo.is_finite() && hi.is_finite()) {
        return out;
    }
    let w_hi = (x - lo) / (hi - lo);
    let w_lo = 1.0 - w_hi;
    let n_lo = thresholds
        .iter()
        .filter(|&&t| (t - lo).abs() <= EPS)
        .count() as f32;
    let n_hi = thresholds
        .iter()
        .filter(|&&t| (t - hi).abs() <= EPS)
        .count() as f32;
    for (i, &t) in thresholds.iter().enumerate() {
        if (t - lo).abs() <= EPS {
            out[i] = w_lo / n_lo;
        } else if (t - hi).abs() <= EPS {
            out[i] = w_hi / n_hi;
        }
    }
    out
}

/// Gradient-band weights over 2D motion positions.
///
/// The query is NaN-safe and clamped into the motions' bounding box. Each motion's
/// raw weight is the minimum, over the other distinct motions `j`, of
/// `clamp(1 - t_j, 0, 1)`, where `t_j` is the projection of the query onto the
/// segment `p_i -> p_j` as a fraction of its length. Motions that lie on one
/// straight line therefore reproduce the 1D piecewise-linear weights, including
/// uneven spacing, for queries inside the bounding box. A query outside the box
/// is clamped onto it first, so on a non-axis-aligned line it does not extrapolate
/// along that line. Coincident motions share their point's weight equally. Raw
/// weights are normalized. If every raw weight is zero, the nearest point wins.
pub fn weights_2d(positions: &[[f32; 2]], q: [f32; 2]) -> Vec<f32> {
    let n = positions.len();
    let mut out = zeros(n);
    if n == 0 {
        return out;
    }
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for p in positions {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let qc = [
        sanitize_input(q[0], min[0], max[0]),
        sanitize_input(q[1], min[1], max[1]),
    ];

    let mut raw = zeros(n);
    for (i, &p) in positions.iter().enumerate() {
        let mut band = 1.0_f32;
        for (j, &pj) in positions.iter().enumerate() {
            if j == i {
                continue;
            }
            let d = [pj[0] - p[0], pj[1] - p[1]];
            let len2 = d[0] * d[0] + d[1] * d[1];
            if len2 <= EPS2 {
                continue;
            }
            let along = ((qc[0] - p[0]) * d[0] + (qc[1] - p[1]) * d[1]) / len2;
            band = band.min((1.0 - along).clamp(0.0, 1.0));
        }
        raw[i] = band;
    }

    let total: f32 = raw.iter().sum();
    if !(total > 0.0 && total.is_finite()) {
        let nearest = positions
            .iter()
            .map(|&p| dist2(p, qc))
            .fold(f32::INFINITY, |a, b| a.min(b));
        for (i, &p) in positions.iter().enumerate() {
            raw[i] = if dist2(p, qc) <= nearest + EPS2 {
                1.0
            } else {
                0.0
            };
        }
    }

    for (i, &p) in positions.iter().enumerate() {
        let shared = positions.iter().filter(|&&o| dist2(o, p) <= EPS2).count() as f32;
        out[i] = raw[i] / shared;
    }
    let sum: f32 = out.iter().sum();
    for v in out.iter_mut() {
        *v /= sum;
    }
    out
}

/// Pick the dominant motion. Keeps `prev` when it ties the maximum (no flicker at 50/50).
pub fn choose_active(weights: &[f32], prev: usize) -> usize {
    if weights.is_empty() {
        return 0;
    }
    let max = weights.iter().fold(0.0_f32, |m, &w| m.max(w));
    if prev < weights.len() && weights[prev] >= max - EPS {
        return prev;
    }
    let mut best = 0;
    for (i, &w) in weights.iter().enumerate() {
        if w > weights[best] {
            best = i;
        }
    }
    best
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;

    fn assert_simplex(w: &[f32]) {
        assert!(!w.is_empty());
        assert!(w.iter().all(|&v| v >= 0.0 && v.is_finite()), "{w:?}");
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "sum {sum} for {w:?}");
    }

    fn assert_close(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 1e-5, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn one_d_single_motion_is_full_weight() {
        for x in [-100.0, 0.0, 0.3, 42.0] {
            assert_close(&weights_1d(&[0.5], x), &[1.0]);
        }
    }

    #[test]
    fn one_d_linear_between_thresholds() {
        let t = [0.0, 1.0, 3.0];
        assert_close(&weights_1d(&t, 0.25), &[0.75, 0.25, 0.0]);
        assert_close(&weights_1d(&t, 2.0), &[0.0, 0.5, 0.5]);
        assert_close(&weights_1d(&t, 1.0), &[0.0, 1.0, 0.0]);
    }

    #[test]
    fn one_d_clamps_outside_range() {
        let t = [0.0, 1.0];
        assert_close(&weights_1d(&t, -5.0), &[1.0, 0.0]);
        assert_close(&weights_1d(&t, 9.0), &[0.0, 1.0]);
        assert_close(&weights_1d(&t, f32::NAN), &[1.0, 0.0]);
    }

    #[test]
    fn one_d_nan_clamps_after_zeroing() {
        assert_close(&weights_1d(&[1.0, 2.0], f32::NAN), &[1.0, 0.0]);
        assert_close(&weights_1d(&[-3.0, -1.0], f32::NAN), &[0.0, 1.0]);
        assert_close(&weights_1d(&[-1.0, 1.0], f32::NAN), &[0.5, 0.5]);
    }

    #[test]
    fn one_d_equal_thresholds_split_evenly() {
        let t = [0.0, 1.0, 1.0];
        assert_close(&weights_1d(&t, 1.0), &[0.0, 0.5, 0.5]);
        assert_close(&weights_1d(&t, 0.5), &[0.5, 0.25, 0.25]);
        let stacked_low = [0.0, 0.0, 1.0];
        assert_close(&weights_1d(&stacked_low, 0.5), &[0.25, 0.25, 0.5]);
    }

    #[test]
    fn one_d_weights_are_simplex_over_sweep() {
        let t = [-1.0, 0.0, 0.0, 2.0];
        let mut x = -3.0;
        while x <= 4.0 {
            assert_simplex(&weights_1d(&t, x));
            x += 0.05;
        }
    }

    #[test]
    fn two_d_single_motion_is_full_weight() {
        assert_close(&weights_2d(&[[0.0, 0.0]], [0.7, -0.2]), &[1.0]);
    }

    #[test]
    fn two_d_exact_hit_selects_motion() {
        let p = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        assert_close(&weights_2d(&p, [1.0, 0.0]), &[0.0, 1.0, 0.0]);
        assert_close(&weights_2d(&p, [0.0, 1.0]), &[0.0, 0.0, 1.0]);
    }

    #[test]
    fn two_d_straight_line_matches_one_d() {
        let p = [[-1.0, 0.0], [0.0, 0.0], [1.0, 0.0]];
        let t = [-1.0, 0.0, 1.0];
        for x in [-1.0, -0.6, -0.1, 0.0, 0.25, 0.5, 0.9, 1.0] {
            assert_close(&weights_2d(&p, [x, 0.0]), &weights_1d(&t, x));
        }
        assert_close(&weights_2d(&p, [0.5, 0.0]), &[0.0, 0.5, 0.5]);
        assert_close(&weights_2d(&p, [0.5, 0.8]), &[0.0, 0.5, 0.5]);
    }

    #[test]
    fn two_d_uneven_colinear_matches_one_d() {
        let p = [[0.0, 0.0], [0.2, 0.0], [1.0, 0.0]];
        let t = [0.0, 0.2, 1.0];
        assert_close(&weights_2d(&p, [0.4, 0.0]), &[0.0, 0.75, 0.25]);
        for x in [-0.5, 0.0, 0.1, 0.2, 0.4, 0.6, 0.95, 1.0, 1.5] {
            assert_close(&weights_2d(&p, [x, 0.0]), &weights_1d(&t, x));
        }
    }

    #[test]
    fn two_d_uneven_diagonal_line_matches_one_d() {
        let p = [[0.0, 0.0], [0.2, 0.2], [1.0, 1.0]];
        let t = [0.0, 0.2, 1.0];
        assert_close(&weights_2d(&p, [0.4, 0.4]), &[0.0, 0.75, 0.25]);
        assert_close(&weights_2d(&p, [0.4, 0.4]), &weights_1d(&t, 0.4));
    }

    #[test]
    fn two_d_nan_clamps_after_zeroing() {
        let p = [[1.0, 0.0], [2.0, 0.0]];
        assert_close(&weights_2d(&p, [f32::NAN, 0.0]), &[1.0, 0.0]);
        assert_close(&weights_2d(&p, [f32::NAN, f32::NAN]), &[1.0, 0.0]);
    }

    #[test]
    fn two_d_stacked_motions_split_one_point() {
        let p = [[0.0, 0.0], [0.0, 0.0], [1.0, 0.0]];
        assert_close(&weights_2d(&p, [0.0, 0.0]), &[0.5, 0.5, 0.0]);
        assert_close(&weights_2d(&p, [0.5, 0.0]), &[0.25, 0.25, 0.5]);
        assert_simplex(&weights_2d(&p, [0.3, 0.4]));
    }

    #[test]
    fn two_d_all_stacked_is_even() {
        let p = [[0.2, 0.2], [0.2, 0.2], [0.2, 0.2]];
        assert_close(&weights_2d(&p, [0.9, -3.0]), &[1.0 / 3.0; 3]);
    }

    #[test]
    fn two_d_four_way_centre_is_shared() {
        let p = [[0.0, 0.0], [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]];
        let w = weights_2d(&p, [0.5, 0.5]);
        assert_simplex(&w);
        assert!(w[1] > 0.0 && w[3] > 0.0 && w[0] > 0.0);
        assert!(w[2] == 0.0 && w[4] == 0.0);
    }

    #[test]
    fn two_d_weights_are_simplex_over_sweep() {
        let p = [
            [-1.0, -1.0],
            [1.0, -1.0],
            [0.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ];
        let mut y = -1.5;
        while y <= 1.5 {
            let mut x = -1.5;
            while x <= 1.5 {
                assert_simplex(&weights_2d(&p, [x, y]));
                x += 0.125;
            }
            y += 0.125;
        }
    }

    #[test]
    fn choose_active_keeps_previous_on_tie() {
        assert_eq!(choose_active(&[0.5, 0.5], 1), 1);
        assert_eq!(choose_active(&[0.5, 0.5], 0), 0);
        assert_eq!(choose_active(&[0.2, 0.8], 0), 1);
        assert_eq!(choose_active(&[], 3), 0);
    }
}
