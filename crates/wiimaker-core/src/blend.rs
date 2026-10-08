//! Blend-tree weights for Animator states (1D thresholds, 2D positions).
//!
//! Pure math shared by host and Wii builds (`no_std` + alloc). Every weight
//! vector is non-negative and sums to 1 when it is non-empty. Coincident
//! motions split their point's weight equally, so a stacked pair behaves like
//! one motion with a shared clip choice.

use crate::float::sqrt;

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

/// Piecewise-linear weights over sorted-by-value thresholds.
///
/// The input is NaN-safe (NaN reads as 0) and clamped to `[min, max]` of the thresholds.
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
    let x = if x.is_nan() {
        0.0
    } else {
        x.clamp(min_t, max_t)
    };

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

fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

/// Radial hat weights over 2D motion positions.
///
/// The query is clamped into the motions' bounding box (so a straight line of
/// motions behaves like a 1D blend along that line). Each motion's radius is its
/// distance to the nearest distinct position; its raw weight is
/// `max(0, 1 - d / radius)`. Those weights are normalized. Exact hits on a
/// motion give 1 to that point. If every raw weight is zero, the nearest point
/// wins.
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
        if q[0].is_nan() {
            0.0
        } else {
            q[0].clamp(min[0], max[0])
        },
        if q[1].is_nan() {
            0.0
        } else {
            q[1].clamp(min[1], max[1])
        },
    ];

    let mut radius = zeros(n);
    let mut any_distinct = false;
    for i in 0..n {
        let mut best = f32::INFINITY;
        for j in 0..n {
            let d2 = dist2(positions[i], positions[j]);
            if d2 > EPS2 && d2 < best {
                best = d2;
            }
        }
        if best.is_finite() {
            radius[i] = sqrt(best);
            any_distinct = true;
        }
    }
    if !any_distinct {
        let w = 1.0 / n as f32;
        for v in out.iter_mut() {
            *v = w;
        }
        return out;
    }

    let mut same = zeros(n);
    for i in 0..n {
        same[i] = positions
            .iter()
            .filter(|p| dist2(**p, positions[i]) <= EPS2)
            .count() as f32;
    }

    let mut nearest = f32::INFINITY;
    let mut dist_q = zeros(n);
    for i in 0..n {
        let d2 = dist2(positions[i], qc);
        dist_q[i] = d2;
        if d2 < nearest {
            nearest = d2;
        }
        out[i] = (1.0 - sqrt(d2) / radius[i]).max(0.0);
    }

    let total: f32 = out.iter().sum();
    if !(total > 0.0 && total.is_finite()) {
        for i in 0..n {
            out[i] = if dist_q[i] <= nearest + EPS2 {
                1.0
            } else {
                0.0
            };
        }
    }
    for i in 0..n {
        out[i] /= same[i];
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
