//! Re-export core World → DrawList so host / editor / games keep compiling unchanged.

pub use wiimaker_core::{render_world, render_world_ex};

#[cfg(test)]
mod tests {
    use wiimaker_core::draw::DrawCmd;
    use wiimaker_core::draw::DrawList;
    use wiimaker_core::Rgba8;

    use super::render_world;
    use crate::hydrate::{hydrate_lenient, TextureMap};
    use crate::mutate::{add_entity, set_entity_rotation_z, MutateOpts};
    use crate::scene::Scene;

    fn disc_centers(cmds: &[DrawCmd]) -> Vec<(f32, f32)> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::DrawDisc { center, .. } => Some((center.x, center.y)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn parented_disc_draws_at_composed_world_pose() {
        let mut scene = Scene::new("orbit");
        add_entity(
            &mut scene,
            "RotParent",
            &MutateOpts {
                x: Some(320.0),
                y: Some(240.0),
                radius: Some(28.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "RotParent", 45f32.to_radians()).unwrap();
        add_entity(
            &mut scene,
            "RotChild",
            &MutateOpts {
                x: Some(400.0),
                y: Some(240.0),
                radius: Some(16.0),
                ..Default::default()
            },
        )
        .unwrap();
        let child = scene
            .entities
            .iter_mut()
            .find(|e| e.name == "RotChild")
            .unwrap();
        child.transform.translation = [80.0, 0.0, 0.0];
        child.parent = Some("RotParent".into());

        let world = hydrate_lenient(&scene, &TextureMap::new());
        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        let centers = disc_centers(draw.cmds());
        assert_eq!(centers.len(), 2);
        let s = 45f32.to_radians().sin();
        let c = 45f32.to_radians().cos();
        let want_child = (320.0 + 80.0 * c, 240.0 + 80.0 * s);
        let child_center = centers
            .iter()
            .find(|(x, y)| (*x - 320.0).abs() > 1.0 || (*y - 240.0).abs() > 1.0)
            .copied()
            .expect("child disc");
        assert!((child_center.0 - want_child.0).abs() < 1e-3);
        assert!((child_center.1 - want_child.1).abs() < 1e-3);
        assert!(centers
            .iter()
            .any(|(x, y)| (*x - 320.0).abs() < 1e-3 && (*y - 240.0).abs() < 1e-3));
    }
}
