//! Screen-space Unity-like 2D tool handles.
//!
//! Move (red X, green Y, XY square), Rotate (blue Z ring), and Scale (axis
//! cubes + uniform center) are pure geometry. The editor paints and routes
//! input; drag math lives here so CLI tests can match the same deltas.

use crate::prefs::HandlePosition;
use crate::scene::Scene;

/// Which part of the Move gizmo the pointer grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslateHandle {
    /// Constrain to the handle's X axis (red). World +X when Global.
    X,
    /// Constrain to the handle's Y axis (green). World +Y when Global.
    Y,
    /// Free XY (corner square, or entity body).
    Free,
}

/// Which part of the Scale gizmo the pointer grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleHandle {
    /// Local X only (red cube / shaft).
    X,
    /// Local Y only (green cube / shaft).
    Y,
    /// Both axes, same factor (grey center square, or entity body).
    Uniform,
}

/// Which part of the Rotate gizmo the pointer grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotateHandle {
    /// Blue Z ring.
    Z,
    /// Faint outer ring. In 2D this is the same Z sweep.
    Free,
}

/// Minimum |scale| written by a scale drag. Never zero.
pub const SCALE_MIN: f32 = 0.05;
/// Maximum |scale| written by a scale drag.
pub const SCALE_MAX: f32 = 32.0;
/// Unity rotate snap (degrees) when Snap or Ctrl is held.
pub const ROTATE_SNAP_DEG: f32 = 15.0;
/// [`ROTATE_SNAP_DEG`] in radians.
pub const ROTATE_SNAP_RAD: f32 = ROTATE_SNAP_DEG * std::f32::consts::PI / 180.0;

/// Pixel layout of a 2D translate gizmo at a screen-space origin.
///
/// `x_dir` / `y_dir` are unit screen directions. Global uses `(1,0)` / `(0,1)`
/// (Y down, matching the Scene blit). Local uses the entity's world Z.
#[derive(Clone, Copy, Debug)]
pub struct MoveHandleLayout {
    pub origin: [f32; 2],
    pub x_dir: [f32; 2],
    pub y_dir: [f32; 2],
    pub axis_len: f32,
    pub shaft_half: f32,
    pub free_half: f32,
    pub free_inset: f32,
}

impl MoveHandleLayout {
    pub const AXIS_LEN: f32 = 72.0;
    pub const SHAFT_HALF: f32 = 8.0;
    pub const FREE_HALF: f32 = 6.0;
    pub const FREE_INSET: f32 = 10.0;
    pub const ARROW: f32 = 11.0;

    pub fn at(origin: [f32; 2]) -> Self {
        Self::oriented(origin, [1.0, 0.0], [0.0, 1.0])
    }

    pub fn oriented(origin: [f32; 2], x_dir: [f32; 2], y_dir: [f32; 2]) -> Self {
        Self {
            origin,
            x_dir: safe_unit(x_dir, [1.0, 0.0]),
            y_dir: safe_unit(y_dir, [0.0, 1.0]),
            axis_len: Self::AXIS_LEN,
            shaft_half: Self::SHAFT_HALF,
            free_half: Self::FREE_HALF,
            free_inset: Self::FREE_INSET,
        }
    }

    pub fn x_tip(&self) -> [f32; 2] {
        self.along(self.x_dir, self.axis_len)
    }

    pub fn y_tip(&self) -> [f32; 2] {
        self.along(self.y_dir, self.axis_len)
    }

    /// Shaft starts just past the origin so the free square is easier to grab.
    pub fn x_base(&self) -> [f32; 2] {
        self.along(self.x_dir, self.free_half + 2.0)
    }

    pub fn y_base(&self) -> [f32; 2] {
        self.along(self.y_dir, self.free_half + 2.0)
    }

    /// XY free-move square in the +X/+Y quadrant of the handle axes.
    pub fn free_corners(&self) -> [[f32; 2]; 4] {
        oriented_square_corners(
            self.origin,
            self.x_dir,
            self.y_dir,
            self.free_inset,
            self.free_half * 2.0,
        )
    }

    pub fn free_min(&self) -> [f32; 2] {
        aabb_min(&self.free_corners())
    }

    pub fn free_max(&self) -> [f32; 2] {
        aabb_max(&self.free_corners())
    }

    pub fn x_arrow(&self) -> [[f32; 2]; 3] {
        arrow_tri(self.x_tip(), self.x_dir, Self::ARROW)
    }

    pub fn y_arrow(&self) -> [[f32; 2]; 3] {
        arrow_tri(self.y_tip(), self.y_dir, Self::ARROW)
    }

    pub fn hit(&self, pointer: [f32; 2]) -> Option<TranslateHandle> {
        if point_in_poly(pointer, &self.free_corners()) {
            return Some(TranslateHandle::Free);
        }
        let dx = dist_to_segment(pointer, self.x_base(), self.x_tip());
        let dy = dist_to_segment(pointer, self.y_base(), self.y_tip());
        let arrow = Self::ARROW;
        let near_x_tip = dist(pointer, self.x_tip()) <= arrow;
        let near_y_tip = dist(pointer, self.y_tip()) <= arrow;
        if dx <= self.shaft_half || near_x_tip {
            return Some(TranslateHandle::X);
        }
        if dy <= self.shaft_half || near_y_tip {
            return Some(TranslateHandle::Y);
        }
        None
    }

    fn along(&self, dir: [f32; 2], dist: f32) -> [f32; 2] {
        [
            self.origin[0] + dir[0] * dist,
            self.origin[1] + dir[1] * dist,
        ]
    }
}

/// Pixel layout of a 2D scale gizmo (axis lines, end cubes, center square).
#[derive(Clone, Copy, Debug)]
pub struct ScaleHandleLayout {
    pub origin: [f32; 2],
    pub x_dir: [f32; 2],
    pub y_dir: [f32; 2],
    pub axis_len: f32,
    pub shaft_half: f32,
    pub cube_half: f32,
    pub center_half: f32,
}

impl ScaleHandleLayout {
    pub const AXIS_LEN: f32 = 72.0;
    pub const SHAFT_HALF: f32 = 6.0;
    pub const CUBE_HALF: f32 = 5.0;
    pub const CENTER_HALF: f32 = 7.0;

    pub fn at(origin: [f32; 2]) -> Self {
        Self::oriented(origin, [1.0, 0.0], [0.0, 1.0])
    }

    pub fn oriented(origin: [f32; 2], x_dir: [f32; 2], y_dir: [f32; 2]) -> Self {
        Self {
            origin,
            x_dir: safe_unit(x_dir, [1.0, 0.0]),
            y_dir: safe_unit(y_dir, [0.0, 1.0]),
            axis_len: Self::AXIS_LEN,
            shaft_half: Self::SHAFT_HALF,
            cube_half: Self::CUBE_HALF,
            center_half: Self::CENTER_HALF,
        }
    }

    pub fn x_base(&self) -> [f32; 2] {
        self.along(self.x_dir, self.center_half + 2.0)
    }

    pub fn y_base(&self) -> [f32; 2] {
        self.along(self.y_dir, self.center_half + 2.0)
    }

    pub fn x_tip(&self) -> [f32; 2] {
        self.along(self.x_dir, self.axis_len)
    }

    pub fn y_tip(&self) -> [f32; 2] {
        self.along(self.y_dir, self.axis_len)
    }

    pub fn center_corners(&self) -> [[f32; 2]; 4] {
        centered_square(self.origin, self.x_dir, self.y_dir, self.center_half)
    }

    pub fn x_cube(&self) -> [[f32; 2]; 4] {
        centered_square(self.x_tip(), self.x_dir, self.y_dir, self.cube_half)
    }

    pub fn y_cube(&self) -> [[f32; 2]; 4] {
        centered_square(self.y_tip(), self.x_dir, self.y_dir, self.cube_half)
    }

    pub fn hit(&self, pointer: [f32; 2]) -> Option<ScaleHandle> {
        if point_in_poly(pointer, &self.center_corners()) {
            return Some(ScaleHandle::Uniform);
        }
        if point_in_poly(pointer, &self.x_cube())
            || dist_to_segment(pointer, self.x_base(), self.x_tip()) <= self.shaft_half
        {
            return Some(ScaleHandle::X);
        }
        if point_in_poly(pointer, &self.y_cube())
            || dist_to_segment(pointer, self.y_base(), self.y_tip()) <= self.shaft_half
        {
            return Some(ScaleHandle::Y);
        }
        None
    }

    fn along(&self, dir: [f32; 2], dist: f32) -> [f32; 2] {
        [
            self.origin[0] + dir[0] * dist,
            self.origin[1] + dir[1] * dist,
        ]
    }
}

/// Pixel layout of a 2D rotate gizmo (Z ring + faint outer ring).
#[derive(Clone, Copy, Debug)]
pub struct RotateHandleLayout {
    pub origin: [f32; 2],
    pub radius: f32,
    pub band: f32,
    pub outer_radius: f32,
}

impl RotateHandleLayout {
    pub const RADIUS: f32 = 64.0;
    pub const BAND: f32 = 8.0;
    pub const OUTER_RADIUS: f32 = 86.0;

    pub fn at(origin: [f32; 2]) -> Self {
        Self {
            origin,
            radius: Self::RADIUS,
            band: Self::BAND,
            outer_radius: Self::OUTER_RADIUS,
        }
    }

    pub fn hit(&self, pointer: [f32; 2]) -> Option<RotateHandle> {
        let d = dist(pointer, self.origin);
        if (d - self.radius).abs() <= self.band {
            return Some(RotateHandle::Z);
        }
        if (d - self.outer_radius).abs() <= self.band * 0.75 {
            return Some(RotateHandle::Free);
        }
        None
    }
}

/// Lock the unused axis to `start` for X/Y handles (world-aligned).
pub fn constrain_translate(handle: TranslateHandle, x: f32, y: f32, start: [f32; 2]) -> [f32; 2] {
    constrain_translate_oriented(handle, x, y, start, 0.0)
}

/// Like [`constrain_translate`], but X/Y follow `axis_angle` (radians, Y-down).
///
/// `axis_angle` 0 keeps world axes. Local mode passes the active entity's world Z.
pub fn constrain_translate_oriented(
    handle: TranslateHandle,
    x: f32,
    y: f32,
    start: [f32; 2],
    axis_angle: f32,
) -> [f32; 2] {
    if !x.is_finite() || !y.is_finite() || !axis_angle.is_finite() {
        return start;
    }
    match handle {
        TranslateHandle::Free => [x, y],
        TranslateHandle::X | TranslateHandle::Y => {
            let (x_axis, y_axis) = axis_dirs(axis_angle);
            let axis = if handle == TranslateHandle::X {
                x_axis
            } else {
                y_axis
            };
            let t = (x - start[0]) * axis[0] + (y - start[1]) * axis[1];
            if !t.is_finite() {
                return start;
            }
            [start[0] + axis[0] * t, start[1] + axis[1] * t]
        }
    }
}

/// World-space unit axes for a handle rotation. Angle 0 is world +X / +Y.
pub fn axis_dirs(angle: f32) -> ([f32; 2], [f32; 2]) {
    if !angle.is_finite() {
        return ([1.0, 0.0], [0.0, 1.0]);
    }
    let (c, s) = (angle.cos(), angle.sin());
    ([c, s], [-s, c])
}

/// Screen-pixel unit axes. `px_per_scene_*` is the Scene blit scale.
pub fn screen_axis_dirs(
    angle: f32,
    px_per_scene_x: f32,
    px_per_scene_y: f32,
) -> ([f32; 2], [f32; 2]) {
    let (x, y) = axis_dirs(angle);
    let sx = if px_per_scene_x.is_finite() && px_per_scene_x.abs() > 1e-6 {
        px_per_scene_x
    } else {
        1.0
    };
    let sy = if px_per_scene_y.is_finite() && px_per_scene_y.abs() > 1e-6 {
        px_per_scene_y
    } else {
        1.0
    };
    (
        safe_unit([x[0] * sx, x[1] * sy], [1.0, 0.0]),
        safe_unit([y[0] * sx, y[1] * sy], [0.0, 1.0]),
    )
}

/// Z angle of a 2D quaternion stored as scene `[x, y, z, w]`.
pub fn quat_z_radians(q: [f32; 4]) -> f32 {
    let z = 2.0 * q[2] * q[3];
    let w = q[3] * q[3] - q[2] * q[2];
    let a = z.atan2(w);
    if a.is_finite() {
        a
    } else {
        0.0
    }
}

/// Wrap `a` into (−π, π].
pub fn wrap_pi(a: f32) -> f32 {
    if !a.is_finite() {
        return 0.0;
    }
    let tau = std::f32::consts::TAU;
    let mut a = a % tau;
    if a > std::f32::consts::PI {
        a -= tau;
    } else if a <= -std::f32::consts::PI {
        a += tau;
    }
    a
}

/// Shortest signed sweep from `start` to `pointer` around `origin` (radians).
pub fn pointer_angle_delta(origin: [f32; 2], start: [f32; 2], pointer: [f32; 2]) -> f32 {
    let a0 = (start[1] - origin[1]).atan2(start[0] - origin[0]);
    let a1 = (pointer[1] - origin[1]).atan2(pointer[0] - origin[0]);
    if !a0.is_finite() || !a1.is_finite() {
        return 0.0;
    }
    wrap_pi(a1 - a0)
}

/// Snap `radians` to the nearest multiple of `step`.
pub fn snap_angle(radians: f32, step: f32) -> f32 {
    if !radians.is_finite() || !step.is_finite() || step.abs() < 1e-8 {
        return if radians.is_finite() { radians } else { 0.0 };
    }
    let s = (radians / step).round() * step;
    if s.is_finite() {
        s
    } else {
        radians
    }
}

/// Absolute Z rotation after a ring drag. `snap` uses [`ROTATE_SNAP_RAD`].
pub fn rotation_after_drag(
    start_rot: f32,
    origin: [f32; 2],
    start_pointer: [f32; 2],
    pointer: [f32; 2],
    snap: bool,
) -> f32 {
    let delta = pointer_angle_delta(origin, start_pointer, pointer);
    let mut r = if start_rot.is_finite() {
        start_rot + delta
    } else {
        delta
    };
    if snap {
        r = snap_angle(r, ROTATE_SNAP_RAD);
    }
    if r.is_finite() {
        r
    } else {
        0.0
    }
}

/// Polyline of a swept arc (inclusive). `delta` may be negative.
pub fn arc_points(
    origin: [f32; 2],
    radius: f32,
    start: f32,
    delta: f32,
    steps: usize,
) -> Vec<[f32; 2]> {
    let steps = steps.max(1);
    if !radius.is_finite() || !start.is_finite() || !delta.is_finite() {
        return vec![origin];
    }
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = start + delta * t;
        pts.push([origin[0] + radius * a.cos(), origin[1] + radius * a.sin()]);
    }
    pts
}

/// `(scale_x factor, scale_y factor)` for a scale drag. Never NaN.
///
/// X/Y project onto the handle axis (`axis_angle` 0 = world). Uniform uses
/// pointer distance from `origin`, or an exponential of the drag when the
/// grab is on the origin (center square). A near-zero axis projection
/// returns 1 (no divide by zero).
pub fn scale_factors(
    handle: ScaleHandle,
    origin: [f32; 2],
    axis_angle: f32,
    start_pointer: [f32; 2],
    pointer: [f32; 2],
) -> [f32; 2] {
    let (x_axis, y_axis) = axis_dirs(axis_angle);
    match handle {
        ScaleHandle::X => [factor_along(origin, x_axis, start_pointer, pointer), 1.0],
        ScaleHandle::Y => [1.0, factor_along(origin, y_axis, start_pointer, pointer)],
        ScaleHandle::Uniform => {
            let f = factor_uniform(origin, start_pointer, pointer);
            [f, f]
        }
    }
}

/// `start * factor`, sign preserved, magnitude clamped to [`SCALE_MIN`]..=[`SCALE_MAX`].
pub fn apply_axis_scale(start: f32, factor: f32) -> f32 {
    let fallback = if start.is_sign_negative() {
        -SCALE_MIN
    } else {
        SCALE_MIN
    };
    if !start.is_finite() || !factor.is_finite() {
        return fallback;
    }
    let v = start * factor;
    if !v.is_finite() || v == 0.0 {
        return fallback;
    }
    let sign = if v.is_sign_negative() { -1.0 } else { 1.0 };
    sign * v.abs().clamp(SCALE_MIN, SCALE_MAX)
}

/// Scale `point` about `origin` in the handle's axis frame.
pub fn scale_point_about(
    origin: [f32; 2],
    point: [f32; 2],
    axis_angle: f32,
    factors: [f32; 2],
) -> [f32; 2] {
    if !factors[0].is_finite() || !factors[1].is_finite() {
        return point;
    }
    let (x_axis, y_axis) = axis_dirs(axis_angle);
    let d = [point[0] - origin[0], point[1] - origin[1]];
    let lx = (d[0] * x_axis[0] + d[1] * x_axis[1]) * factors[0];
    let ly = (d[0] * y_axis[0] + d[1] * y_axis[1]) * factors[1];
    if !lx.is_finite() || !ly.is_finite() {
        return point;
    }
    [
        origin[0] + x_axis[0] * lx + y_axis[0] * ly,
        origin[1] + x_axis[1] * lx + y_axis[1] * ly,
    ]
}

/// Orbit `point` around `origin` by `delta` radians (Y-down, matching atan2).
pub fn rotate_point_about(origin: [f32; 2], point: [f32; 2], delta: f32) -> [f32; 2] {
    if !delta.is_finite() {
        return point;
    }
    let d = [point[0] - origin[0], point[1] - origin[1]];
    let (c, s) = (delta.cos(), delta.sin());
    [
        origin[0] + d[0] * c - d[1] * s,
        origin[1] + d[0] * s + d[1] * c,
    ]
}

/// True when `name` is nested under another entry in `selected` (both in the scene).
///
/// Rotate/Scale gizmo drags skip such entities so parent-driven hierarchy is not
/// applied twice (Move uses absolute world XY and does not need this filter).
pub fn gizmo_has_selected_ancestor(scene: &Scene, name: &str, selected: &[String]) -> bool {
    selected
        .iter()
        .any(|other| other != name && scene.is_descendant_of(name, other))
}

/// Names in `selected` that should receive explicit Rotate/Scale writes.
pub fn gizmo_rotate_scale_targets(scene: &Scene, selected: &[String]) -> Vec<String> {
    selected
        .iter()
        .filter(|name| !gizmo_has_selected_ancestor(scene, name, selected))
        .cloned()
        .collect()
}

/// Clamp one local scale axis the same way [`apply_axis_scale`] does for factors.
pub fn clamp_entity_scale_axis(s: f32) -> f32 {
    apply_axis_scale(1.0, if s.is_finite() && s != 0.0 { s } else { f32::NAN })
}

/// Pivot: active origin. Center: bounds center when more than one entity is selected.
/// A single selection uses the active origin for both modes.
pub fn gizmo_origin(
    position: HandlePosition,
    active: [f32; 2],
    bounds_center: Option<[f32; 2]>,
    selected_count: usize,
) -> [f32; 2] {
    if selected_count <= 1 || position == HandlePosition::Pivot {
        return active;
    }
    bounds_center.unwrap_or(active)
}

/// Center of the union of AABBs `(min, max)`.
pub fn bounds_center(aabbs: &[([f32; 2], [f32; 2])]) -> Option<[f32; 2]> {
    let (min, max) = union_aabbs(aabbs)?;
    Some([(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5])
}

pub fn union_aabbs(aabbs: &[([f32; 2], [f32; 2])]) -> Option<([f32; 2], [f32; 2])> {
    let mut iter = aabbs.iter().copied().filter(|(mn, mx)| {
        mn[0].is_finite() && mn[1].is_finite() && mx[0].is_finite() && mx[1].is_finite()
    });
    let (mut min, mut max) = iter.next()?;
    for (mn, mx) in iter {
        min[0] = min[0].min(mn[0]);
        min[1] = min[1].min(mn[1]);
        max[0] = max[0].max(mx[0]);
        max[1] = max[1].max(mx[1]);
    }
    Some((min, max))
}

fn factor_along(origin: [f32; 2], axis: [f32; 2], start: [f32; 2], pointer: [f32; 2]) -> f32 {
    let s = (start[0] - origin[0]) * axis[0] + (start[1] - origin[1]) * axis[1];
    let p = (pointer[0] - origin[0]) * axis[0] + (pointer[1] - origin[1]) * axis[1];
    if !s.is_finite() || !p.is_finite() || s.abs() < 1e-4 {
        return 1.0;
    }
    let f = p / s;
    if f.is_finite() {
        f
    } else {
        1.0
    }
}

/// Uniform scale. Grabs away from the origin use the distance ratio (toward
/// the origin shrinks). A center-square grab sits on the origin, so the ratio
/// is undefined — use an exponential of the pointer travel instead (1 at the
/// grab, grows one way, shrinks the other, never divides by zero).
fn factor_uniform(origin: [f32; 2], start: [f32; 2], pointer: [f32; 2]) -> f32 {
    let s = dist(start, origin);
    if !s.is_finite() {
        return 1.0;
    }
    if s >= 8.0 {
        return factor_radial(origin, start, pointer);
    }
    let dx = pointer[0] - start[0];
    let dy = pointer[1] - start[1];
    if !dx.is_finite() || !dy.is_finite() {
        return 1.0;
    }
    let travel = if dx.abs() >= dy.abs() { dx } else { dy };
    let f = (travel / 64.0).exp();
    if f.is_finite() {
        f
    } else {
        1.0
    }
}

fn factor_radial(origin: [f32; 2], start: [f32; 2], pointer: [f32; 2]) -> f32 {
    let s = dist(start, origin);
    let p = dist(pointer, origin);
    if s < 1e-4 || !p.is_finite() {
        return 1.0;
    }
    let f = p / s;
    if f.is_finite() {
        f
    } else {
        1.0
    }
}

fn safe_unit(v: [f32; 2], fallback: [f32; 2]) -> [f32; 2] {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if !len.is_finite() || len < 1e-6 {
        fallback
    } else {
        [v[0] / len, v[1] / len]
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

fn dist_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let vx = b[0] - a[0];
    let vy = b[1] - a[1];
    let len2 = vx * vx + vy * vy;
    if len2 < 1e-8 {
        return dist(p, a);
    }
    let t = ((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / len2;
    let t = t.clamp(0.0, 1.0);
    dist(p, [a[0] + t * vx, a[1] + t * vy])
}

fn arrow_tri(tip: [f32; 2], dir: [f32; 2], size: f32) -> [[f32; 2]; 3] {
    let n = [-dir[1], dir[0]];
    let back = [tip[0] - dir[0] * size, tip[1] - dir[1] * size];
    let w = size * 0.55;
    [
        tip,
        [back[0] + n[0] * w, back[1] + n[1] * w],
        [back[0] - n[0] * w, back[1] - n[1] * w],
    ]
}

/// Square sitting in the +X/+Y quadrant, `inset` from the origin along both axes.
fn oriented_square_corners(
    origin: [f32; 2],
    x_dir: [f32; 2],
    y_dir: [f32; 2],
    inset: f32,
    size: f32,
) -> [[f32; 2]; 4] {
    let o = [
        origin[0] + x_dir[0] * inset + y_dir[0] * inset,
        origin[1] + x_dir[1] * inset + y_dir[1] * inset,
    ];
    let x = [x_dir[0] * size, x_dir[1] * size];
    let y = [y_dir[0] * size, y_dir[1] * size];
    [
        o,
        [o[0] + x[0], o[1] + x[1]],
        [o[0] + x[0] + y[0], o[1] + x[1] + y[1]],
        [o[0] + y[0], o[1] + y[1]],
    ]
}

fn centered_square(center: [f32; 2], x_dir: [f32; 2], y_dir: [f32; 2], half: f32) -> [[f32; 2]; 4] {
    let x = [x_dir[0] * half, x_dir[1] * half];
    let y = [y_dir[0] * half, y_dir[1] * half];
    [
        [center[0] - x[0] - y[0], center[1] - x[1] - y[1]],
        [center[0] + x[0] - y[0], center[1] + x[1] - y[1]],
        [center[0] + x[0] + y[0], center[1] + x[1] + y[1]],
        [center[0] - x[0] + y[0], center[1] - x[1] + y[1]],
    ]
}

fn point_in_poly(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut sign = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if cross.abs() < 1e-5 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign * cross < 0.0 {
            return false;
        }
    }
    sign != 0.0
}

fn aabb_min(pts: &[[f32; 2]]) -> [f32; 2] {
    [
        pts.iter().map(|p| p[0]).fold(f32::MAX, f32::min),
        pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min),
    ]
}

fn aabb_max(pts: &[[f32; 2]]) -> [f32; 2] {
    [
        pts.iter().map(|p| p[0]).fold(f32::MIN, f32::max),
        pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_shaft_hits_x() {
        let l = MoveHandleLayout::at([100.0, 200.0]);
        assert_eq!(l.hit([140.0, 200.0]), Some(TranslateHandle::X));
        assert_eq!(l.hit([140.0, 204.0]), Some(TranslateHandle::X));
    }

    #[test]
    fn y_shaft_hits_y() {
        let l = MoveHandleLayout::at([100.0, 200.0]);
        assert_eq!(l.hit([100.0, 240.0]), Some(TranslateHandle::Y));
        assert_eq!(l.hit([103.0, 250.0]), Some(TranslateHandle::Y));
    }

    #[test]
    fn free_square_hits_free() {
        let l = MoveHandleLayout::at([100.0, 200.0]);
        let mid = [
            (l.free_min()[0] + l.free_max()[0]) * 0.5,
            (l.free_min()[1] + l.free_max()[1]) * 0.5,
        ];
        assert_eq!(l.hit(mid), Some(TranslateHandle::Free));
    }

    #[test]
    fn miss_away_from_gizmo() {
        let l = MoveHandleLayout::at([100.0, 200.0]);
        assert_eq!(l.hit([300.0, 400.0]), None);
    }

    #[test]
    fn constrain_locks_unused_axis() {
        let start = [10.0, 20.0];
        assert_eq!(
            constrain_translate(TranslateHandle::X, 50.0, 99.0, start),
            [50.0, 20.0]
        );
        assert_eq!(
            constrain_translate(TranslateHandle::Y, 50.0, 99.0, start),
            [10.0, 99.0]
        );
        assert_eq!(
            constrain_translate(TranslateHandle::Free, 50.0, 99.0, start),
            [50.0, 99.0]
        );
    }

    #[test]
    fn local_move_follows_rotated_axes() {
        let start = [10.0, 20.0];
        let quarter = std::f32::consts::FRAC_PI_2;
        // Local +X is world +Y. Motion along world X is dropped.
        let along_y = constrain_translate_oriented(TranslateHandle::X, 50.0, 99.0, start, quarter);
        assert!((along_y[0] - 10.0).abs() < 1e-4, "{along_y:?}");
        assert!((along_y[1] - 99.0).abs() < 1e-4, "{along_y:?}");
        // Local +Y is world −X.
        let along_x = constrain_translate_oriented(TranslateHandle::Y, 50.0, 99.0, start, quarter);
        assert!((along_x[0] - 50.0).abs() < 1e-4, "{along_x:?}");
        assert!((along_x[1] - 20.0).abs() < 1e-4, "{along_x:?}");
        // Angle 0 matches the world-axis helper.
        assert_eq!(
            constrain_translate_oriented(TranslateHandle::X, 50.0, 99.0, start, 0.0),
            constrain_translate(TranslateHandle::X, 50.0, 99.0, start)
        );
    }

    #[test]
    fn rotated_move_shaft_hits_local_x() {
        let (x, y) = axis_dirs(std::f32::consts::FRAC_PI_2);
        let l = MoveHandleLayout::oriented([100.0, 200.0], x, y);
        // Local +X points down the screen.
        assert_eq!(l.hit([100.0, 250.0]), Some(TranslateHandle::X));
        // Local +Y points left.
        assert_eq!(l.hit([50.0, 200.0]), Some(TranslateHandle::Y));
    }

    #[test]
    fn rotate_ring_hit_and_miss() {
        let l = RotateHandleLayout::at([80.0, 90.0]);
        assert_eq!(
            l.hit([80.0 + RotateHandleLayout::RADIUS, 90.0]),
            Some(RotateHandle::Z)
        );
        assert_eq!(
            l.hit([80.0 + RotateHandleLayout::OUTER_RADIUS, 90.0]),
            Some(RotateHandle::Free)
        );
        assert_eq!(l.hit([80.0, 90.0]), None);
        assert_eq!(l.hit([80.0 + 200.0, 90.0]), None);
    }

    #[test]
    fn rotate_sweep_quarter_turn() {
        let origin = [0.0, 0.0];
        let start = [40.0, 0.0];
        let end = [0.0, 40.0];
        let d = pointer_angle_delta(origin, start, end);
        assert!((d - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{d}");
        let r = rotation_after_drag(0.2, origin, start, end, false);
        assert!((r - (0.2 + std::f32::consts::FRAC_PI_2)).abs() < 1e-4);
    }

    #[test]
    fn rotate_wraps_across_plus_minus_180() {
        let origin = [0.0, 0.0];
        let a = |deg: f32| {
            let r = deg.to_radians();
            [r.cos(), r.sin()]
        };
        // 170° → −170° is a +20° sweep, not −340°.
        let d = pointer_angle_delta(origin, a(170.0), a(-170.0));
        assert!(
            (d - 20.0_f32.to_radians()).abs() < 1e-3,
            "short sweep {d} rad"
        );
        // The other way is −20°.
        let d = pointer_angle_delta(origin, a(-170.0), a(170.0));
        assert!((d - (-20.0_f32.to_radians())).abs() < 1e-3, "{d}");

        let start_rot = 170.0_f32.to_radians();
        let r = rotation_after_drag(start_rot, origin, a(170.0), a(-170.0), false);
        assert!((wrap_pi(r - start_rot) - 20.0_f32.to_radians()).abs() < 1e-3);
    }

    #[test]
    fn rotate_snap_is_15_degrees() {
        let origin = [0.0, 0.0];
        let start_rot = 0.0;
        // 10° pointer sweep snaps to 15°.
        let end = 10.0_f32.to_radians();
        let r = rotation_after_drag(start_rot, origin, [1.0, 0.0], [end.cos(), end.sin()], true);
        assert!((r - 15.0_f32.to_radians()).abs() < 1e-3, "{r}");
        // 7° snaps back to 0.
        let end = 7.0_f32.to_radians();
        let r = rotation_after_drag(start_rot, origin, [1.0, 0.0], [end.cos(), end.sin()], true);
        assert!(r.abs() < 1e-3, "{r}");
        // Crossing ±180 with snap: 170° + 20° = 190° → 195°.
        let start_rot = 170.0_f32.to_radians();
        let r = rotation_after_drag(
            start_rot,
            origin,
            {
                let a = 170.0_f32.to_radians();
                [a.cos(), a.sin()]
            },
            {
                let a = (-170.0_f32).to_radians();
                [a.cos(), a.sin()]
            },
            true,
        );
        assert!((r - 195.0_f32.to_radians()).abs() < 1e-2, "{r}");
    }

    #[test]
    fn scale_hit_center_axes_and_rotated() {
        let l = ScaleHandleLayout::at([100.0, 200.0]);
        assert_eq!(l.hit([100.0, 200.0]), Some(ScaleHandle::Uniform));
        assert_eq!(l.hit([150.0, 200.0]), Some(ScaleHandle::X));
        assert_eq!(l.hit(l.x_tip()), Some(ScaleHandle::X));
        assert_eq!(l.hit([100.0, 250.0]), Some(ScaleHandle::Y));
        assert_eq!(l.hit([400.0, 400.0]), None);

        let (x, y) = axis_dirs(std::f32::consts::FRAC_PI_2);
        let l = ScaleHandleLayout::oriented([100.0, 200.0], x, y);
        assert_eq!(l.hit([100.0, 250.0]), Some(ScaleHandle::X));
        assert_eq!(l.hit([60.0, 200.0]), Some(ScaleHandle::Y));
    }

    #[test]
    fn scale_axis_math_and_min_clamp() {
        let origin = [0.0, 0.0];
        let start = [40.0, 10.0];
        let ptr = [80.0, -30.0];
        let f = scale_factors(ScaleHandle::X, origin, 0.0, start, ptr);
        assert!((f[0] - 2.0).abs() < 1e-4, "{f:?}");
        assert!((f[1] - 1.0).abs() < 1e-4, "{f:?}");
        let f = scale_factors(ScaleHandle::Y, origin, 0.0, [10.0, 20.0], [10.0, 50.0]);
        assert!((f[0] - 1.0).abs() < 1e-4);
        assert!((f[1] - 2.5).abs() < 1e-4);

        let f = scale_factors(ScaleHandle::Uniform, origin, 0.0, [30.0, 0.0], [60.0, 0.0]);
        assert!((f[0] - 2.0).abs() < 1e-4 && (f[1] - 2.0).abs() < 1e-4);

        // Start on the origin: no divide-by-zero.
        let f = scale_factors(ScaleHandle::X, origin, 0.0, origin, [10.0, 0.0]);
        assert!((f[0] - 1.0).abs() < 1e-4);
        let f = scale_factors(ScaleHandle::Uniform, origin, 0.0, origin, origin);
        assert!((f[0] - 1.0).abs() < 1e-4 && (f[1] - 1.0).abs() < 1e-4);
        let grown = scale_factors(ScaleHandle::Uniform, origin, 0.0, origin, [64.0, 0.0]);
        assert!(grown[0] > 2.0 && grown[0].is_finite(), "{grown:?}");
        let shrunk = scale_factors(ScaleHandle::Uniform, origin, 0.0, origin, [-64.0, 0.0]);
        assert!(
            shrunk[0] < 0.5 && shrunk[0] > 0.0 && shrunk[0].is_finite(),
            "{shrunk:?}"
        );
        assert!(apply_axis_scale(2.0, shrunk[0]).is_finite());

        assert!((apply_axis_scale(2.0, 0.0) - SCALE_MIN).abs() < 1e-6);
        assert!((apply_axis_scale(2.0, f32::NAN) - SCALE_MIN).abs() < 1e-6);
        assert!((apply_axis_scale(1.0, 1000.0) - SCALE_MAX).abs() < 1e-4);
        assert!((apply_axis_scale(-3.0, 0.001) - -SCALE_MIN).abs() < 1e-4);
        assert!(apply_axis_scale(2.0, 1.5).is_finite());

        let p = scale_point_about([10.0, 10.0], [20.0, 14.0], 0.0, [2.0, 1.0]);
        assert!((p[0] - 30.0).abs() < 1e-4, "{p:?}");
        assert!((p[1] - 14.0).abs() < 1e-4, "{p:?}");
    }

    #[test]
    fn pivot_and_center_origins() {
        let active = [1.0, 2.0];
        let center = Some([9.0, 8.0]);
        assert_eq!(
            gizmo_origin(HandlePosition::Pivot, active, center, 3),
            active
        );
        assert_eq!(
            gizmo_origin(HandlePosition::Center, active, center, 3),
            [9.0, 8.0]
        );
        assert_eq!(
            gizmo_origin(HandlePosition::Center, active, center, 1),
            active
        );
        assert_eq!(
            gizmo_origin(HandlePosition::Pivot, active, center, 1),
            active
        );
        let c = bounds_center(&[([0.0, 0.0], [10.0, 4.0]), ([2.0, -2.0], [12.0, 6.0])]).unwrap();
        assert!((c[0] - 6.0).abs() < 1e-4, "{c:?}");
        assert!((c[1] - 2.0).abs() < 1e-4, "{c:?}");
    }

    #[test]
    fn rotate_point_about_quarter_turn() {
        let p = rotate_point_about([0.0, 0.0], [10.0, 0.0], std::f32::consts::FRAC_PI_2);
        assert!(p[0].abs() < 1e-3, "{p:?}");
        assert!((p[1] - 10.0).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn gizmo_rotate_scale_targets_skip_nested_selection() {
        use crate::mutate::{add_entity, set_entity_parent, MutateOpts};
        use crate::scene::Scene;

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "P",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "C",
            &MutateOpts {
                x: Some(10.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_parent(&mut scene, "C", Some("P")).unwrap();

        let selected = vec!["P".to_string(), "C".to_string()];
        assert_eq!(
            gizmo_rotate_scale_targets(&scene, &selected),
            vec!["P".to_string()]
        );
        assert!(gizmo_has_selected_ancestor(&scene, "C", &selected));
        assert!(!gizmo_has_selected_ancestor(&scene, "P", &selected));
    }

    #[test]
    fn rotate_and_scale_apply_once_when_parent_and_child_selected() {
        use crate::mutate::{
            add_entity, set_entity_parent, set_entity_rotation_z, set_entity_scale,
            set_entity_world_xy, MutateOpts,
        };
        use crate::scene::Scene;

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "P",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "C",
            &MutateOpts {
                x: Some(10.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_parent(&mut scene, "C", Some("P")).unwrap();
        set_entity_scale(&mut scene, "P", 2.0, 2.0).unwrap();
        set_entity_scale(&mut scene, "C", 2.0, 2.0).unwrap();

        let selected = vec!["P".to_string(), "C".to_string()];
        let delta = std::f32::consts::FRAC_PI_2;
        let factors = [2.0, 2.0];
        let origin = [0.0, 0.0];

        for name in gizmo_rotate_scale_targets(&scene, &selected) {
            let snap = scene.world_transform(&name).unwrap();
            let (rot_z, scale0, scale1) = {
                let local = scene.find_entity(&name).unwrap();
                (
                    quat_z_radians(local.transform.rotation),
                    local.transform.scale[0],
                    local.transform.scale[1],
                )
            };
            set_entity_rotation_z(&mut scene, &name, rot_z + delta).unwrap();
            let p = rotate_point_about(origin, [snap.translation[0], snap.translation[1]], delta);
            set_entity_world_xy(&mut scene, &name, p[0], p[1]).unwrap();
            let sx = apply_axis_scale(scale0, factors[0]);
            let sy = apply_axis_scale(scale1, factors[1]);
            set_entity_scale(&mut scene, &name, sx, sy).unwrap();
        }

        let child_world = scene.world_transform("C").unwrap();
        assert!(
            (quat_z_radians(child_world.rotation) - delta).abs() < 1e-3,
            "child world rot {:?}",
            quat_z_radians(child_world.rotation).to_degrees()
        );
        assert!(
            (child_world.scale[0] - 8.0).abs() < 1e-3,
            "child world scale {:?}",
            child_world.scale
        );
        assert!(
            (quat_z_radians(scene.find_entity("C").unwrap().transform.rotation)).abs() < 1e-3,
            "child local rot should stay 0"
        );
    }
}
