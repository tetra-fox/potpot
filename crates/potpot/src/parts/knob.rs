//! Knobs, each a grip under a top, and the markers that lie on them.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Mat4, Vec2, Vec3};

use super::grip::Grip;
use super::{CREASE, PAINT, ROLL, TOP_RINGS};
use crate::mesh::Mesh;

/// The sweep of most hardware knobs, in radians.
///
/// The sweep is three quarters of a turn.
pub const KNOB_SWEEP: f32 = 1.5 * PI;

/// The count of levels cut into a grip that changes on its way up.
const LEVELS: usize = 10;

/// One ring of a knob's surface.
#[derive(Clone, Copy)]
struct Ring {
  radius: f32,
  /// The ring's height in points, or `None` to lie on the top's face.
  height: Option<f32>,
  /// The share of the grip's shape the ring takes.
  ///
  /// Zero is a plain circle.
  shaped: f32,
  /// The ring's place up the grip, for the grips that change on the way.
  level: f32,
  /// The amount the ring has rounded off toward a circle.
  ///
  /// The circle is the largest one inside the grip's outline. The top of a scooped or ribbed grip
  /// rounds off toward it on the way to the middle.
  rounded: f32,
}

/// Returns a flat strip `half_width` to each side of a path.
///
/// Each point of `path` is how far out toward negative y and how high. The strip stands just off
/// the surface the path follows.
fn ribbon(path: &[Vec2], half_width: f32) -> Mesh {
  // the way out of the surface at each point: its two neighbors' direction turned a quarter
  let facing = |index: usize| {
    let before = path[index.saturating_sub(1)];
    let after = path[(index + 1).min(path.len() - 1)];
    let along = (after - before).normalize_or_zero();
    Vec2::new(along.y, -along.x)
  };
  let mut ribbon = Mesh::default();
  for index in 0..path.len().saturating_sub(1) {
    let corner = |index: usize, side: f32| {
      let (point, facing) = (path[index], facing(index));
      let normal = Vec3::new(0.0, -facing.x, facing.y);
      // twice as far off as an inlay, to lie over one
      (
        Vec3::new(side * half_width, -point.x, point.y) + normal * PAINT * 2.0,
        normal,
      )
    };
    let corners = [
      corner(index, -1.0),
      corner(index, 1.0),
      corner(index + 1, 1.0),
      corner(index + 1, -1.0),
    ];
    ribbon.quad_with_normals(
      corners.map(|(position, _)| position),
      corners.map(|(_, normal)| normal),
    );
  }
  ribbon
}

/// A knob, described by its parts.
///
/// The named constructors, like [`Knob::fluted`], are starting points whose fields can be set to
/// anything else.
///
/// Every knob points up the panel, toward negative y, before it turns.
#[derive(Clone, Copy, Debug)]
pub struct Knob {
  /// The knob's widest radius, at its skirt if it has one, in points.
  pub radius: f32,
  /// The height of the rim of its top along z, in points.
  pub height: f32,
  /// The grip's inset from a skirt under it, as a share of the radius.
  ///
  /// Zero is a knob with straight sides and no skirt.
  pub skirt: f32,
  /// The outline of its side.
  pub grip: Grip,
  /// The shape of its face.
  pub top: Top,
  /// The grip's narrowing from foot to top, as a share of its radius.
  ///
  /// A knob with a taper is a cone.
  pub taper: f32,
  /// The way the side meets the top.
  pub edge: Edge,
}

/// The edge between a knob's side and its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
  /// A hard corner.
  Sharp,
  /// One flat cut across the corner.
  ///
  /// The cut is this share of the height tall and as wide.
  Chamfered(f32),
  /// A quarter round of this share of the height.
  Rolled(f32),
}

/// The face of a knob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Top {
  /// A face level all over.
  Flat,
  /// A face hollowed like a shallow bowl.
  Dished {
    /// The depth of the middle, as a share of the height.
    depth: f32,
  },
  /// A face raised to a round crown.
  Domed {
    /// The rise of the middle, as a share of the height.
    rise: f32,
  },
  /// A flat face with one fingertip-sized divot near the rim.
  ///
  /// The divot is on the side the knob points at, and is the only marker such a knob needs.
  Dimpled,
}

impl Knob {
  /// Creates a plain round knob on a skirt.
  pub fn round(radius: f32, height: f32) -> Self {
    Self {
      radius,
      height,
      skirt: 0.2,
      grip: Grip::Smooth,
      // barely crowned, which is what lets a polished top catch the light unevenly
      top: Top::Domed { rise: 0.05 },
      taper: 0.0,
      edge: Edge::Rolled(0.14),
    }
  }

  /// Creates a knob with scooped sides, straight down to the panel.
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

  /// Creates a knob with scooped sides over a wide skirt.
  ///
  /// It is the kind with the scale printed on the skirt.
  pub fn skirted(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.32,
      grip: Grip::Fluted {
        flutes: 9,
        depth: 0.1,
      },
      top: Top::Flat,
      ..Self::round(radius, height)
    }
  }

  /// Creates a knob with fine ridges all around and a domed top.
  pub fn knurled(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Knurled(36),
      top: Top::Domed { rise: 0.08 },
      ..Self::round(radius, height)
    }
  }

  /// Creates a soft-looking knob with a bowl for a top.
  pub fn dished(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      top: Top::Dished { depth: 0.22 },
      ..Self::round(radius, height)
    }
  }

  /// Creates a low wide knob turned by a fingertip in its divot.
  pub fn dimpled(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      top: Top::Dimpled,
      ..Self::round(radius, height)
    }
  }

  /// Creates a knob with a few flat sides.
  pub fn faceted(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Faceted(9),
      top: Top::Flat,
      edge: Edge::Chamfered(0.1),
      ..Self::round(radius, height)
    }
  }

  /// Creates a round body drawn out to a point.
  pub fn chicken_head(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Pointed { reach: 1.1 },
      ..Self::round(radius, height)
    }
  }

  /// Creates a tall handle across a low round base.
  ///
  /// It is the kind a toe can nudge.
  pub fn bar(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.12,
      grip: Grip::Bar { width: 0.42 },
      ..Self::round(radius, height)
    }
  }

  /// Creates a small round knob with a long handle sticking out.
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

  /// Creates a tall pointer blade on a small base, like a DM-2300.
  pub fn ridge(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Ridge { width: 1.0 },
      top: Top::Flat,
      // the crest is too thin to carry an edge of its own
      edge: Edge::Sharp,
      ..Self::round(radius, height)
    }
  }

  /// Creates six deep scoops on a cone with a cut edge.
  ///
  /// It is the pedal knob with a hexagon's outline.
  pub fn hex(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Fluted {
        flutes: 6,
        depth: 0.16,
      },
      top: Top::Flat,
      taper: 0.16,
      edge: Edge::Chamfered(0.12),
      ..Self::round(radius, height)
    }
  }

  /// Creates a straight cylinder with round ribs and a cut edge.
  ///
  /// It is made for a face of another material.
  pub fn ribbed(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Ribbed {
        ribs: 24,
        depth: 0.05,
      },
      top: Top::Flat,
      edge: Edge::Chamfered(0.07),
      ..Self::round(radius, height)
    }
  }

  /// Creates a ribbed cone widening down to the panel.
  ///
  /// It is like an amplifier's knob.
  pub fn conical(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Ribbed {
        ribs: 30,
        depth: 0.035,
      },
      top: Top::Flat,
      taper: 0.34,
      edge: Edge::Chamfered(0.05),
      ..Self::round(radius, height)
    }
  }

  /// Returns the radius of the grip's round parts.
  fn grip_radius(&self) -> f32 {
    match self.grip {
      // the point reaches the full radius and the rest stays inside it
      Grip::Pointed { reach } => self.radius / (1.0 + reach),
      // the handle's end reaches the full radius
      Grip::Lever { .. } | Grip::Ridge { .. } => self.radius,
      _ => self.radius * (1.0 - self.skirt),
    }
  }

  /// Returns the radius of the grip's round parts at its top.
  fn crown_radius(&self) -> f32 {
    self.grip_radius() * (1.0 - self.taper)
  }

  /// Returns how tall and how wide the edge's cut or roll is.
  fn edge_size(&self) -> f32 {
    match self.edge {
      Edge::Sharp => 0.0,
      // never so wide that it eats a small face
      Edge::Chamfered(share) | Edge::Rolled(share) => {
        (self.height * share).min(self.crown_radius() * 0.2)
      }
    }
  }

  /// Returns the radius where the edge ends and the face begins.
  fn face_radius(&self) -> f32 {
    self.crown_radius() - self.edge_size()
  }

  /// Returns the height of the top's face at a point of the knob.
  ///
  /// A marker laid on the face lies at this height.
  pub fn top_at(&self, at: Vec2) -> f32 {
    self.top_height(at, (at.length() / self.face_radius()).min(1.0))
  }

  /// Returns the height of the top's face at a point.
  ///
  /// The point is `out` of the way from the face's middle to its rim. On a grip that is not round,
  /// distance from the middle does not give `out`.
  fn top_height(&self, at: Vec2, out: f32) -> f32 {
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

  /// Returns the middle and the radius of a dimpled top's divot.
  fn dimple(&self) -> (Vec2, f32) {
    let face = self.face_radius();
    (Vec2::new(0.0, -face * 0.52), face * 0.36)
  }

  /// Returns the rings of the knob's surface and the grip's foot.
  ///
  /// The rings run from the panel up and in to the middle of the top. The index is of the ring that
  /// is the foot of the grip.
  fn rings(&self) -> (Vec<Ring>, usize) {
    let (grip, crown, face) = (self.grip_radius(), self.crown_radius(), self.face_radius());
    let shoulder = self.height - self.edge_size();
    let round = |radius: f32, height: f32| Ring {
      radius,
      height: Some(height),
      shaped: 0.0,
      level: 0.0,
      rounded: 0.0,
    };
    let mut rings = Vec::new();
    let skirtless = matches!(self.grip, Grip::Pointed { .. } | Grip::Lever { .. });
    let waist = if self.skirt > 0.0 && !skirtless {
      rings.push(round(self.radius, 0.0));
      rings.push(round(self.radius, self.height * 0.13));
      // a rolled lip, then the skirt slopes up to a round collar that the grip's shape
      // rises out of
      rings.push(round(self.radius * 0.985, self.height * 0.165));
      rings.push(round(self.radius * 0.95, self.height * 0.185));
      rings.push(round(grip * 1.03, self.height * 0.29));
      rings.push(round(grip, self.height * 0.31));
      self.height * 0.33
    } else if matches!(self.grip, Grip::Ridge { .. }) {
      // the blade stands on a round base narrower than itself
      let base = self.radius * 0.56;
      rings.push(round(base, 0.0));
      rings.push(round(base, self.height * 0.11));
      rings.push(round(base * 0.93, self.height * 0.15));
      self.height * 0.15
    } else {
      0.0
    };
    let foot = rings.len();
    let shaped = |radius: f32, height: f32, level: f32| Ring {
      radius,
      height: Some(height),
      shaped: 1.0,
      level,
      rounded: 0.0,
    };
    // a grip whose outline changes on the way up takes a ring at every level, any other
    // only its foot and its top
    let levels = if matches!(self.grip, Grip::Ridge { .. }) {
      LEVELS
    } else {
      1
    };
    for step in 0..=levels {
      let level = step as f32 / levels as f32;
      rings.push(shaped(
        grip + (crown - grip) * level,
        waist + (shoulder - waist) * level,
        level,
      ));
    }
    // a roll takes the shoulder over to the face in a quarter turn. a chamfer is the one
    // flat band from the shoulder to the face's rim, and a sharp edge is no band at all
    if let Edge::Rolled(_) = self.edge {
      for step in 1..ROLL {
        let (up, inward) = (step as f32 / ROLL as f32 * FRAC_PI_2).sin_cos();
        rings.push(shaped(
          face + (crown - face) * inward,
          shoulder + (self.height - shoulder) * up,
          1.0,
        ));
      }
    }
    let top_rings = if self.top == Top::Flat { 1 } else { TOP_RINGS };
    // with a sharp edge the shoulder is the face's rim already
    let first = usize::from(self.edge_size() == 0.0);
    for ring in first..=top_rings {
      // the face's rim has the grip's outline, so that flutes cut into the top and a
      // handle's top is a handle
      let inward = ring as f32 / top_rings as f32;
      rings.push(Ring {
        radius: face * (1.0 - inward),
        height: None,
        shaped: 1.0,
        level: 1.0,
        rounded: inward,
      });
    }
    (rings, foot)
  }

  /// Returns the point of a ring at an angle around the knob.
  ///
  /// `crests` leaves out a knurled grip's grooves, for what lies over them.
  fn surface(&self, ring: Ring, angle: f32, crests: bool) -> Vec3 {
    let mut out = 1.0 + (self.grip.at(angle, ring.level) - 1.0) * ring.shaped;
    match self.grip {
      // scoops die out quickly toward the middle of the top, so that most of the face is
      // plain and round
      Grip::Fluted { depth, .. } | Grip::Ribbed { depth, .. } => {
        out += (1.0 - depth - out) * (1.0 - (1.0 - ring.rounded).powi(4));
      }
      Grip::Knurled(_) if !crests && ring.shaped > 0.0 => {
        // every other point around sits in a groove, and the grooves die out the same way
        let step = (angle / TAU * self.grip.segments() as f32).round() as u32;
        out -= 0.045 * (step % 2) as f32 * ring.shaped * (1.0 - ring.rounded);
      }
      _ => {}
    }
    let at = Vec2::from_angle(angle) * ring.radius * out;
    // a ring on the face has rounded off by as much as it has come in from the rim
    let height = ring
      .height
      .unwrap_or_else(|| self.top_height(at, 1.0 - ring.rounded));
    at.extend(height)
  }

  /// Returns the mesh of the knob itself, without a marker.
  pub fn body(&self) -> Mesh {
    let (rings, _) = self.rings();
    // a faceted knob keeps every edge between its sides hard, however many it has
    let crease = if matches!(self.grip, Grip::Faceted(_)) {
      0.999
    } else {
      CREASE
    };
    Mesh::rings(rings.len(), self.grip.segments(), crease, |ring, angle| {
      self.surface(rings[ring], angle, false)
    })
  }

  /// Returns a line painted on the knob where it points.
  ///
  /// The line runs across the top, over the shoulder and down the side. It starts at the middle of
  /// the top on a grip with a handle, a point or a blade, and partway out on any other.
  pub fn line(&self) -> Mesh {
    let (rings, foot) = self.rings();
    let face = self.face_radius();
    // a handle's line runs its whole length, any other stops short of the middle
    let (from, half_width) = match self.grip {
      Grip::Pointed { .. } | Grip::Lever { .. } | Grip::Bar { .. } => (0.0, self.radius * 0.045),
      // a blade's crest has room only for a hairline
      Grip::Ridge { .. } => (0.0, self.radius * 0.04),
      _ => (face * 0.4, self.radius * 0.045),
    };
    // the way from the grip's foot up the side and in across the top, as how far out from
    // the middle and how high
    let mut path: Vec<Vec2> = Vec::new();
    for ring in &rings[foot..] {
      let point = self.surface(*ring, 1.5 * PI, true);
      let next = Vec2::new(-point.y, point.z);
      if ring.height.is_none() && next.x < from {
        if let Some(&last) = path.last()
          && last.x > from
        {
          path.push(last.lerp(next, (last.x - from) / (last.x - next.x)));
        }
        break;
      }
      path.push(next);
    }
    ribbon(&path, half_width)
  }

  /// Returns a thin disc lying in the top's face.
  ///
  /// The disc follows the face's shape, for a face of another material than the body, like an
  /// aluminum insert. `inset` is the share of the face it covers, by radius.
  pub fn cap(&self, inset: f32) -> Mesh {
    let face = self.face_radius();
    Mesh::rings(
      TOP_RINGS + 1,
      self.grip.segments(),
      CREASE,
      |ring, angle| {
        let inward = ring as f32 / TOP_RINGS as f32;
        let ring = Ring {
          radius: face * inset * (1.0 - inward),
          height: None,
          shaped: 1.0,
          level: 1.0,
          rounded: 1.0 - inset * (1.0 - inward),
        };
        self.surface(ring, angle, true) + Vec3::Z * PAINT
      },
    )
  }

  /// Returns a small raised dot on the top near the rim.
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

/// Returns the transform of a knob at a place and a turn.
///
/// `turn` runs from zero to one over `sweep` radians, centered on pointing up the panel.
pub fn knob_transform(x: f32, y: f32, turn: f32, sweep: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z((turn - 0.5) * sweep)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn all(radius: f32, height: f32) -> [Knob; 14] {
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
      Knob::ridge(radius, height),
      Knob::hex(radius, height),
      Knob::ribbed(radius, height),
      Knob::conical(radius, height),
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
          "{knob:?} stands on the panel at {position}"
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
  fn a_dot_lies_on_the_top_it_marks() {
    for knob in all(40.0, 30.0) {
      for vertex in &knob.dot().vertices {
        let position = Vec3::from(vertex.position);
        let face = knob.top_at(position.truncate());
        assert!(
          (position.z - face).abs() < 30.0 * 0.25,
          "{knob:?} dot at {position}"
        );
      }
    }
  }

  #[test]
  fn a_line_runs_from_the_top_down_the_side_the_knob_points_at() {
    for knob in all(40.0, 30.0) {
      let line = knob.line();
      let positions: Vec<Vec3> = (line.vertices.iter())
        .map(|vertex| vertex.position.into())
        .collect();
      assert!(!positions.is_empty(), "{knob:?} has a line");
      for position in &positions {
        // it is a narrow strip up the panel from the middle, no further out than the knob
        assert!(
          position.x.abs() < 40.0 * 0.05,
          "{knob:?} line at {position}"
        );
        assert!(
          (-41.0..=0.5).contains(&position.y),
          "{knob:?} line at {position}"
        );
      }
      for vertex in &line.vertices {
        assert!(
          Vec3::from(vertex.normal).is_normalized(),
          "{knob:?} line faces {:?}",
          vertex.normal
        );
      }
      // from the height of the top to well down the side
      let highest = positions
        .iter()
        .map(|position| position.z)
        .fold(f32::MIN, f32::max);
      let lowest = positions
        .iter()
        .map(|position| position.z)
        .fold(f32::MAX, f32::min);
      assert!(
        highest > 30.0 * 0.95 && lowest < 30.0 * 0.5,
        "{knob:?}: {lowest} to {highest}"
      );
    }
  }

  #[test]
  fn a_cap_lies_just_over_the_face_and_inside_it() {
    for knob in all(40.0, 30.0) {
      for vertex in &knob.cap(0.85).vertices {
        let position = Vec3::from(vertex.position);
        let over = position.z - knob.top_at(position.truncate());
        // a grip that is not round shapes its top by rings, which `top_at` only comes near
        let near = if knob.grip == Grip::Smooth {
          0.0..0.5
        } else {
          -2.0..2.0
        };
        assert!(
          near.contains(&over),
          "{knob:?} cap at {position} is {over} over"
        );
        assert!(position.truncate().length() <= knob.radius * 0.851);
      }
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

  #[test]
  fn a_knob_tapered_to_a_point_has_a_place_for_every_vertex() {
    let knob = Knob {
      taper: 1.0,
      ..Knob::round(40.0, 30.0)
    };
    for mesh in [knob.body(), knob.line()] {
      assert!(!mesh.vertices.is_empty());
      for vertex in &mesh.vertices {
        assert!(
          Vec3::from(vertex.position).is_finite(),
          "{:?}",
          vertex.position
        );
      }
    }
  }
}
