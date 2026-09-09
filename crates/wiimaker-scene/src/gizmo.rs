//! Screen-space Unity-like 2D Move handles (red X, green Y, XY free square).
//!
//! Geometry is in *screen pixels* relative to the entity origin after the
//! Scene blit. The editor paints and hit-tests; CLI does not expose this.

/// Which part of the Move gizmo the pointer grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslateHandle {
    /// Constrain to world X (red).
    X,
    /// Constrain to world Y (green).
    Y,
    /// Free XY (center/corner square, or entity body).
    Free,
}

/// Pixel layout of a 2D translate gizmo at a screen-space origin.
#[derive(Clone, Copy, Debug)]
pub struct MoveHandleLayout {
    pub origin: [f32; 2],
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
        Self {
            origin,
            axis_len: Self::AXIS_LEN,
            shaft_half: Self::SHAFT_HALF,
            free_half: Self::FREE_HALF,
            free_inset: Self::FREE_INSET,
        }
    }

    pub fn x_tip(&self) -> [f32; 2] {
        [self.origin[0] + self.axis_len, self.origin[1]]
    }

    pub fn y_tip(&self) -> [f32; 2] {
        [self.origin[0], self.origin[1] + self.axis_len]
    }

    /// Shaft starts just past the origin so the free square is easier to grab.
    pub fn x_base(&self) -> [f32; 2] {
        [self.origin[0] + self.free_half + 2.0, self.origin[1]]
    }

    pub fn y_base(&self) -> [f32; 2] {
        [self.origin[0], self.origin[1] + self.free_half + 2.0]
    }

    /// XY free-move square in the +X/+Y quadrant (Unity-style, not on the origin).
    pub fn free_min(&self) -> [f32; 2] {
        [
            self.origin[0] + self.free_inset,
            self.origin[1] + self.free_inset,
        ]
    }

    pub fn free_max(&self) -> [f32; 2] {
        [
            self.origin[0] + self.free_inset + self.free_half * 2.0,
            self.origin[1] + self.free_inset + self.free_half * 2.0,
        ]
    }

    pub fn hit(&self, pointer: [f32; 2]) -> Option<TranslateHandle> {
        let fmin = self.free_min();
        let fmax = self.free_max();
        if pointer[0] >= fmin[0]
            && pointer[0] <= fmax[0]
            && pointer[1] >= fmin[1]
            && pointer[1] <= fmax[1]
        {
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
}

/// Lock the unused axis to `start` for X/Y handles.
pub fn constrain_translate(handle: TranslateHandle, x: f32, y: f32, start: [f32; 2]) -> [f32; 2] {
    match handle {
        TranslateHandle::X => [x, start[1]],
        TranslateHandle::Y => [start[0], y],
        TranslateHandle::Free => [x, y],
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
}
