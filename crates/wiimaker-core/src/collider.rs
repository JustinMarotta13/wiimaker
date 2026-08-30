//! AABB / circle colliders + overlap / move-and-collide (Unity BoxCollider2D analogue).

use crate::math::Vec2;
use crate::world::{EntityId, Transform, World};

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Collider shape in local space (centered on transform + [`Collider::offset`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColliderKind {
    /// Axis-aligned box. `size` is full width/height (Unity BoxCollider2D.size).
    Aabb { size: Vec2 },
    /// Circle. `radius` is scaled by max(|sx|, |sy|) like [`crate::world::Disc`].
    Circle { radius: f32 },
}

/// Physics shape on an entity. Not drawn; editor shows an outline gizmo.
///
/// `trigger` is independent of `solid` (Unity keeps both). When `trigger` is
/// true, [`overlap_solid`] / [`move_and_collide`] never treat this collider as a
/// wall — use [`triggers_entered`] to detect collectibles / sensors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collider {
    pub kind: ColliderKind,
    /// Local offset from the entity transform, scaled by the transform.
    pub offset: Vec2,
    /// When true, [`move_and_collide`] treats this as a wall (unless `trigger`).
    pub solid: bool,
    /// Unity `isTrigger`: never blocks movement; participates in [`triggers_entered`].
    pub trigger: bool,
    /// When non-zero on a trigger, the other entity's [`World::tag`] must match.
    pub filter_tag: u32,
}

impl Collider {
    pub fn aabb(width: f32, height: f32) -> Self {
        Self {
            kind: ColliderKind::Aabb {
                size: Vec2::new(width.max(0.0), height.max(0.0)),
            },
            offset: Vec2::ZERO,
            solid: true,
            trigger: false,
            filter_tag: 0,
        }
    }

    pub fn circle(radius: f32) -> Self {
        Self {
            kind: ColliderKind::Circle {
                radius: radius.max(0.0),
            },
            offset: Vec2::ZERO,
            solid: true,
            trigger: false,
            filter_tag: 0,
        }
    }

    pub fn world_center(&self, xf: &Transform) -> Vec2 {
        Vec2::new(
            xf.translation.x + self.offset.x * xf.scale.x,
            xf.translation.y + self.offset.y * xf.scale.y,
        )
    }

    /// World-space AABB (min, max), enclosing circles as their bounding box.
    pub fn world_aabb(&self, xf: &Transform) -> (Vec2, Vec2) {
        let c = self.world_center(xf);
        let half = self.world_half_extents(xf);
        (c - half, c + half)
    }

    pub fn world_half_extents(&self, xf: &Transform) -> Vec2 {
        match self.kind {
            ColliderKind::Aabb { size } => Vec2::new(
                (size.x * 0.5 * xf.scale.x).abs(),
                (size.y * 0.5 * xf.scale.y).abs(),
            ),
            ColliderKind::Circle { radius } => {
                let r = (radius * xf.scale.x.abs().max(xf.scale.y.abs())).abs();
                Vec2::new(r, r)
            }
        }
    }

    /// World-space radius when this is a circle.
    pub fn world_radius(&self, xf: &Transform) -> Option<f32> {
        match self.kind {
            ColliderKind::Circle { radius } => {
                Some((radius * xf.scale.x.abs().max(xf.scale.y.abs())).abs())
            }
            ColliderKind::Aabb { .. } => None,
        }
    }
}

/// Result of [`move_and_collide`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveHit {
    /// Translation actually applied (may be shorter than the requested delta).
    pub applied: Vec2,
    /// First solid collider that blocked a swept axis, if any.
    pub hit: Option<EntityId>,
}

/// True if both entities have colliders that intersect (enabled at hydrate time).
pub fn overlaps(world: &World, a: EntityId, b: EntityId) -> bool {
    if a == b {
        return false;
    }
    let (Some(ca), Some(xa)) = (world.collider(a), world.transform(a)) else {
        return false;
    };
    let (Some(cb), Some(xb)) = (world.collider(b), world.transform(b)) else {
        return false;
    };
    shapes_overlap(ca, xa, cb, xb)
}

/// Every other entity whose collider overlaps `id`.
pub fn overlapping(world: &World, id: EntityId) -> Vec<EntityId> {
    let mut out = Vec::new();
    for other in world.iter_entities() {
        if overlaps(world, id, other) {
            out.push(other);
        }
    }
    out
}

fn filter_ok(trigger_collider: &Collider, other_id: EntityId, world: &World) -> bool {
    trigger_collider.filter_tag == 0 || world.tag(other_id) == Some(trigger_collider.filter_tag)
}

fn trigger_pair_matches(world: &World, id: EntityId, other: EntityId) -> bool {
    let (Some(ca), Some(cb)) = (world.collider(id), world.collider(other)) else {
        return false;
    };
    (ca.trigger && filter_ok(ca, other, world)) || (cb.trigger && filter_ok(cb, id, world))
}

/// Other entities that overlap `id` where at least one collider is a trigger
/// and any non-zero `filter_tag` accepts the counterpart's world tag.
pub fn triggers_entered(world: &World, id: EntityId) -> Vec<EntityId> {
    let mut out = Vec::new();
    for other in world.iter_entities() {
        if other == id {
            continue;
        }
        if overlaps(world, id, other) && trigger_pair_matches(world, id, other) {
            out.push(other);
        }
    }
    out
}

/// First *solid* collider overlapping `id`, excluding self.
pub fn overlap_solid(world: &World, id: EntityId) -> Option<EntityId> {
    let (Some(ca), Some(xa)) = (world.collider(id), world.transform(id)) else {
        return None;
    };
    for other in world.iter_entities() {
        if other == id {
            continue;
        }
        let Some(cb) = world.collider(other) else {
            continue;
        };
        // Triggers never block, regardless of solid (Unity isTrigger).
        if !cb.solid || cb.trigger {
            continue;
        }
        let Some(xb) = world.transform(other) else {
            continue;
        };
        if shapes_overlap(ca, xa, cb, xb) {
            return Some(other);
        }
    }
    None
}

/// Translate `id` by `delta`, stopping/sliding against solid colliders.
///
/// Axes are resolved independently (X then Y) so the mover can slide along walls.
/// Entities without a collider just apply `delta`. Non-solid colliders never block.
pub fn move_and_collide(world: &mut World, id: EntityId, delta: Vec2) -> MoveHit {
    let Some(start) = world.transform(id).map(|t| t.translation) else {
        return MoveHit {
            applied: Vec2::ZERO,
            hit: None,
        };
    };
    if world.collider(id).is_none() {
        if let Some(xf) = world.transform_mut(id) {
            xf.translation.x += delta.x;
            xf.translation.y += delta.y;
        }
        return MoveHit {
            applied: delta,
            hit: None,
        };
    }

    let mut hit = None;
    if let Some(h) = sweep_axis(world, id, start.x, start.y, delta.x, true) {
        hit = Some(h);
    }
    let after_x = world.transform(id).map(|t| t.translation).unwrap_or(start);
    if let Some(h) = sweep_axis(world, id, after_x.x, after_x.y, delta.y, false) {
        hit = hit.or(Some(h));
    }
    let end = world.transform(id).map(|t| t.translation).unwrap_or(start);
    MoveHit {
        applied: Vec2::new(end.x - start.x, end.y - start.y),
        hit,
    }
}

fn sweep_axis(
    world: &mut World,
    id: EntityId,
    start_x: f32,
    start_y: f32,
    delta: f32,
    along_x: bool,
) -> Option<EntityId> {
    if delta.abs() < 1e-8 {
        return None;
    }
    let target_x = if along_x { start_x + delta } else { start_x };
    let target_y = if along_x { start_y } else { start_y + delta };
    if let Some(xf) = world.transform_mut(id) {
        xf.translation.x = target_x;
        xf.translation.y = target_y;
    }
    let Some(blocker) = overlap_solid(world, id) else {
        return None;
    };
    // Binary-search the last non-overlapping pose along this axis.
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    for _ in 0..12 {
        let mid = (lo + hi) * 0.5;
        if let Some(xf) = world.transform_mut(id) {
            if along_x {
                xf.translation.x = start_x + delta * mid;
                xf.translation.y = start_y;
            } else {
                xf.translation.x = start_x;
                xf.translation.y = start_y + delta * mid;
            }
        }
        if overlap_solid(world, id).is_some() {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    if let Some(xf) = world.transform_mut(id) {
        if along_x {
            xf.translation.x = start_x + delta * lo;
            xf.translation.y = start_y;
        } else {
            xf.translation.x = start_x;
            xf.translation.y = start_y + delta * lo;
        }
    }
    Some(blocker)
}

pub(crate) fn shapes_overlap(a: &Collider, xa: &Transform, b: &Collider, xb: &Transform) -> bool {
    match (a.kind, b.kind) {
        (ColliderKind::Aabb { .. }, ColliderKind::Aabb { .. }) => {
            aabb_aabb(a.world_aabb(xa), b.world_aabb(xb))
        }
        (ColliderKind::Circle { .. }, ColliderKind::Circle { .. }) => {
            let ca = a.world_center(xa);
            let cb = b.world_center(xb);
            let ra = a.world_radius(xa).unwrap_or(0.0);
            let rb = b.world_radius(xb).unwrap_or(0.0);
            circle_circle(ca, ra, cb, rb)
        }
        (ColliderKind::Aabb { .. }, ColliderKind::Circle { .. }) => {
            let (min, max) = a.world_aabb(xa);
            aabb_circle(
                min,
                max,
                b.world_center(xb),
                b.world_radius(xb).unwrap_or(0.0),
            )
        }
        (ColliderKind::Circle { .. }, ColliderKind::Aabb { .. }) => {
            let (min, max) = b.world_aabb(xb);
            aabb_circle(
                min,
                max,
                a.world_center(xa),
                a.world_radius(xa).unwrap_or(0.0),
            )
        }
    }
}

fn aabb_aabb(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
    let (amin, amax) = a;
    let (bmin, bmax) = b;
    amin.x < bmax.x && amax.x > bmin.x && amin.y < bmax.y && amax.y > bmin.y
}

fn circle_circle(ca: Vec2, ra: f32, cb: Vec2, rb: f32) -> bool {
    let d = ca - cb;
    let r = ra + rb;
    d.x * d.x + d.y * d.y < r * r
}

fn aabb_circle(min: Vec2, max: Vec2, c: Vec2, r: f32) -> bool {
    let closest = Vec2::new(c.x.clamp(min.x, max.x), c.y.clamp(min.y, max.y));
    let d = c - closest;
    d.x * d.x + d.y * d.y < r * r
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::world::Transform;

    fn spawn_aabb(
        world: &mut World,
        name: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        solid: bool,
    ) -> EntityId {
        let id = world.spawn_named(name, Transform::from_xy(x, y));
        let mut c = Collider::aabb(w, h);
        c.solid = solid;
        world.set_collider(id, Some(c));
        id
    }

    fn spawn_circle(
        world: &mut World,
        name: &str,
        x: f32,
        y: f32,
        r: f32,
        solid: bool,
    ) -> EntityId {
        let id = world.spawn_named(name, Transform::from_xy(x, y));
        let mut c = Collider::circle(r);
        c.solid = solid;
        world.set_collider(id, Some(c));
        id
    }

    #[test]
    fn aabb_overlap_and_gap() {
        let mut world = World::new();
        let a = spawn_aabb(&mut world, "A", 0.0, 0.0, 10.0, 10.0, true);
        let b = spawn_aabb(&mut world, "B", 8.0, 0.0, 10.0, 10.0, true);
        assert!(overlaps(&world, a, b));
        world.transform_mut(b).unwrap().translation.x = 10.0;
        assert!(!overlaps(&world, a, b), "edge-touching is not overlap");
        world.transform_mut(b).unwrap().translation.x = 11.0;
        assert!(!overlaps(&world, a, b));
        assert!(!overlaps(&world, a, a));
    }

    #[test]
    fn aabb_separated_on_y() {
        let mut world = World::new();
        let a = spawn_aabb(&mut world, "A", 0.0, 0.0, 10.0, 10.0, true);
        let b = spawn_aabb(&mut world, "B", 0.0, 10.0, 10.0, 10.0, true);
        assert!(!overlaps(&world, a, b));
        world.transform_mut(b).unwrap().translation.y = 9.0;
        assert!(overlaps(&world, a, b));
    }

    #[test]
    fn circle_vs_circle() {
        let mut world = World::new();
        let a = spawn_circle(&mut world, "A", 0.0, 0.0, 5.0, true);
        let b = spawn_circle(&mut world, "B", 8.0, 0.0, 5.0, true);
        assert!(overlaps(&world, a, b));
        world.transform_mut(b).unwrap().translation.x = 10.0;
        assert!(!overlaps(&world, a, b));
    }

    #[test]
    fn circle_vs_aabb() {
        let mut world = World::new();
        let box_id = spawn_aabb(&mut world, "Box", 0.0, 0.0, 10.0, 10.0, true);
        let ball = spawn_circle(&mut world, "Ball", 8.0, 0.0, 4.0, true);
        // AABB is [-5,5]; circle center 8 radius 4 reaches 4 → overlap
        assert!(overlaps(&world, box_id, ball));
        world.transform_mut(ball).unwrap().translation.x = 10.0;
        // closest point on box is (5,0); dist 5 > 4
        assert!(!overlaps(&world, box_id, ball));
        // corner: box max (5,5), circle at (8,8) r=5 → dist ~4.24
        world.transform_mut(ball).unwrap().translation.x = 8.0;
        world.transform_mut(ball).unwrap().translation.y = 8.0;
        world.set_collider(ball, Some(Collider::circle(5.0)));
        assert!(overlaps(&world, box_id, ball));
        world.set_collider(ball, Some(Collider::circle(4.0)));
        assert!(!overlaps(&world, box_id, ball));
    }

    #[test]
    fn scale_and_offset_change_world_shape() {
        let mut world = World::new();
        let a = spawn_aabb(&mut world, "A", 0.0, 0.0, 10.0, 10.0, true);
        let b = spawn_aabb(&mut world, "B", 16.0, 0.0, 10.0, 10.0, true);
        assert!(!overlaps(&world, a, b));
        world.transform_mut(a).unwrap().scale.x = 2.0;
        // A half-width becomes 10; occupies [-10,10]; B occupies [11,21] — still gap
        assert!(!overlaps(&world, a, b));
        world.transform_mut(a).unwrap().scale.x = 3.0;
        // half-width 15; occupies [-15,15]; B [11,21] → overlap
        assert!(overlaps(&world, a, b));

        world.transform_mut(a).unwrap().scale.x = 1.0;
        let mut c = Collider::aabb(10.0, 10.0);
        c.offset = Vec2::new(10.0, 0.0);
        world.set_collider(a, Some(c));
        // A center at 10, occupies [5,15]; B [11,21]
        assert!(overlaps(&world, a, b));
    }

    #[test]
    fn move_and_collide_stops_against_solid() {
        let mut world = World::new();
        let player = spawn_aabb(&mut world, "Player", 0.0, 0.0, 8.0, 8.0, true);
        let wall = spawn_aabb(&mut world, "Wall", 20.0, 0.0, 8.0, 8.0, true);
        // halves 4+4=8; contact at x=12
        let hit = move_and_collide(&mut world, player, Vec2::new(20.0, 0.0));
        assert_eq!(hit.hit, Some(wall));
        let x = world.transform(player).unwrap().translation.x;
        assert!(x > 10.0, "should approach the wall, got {x}");
        assert!(x < 12.0 + 0.05, "should not penetrate, got {x}");
        assert!(!overlaps(&world, player, wall));
    }

    #[test]
    fn move_and_collide_slides_along_wall() {
        let mut world = World::new();
        let player = spawn_aabb(&mut world, "Player", 0.0, 0.0, 8.0, 8.0, true);
        let _wall = spawn_aabb(&mut world, "Wall", 20.0, 0.0, 8.0, 80.0, true);
        let hit = move_and_collide(&mut world, player, Vec2::new(20.0, 15.0));
        assert!(hit.hit.is_some());
        let t = world.transform(player).unwrap().translation;
        assert!(t.x < 12.1, "x blocked, got {}", t.x);
        assert!((t.y - 15.0).abs() < 0.01, "y should slide, got {}", t.y);
    }

    #[test]
    fn move_and_collide_ignores_nonsolid() {
        let mut world = World::new();
        let player = spawn_aabb(&mut world, "Player", 0.0, 0.0, 8.0, 8.0, true);
        let dot = spawn_aabb(&mut world, "Dot", 20.0, 0.0, 8.0, 8.0, false);
        let hit = move_and_collide(&mut world, player, Vec2::new(20.0, 0.0));
        assert!(hit.hit.is_none());
        assert!((world.transform(player).unwrap().translation.x - 20.0).abs() < 1e-4);
        assert!(overlaps(&world, player, dot));
    }

    #[test]
    fn overlapping_lists_others() {
        let mut world = World::new();
        let a = spawn_aabb(&mut world, "A", 0.0, 0.0, 10.0, 10.0, true);
        let b = spawn_aabb(&mut world, "B", 2.0, 0.0, 10.0, 10.0, false);
        let c = spawn_aabb(&mut world, "C", 100.0, 0.0, 10.0, 10.0, true);
        let hits = overlapping(&world, a);
        assert_eq!(hits, vec![b]);
        assert!(overlapping(&world, c).is_empty());
    }

    #[test]
    fn no_collider_moves_freely() {
        let mut world = World::new();
        let id = world.spawn_named("Ghost", Transform::from_xy(0.0, 0.0));
        let hit = move_and_collide(&mut world, id, Vec2::new(5.0, -3.0));
        assert!(hit.hit.is_none());
        assert_eq!(hit.applied, Vec2::new(5.0, -3.0));
    }

    #[test]
    fn trigger_does_not_block_move_and_collide() {
        let mut world = World::new();
        let player = spawn_aabb(&mut world, "Player", 0.0, 0.0, 8.0, 8.0, true);
        let mut coin = Collider::aabb(8.0, 8.0);
        coin.solid = true; // even if solid, trigger skips blocking
        coin.trigger = true;
        let coin_id = world.spawn_named("Coin", Transform::from_xy(20.0, 0.0));
        world.set_collider(coin_id, Some(coin));
        let hit = move_and_collide(&mut world, player, Vec2::new(20.0, 0.0));
        assert!(hit.hit.is_none());
        assert!((world.transform(player).unwrap().translation.x - 20.0).abs() < 1e-4);
        assert!(overlaps(&world, player, coin_id));
    }

    #[test]
    fn triggers_entered_returns_collectible() {
        let mut world = World::new();
        let player = spawn_aabb(&mut world, "Player", 0.0, 0.0, 10.0, 10.0, true);
        let mut coin = Collider::aabb(10.0, 10.0);
        coin.solid = false;
        coin.trigger = true;
        let coin_id = world.spawn_named("Coin", Transform::from_xy(2.0, 0.0));
        world.set_collider(coin_id, Some(coin));
        let far = spawn_aabb(&mut world, "Far", 100.0, 0.0, 10.0, 10.0, true);
        let entered = triggers_entered(&world, player);
        assert_eq!(entered, vec![coin_id]);
        assert!(triggers_entered(&world, far).is_empty());
        // Coin also sees the player when coin is the query side.
        assert_eq!(triggers_entered(&world, coin_id), vec![player]);
    }

    #[test]
    fn filter_tag_filters_triggers_entered() {
        let mut world = World::new();
        let player = spawn_aabb(&mut world, "Player", 0.0, 0.0, 10.0, 10.0, true);
        world.set_tag(player, 1);
        let mut pellet = Collider::aabb(10.0, 10.0);
        pellet.trigger = true;
        pellet.filter_tag = 1;
        let pellet_id = world.spawn_named("Pellet", Transform::from_xy(2.0, 0.0));
        world.set_collider(pellet_id, Some(pellet));
        let mut ghost_only = Collider::aabb(10.0, 10.0);
        ghost_only.trigger = true;
        ghost_only.filter_tag = 2;
        let door = world.spawn_named("Door", Transform::from_xy(3.0, 0.0));
        world.set_collider(door, Some(ghost_only));
        let entered = triggers_entered(&world, player);
        assert_eq!(entered, vec![pellet_id], "filter 2 must reject player tag 1");
    }

    #[test]
    fn non_trigger_overlap_not_in_triggers_entered() {
        let mut world = World::new();
        let a = spawn_aabb(&mut world, "A", 0.0, 0.0, 10.0, 10.0, true);
        let b = spawn_aabb(&mut world, "B", 2.0, 0.0, 10.0, 10.0, false);
        assert!(overlaps(&world, a, b));
        assert!(triggers_entered(&world, a).is_empty());
        assert!(triggers_entered(&world, b).is_empty());
    }
}
