//! The panel itself and the parts fixed to it.

use std::f32::consts::FRAC_PI_2;

use glam::{Vec2, Vec3};

use super::CREASE;
use super::button::Button;
use super::outline::{Outline, band};
use crate::mesh::Mesh;

/// Returns a flat plate with its top at height zero.
///
/// The plate is the panel everything stands on. It reaches from the origin to (`width`, `height`).
pub fn plate(width: f32, height: f32, thickness: f32) -> Mesh {
  Mesh::cuboid(
    Vec3::new(0.0, 0.0, -thickness),
    Vec3::new(width, height, 0.0),
  )
}

/// Returns a low raised plate with round corners and a soft edge.
///
/// The plate is centered on the origin, for a section of a panel standing a little proud of the
/// rest.
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

/// Returns a raised frame around a rectangle.
///
/// The frame is centered on the origin, like the bezel of a screen or the rim of a panel. It is
/// `width` by `length` on the outside and `height` tall. Its border is `border` wide, kept to what
/// the shorter side has room for, and its outside corners have radius `corner`.
pub fn frame(width: f32, length: f32, border: f32, height: f32, corner: f32) -> Mesh {
  let border = border.min(width.min(length) / 2.0);
  band(
    Outline::new(width, length, corner),
    Outline::new(width - border * 2.0, length - border * 2.0, corner - border),
    height,
    [height.min(border) * 0.3; 2],
  )
}

/// Returns a polished ring for a knob to stand in.
///
/// The ring lies on the panel around the origin, from `inner` to `outer` away from the middle. It
/// is `height` tall at its lip, which is rolled over on the outside. From the lip its face slopes
/// down toward the middle like a shallow funnel, with a hard edge between the two that catches the
/// light.
pub fn bezel(inner: f32, outer: f32, height: f32) -> Mesh {
  /// The count of steps in the lip's roll.
  ///
  /// Each step is few enough degrees to shade as one curve.
  const LIP: usize = 9;
  let roll = height.min((outer - inner) * 0.3);
  let mut profile = vec![Vec2::new(outer, 0.0)];
  profile.extend((0..=LIP).map(|step| {
    let (up, out) = (step as f32 / LIP as f32 * FRAC_PI_2).sin_cos();
    Vec2::new(outer - roll + roll * out, height - roll + roll * up)
  }));
  profile.push(Vec2::new(inner, height * 0.4));
  profile.push(Vec2::new(inner, 0.0));
  // a small ring needs fewer sides to look round. only the roll's own steps are shallow
  // enough to round over
  let sides = (outer * 3.0).clamp(24.0, 96.0) as u32;
  Mesh::rings(profile.len(), sides, 0.97, |ring, angle| {
    (Vec2::from_angle(angle) * profile[ring].x).extend(profile[ring].y)
  })
}

/// Returns the round head of a screw, a low dome of radius `radius`.
///
/// Nothing is cut into it.
pub fn screw_head(radius: f32) -> Mesh {
  /// The count of rings from the dome's rim to its middle.
  const DOME: usize = 6;
  let rim = Vec2::new(radius, 0.0);
  let dome = (0..=DOME).map(|ring| {
    let (up, out) = (ring as f32 / DOME as f32 * FRAC_PI_2).sin_cos();
    Vec2::new(radius * out, radius * (0.12 + 0.33 * up))
  });
  let profile: Vec<Vec2> = std::iter::once(rim).chain(dome).collect();
  Mesh::rings(profile.len(), 24, CREASE, |ring, angle| {
    (Vec2::from_angle(angle) * profile[ring].x).extend(profile[ring].y)
  })
}

/// Returns a round-headed screw with a slot, of radius `radius`.
pub fn screw(radius: f32) -> Mesh {
  // the slot is a bar standing just proud of the dome, cheaper than cutting into it
  screw_head(radius).merged(&Mesh::cuboid(
    Vec3::new(-radius * 0.75, -radius * 0.12, 0.0),
    Vec3::new(radius * 0.75, radius * 0.12, radius * 0.47),
  ))
}

/// Returns the cross cut into the head of a screw of radius `radius`.
///
/// The cross is two bars standing just proud of [`screw_head`], to be drawn dark.
pub fn screw_cross(radius: f32) -> Mesh {
  let bar = |long: f32, wide: f32| {
    Mesh::cuboid(
      Vec3::new(-radius * long, -radius * wide, 0.0),
      Vec3::new(radius * long, radius * wide, radius * 0.47),
    )
  };
  bar(0.58, 0.1).merged(&bar(0.1, 0.58))
}

/// Returns a slot cut through the panel, as a dark strip.
///
/// The strip lies on the panel, `width` by `length` with round ends. It is an air vent, or the long
/// hole a rack screw goes through.
pub fn slot(width: f32, length: f32) -> Mesh {
  Outline::new(width, length, width.min(length) / 2.0).strip()
}

/// Returns the dark gap around a screw, lying on the panel.
///
/// The gap is `gap` wider all around than the head of a screw of radius `radius`.
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
      assert!(!inside, "the frame is open in its middle at {position}");
      assert!(position.x.abs() <= 100.01 && position.y.abs() <= 50.01);
      assert!((0.0..=6.001).contains(&position.z));
    }
  }

  #[test]
  fn a_frame_with_too_wide_a_border_stays_within_its_size() {
    let frame = frame(20.0, 10.0, 6.0, 3.0, 2.0);
    for vertex in &frame.vertices {
      let position = Vec3::from(vertex.position);
      assert!(
        position.x.abs() <= 10.01 && position.y.abs() <= 5.01,
        "the frame reaches {position}"
      );
      assert!((0.0..=3.001).contains(&position.z));
    }
  }
}
