//! shapes of common panel hardware, sized in panel points and centered on the z axis

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use crate::mesh::Mesh;

/// a knob's body: a skirt, a waist and a chamfered cap, `radius` wide at the skirt and
/// `height` tall. `sides` is how round it is, and few of them with `smooth` off gives a
/// faceted grip
pub fn knob_body(radius: f32, height: f32, sides: u32, smooth: bool) -> Mesh {
  let cap = radius * 0.8;
  Mesh::lathe(
    &[
      [radius, 0.0],
      [radius, height * 0.18],
      [cap, height * 0.3],
      [cap, height * 0.9],
      [cap - height * 0.1, height],
      [0.0, height],
    ],
    sides,
    smooth,
  )
}

/// the line on a knob's cap that shows where it points, lying just above a body of the same
/// `radius` and `height`. it points up the panel, toward negative y, before the knob turns
pub fn knob_pointer(radius: f32, height: f32) -> Mesh {
  let (half_width, raised) = (radius * 0.07, height * 0.03);
  Mesh::cuboid(
    Vec3::new(-half_width, -radius * 0.68, height),
    Vec3::new(half_width, -radius * 0.2, height + raised),
  )
}

/// where a knob sits and how far it has turned. `turn` runs from zero to one over `sweep`
/// radians, centered on pointing up the panel
pub fn knob_transform(x: f32, y: f32, turn: f32, sweep: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z((turn - 0.5) * sweep)
}

/// the sweep of most hardware knobs, three quarters of a turn
pub const KNOB_SWEEP: f32 = 1.5 * PI;

/// a flat plate with its top at height zero, the panel everything stands on
pub fn plate(width: f32, height: f32, thickness: f32) -> Mesh {
  Mesh::cuboid(
    Vec3::new(0.0, 0.0, -thickness),
    Vec3::new(width, height, 0.0),
  )
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
