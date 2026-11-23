//! shapes of common panel hardware, sized in panel points and centered on the z axis

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Mat4, Vec2, Vec3};

use crate::mesh::Mesh;

/// the sweep of most hardware knobs, three quarters of a turn
pub const KNOB_SWEEP: f32 = 1.5 * PI;

/// faces that turn from each other by less than about 35 degrees shade as one curve
const CREASE: f32 = 0.82;
/// the rings a shaped top is cut into from its rim to its middle
const TOP_RINGS: usize = 14;
/// how far up a knob its top's chamfer begins, as a share of its height
const SHOULDER: f32 = 0.88;

/// a knob, described by its parts. the named ones, like [`Knob::fluted`], are starting
/// points whose fields can be set to anything else.
///
/// every knob points up the panel, toward negative y, before it turns
#[derive(Clone, Copy, Debug)]
pub struct Knob {
  /// the widest the knob gets, at its skirt if it has one
  pub radius: f32,
  pub height: f32,
  /// how much narrower the grip is than a skirt under it, as a share of the radius. zero is
  /// a knob with straight sides and no skirt
  pub skirt: f32,
  pub grip: Grip,
  pub top: Top,
}

/// the side of a knob, where fingers hold it
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grip {
  /// round and plain
  Smooth,
  /// this many flat sides
  Faceted(u32),
  /// this many round-bottomed scoops down the side, each `depth` of the radius deep
  Fluted { flutes: u32, depth: f32 },
  /// this many fine sharp ridges
  Knurled(u32),
  /// drawn out to a point on the side it points at, `reach` of the radius further than the
  /// rest, like a chicken head knob
  Pointed { reach: f32 },
  /// a handle standing across the whole knob like a stove dial's, `width` of the radius
  /// wide, with round ends
  Bar { width: f32 },
  /// a round body with one long handle out to the side it points at, for turning with a
  /// foot. the body is `body` of the radius and the handle `width` of it wide
  Lever { width: f32, body: f32 },
}

/// how far out a handle reaches at `angle` from the way it runs: a strip `width` wide with
/// round ends, from the center out to one
fn handle_at(angle: f32, width: f32) -> f32 {
  let half = (width / 2.0).min(1.0);
  let (across, along) = (angle.sin().abs(), angle.cos());
  // the middle of the round end
  let end = 1.0 - half;
  if along <= 0.0 {
    return 0.0;
  }
  let side = if across > f32::EPSILON {
    half / across
  } else {
    f32::MAX
  };
  if side * along <= end {
    side
  } else {
    end * along + (half * half - end * end * across * across).max(0.0).sqrt()
  }
}

/// the face of a knob
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Top {
  Flat,
  /// hollowed like a shallow bowl, `depth` of the height deep at the middle
  Dished {
    depth: f32,
  },
  /// raised to a round crown, `rise` of the height higher at the middle
  Domed {
    rise: f32,
  },
  /// flat but for one fingertip-sized divot near the rim on the side the knob points at,
  /// which is the only marker such a knob needs
  Dimpled,
}

impl Knob {
  /// a plain round knob on a skirt
  pub fn round(radius: f32, height: f32) -> Self {
    Self {
      radius,
      height,
      skirt: 0.2,
      grip: Grip::Smooth,
      top: Top::Flat,
    }
  }

  /// scooped sides, straight down to the panel
  pub fn fluted(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Fluted {
        flutes: 7,
        depth: 0.14,
      },
      ..Self::round(radius, height)
    }
  }

  /// scooped sides over a wide skirt, the kind with the scale printed on the skirt
  pub fn skirted(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.32,
      grip: Grip::Fluted {
        flutes: 9,
        depth: 0.1,
      },
      ..Self::round(radius, height)
    }
  }

  /// fine ridges all around and a domed top
  pub fn knurled(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Knurled(36),
      top: Top::Domed { rise: 0.08 },
      ..Self::round(radius, height)
    }
  }

  /// a soft-looking knob with a bowl for a top
  pub fn dished(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      top: Top::Dished { depth: 0.22 },
      ..Self::round(radius, height)
    }
  }

  /// a low wide knob turned by a fingertip in its divot
  pub fn dimpled(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      top: Top::Dimpled,
      ..Self::round(radius, height)
    }
  }

  /// a few flat sides
  pub fn faceted(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Faceted(9),
      ..Self::round(radius, height)
    }
  }

  /// drawn out to a point, on a skirt
  pub fn chicken_head(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Pointed { reach: 0.75 },
      ..Self::round(radius, height)
    }
  }

  /// a tall handle across a low round base, the kind a toe can nudge
  pub fn bar(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.12,
      grip: Grip::Bar { width: 0.42 },
      ..Self::round(radius, height)
    }
  }

  /// a small round knob with a long handle sticking out of it
  pub fn lever(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Lever {
        width: 0.3,
        body: 0.5,
      },
      ..Self::round(radius, height)
    }
  }

  /// the radius of the grip's round parts
  fn grip_radius(&self) -> f32 {
    match self.grip {
      // the point reaches the full radius and the rest stays inside it
      Grip::Pointed { reach } => self.radius / (1.0 + reach),
      // the handle's end reaches the full radius
      Grip::Lever { .. } => self.radius,
      _ => self.radius * (1.0 - self.skirt),
    }
  }

  /// the radius where the top's chamfer ends and its face begins
  fn face_radius(&self) -> f32 {
    self.grip_radius() - self.height * (1.0 - SHOULDER)
  }

  /// how far out the grip is at an angle, against its plain radius
  fn grip_at(&self, angle: f32) -> f32 {
    match self.grip {
      Grip::Smooth | Grip::Faceted(_) | Grip::Knurled(_) => 1.0,
      Grip::Fluted { flutes, depth } => {
        // a scoop takes up most of its share of the turn, and between two scoops a strip
        // of the round side is left standing
        let across = ((angle * flutes as f32).cos() - 0.2).max(0.0) / 0.8;
        1.0 - depth * (1.0 - (1.0 - across) * (1.0 - across))
      }
      Grip::Pointed { reach } => {
        let toward = (angle + FRAC_PI_2).cos().max(0.0);
        1.0 + reach * toward.powi(10)
      }
      // the angle from the way the knob points, which is up the panel
      Grip::Bar { width } => {
        let from = angle + FRAC_PI_2;
        handle_at(from, width).max(handle_at(from + PI, width))
      }
      Grip::Lever { width, body } => handle_at(angle + FRAC_PI_2, width).max(body),
    }
  }

  /// the height of the top's face at a point of the knob, for laying markers on it
  pub fn top_at(&self, at: Vec2) -> f32 {
    let face = self.face_radius();
    let out = (at.length() / face).min(1.0);
    match self.top {
      Top::Flat => self.height,
      Top::Dished { depth } => self.height * (1.0 - depth * (1.0 - out * out)),
      Top::Domed { rise } => self.height * (1.0 + rise * (1.0 - out * out)),
      Top::Dimpled => {
        let (center, radius) = self.dimple();
        let within = (at.distance(center) / radius).min(1.0);
        // a round-bottomed hollow that meets the flat face without an edge
        let hollow = (1.0 - within * within).powi(2);
        self.height * (1.0 - 0.3 * hollow)
      }
    }
  }

  /// where a dimpled top's divot is, and how wide
  fn dimple(&self) -> (Vec2, f32) {
    let face = self.face_radius();
    (Vec2::new(0.0, -face * 0.52), face * 0.36)
  }

  /// how many points go around the knob
  fn segments(&self) -> u32 {
    match self.grip {
      Grip::Faceted(sides) => sides.max(3),
      // a ridge is one point out and one point in
      Grip::Knurled(ridges) => ridges.max(3) * 2,
      Grip::Fluted { flutes, .. } => (flutes * 12).max(48),
      Grip::Smooth | Grip::Pointed { .. } => 72,
      // enough to keep a handle's long sides straight
      Grip::Bar { .. } | Grip::Lever { .. } => 180,
    }
  }

  /// the knob itself, without a marker
  pub fn body(&self) -> Mesh {
    let (grip, face) = (self.grip_radius(), self.face_radius());
    let shoulder = self.height * SHOULDER;
    // each ring is its radius, its height, and how much of the grip's shape it takes
    let mut rings = Vec::new();
    let skirtless = matches!(self.grip, Grip::Pointed { .. } | Grip::Lever { .. });
    let waist = if self.skirt > 0.0 && !skirtless {
      rings.push((self.radius, 0.0, 0.0));
      rings.push((self.radius, self.height * 0.16, 0.0));
      // the skirt slopes up to a round collar that the grip's shape then rises out of
      rings.push((grip, self.height * 0.28, 0.0));
      self.height * 0.32
    } else {
      0.0
    };
    rings.push((grip, waist, 1.0));
    rings.push((grip, shoulder, 1.0));
    let shaped_top = self.top != Top::Flat;
    let top_rings = if shaped_top { TOP_RINGS } else { 1 };
    for ring in 0..=top_rings {
      // the face's rim follows the grip, so that flutes cut into the top and a point carries
      // out along it, and the shape fades toward the middle
      let inward = ring as f32 / top_rings as f32;
      rings.push((face * (1.0 - inward), f32::NAN, 1.0 - inward));
    }

    let knurled = matches!(self.grip, Grip::Knurled(_));
    let segments = self.segments();
    // a faceted knob keeps every edge between its sides hard, however many it has
    let crease = if matches!(self.grip, Grip::Faceted(_)) {
      0.999
    } else {
      CREASE
    };
    Mesh::rings(rings.len(), segments, crease, |ring, angle| {
      let (radius, height, shaped) = rings[ring];
      let mut out = 1.0 + (self.grip_at(angle) - 1.0) * shaped;
      if knurled && shaped > 0.0 {
        // every other point around sits in a groove
        let step = (angle / TAU * segments as f32).round() as u32;
        out -= 0.05 * (step % 2) as f32;
      }
      let at = Vec2::from_angle(angle) * radius * out;
      // a ring without a height of its own lies on the top's face
      let height = if height.is_nan() {
        self.top_at(at)
      } else {
        height
      };
      at.extend(height)
    })
  }

  /// a raised line on the top from near the middle to near the rim, following the face
  pub fn line(&self) -> Mesh {
    let face = self.face_radius();
    let (from, to) = match self.grip {
      // a pointed knob's line runs out along its point
      Grip::Pointed { .. } => (-face * 0.5, self.radius * 0.8),
      Grip::Lever { .. } => (0.0, face * 0.9),
      _ => (face * 0.3, face * 0.9),
    };
    let (half_width, raised) = (self.radius * 0.05, self.height * 0.03);
    // in short pieces, each at the height of the face under it
    const PIECES: usize = 6;
    let mut line = Mesh::default();
    for piece in 0..PIECES {
      let start = from + (to - from) * piece as f32 / PIECES as f32;
      let end = from + (to - from) * (piece + 1) as f32 / PIECES as f32;
      let under = self
        .top_at(Vec2::new(0.0, -start))
        .min(self.top_at(Vec2::new(0.0, -end)));
      let over = self
        .top_at(Vec2::new(0.0, -start))
        .max(self.top_at(Vec2::new(0.0, -end)));
      line = line.merged(&Mesh::cuboid(
        Vec3::new(-half_width, -end, under - raised),
        Vec3::new(half_width, -start, over + raised),
      ));
    }
    line
  }

  /// a small raised dot on the top near the rim
  pub fn dot(&self) -> Mesh {
    let face = self.face_radius();
    let (at, radius) = (Vec2::new(0.0, -face * 0.68), self.radius * 0.09);
    let height = self.top_at(at);
    Mesh::lathe(
      &[[radius, -radius], [radius, 0.0], [0.0, radius * 0.4]],
      16,
      true,
    )
    .transformed(Mat4::from_translation(at.extend(height)))
  }
}

/// where a knob sits and how far it has turned. `turn` runs from zero to one over `sweep`
/// radians, centered on pointing up the panel
pub fn knob_transform(x: f32, y: f32, turn: f32, sweep: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z((turn - 0.5) * sweep)
}

/// the scale around a knob: `count` thin marks lying on the panel, spread over `sweep`
/// radians the way [`knob_transform`] turns, from `inner` to `outer` away from the center.
/// every `long_every`th mark is half as long again
pub fn ticks(count: u32, sweep: f32, inner: f32, outer: f32, long_every: u32) -> Mesh {
  let mut ticks = Mesh::default();
  for tick in 0..count {
    let turn = if count > 1 {
      tick as f32 / (count - 1) as f32
    } else {
      0.5
    };
    let long = long_every > 0 && tick % long_every == 0;
    let reach = if long {
      outer + (outer - inner) * 0.5
    } else {
      outer
    };
    let half_width = if long { 1.0 } else { 0.6 };
    let mark = Mesh::cuboid(
      Vec3::new(-half_width, -reach, 0.0),
      Vec3::new(half_width, -inner, 0.3),
    );
    ticks = ticks.merged(&mark.transformed(Mat4::from_rotation_z((turn - 0.5) * sweep)));
  }
  ticks
}

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

#[cfg(test)]
mod tests {
  use super::*;

  fn all(radius: f32, height: f32) -> [Knob; 10] {
    [
      Knob::round(radius, height),
      Knob::fluted(radius, height),
      Knob::skirted(radius, height),
      Knob::knurled(radius, height),
      Knob::dished(radius, height),
      Knob::dimpled(radius, height),
      Knob::faceted(radius, height),
      Knob::chicken_head(radius, height),
      Knob::bar(radius, height),
      Knob::lever(radius, height),
    ]
  }

  #[test]
  fn every_knob_stays_within_its_radius_and_stands_on_the_panel() {
    for knob in all(40.0, 30.0) {
      let body = knob.body();
      let mut widest = 0.0_f32;
      for vertex in &body.vertices {
        let position = Vec3::from(vertex.position);
        widest = widest.max(position.truncate().length());
        assert!(
          position.z >= 0.0,
          "{knob:?} goes under the panel at {position}"
        );
        assert!(
          Vec3::from(vertex.normal).is_normalized(),
          "{knob:?} at {position}"
        );
      }
      assert!((widest - 40.0).abs() < 0.01, "{knob:?} is {widest} wide");
    }
  }

  #[test]
  fn markers_lie_on_the_top_they_mark() {
    for knob in all(40.0, 30.0) {
      for marker in [knob.line(), knob.dot()] {
        for vertex in &marker.vertices {
          let position = Vec3::from(vertex.position);
          let face = knob.top_at(position.truncate());
          assert!(
            (position.z - face).abs() < 30.0 * 0.25,
            "{knob:?} marker at {position}"
          );
        }
      }
    }
  }

  #[test]
  fn a_handle_is_a_strip_with_round_ends() {
    // straight along it reaches all the way out, and behind it there is none
    assert!((handle_at(0.0, 0.4) - 1.0).abs() < 1e-6);
    assert_eq!(handle_at(PI, 0.4), 0.0);
    // far enough to the side the ray leaves through the long edge, half the width away
    let aside = 1.0_f32;
    assert!((handle_at(aside, 0.4) * aside.sin() - 0.2).abs() < 1e-6);
    // and near the end it leaves through the round one, which is nowhere wider
    for step in 0..100 {
      let angle = step as f32 / 100.0 * FRAC_PI_2;
      let out = handle_at(angle, 0.4);
      assert!(
        out * angle.sin() <= 0.2 + 1e-5 && out <= 1.0 + 1e-5,
        "{angle}: {out}"
      );
    }
  }

  #[test]
  fn a_dimple_is_the_lowest_place_on_its_top() {
    let knob = Knob::dimpled(40.0, 30.0);
    let (center, radius) = knob.dimple();
    assert!(knob.top_at(center) < 30.0 * 0.75);
    assert_eq!(knob.top_at(center + Vec2::X * radius * 1.01), 30.0);
    assert_eq!(knob.top_at(-center), 30.0);
  }
}
