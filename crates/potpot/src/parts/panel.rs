//! The panel itself and the parts fixed to it.

use std::f32::consts::FRAC_PI_2;

use glam::{Vec2, Vec3};

use super::button::Button;
use super::grip::Grip;
use super::outline::{Outline, band};
use super::{CREASE, ROLL};
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

/// Returns a low plate with its edge cut off at an angle.
///
/// The plate is centered on the origin, `width` by `length` and `height` tall, with corners of
/// radius `corner` seen from above. Its top edge is cut `chamfer` wide and as deep, kept to what
/// the height and the shorter side have room for, in one flat face that catches the light.
pub fn chamfered_slab(width: f32, length: f32, height: f32, chamfer: f32, corner: f32) -> Mesh {
  let chamfer = chamfer.min(height).min(width.min(length) / 2.0);
  let outline = Outline::new(width, length, corner);
  let top = outline.grown(-chamfer);
  let rings = [(outline, 0.0), (outline, height - chamfer), (top, height)];
  // the top closes to its middle, along the same angles as the outline's steps
  let point = |ring: usize, around: f32| {
    let angle = outline.spread(around);
    let Some(&(shape, height)) = rings.get(ring) else {
      return Vec3::Z * height;
    };
    (Vec2::from_angle(angle) * shape.at(angle)).extend(height)
  };
  Mesh::rings(rings.len() + 1, 144, CREASE, point)
}

/// Returns a raised rim around a field of the panel, cut down toward the field.
///
/// The rim is centered on the origin, `width` by `length` on the outside with corners of radius
/// `corner`, and `height` tall. Its border is `border` wide, kept to what the shorter side has
/// room for. Its inner edge slopes down to the panel as wide as the rim is tall, so the field
/// inside reads as set into the panel. Its outer edge has a small cut.
pub fn inset(width: f32, length: f32, border: f32, height: f32, corner: f32) -> Mesh {
  let border = border.min(width.min(length) / 2.0);
  // the slope takes at most most of the border, which leaves a flat on top
  let slope = height.min(border * 0.7);
  band(
    Outline::new(width, length, corner),
    Outline::new(width - border * 2.0, length - border * 2.0, corner - border),
    height,
    [(height * 0.25).min(border - slope), slope],
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

/// The height of a socket cap screw's head, as a share of its radius.
const CAP_HEIGHT: f32 = 0.7;
/// The distance from the middle of a cap screw's socket to its corners, as a share of the radius.
const SOCKET: f32 = 0.5;
/// The depth of a cap screw's socket, as a share of the radius.
const SOCKET_DEPTH: f32 = 0.35;
/// The count of sides around a cap screw, a multiple of six so that the socket's corners are sharp.
const CAP_SIDES: u32 = 36;

/// Returns a point of a ring around a cap screw's head.
///
/// The ring is `out` from the middle and `height` high, round if `hex` is false and otherwise the
/// socket's hexagon, which `out` reaches at its corners.
fn cap_ring(out: f32, height: f32, hex: bool, angle: f32) -> Vec3 {
  let shape = if hex {
    Grip::Faceted(6).at(angle, 0.0)
  } else {
    1.0
  };
  (Vec2::from_angle(angle) * out * shape).extend(height)
}

/// Returns the head of a socket cap screw, of radius `radius`.
///
/// The head is a short cylinder with a rolled top edge, and a hexagonal socket opens in its
/// top. The socket's walls and floor are [`hex_socket`], to be drawn darker.
pub fn cap_screw(radius: f32) -> Mesh {
  let height = radius * CAP_HEIGHT;
  let roll = radius * 0.15;
  let mut rings = vec![(radius, 0.0, false), (radius, height - roll, false)];
  rings.extend((1..=ROLL).map(|step| {
    let (up, out) = (step as f32 / ROLL as f32 * FRAC_PI_2).sin_cos();
    (radius - roll + roll * out, height - roll + roll * up, false)
  }));
  rings.push((radius * SOCKET, height, true));
  Mesh::rings(rings.len(), CAP_SIDES, CREASE, |ring, angle| {
    let (out, height, hex) = rings[ring];
    cap_ring(out, height, hex, angle)
  })
}

/// Returns the hexagonal socket in the head of a [`cap_screw`] of radius `radius`.
///
/// The socket is its six walls and its floor, which meet the head's top at its rim.
pub fn hex_socket(radius: f32) -> Mesh {
  let (top, floor) = (radius * CAP_HEIGHT, radius * (CAP_HEIGHT - SOCKET_DEPTH));
  let rings = [
    (radius * SOCKET, top),
    (radius * SOCKET, floor),
    (0.0, floor),
  ];
  Mesh::rings(rings.len(), CAP_SIDES, CREASE, |ring, angle| {
    let (out, height) = rings[ring];
    cap_ring(out, height, true, angle)
  })
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

  #[test]
  fn a_chamfered_slab_has_a_flat_top_and_one_cut_face_all_around() {
    let (width, length, chamfer) = (120.0, 60.0, 3.0);
    let slab = chamfered_slab(width, length, 5.0, chamfer, 8.0);
    let half = Vec2::new(width, length) / 2.0;
    for vertex in &slab.vertices {
      let (position, normal) = (Vec3::from(vertex.position), Vec3::from(vertex.normal));
      assert!(normal.is_normalized(), "{position} faces {normal}");
      let out = position.truncate().abs();
      assert!(
        out.x <= half.x + 1e-3 && out.y <= half.y + 1e-3,
        "{position}"
      );
      assert!((0.0..=5.0 + 1e-4).contains(&position.z));
      // the top is the outline drawn in by the chamfer, and the side stands upright below it
      if position.z > 5.0 - 1e-4 && normal.z > 0.99 {
        assert!(out.x <= half.x - chamfer + 1e-3 && out.y <= half.y - chamfer + 1e-3);
      }
    }
    // the cut faces up and out at half a right angle in the middle of each long side
    let middle = Vec3::new(0.0, half.y - chamfer / 2.0, 5.0 - chamfer / 2.0);
    let cut = (slab.vertices.iter()).filter(|vertex| {
      let normal = Vec3::from(vertex.normal);
      normal.y > 0.5 && normal.z > 0.5
    });
    let distance = |vertex: &&crate::Vertex| Vec3::from(vertex.position).distance(middle);
    let nearest = cut.min_by(|a, b| distance(a).total_cmp(&distance(b)));
    let nearest = nearest.expect("the long side has a cut face");
    let (position, normal) = (Vec3::from(nearest.position), Vec3::from(nearest.normal));
    assert!(
      (position.y - middle.y).abs() <= chamfer / 2.0 + 1e-3,
      "{position}"
    );
    let slanted = Vec3::new(0.0, 1.0, 1.0).normalize();
    assert!((normal - slanted).length() < 0.05, "{normal}");
  }

  #[test]
  fn an_inset_slopes_down_into_an_open_field() {
    let (width, length, border, height) = (300.0, 160.0, 12.0, 4.0);
    let rim = inset(width, length, border, height, 10.0);
    let field = Vec2::new(width, length) / 2.0 - Vec2::splat(border);
    let mut sloped = 0;
    for vertex in &rim.vertices {
      let (position, normal) = (Vec3::from(vertex.position), Vec3::from(vertex.normal));
      let out = position.truncate().abs();
      assert!(
        out.x > field.x - 1e-3 || out.y > field.y - 1e-3,
        "the field is open at {position}"
      );
      assert!(out.x <= width / 2.0 + 1e-3 && out.y <= length / 2.0 + 1e-3);
      // along a long side, a slanted face on the inner half of the border is the slope down
      // to the field, which faces in toward the field
      let slanted = normal.z > 0.6 && normal.y.abs() > 0.6;
      if slanted && out.x < field.x && out.y < field.y + border / 2.0 {
        sloped += 1;
        assert!(normal.y * position.y < 0.0, "{position} faces {normal}");
        assert!(out.y <= field.y + height + 1e-3, "{position}");
      }
      // and every cut, in or out, is as wide as it is deep along the side, short of where
      // the shading rounds into a corner
      if slanted && out.x < field.x - border * 2.0 {
        let across = Vec3::new(0.0, normal.y.signum(), 1.0).normalize();
        assert!(
          (normal - across).length() < 0.02,
          "{position} faces {normal}"
        );
      }
    }
    assert!(sloped > 0, "no vertex lies on the slope");
  }

  #[test]
  fn a_cap_screw_is_a_round_head_around_a_hexagonal_socket() {
    let (head, socket) = (cap_screw(10.0), hex_socket(10.0));
    let top = 10.0 * CAP_HEIGHT;
    for vertex in &head.vertices {
      let position = Vec3::from(vertex.position);
      assert!(position.truncate().length() <= 10.0 + 1e-4, "{position}");
      assert!((0.0..=top + 1e-4).contains(&position.z), "{position}");
    }
    // the socket's walls reach its corners every sixth of a turn and its flats between them
    let corners = 10.0 * SOCKET;
    let flats = corners * (std::f32::consts::PI / 6.0).cos();
    let reaches: Vec<f32> = (socket.vertices.iter())
      .map(|vertex| Vec3::from(vertex.position))
      .filter(|position| (position.z - top).abs() < 1e-4)
      .map(|position| position.truncate().length())
      .collect();
    let most = reaches.iter().copied().fold(0.0, f32::max);
    let least = reaches.iter().copied().fold(f32::MAX, f32::min);
    assert!(
      (most - corners).abs() < 1e-4 && (least - flats).abs() < 1e-4,
      "{least} to {most}"
    );
    let corner = Vec2::from_angle(std::f32::consts::FRAC_PI_3) * corners;
    let at_corner = |mesh: &Mesh| {
      (mesh.vertices.iter())
        .any(|vertex| Vec3::from(vertex.position).distance(corner.extend(top)) < 1e-4)
    };
    // and the head's top meets the socket at the same rim
    assert!(at_corner(&head) && at_corner(&socket));
    // the walls face in, toward the socket's middle, and the floor up
    for vertex in &socket.vertices {
      let (position, normal) = (Vec3::from(vertex.position), Vec3::from(vertex.normal));
      assert!(
        normal.z > 0.99 || normal.truncate().dot(position.truncate()) < 0.0,
        "{position} faces {normal}"
      );
    }
  }
}
