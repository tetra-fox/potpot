//! the panel itself and what is fixed to it

use std::f32::consts::FRAC_PI_2;

use glam::{Vec2, Vec3};

use super::button::Button;
use super::outline::{Outline, band};
use crate::mesh::Mesh;

/// a flat plate with its top at height zero, the panel everything stands on
pub fn plate(width: f32, height: f32, thickness: f32) -> Mesh {
  Mesh::cuboid(
    Vec3::new(0.0, 0.0, -thickness),
    Vec3::new(width, height, 0.0),
  )
}

/// a low raised plate with round corners and a soft edge, centered on the origin, for a
/// section of a panel standing a little proud of the rest
pub fn slab(width: f32, length: f32, height: f32, corner: f32) -> Mesh {
  Button {
    width,
    length,
    height,
    corner: (corner / (width.min(length) / 2.0)).clamp(0.0, 1.0),
    roll: 1.0,
    crown: 0.0,
  }
  .body()
}

/// a raised frame around a rectangle centered on the origin, like the bezel of a screen or
/// the rim of a panel: `width` by `length` on the outside, `border` wide, `height` tall,
/// with corners of radius `corner` on the outside
pub fn frame(width: f32, length: f32, border: f32, height: f32, corner: f32) -> Mesh {
  band(
    Outline::new(width, length, corner),
    Outline::new(width - border * 2.0, length - border * 2.0, corner - border),
    height,
    height.min(border) * 0.3,
  )
}

/// a polished ring lying on the panel around the origin, for a knob to stand in: from
/// `inner` to `outer` away from the middle and `height` tall at its lip, which is rolled
/// over on the outside. from the lip its face slopes down toward the middle like a
/// shallow funnel, with a hard edge between the two that catches the light
pub fn bezel(inner: f32, outer: f32, height: f32) -> Mesh {
  /// the steps of the lip's roll, few enough degrees each to shade as one curve
  const LIP: usize = 9;
  let roll = height.min((outer - inner) * 0.3);
  let mut profile = vec![Vec2::new(outer, 0.0)];
  profile.extend((0..=LIP).map(|step| {
    let (up, out) = (step as f32 / LIP as f32 * FRAC_PI_2).sin_cos();
    Vec2::new(outer - roll + roll * out, height - roll + roll * up)
  }));
  profile.push(Vec2::new(inner, height * 0.4));
  profile.push(Vec2::new(inner, 0.0));
  // only the roll's own steps are shallow enough to round over
  Mesh::rings(profile.len(), 96, 0.97, |ring, angle| {
    (Vec2::from_angle(angle) * profile[ring].x).extend(profile[ring].y)
  })
}

/// a round-headed screw with a slot, `radius` wide
pub fn screw(radius: f32) -> Mesh {
  let head = Mesh::lathe(
    &[
      [radius, 0.0],
      [radius * 0.85, radius * 0.35],
      [0.0, radius * 0.45],
    ],
    16,
    true,
  );
  // the slot is a dark bar standing just proud of the dome, cheaper than cutting into it
  head.merged(&Mesh::cuboid(
    Vec3::new(-radius * 0.75, -radius * 0.12, 0.0),
    Vec3::new(radius * 0.75, radius * 0.12, radius * 0.47),
  ))
}

/// the dark gap around a screw of a radius, `gap` wider than its head all around, lying on
/// the panel
pub fn screw_seat(radius: f32, gap: f32) -> Mesh {
  Outline::new(radius * 2.0, radius * 2.0, radius)
    .grown(gap)
    .strip()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_frame_is_a_rim_with_nothing_in_its_middle() {
    let frame = frame(200.0, 100.0, 10.0, 6.0, 4.0);
    for vertex in &frame.vertices {
      let position = Vec3::from(vertex.position);
      let inside = position.x.abs() < 89.9 && position.y.abs() < 39.9;
      assert!(!inside, "the frame fills its middle at {position}");
      assert!(position.x.abs() <= 100.01 && position.y.abs() <= 50.01);
      assert!((0.0..=6.001).contains(&position.z));
    }
  }
}
