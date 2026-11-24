//! which object of a scene is seen at a point of its picture

use glam::{Vec2, Vec3};

use crate::scene::Scene;

impl Scene {
  /// the object seen at a point of the picture, as its place in [`Scene::objects`]. the point
  /// is in the panel's points from its top left, where the picture is stretched over the
  /// panel. it is found by the line of sight through the point, so an object's sides and
  /// whatever the lean moves count as they are drawn, and of everything on the line the one
  /// nearest the viewer is seen. nothing outside the scene's heights or off the picture is
  /// drawn, so nothing there is found
  pub fn pick(&self, at: Vec2) -> Option<usize> {
    let across = at / self.size * 2.0 - 1.0;
    if across.abs().max_element() > 1.0 {
      return None;
    }
    let from_clip = self.view().inverse();
    // the line of sight from the highest the scene reaches to the lowest
    let near = from_clip.transform_point3(Vec3::new(across.x, -across.y, 0.0));
    let far = from_clip.transform_point3(Vec3::new(across.x, -across.y, 1.0));

    let mut seen: Option<(f32, usize)> = None;
    for (index, object) in self.objects.iter().enumerate() {
      // the same line in the mesh's own space, where a point is as far along it as in the
      // panel's
      let to_mesh = object.transform.inverse();
      let from = to_mesh.transform_point3(near);
      let along = to_mesh.transform_vector3(far - near);
      let corner = |index: u32| Vec3::from(object.mesh.vertices[index as usize].position);
      for triangle in object.mesh.indices.as_chunks::<3>().0 {
        if let Some(distance) = crossing(from, along, triangle.map(corner))
          && seen.is_none_or(|(nearest, _)| distance < nearest)
        {
          seen = Some((distance, index));
        }
      }
    }
    seen.map(|(_, index)| index)
  }
}

/// how far past its edges a triangle counts as crossed, as a part of its size. a line of
/// sight along the edge two triangles share comes out a rounding error outside one or the
/// other, and with none of this outside both
const EDGE: f32 = 1e-4;

/// how far along a line it goes through a triangle, from zero at `from` to one at
/// `from + along`, if it does between the two
fn crossing(from: Vec3, along: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
  let (ab, ac) = (b - a, c - a);
  let across = along.cross(ac);
  let facing = ab.dot(across);
  // a triangle seen edge on, or one with no area
  if facing.abs() < f32::EPSILON {
    return None;
  }
  let offset = from - a;
  let u = offset.dot(across) / facing;
  let around = offset.cross(ab);
  let v = along.dot(around) / facing;
  if u < -EDGE || v < -EDGE || u + v > 1.0 + EDGE {
    return None;
  }
  let distance = ac.dot(around) / facing;
  (0.0..=1.0).contains(&distance).then_some(distance)
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use glam::Mat4;

  use super::*;
  use crate::material::Material;
  use crate::mesh::Mesh;
  use crate::scene::{Light, Look, Object};

  /// a panel of 400 by 200 with a plate over all of it, and on the plate whatever boxes are
  /// asked for, each between two corners
  fn scene(lean: Vec2, boxes: &[(Vec3, Vec3)]) -> Scene {
    let plate = (Vec3::new(0.0, 0.0, -4.0), Vec3::new(400.0, 200.0, 0.0));
    let objects = std::iter::once(&plate)
      .chain(boxes)
      .map(|&(min, max)| Object {
        mesh: Arc::new(Mesh::cuboid(min, max)),
        transform: Mat4::IDENTITY,
        material: Material::matte(Vec3::ONE),
      })
      .collect();
    Scene {
      size: Vec2::new(400.0, 200.0),
      heights: [-10.0, 40.0],
      lean,
      light: Light::default(),
      look: Look::default(),
      objects,
      lamps: Vec::new(),
    }
  }

  #[test]
  fn straight_down_a_box_is_picked_over_its_footprint_only() {
    let boxed = (Vec3::new(100.0, 50.0, 0.0), Vec3::new(140.0, 90.0, 20.0));
    let scene = scene(Vec2::ZERO, &[boxed]);
    assert_eq!(scene.pick(Vec2::new(120.0, 70.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(101.0, 51.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(99.0, 70.0)), Some(0));
    assert_eq!(scene.pick(Vec2::new(120.0, 91.0)), Some(0));
  }

  #[test]
  fn a_leaning_view_picks_a_box_where_it_is_drawn() {
    // each point slides half a point up the picture for each of its height, so the top of
    // a box 20 tall is drawn 10 higher than its foot and its front shows under that
    let boxed = (Vec3::new(100.0, 50.0, 0.0), Vec3::new(140.0, 90.0, 20.0));
    let scene = scene(Vec2::new(0.0, -0.5), &[boxed]);
    // the top is drawn over 40 to 80 and the front from there down to 90
    assert_eq!(scene.pick(Vec2::new(120.0, 41.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(120.0, 39.0)), Some(0));
    assert_eq!(scene.pick(Vec2::new(120.0, 85.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(120.0, 89.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(120.0, 91.0)), Some(0));
    // the lean is up the picture only, so the sides stay where they stand
    assert_eq!(scene.pick(Vec2::new(99.0, 60.0)), Some(0));
    assert_eq!(scene.pick(Vec2::new(141.0, 60.0)), Some(0));
  }

  #[test]
  fn the_picked_point_is_where_the_view_draws_it() {
    // a small box high up, found at the place the view's own matrix puts its top
    let boxed = (Vec3::new(300.0, 120.0, 30.0), Vec3::new(304.0, 124.0, 32.0));
    let scene = scene(Vec2::new(0.3, -0.4), &[boxed]);
    let drawn = scene.view().project_point3(Vec3::new(302.0, 122.0, 32.0));
    let at = (Vec2::new(drawn.x, -drawn.y) + 1.0) / 2.0 * scene.size;
    assert_eq!(scene.pick(at), Some(1));
    // and not where it stands on the panel, which the lean has moved it off
    assert_eq!(scene.pick(Vec2::new(302.0, 122.0)), Some(0));
  }

  #[test]
  fn of_things_over_each_other_the_higher_is_picked() {
    let low = (Vec3::new(100.0, 50.0, 0.0), Vec3::new(200.0, 150.0, 5.0));
    let high = (Vec3::new(150.0, 100.0, 20.0), Vec3::new(250.0, 180.0, 22.0));
    // whichever comes first in the scene
    for (boxes, [low_at, high_at]) in [([low, high], [1, 2]), ([high, low], [2, 1])] {
      let scene = scene(Vec2::ZERO, &boxes);
      assert_eq!(scene.pick(Vec2::new(120.0, 70.0)), Some(low_at));
      assert_eq!(scene.pick(Vec2::new(175.0, 125.0)), Some(high_at));
      assert_eq!(scene.pick(Vec2::new(240.0, 170.0)), Some(high_at));
    }
    // leaning, the higher one hides more of the lower one toward the top of the picture
    let leaning = scene(Vec2::new(0.0, -0.5), &[low, high]);
    assert_eq!(leaning.pick(Vec2::new(175.0, 92.0)), Some(2));
    assert_eq!(leaning.pick(Vec2::new(175.0, 88.0)), Some(1));
  }

  #[test]
  fn a_point_off_the_panel_picks_nothing() {
    // leaning, the plate's front edge would show just under the picture if it went on
    let scene = scene(Vec2::new(0.0, -0.5), &[]);
    assert_eq!(scene.pick(Vec2::new(200.0, 100.0)), Some(0));
    for at in [
      Vec2::new(-1.0, 100.0),
      Vec2::new(401.0, 100.0),
      Vec2::new(200.0, -1.0),
      Vec2::new(200.0, 201.0),
      Vec2::new(-500.0, -500.0),
    ] {
      assert_eq!(scene.pick(at), None, "{at}");
    }
  }

  #[test]
  fn an_object_is_picked_where_its_transform_puts_it() {
    // a bar along x, turned a quarter and moved, so that it lies along y
    let mut scene = scene(Vec2::ZERO, &[]);
    scene.objects.push(Object {
      mesh: Arc::new(Mesh::cuboid(
        Vec3::new(-30.0, -2.0, 0.0),
        Vec3::new(30.0, 2.0, 3.0),
      )),
      transform: Mat4::from_translation(Vec3::new(200.0, 100.0, 0.0))
        * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2),
      material: Material::matte(Vec3::ONE),
    });
    assert_eq!(scene.pick(Vec2::new(200.0, 75.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(200.0, 128.0)), Some(1));
    assert_eq!(scene.pick(Vec2::new(225.0, 100.0)), Some(0));
  }

  #[test]
  fn a_line_of_sight_along_an_edge_or_through_a_corner_does_not_slip_between_triangles() {
    // a slab's faces meet along its middle, and a cone's at its tip. both stand where their
    // own middles come out at numbers that do not round evenly
    let mut scene = scene(Vec2::new(0.0, -0.3), &[]);
    let at = Vec3::new(282.0, 100.05, 0.0);
    scene.objects.push(Object {
      mesh: Arc::new(crate::parts::slab(92.0, 38.0, 3.5, 3.0)),
      transform: Mat4::from_translation(at),
      material: Material::matte(Vec3::ONE),
    });
    scene.objects.push(Object {
      mesh: Arc::new(Mesh::lathe(&[[10.0, 0.0], [0.0, 5.0]], 24, true)),
      transform: Mat4::from_translation(Vec3::new(96.3, 94.7, 0.0)),
      material: Material::matte(Vec3::ONE),
    });
    for tenth in -170..=170 {
      let down = tenth as f32 / 10.0;
      assert_eq!(scene.pick(Vec2::new(282.0, 99.0 + down)), Some(1), "{down}");
    }
    for tenth in -400..=400 {
      let across = tenth as f32 / 10.0;
      assert_eq!(
        scene.pick(Vec2::new(282.0 + across, 99.0)),
        Some(1),
        "{across}"
      );
    }
    // the tip is drawn five times the lean up the picture from where the cone stands
    assert_eq!(scene.pick(Vec2::new(96.3, 94.7 - 1.5)), Some(2));
    for tenth in -80..=80 {
      let across = tenth as f32 / 10.0;
      assert_eq!(
        scene.pick(Vec2::new(96.3 + across, 94.7)),
        Some(2),
        "{across}"
      );
      assert_eq!(
        scene.pick(Vec2::new(96.3, 94.7 + across)),
        Some(2),
        "{across}"
      );
    }
  }

  #[test]
  fn nothing_outside_the_heights_is_picked() {
    // the view draws nothing above its highest, so a box up there is not seen
    let above = (Vec3::new(100.0, 50.0, 50.0), Vec3::new(140.0, 90.0, 60.0));
    let scene = scene(Vec2::ZERO, &[above]);
    assert_eq!(scene.pick(Vec2::new(120.0, 70.0)), Some(0));
  }
}
