//! shapes of common panel hardware, sized in panel points and centered on the z axis

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Mat4, Vec2, Vec3};

use crate::mesh::Mesh;
use crate::scene::Object;

/// the sweep of most hardware knobs, three quarters of a turn
pub const KNOB_SWEEP: f32 = 1.5 * PI;

/// faces that turn from each other by less than about 35 degrees shade as one curve
const CREASE: f32 = 0.82;
/// the rings a shaped top is cut into from its rim to its middle
const TOP_RINGS: usize = 14;
/// the levels a grip that changes on its way up is cut into
const LEVELS: usize = 10;
/// the rings the roll from the side to the top is cut into
const ROLL: usize = 6;
/// how far paint and inlays stand off the surface they are on
const PAINT: f32 = 0.25;

/// one ring of a knob's surface
#[derive(Clone, Copy)]
struct Ring {
  radius: f32,
  /// none for a ring that lies on the top's face, wherever that is
  height: Option<f32>,
  /// how much of the grip's shape the ring takes, down to none for a plain circle
  shaped: f32,
  /// how far up the grip it is, for the grips that change on the way
  level: f32,
  /// how far the ring has rounded off toward the largest circle inside the grip's outline,
  /// which a scooped or ridged grip's top does on its way to the middle
  rounded: f32,
}

/// a flat strip `half_width` to each side of a path of points, each given as how far out
/// toward negative y and how high, standing just off the surface the path follows
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
  /// how much narrower the grip is at its top than at its foot, as a share of its radius,
  /// for a knob that is a cone
  pub taper: f32,
  /// how the side meets the top
  pub edge: Edge,
}

/// the edge between a knob's side and its top
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
  /// a hard corner
  Sharp,
  /// one flat cut across the corner, this share of the height tall and as wide
  Chamfered(f32),
  /// a quarter round of this share of the height
  Rolled(f32),
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
  /// this many round ribs with round grooves between them, each `depth` of the radius deep
  Ribbed { ribs: u32, depth: f32 },
  /// drawn out to a point on the side it points at, `reach` of the radius further than the
  /// rest, like a chicken head knob
  Pointed { reach: f32 },
  /// a handle standing across the whole knob like a stove dial's, `width` of the radius
  /// wide, with round ends
  Bar { width: f32 },
  /// a round body with one long handle out to the side it points at, for turning with a
  /// foot. the body is `body` of the radius and the handle `width` of it wide
  Lever { width: f32, body: f32 },
  /// a tall blade that narrows from `width` of the radius at its foot to a thin crest, with
  /// a long nose sloping down to a point on the side it points at and a short round tail,
  /// standing on a small round base
  Ridge { width: f32 },
}

/// how far out a blade's outline reaches at `angle` from its nose: straight edges running
/// in to a point `nose` ahead, a round back `tail` behind, and `half` wide to each side of
/// the center
fn blade_at(angle: f32, nose: f32, tail: f32, half: f32) -> f32 {
  let (across, along) = (angle.sin().abs(), angle.cos());
  if along >= 0.0 {
    half / (across + half * along / nose)
  } else {
    // half an ellipse
    1.0 / (along / tail).hypot(across / half)
  }
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
      // barely crowned, which is what lets a polished top catch the light unevenly
      top: Top::Domed { rise: 0.05 },
      taper: 0.0,
      edge: Edge::Rolled(0.14),
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
      top: Top::Flat,
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
      top: Top::Flat,
      edge: Edge::Chamfered(0.1),
      ..Self::round(radius, height)
    }
  }

  /// drawn out to a point, on a skirt
  pub fn chicken_head(radius: f32, height: f32) -> Self {
    Self {
      skirt: 0.0,
      grip: Grip::Pointed { reach: 1.1 },
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

  /// a tall pointer blade on a small base, like a DM-2300
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

  /// six deep scoops on a cone with a cut edge, the pedal knob with a hexagon's outline
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

  /// a straight cylinder with round ribs and a cut edge, made for a face of another material
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

  /// a ribbed cone widening down to the panel, like an amplifier's
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

  /// the radius of the grip's round parts
  fn grip_radius(&self) -> f32 {
    match self.grip {
      // the point reaches the full radius and the rest stays inside it
      Grip::Pointed { reach } => self.radius / (1.0 + reach),
      // the handle's end reaches the full radius
      Grip::Lever { .. } | Grip::Ridge { .. } => self.radius,
      _ => self.radius * (1.0 - self.skirt),
    }
  }

  /// the radius of the grip's round parts at its top
  fn crown_radius(&self) -> f32 {
    self.grip_radius() * (1.0 - self.taper)
  }

  /// how tall and how wide the cut or roll between the side and the top is
  fn edge_size(&self) -> f32 {
    match self.edge {
      Edge::Sharp => 0.0,
      // never so wide that it eats a small face
      Edge::Chamfered(share) | Edge::Rolled(share) => {
        (self.height * share).min(self.crown_radius() * 0.2)
      }
    }
  }

  /// the radius where the edge ends and the top's face begins
  fn face_radius(&self) -> f32 {
    self.crown_radius() - self.edge_size()
  }

  /// how far out the grip is at an angle, against its plain radius. `level` is how far up the
  /// grip, from zero at its foot to one at its top, for the grips that change on the way
  fn grip_at(&self, angle: f32, level: f32) -> f32 {
    match self.grip {
      Grip::Smooth | Grip::Faceted(_) | Grip::Knurled(_) => 1.0,
      Grip::Ribbed { ribs, depth } => {
        // turned so that a rib is where the knob points
        let turned = (angle + FRAC_PI_2) * ribs as f32;
        1.0 - depth * (0.5 - 0.5 * turned.cos())
      }
      Grip::Fluted { flutes, depth } => {
        // a scoop takes up most of its share of the turn, and between two scoops a strip
        // of the round side is left standing
        // turned so that the middle of a strip is where the knob points, for its line
        let turned = (angle + FRAC_PI_2) * flutes as f32 + PI;
        let across = (turned.cos() - 0.2).max(0.0) / 0.8;
        1.0 - depth * (1.0 - (1.0 - across) * (1.0 - across))
      }
      Grip::Pointed { reach } => {
        // a round body, and a beak whose straight sides leave it without a corner and run
        // in to a point
        let nose = 1.0 + reach;
        let half = nose / (nose * nose - 1.0).sqrt();
        blade_at(angle + FRAC_PI_2, nose, 1.0, half).max(1.0)
      }
      // the angle from the way the knob points, which is up the panel
      Grip::Bar { width } => {
        let from = angle + FRAC_PI_2;
        handle_at(from, width).max(handle_at(from + PI, width))
      }
      Grip::Lever { width, body } => handle_at(angle + FRAC_PI_2, width).max(body),
      Grip::Ridge { width } => {
        // the flanks sweep in from a wide foot to a thin crest, hollow like a tent's, and
        // the nose sweeps up from its point at the foot to where the crest ends
        let up = 1.0 - level;
        let between = |foot: f32, crest: f32, sweep: f32| crest + (foot - crest) * up.powf(sweep);
        blade_at(
          angle + FRAC_PI_2,
          between(1.0, 0.34, 1.5),
          between(0.72, 0.52, 1.5),
          between(width / 2.0, 0.085, 2.4),
        )
      }
    }
  }

  /// the height of the top's face at a point of the knob, for laying markers on it
  pub fn top_at(&self, at: Vec2) -> f32 {
    self.top_height(at, (at.length() / self.face_radius()).min(1.0))
  }

  /// the height of the top's face at a point that is `out` of the way from its middle to
  /// its rim, which on a grip that is not round is not a matter of distance
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
      Grip::Ribbed { ribs, .. } => ribs.max(3) * 8,
      Grip::Smooth => 72,
      // enough to keep a beak's sides straight and its point sharp
      Grip::Pointed { .. } => 180,
      // enough to keep a handle's long sides straight
      Grip::Bar { .. } | Grip::Lever { .. } | Grip::Ridge { .. } => 180,
    }
  }

  /// the rings of the knob's surface from the panel up and in to the middle of its top, and
  /// which of them is the foot of the grip
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

  /// the point of a ring at an angle around the knob. `crests` leaves out a knurled grip's
  /// grooves, for what lies over them
  fn surface(&self, ring: Ring, angle: f32, crests: bool) -> Vec3 {
    let mut out = 1.0 + (self.grip_at(angle, ring.level) - 1.0) * ring.shaped;
    match self.grip {
      // scoops die out toward the middle of the top, which is a plain round face
      // and do so quickly, so that most of the face is plain and round
      Grip::Fluted { depth, .. } | Grip::Ribbed { depth, .. } => {
        out += (1.0 - depth - out) * (1.0 - (1.0 - ring.rounded).powi(4));
      }
      Grip::Knurled(_) if !crests && ring.shaped > 0.0 => {
        // every other point around sits in a groove, and the grooves die out the same way
        let step = (angle / TAU * self.segments() as f32).round() as u32;
        out -= 0.045 * (step % 2) as f32 * ring.shaped * (1.0 - ring.rounded);
      }
      _ => {}
    }
    let at = Vec2::from_angle(angle) * ring.radius * out;
    let height = ring
      .height
      .unwrap_or_else(|| self.top_height(at, ring.radius / self.face_radius()));
    at.extend(height)
  }

  /// the knob itself, without a marker
  pub fn body(&self) -> Mesh {
    let (rings, _) = self.rings();
    // a faceted knob keeps every edge between its sides hard, however many it has
    let crease = if matches!(self.grip, Grip::Faceted(_)) {
      0.999
    } else {
      CREASE
    };
    Mesh::rings(rings.len(), self.segments(), crease, |ring, angle| {
      self.surface(rings[ring], angle, false)
    })
  }

  /// a line painted on the knob where it points: across the top from partway out, over the
  /// shoulder and down the side
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

  /// a thin disc lying in the top's face and following its shape, for a face of another
  /// material than the body, like an aluminum insert. `inset` is how much of the face it
  /// covers, by radius
  pub fn cap(&self, inset: f32) -> Mesh {
    let face = self.face_radius();
    Mesh::rings(TOP_RINGS + 1, self.segments(), CREASE, |ring, angle| {
      let inward = ring as f32 / TOP_RINGS as f32;
      let ring = Ring {
        radius: face * inset * (1.0 - inward),
        height: None,
        shaped: 1.0,
        level: 1.0,
        rounded: 1.0 - inset * (1.0 - inward),
      };
      self.surface(ring, angle, true) + Vec3::Z * PAINT
    })
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

/// a fader's cap, described by its parts. it stands with its middle on the origin and slides
/// along y, and its stripe runs across it
#[derive(Clone, Copy, Debug)]
pub struct Fader {
  /// how wide the cap is, across the way it slides
  pub width: f32,
  /// how long it is, along the way it slides
  pub length: f32,
  pub height: f32,
  pub cap: Cap,
  /// how much narrower the cap is at its top than at its foot, as a share of its width
  pub taper: f32,
}

/// the shape of a fader's cap, seen from its side
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cap {
  /// hollowed along its length for a fingertip, the mixing console's cap. with `terraces`
  /// the hollow is cut in that many flat steps down each side instead of one curve
  Saddle { terraces: u32 },
  /// a block with slanted ends and a flat top, the dj mixer's cap, which is made much wider
  /// across than it is long
  Block,
  /// round over the top like half a barrel
  Barrel,
  /// a low flat body with one fin standing across its middle, a synthesizer slider's cap
  Fin,
  /// flat steps rising to the middle
  Stepped { steps: u32 },
  /// a ramp rising in a hollow curve from the front to a tall back, to push with a thumb
  Wedge,
}

impl Fader {
  /// the wide hollow cap of a mixing console
  pub fn console(width: f32, length: f32, height: f32) -> Self {
    Self {
      width,
      length,
      height,
      cap: Cap::Saddle { terraces: 0 },
      taper: 0.12,
    }
  }

  /// a console cap whose hollow is cut in steps
  pub fn terraced(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Saddle { terraces: 5 },
      ..Self::console(width, length, height)
    }
  }

  /// the slanted block of a dj mixer, to be made about twice as wide as it is long
  pub fn block(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Block,
      taper: 0.1,
      ..Self::console(width, length, height)
    }
  }

  /// a round-topped cap
  pub fn barrel(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Barrel,
      taper: 0.06,
      ..Self::console(width, length, height)
    }
  }

  /// a low cap with a fin, like a synthesizer's sliders
  pub fn fin(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Fin,
      taper: 0.0,
      ..Self::console(width, length, height)
    }
  }

  /// a cap of steps up to its middle
  pub fn stepped(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Stepped { steps: 3 },
      taper: 0.08,
      ..Self::console(width, length, height)
    }
  }

  /// a thumb ramp
  pub fn wedge(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Wedge,
      ..Self::console(width, length, height)
    }
  }

  /// the cap from its side, front to back: how far along and how high
  fn outline(&self) -> Vec<Vec2> {
    let (half, tall) = (self.length / 2.0, self.height);
    // a small cut across every top corner, so that none is a knife's edge
    let cut = (tall * 0.07).min(half * 0.1);
    // a symmetric cap, given how high its top is at a share of the way from the middle
    // out to an end, in so many straight stretches
    let mirrored = |stretches: usize, top: &dyn Fn(f32) -> f32| {
      let mut outline = vec![Vec2::new(-half, 0.0), Vec2::new(-half, top(1.0) - cut)];
      let reach = half - cut;
      for step in 0..=stretches * 2 {
        let out = step as f32 / stretches as f32 - 1.0;
        outline.push(Vec2::new(out * reach, top(out.abs())));
      }
      outline.extend([Vec2::new(half, top(1.0) - cut), Vec2::new(half, 0.0)]);
      outline
    };
    // steps as a top: each flat, with an upright riser to the next
    let flights = |steps: u32, top: &dyn Fn(f32) -> f32| {
      let mut outline = vec![Vec2::new(-half, 0.0)];
      let steps = steps.max(1) as i32;
      // a landing in the middle as wide as one tread and a half
      let tread = half / (steps as f32 + 0.75);
      for step in -steps..=steps {
        let height = top(step.abs() as f32 / steps as f32);
        let (from, to) = if step == 0 {
          (-tread * 0.75, tread * 0.75)
        } else {
          let inner = tread * (step.abs() as f32 - 0.25);
          let (near, far) = (
            inner * step.signum() as f32,
            (inner + tread) * step.signum() as f32,
          );
          (near.min(far), near.max(far))
        };
        outline.extend([Vec2::new(from, height), Vec2::new(to, height)]);
      }
      outline.push(Vec2::new(half, 0.0));
      outline
    };
    match self.cap {
      Cap::Saddle { terraces: 0 } => mirrored(12, &|out| tall * (0.58 + 0.42 * out * out)),
      Cap::Saddle { terraces } => flights(terraces, &|out| tall * (0.5 + 0.5 * out.powf(1.3))),
      Cap::Stepped { steps } => flights(steps, &|out| tall * (1.0 - 0.6 * out)),
      Cap::Barrel => mirrored(14, &|out| {
        // round over the top and steep at the ends, which stand a third of the way up
        tall * (0.34 + 0.66 * (1.0 - out.powi(4)).max(0.0).sqrt())
      }),
      Cap::Block => {
        let top = half * 0.6;
        vec![
          Vec2::new(-half, 0.0),
          Vec2::new(-half, tall * 0.12),
          Vec2::new(-top - cut, tall - cut),
          Vec2::new(-top, tall),
          Vec2::new(top, tall),
          Vec2::new(top + cut, tall - cut),
          Vec2::new(half, tall * 0.12),
          Vec2::new(half, 0.0),
        ]
      }
      Cap::Fin => {
        let (body, fin) = (tall * 0.42, half * 0.13);
        let mut outline = vec![
          Vec2::new(-half, 0.0),
          Vec2::new(-half, body - cut),
          Vec2::new(-half + cut, body),
        ];
        // the body sweeps up into the fin in a hollow curve
        const SWEEP: usize = 6;
        let sweep = |step: usize| {
          let along = step as f32 / SWEEP as f32;
          Vec2::new(
            fin + (half * 0.45 - fin) * (1.0 - along),
            body + (tall - body) * along * along,
          )
        };
        outline.extend(
          (0..=SWEEP)
            .map(sweep)
            .map(|point| Vec2::new(-point.x, point.y)),
        );
        outline.extend((0..=SWEEP).rev().map(sweep));
        outline.extend([
          Vec2::new(half - cut, body),
          Vec2::new(half, body - cut),
          Vec2::new(half, 0.0),
        ]);
        outline
      }
      Cap::Wedge => {
        let mut outline = vec![Vec2::new(-half, 0.0), Vec2::new(-half, tall * 0.3)];
        const RAMP: usize = 10;
        for step in 0..=RAMP {
          let along = step as f32 / RAMP as f32;
          outline.push(Vec2::new(
            -half + cut + (self.length - 2.0 * cut) * along,
            tall * (0.36 + 0.64 * along * along),
          ));
        }
        outline.extend([Vec2::new(half, tall - cut), Vec2::new(half, 0.0)]);
        outline
      }
    }
  }

  /// half the cap's width at a height
  fn half_width(&self, height: f32) -> f32 {
    self.width / 2.0 * (1.0 - self.taper * (height / self.height).clamp(0.0, 1.0))
  }

  /// how high the cap's top is over its middle, where its stripe crosses
  fn middle(&self) -> f32 {
    let outline = self.outline();
    (outline.windows(2))
      .find(|span| span[0].x <= 0.0 && span[1].x >= 0.0 && span[1].x > span[0].x)
      .map_or(self.height, |span| {
        span[0].y + (span[1].y - span[0].y) * (0.0 - span[0].x) / (span[1].x - span[0].x)
      })
  }

  /// the cap itself, without its stripe
  pub fn body(&self) -> Mesh {
    Mesh::extruded(&self.outline(), |height| self.half_width(height), CREASE)
  }

  /// the stripe painted across the cap's middle, over the top and down both sides
  pub fn line(&self) -> Mesh {
    let (top, half_stripe) = (self.middle(), (self.length * 0.035).max(0.8));
    let (crown, foot) = (self.half_width(top), self.half_width(0.0));
    let lift = PAINT * 2.0;
    let mut line = Mesh::default();
    line.quad(
      [
        Vec3::new(-crown, -half_stripe, top + lift),
        Vec3::new(crown, -half_stripe, top + lift),
        Vec3::new(crown, half_stripe, top + lift),
        Vec3::new(-crown, half_stripe, top + lift),
      ],
      Vec3::Z,
    );
    for side in [-1.0, 1.0] {
      let lean = (foot - crown) / top.max(f32::EPSILON);
      line.quad(
        [
          Vec3::new(side * (foot + lift), -half_stripe, 0.0),
          Vec3::new(side * (crown + lift), -half_stripe, top + lift),
          Vec3::new(side * (crown + lift), half_stripe, top + lift),
          Vec3::new(side * (foot + lift), half_stripe, 0.0),
        ],
        Vec3::new(side, 0.0, lean).normalize(),
      );
    }
    line
  }
}

/// where a fader's cap sits: on a track `travel` long centered on (x, y), at `value` of the
/// way from its bottom end, down the panel, to its top
pub fn fader_transform(x: f32, y: f32, travel: f32, value: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y + travel * (0.5 - value), 0.0))
}

/// the slot a fader runs in, as a dark strip lying on the panel: `travel` long between the
/// ends the cap's middle reaches, and `width` wide, with round ends
pub fn fader_slot(travel: f32, width: f32) -> Mesh {
  let half = width / 2.0;
  let strip = Mesh::cuboid(
    Vec3::new(-half, -travel / 2.0, 0.0),
    Vec3::new(half, travel / 2.0, 0.2),
  );
  let end = Mesh::lathe(&[[half, 0.0], [half, 0.2], [0.0, 0.2]], 16, true);
  [-0.5, 0.5].into_iter().fold(strip, |slot, side| {
    slot.merged(
      &end
        .clone()
        .transformed(Mat4::from_translation(Vec3::Y * travel * side)),
    )
  })
}

/// the scale beside a fader: `count` thin marks lying on the panel down the length of its
/// `travel`, on both sides of the slot from `inner` to `outer` away from it. every
/// `long_every`th mark is half as long again
pub fn fader_ticks(count: u32, travel: f32, inner: f32, outer: f32, long_every: u32) -> Mesh {
  let mut ticks = Mesh::default();
  for tick in 0..count {
    let along = if count > 1 {
      tick as f32 / (count - 1) as f32 - 0.5
    } else {
      0.0
    };
    let long = long_every > 0 && tick % long_every == 0;
    let reach = if long {
      outer + (outer - inner) * 0.5
    } else {
      outer
    };
    let half = if long { 1.0 } else { 0.6 };
    let y = along * travel;
    for side in [-1.0_f32, 1.0] {
      let (from, to) = (side * inner, side * reach);
      ticks = ticks.merged(&Mesh::cuboid(
        Vec3::new(from.min(to), y - half, 0.0),
        Vec3::new(from.max(to), y + half, 0.3),
      ));
    }
  }
  ticks
}

/// a push button, described by its parts: a pad with rounded corners that stands on the
/// origin
#[derive(Clone, Copy, Debug)]
pub struct Button {
  /// how wide the button is, along x
  pub width: f32,
  /// how long it is, along y
  pub length: f32,
  pub height: f32,
  /// how round its corners are, from zero for square to one for as round as its shorter
  /// side allows, which makes a square button a circle
  pub corner: f32,
  /// how far its top's edge is rolled over, as a share of its height
  pub roll: f32,
  /// how much higher its top is in the middle than at the rim, as a share of its height.
  /// below zero the top is hollow
  pub crown: f32,
}

impl Button {
  /// a square rubber pad with soft corners, like a drum pad or a clip launcher's
  pub fn pad(side: f32, height: f32) -> Self {
    Self {
      width: side,
      length: side,
      height,
      corner: 0.3,
      roll: 0.35,
      crown: 0.06,
    }
  }

  /// a round button with a crowned top
  pub fn round(diameter: f32, height: f32) -> Self {
    Self {
      corner: 1.0,
      roll: 0.4,
      crown: 0.15,
      ..Self::pad(diameter, height)
    }
  }

  /// a long button with round ends
  pub fn pill(width: f32, length: f32, height: f32) -> Self {
    Self {
      width,
      length,
      corner: 1.0,
      ..Self::pad(width, height)
    }
  }

  /// a low wide key with small corners and a nearly flat top, like a function button
  pub fn key(width: f32, length: f32, height: f32) -> Self {
    Self {
      width,
      length,
      corner: 0.25,
      roll: 0.25,
      crown: 0.02,
      ..Self::pad(width, height)
    }
  }

  /// the radius of the corners
  fn corner_radius(&self) -> f32 {
    self.width.min(self.length) / 2.0 * self.corner.clamp(0.0, 1.0)
  }

  /// how far out the button's outline is at an angle around its middle
  fn outline_at(&self, angle: f32) -> f32 {
    let (half, round) = (
      Vec2::new(self.width, self.length) / 2.0,
      self.corner_radius(),
    );
    let toward = Vec2::from_angle(angle).abs();
    // out to the nearer of the two straight sides
    let out = (half / toward.max(Vec2::splat(f32::EPSILON))).min_element();
    let center = half - Vec2::splat(round);
    let hit = toward * out;
    if hit.x <= center.x || hit.y <= center.y {
      return out;
    }
    // past where both sides have begun to curve, it is out to the corner's circle instead
    let along = toward.dot(center);
    along
      + (along * along - center.length_squared() + round * round)
        .max(0.0)
        .sqrt()
  }

  /// the rings of the button's surface as how far out each is, against its outline, and
  /// how high
  fn rings(&self) -> Vec<Vec2> {
    let roll = (self.height * self.roll).min(self.width.min(self.length) * 0.25);
    // the roll takes this much off the outline, by the shorter side
    let inset = roll / (self.width.min(self.length) / 2.0);
    let shoulder = self.height - roll;
    let mut rings = vec![Vec2::new(1.0, 0.0), Vec2::new(1.0, shoulder)];
    for step in 1..ROLL {
      let (up, inward) = (step as f32 / ROLL as f32 * FRAC_PI_2).sin_cos();
      rings.push(Vec2::new(
        1.0 - inset * (1.0 - inward),
        shoulder + roll * up,
      ));
    }
    let face = 1.0 - inset;
    for ring in 0..=TOP_RINGS {
      let out = 1.0 - ring as f32 / TOP_RINGS as f32;
      let crown = self.height * self.crown * (1.0 - out * out);
      rings.push(Vec2::new(face * out, self.height + crown));
    }
    rings
  }

  /// the button itself
  pub fn body(&self) -> Mesh {
    let rings = self.rings();
    Mesh::rings(rings.len(), 96, CREASE, |ring, angle| {
      let ring = rings[ring];
      (Vec2::from_angle(angle) * self.outline_at(angle) * ring.x).extend(ring.y)
    })
  }

  /// the gap around the button, as a dark strip lying on the panel `gap` wider than the
  /// button all around
  pub fn seat(&self, gap: f32) -> Mesh {
    let grown = Self {
      width: self.width + gap * 2.0,
      length: self.length + gap * 2.0,
      // the same corners, pushed out by the gap
      corner: ((self.corner_radius() + gap) / (self.width.min(self.length) / 2.0 + gap)).min(1.0),
      ..*self
    };
    let rings = [
      Vec2::new(1.0, 0.0),
      Vec2::new(1.0, 0.2),
      Vec2::new(0.0, 0.2),
    ];
    Mesh::rings(rings.len(), 96, CREASE, |ring, angle| {
      let ring = rings[ring];
      (Vec2::from_angle(angle) * grown.outline_at(angle) * ring.x).extend(ring.y)
    })
  }

  /// the light a lit button throws on what is around it, for a button at (x, y) lit in
  /// `color` at `level` from zero to one
  pub fn lamp(&self, x: f32, y: f32, color: Vec3, level: f32) -> crate::Lamp {
    crate::Lamp {
      at: Vec3::new(x, y, self.height * 1.5),
      // strong enough to show on a dark panel, which gives little of any light back
      color: color * level.clamp(0.0, 1.0) * 2.4,
      reach: self.width.max(self.length) * 1.5,
    }
  }

  /// how far from its middle a lit button's glow fades, for [`crate::Material::lit`]
  pub fn glow_radius(&self) -> f32 {
    self.width.min(self.length) * 0.55
  }
}

/// where a button sits, and how far down it is held: `pressed` runs from zero for up to one
/// for all the way down, which sinks it a quarter of the `height` it is told
pub fn button_transform(x: f32, y: f32, height: f32, pressed: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y, -height * 0.25 * pressed.clamp(0.0, 1.0)))
}

/// a push switch: a plunger standing in a collar on the panel, sprung so that it comes back
/// out by itself
#[derive(Clone, Copy, Debug)]
pub struct PushSwitch {
  /// the plunger, as tall as it stands out of the panel when it is out
  pub plunger: Button,
  /// how far the plunger goes in when it is pushed all the way
  pub travel: f32,
  /// how wide the collar around the plunger is
  pub collar: f32,
  /// how high the collar stands
  pub collar_height: f32,
}

/// what a push switch does when it is pushed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Push {
  /// each push turns it over: in and on, then out and off. it stays most of the way in
  /// while it is on
  Latching,
  /// it is on only while it is held, like any button
  Momentary,
}

impl PushSwitch {
  /// a square plunger in a square collar
  pub fn square(side: f32, height: f32) -> Self {
    Self {
      plunger: Button {
        corner: 0.18,
        roll: 0.12,
        crown: -0.04,
        ..Button::pad(side, height)
      },
      travel: height * 0.45,
      collar: side * 0.16,
      collar_height: height * 0.4,
    }
  }

  /// a round plunger in a round collar
  pub fn round(diameter: f32, height: f32) -> Self {
    Self {
      plunger: Button {
        roll: 0.14,
        crown: 0.05,
        ..Button::round(diameter, height)
      },
      ..Self::square(diameter, height)
    }
  }

  /// the plunger, out
  pub fn plunger(&self) -> Mesh {
    self.plunger.body()
  }

  /// the collar, a ring around the plunger with a hole for it to move in
  pub fn collar(&self) -> Mesh {
    // the plunger's outline pushed out by a distance, keeping its corners
    let grown = |by: f32| {
      let button = self.plunger;
      let shorter = button.width.min(button.length) / 2.0;
      Button {
        width: button.width + by * 2.0,
        length: button.length + by * 2.0,
        corner: ((button.corner_radius() + by) / (shorter + by)).min(1.0),
        ..button
      }
    };
    // a hair of room around the plunger
    let cut = self.collar_height.min(self.collar) * 0.25;
    band(
      grown(self.collar),
      grown(self.collar * 0.08),
      self.collar_height,
      cut,
    )
  }

  /// how far in the plunger is, from zero for out to one for all the way: all the way
  /// while it is `held`, and most of the way while a latching switch is `on`
  pub fn depth(&self, push: Push, on: bool, held: bool) -> f32 {
    match (held, push, on) {
      (true, ..) => 1.0,
      (false, Push::Latching, true) => 0.7,
      _ => 0.0,
    }
  }

  /// whether the switch is on after a push is let go, given whether it was on before. a
  /// momentary switch is never on once it is let go
  pub fn released(push: Push, was_on: bool) -> bool {
    push == Push::Latching && !was_on
  }

  /// where the plunger sits for a switch at (x, y) that is `depth` of the way in
  pub fn transform(&self, x: f32, y: f32, depth: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, -self.travel * depth.clamp(0.0, 1.0)))
  }
}

/// a level meter: a ladder of lights, green at the bottom, then yellow, then red, each the
/// end of its own light pipe set nearly flush in the panel. it runs up the panel from its
/// foot at the origin
#[derive(Clone, Copy, Debug)]
pub struct Meter {
  /// how many lights it has
  pub segments: u32,
  /// how wide each light is, across the meter
  pub width: f32,
  /// how long each light is, along the meter
  pub length: f32,
  /// the distance from one light to the next
  pub pitch: f32,
  /// how wide the dark gap around each window is
  pub margin: f32,
  /// how round each light is, from zero for square corners to one for round
  pub corner: f32,
  /// the share of the lights, from the bottom, where they turn yellow and then red
  pub yellow_from: f32,
  pub red_from: f32,
}

impl Meter {
  /// a ladder of `segments` lights, each `width` across, filling `length` of the panel
  pub fn ladder(segments: u32, width: f32, length: f32) -> Self {
    let pitch = length / segments.max(1) as f32;
    Self {
      segments,
      width,
      length: pitch * 0.72,
      pitch,
      margin: 0.8,
      corner: 0.16,
      yellow_from: 0.6,
      red_from: 0.85,
    }
  }

  /// a ladder of `segments` round lights, each `diameter` across, filling `length`
  pub fn dots(segments: u32, diameter: f32, length: f32) -> Self {
    Self {
      length: diameter,
      corner: 1.0,
      ..Self::ladder(segments, diameter, length)
    }
  }

  /// how far the meter runs up the panel
  pub fn reach(&self) -> f32 {
    self.pitch * self.segments as f32
  }

  /// the shape of one light pipe's end: a low flat rectangle with tight corners and a
  /// tight rolled edge, which is what catches a glint and where its light comes out
  fn lens(&self) -> Button {
    Button {
      corner: self.corner,
      roll: 0.5 + 0.4 * self.corner,
      crown: 0.03 + 0.2 * self.corner,
      ..Button::key(self.width, self.length, self.length.min(self.width) * 0.22)
    }
  }

  /// one light's window, lying on the origin. every light of the meter is this mesh
  pub fn segment(&self) -> Mesh {
    self.lens().body()
  }

  /// the dark gap around one window where it is set into the panel
  pub fn seat(&self) -> Mesh {
    self.lens().seat(self.margin)
  }

  /// the gaps around every window, as objects of the mesh at `seat`, for a meter whose foot
  /// is at (x, y)
  pub fn seats(&self, seat: usize, x: f32, y: f32) -> Vec<Object> {
    (0..self.segments)
      .map(|index| Object {
        mesh: seat,
        transform: self.place(x, y, index),
        material: crate::Material::matte(Vec3::splat(0.008)),
      })
      .collect()
  }

  /// where the light at `index` goes for a meter whose foot is at (x, y)
  fn place(&self, x: f32, y: f32, index: u32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y - self.pitch * (index as f32 + 0.5), 0.0))
  }

  /// the color the light at `index`, counted from the bottom, shows when it is lit
  pub fn color(&self, index: u32) -> Vec3 {
    let up = index as f32 / self.segments.max(1) as f32;
    if up >= self.red_from {
      Vec3::new(1.0, 0.10, 0.06)
    } else if up >= self.yellow_from {
      Vec3::new(1.0, 0.80, 0.05)
    } else {
      Vec3::new(0.08, 0.95, 0.2)
    }
  }

  /// the meter's lights as objects of the mesh at `segment`, for a meter whose foot is at
  /// (x, y) showing `level` from zero to one. the topmost lit light is as bright as the
  /// part of it the level covers. `peak`, if there is one, lights the one light at that
  /// level too, the way a meter holds its highest reading
  pub fn lights(
    &self,
    segment: usize,
    x: f32,
    y: f32,
    level: f32,
    peak: Option<f32>,
  ) -> Vec<Object> {
    let count = self.segments as f32;
    let held = peak
      .filter(|&peak| peak > 0.0)
      .map(|peak| ((peak.clamp(0.0, 1.0) * count).ceil() as u32).saturating_sub(1));
    (0..self.segments)
      .map(|index| {
        let color = self.color(index);
        let covered = (level.clamp(0.0, 1.0) * count - index as f32).clamp(0.0, 1.0);
        let lit = if held == Some(index) { 1.0 } else { covered };
        // each is the end of a light pipe: smoky and barely tinted when dark, and when lit
        // brightest over the light at the bottom of it and along its walls
        let dark = crate::Material::pipe();
        let dark = crate::Material {
          color: dark.color + color * 0.03,
          ..dark
        };
        Object {
          mesh: segment,
          transform: self.place(x, y, index),
          material: dark.lit(color, lit, self.width.min(self.length) * 0.5),
        }
      })
      .collect()
  }

  /// the light the meter throws on what is around it at a level: one lamp at the top of
  /// its lit part, in the color it has reached
  pub fn lamp(&self, x: f32, y: f32, level: f32) -> Option<crate::Lamp> {
    let lit = (level.clamp(0.0, 1.0) * self.segments as f32).ceil() as u32;
    let top = lit.checked_sub(1)?;
    Some(crate::Lamp {
      at: Vec3::new(x, y - self.pitch * (top as f32 + 0.5), self.width),
      color: self.color(top) * 0.6,
      reach: self.width * 3.0,
    })
  }
}

/// a raised band between two outlines, with a hole in its middle: up the outside, over a
/// corner cut `cut` wide, across the top `tall` above the panel, and down the inside
fn band(outer: Button, inner: Button, tall: f32, cut: f32) -> Mesh {
  let rings = [
    (outer, 0.0, 0.0),
    (outer, 0.0, tall - cut),
    (outer, cut, tall),
    (inner, -cut, tall),
    (inner, 0.0, tall - cut),
    (inner, 0.0, 0.0),
  ];
  Mesh::rings(rings.len(), 144, CREASE, |ring, angle| {
    let (outline, inset, height) = rings[ring];
    (Vec2::from_angle(angle) * (outline.outline_at(angle) - inset)).extend(height)
  })
}

/// a raised frame around a rectangle centered on the origin, like the bezel of a screen or
/// the rim of a panel: `width` by `length` on the outside, `border` wide, `height` tall,
/// with corners of radius `corner` on the outside
pub fn frame(width: f32, length: f32, border: f32, height: f32, corner: f32) -> Mesh {
  let shape = |width: f32, length: f32, corner: f32| Button {
    width,
    length,
    height,
    corner: (corner / (width.min(length) / 2.0)).clamp(0.0, 1.0),
    roll: 0.0,
    crown: 0.0,
  };
  band(
    shape(width, length, corner),
    shape(
      width - border * 2.0,
      length - border * 2.0,
      (corner - border).max(0.0),
    ),
    height,
    height.min(border) * 0.3,
  )
}

/// the strokes of a letter on a grid four wide and six tall: each run of digit pairs is a
/// line through those points, across and then up from the letter's foot
fn strokes(letter: char) -> &'static [&'static str] {
  match letter {
    'A' => &["002640", "1232"],
    'B' => &["00063645443303", "3342413000"],
    'C' => &["4536160501103041"],
    'D' => &["00063645413000"],
    'E' => &["46060040", "0333"],
    'F' => &["460600", "0333"],
    'G' => &["45361605011030414323"],
    'H' => &["0006", "4046", "0343"],
    'I' => &["2026", "1030", "1636"],
    'J' => &["0110304146"],
    'K' => &["0006", "460340"],
    'L' => &["060040"],
    'M' => &["0006234640"],
    'N' => &["00064046"],
    'O' | '0' => &["160501103041453616"],
    'P' => &["00063645443303"],
    'Q' => &["160501103041453616", "2240"],
    'R' => &["00063645443303", "2340"],
    'S' => &["453616050413334241301001"],
    'T' => &["0646", "2620"],
    'U' => &["060110304146"],
    'V' => &["062046"],
    'W' => &["0610233046"],
    'X' => &["0046", "0640"],
    'Y' => &["062346", "2320"],
    'Z' => &["06460040"],
    '1' => &["152620", "1030"],
    '2' => &["05163645440040"],
    '3' => &["05163645443323", "334241301001"],
    '4' => &["30360242"],
    '5' => &["460603334241301001"],
    '6' => &["4536160501103041423303"],
    '7' => &["064610"],
    '8' => &["1304051636454433", "1302011030414233"],
    '9' => &["0110304145361605041343"],
    '-' => &["1333"],
    '+' => &["1333", "2224"],
    '.' => &["2020"],
    ':' => &["2121", "2424"],
    '/' => &["0046"],
    '%' => &["0046", "0505", "4141"],
    '>' => &["052301"],
    '<' => &["450341"],
    _ => &[],
  }
}

/// lettering painted flat on the panel in a plain drawn face of capitals and figures,
/// centered on the origin: letters `height` tall with strokes `weight` wide. anything it
/// has no letter for is a space
pub fn legend(text: &str, height: f32, weight: f32) -> Mesh {
  let unit = height / 6.0;
  // each letter is four wide, and two more go to the gap after it
  let count = text.chars().count() as f32;
  let left = -(count * 6.0 - 2.0) * unit / 2.0;
  let mut legend = Mesh::default();
  for (index, letter) in text.chars().enumerate() {
    let origin = Vec2::new(left + index as f32 * 6.0 * unit, height / 2.0);
    for run in strokes(letter.to_ascii_uppercase()) {
      let points: Vec<Vec2> = (run.as_bytes().chunks(2))
        .map(|pair| {
          let (across, up) = (f32::from(pair[0] - b'0'), f32::from(pair[1] - b'0'));
          // up the letter is up the panel, which is toward negative y
          origin + Vec2::new(across, -up) * unit
        })
        .collect();
      for line in points.windows(2) {
        // a stroke with its ends squared off half its width past them, so that strokes
        // meet without a notch and one of no length is a dot
        let along = (line[1] - line[0]).try_normalize().unwrap_or(Vec2::X) * weight / 2.0;
        let across = along.perp();
        let (from, to) = (line[0] - along, line[1] + along);
        legend.quad(
          [from - across, from + across, to + across, to - across].map(|at| at.extend(PAINT)),
          Vec3::Z,
        );
      }
    }
  }
  legend
}

/// the dashed ring around a knob that old soft synths draw: `count` short bars lying on the
/// panel end to end around the origin at `radius`, over `sweep` radians the way
/// [`knob_transform`] turns. each is `width` across, and `fill` is how much of its share of
/// the ring a bar takes, the rest being the gap to the next
pub fn dashes(count: u32, sweep: f32, radius: f32, width: f32, fill: f32) -> Mesh {
  let mut ring = Mesh::default();
  let share = sweep / count.max(1) as f32;
  let half_length = radius * (share * fill.clamp(0.0, 1.0) / 2.0).tan();
  for dash in 0..count {
    let turn = (dash as f32 + 0.5) * share - sweep / 2.0;
    let bar = Mesh::cuboid(
      Vec3::new(-half_length, -radius - width / 2.0, 0.0),
      Vec3::new(half_length, -radius + width / 2.0, 0.3),
    );
    ring = ring.merged(&bar.transformed(Mat4::from_rotation_z(turn)));
  }
  ring
}

/// a small round light with a domed top, like the indicator beside a switch
pub fn led(diameter: f32) -> Mesh {
  Button {
    roll: 0.9,
    crown: 0.25,
    ..Button::round(diameter, diameter * 0.32)
  }
  .body()
}

/// the dark ring a small round light sits in, lying on the panel
pub fn led_seat(diameter: f32) -> Mesh {
  Button::round(diameter, 1.0).seat(diameter * 0.12)
}

/// what a small light of a color is made of at a level from zero for dark to one for lit:
/// dark tinted plastic that glows from its middle
pub fn led_material(color: Vec3, level: f32, diameter: f32) -> crate::Material {
  let dark = crate::Material::pipe();
  crate::Material {
    color: dark.color * 0.6 + color * 0.07,
    ..dark
  }
  .lit(color, level, diameter * 0.45)
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
      assert!(!positions.is_empty(), "{knob:?} has no line");
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
        assert!(Vec3::from(line.vertices[0].normal).is_normalized());
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
        // a grip that is not round shapes its top by rings and not by distance, which
        // `top_at` only comes near
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
  fn a_blade_reaches_its_nose_tail_and_sides() {
    let (nose, tail, half) = (1.0, 0.6, 0.3);
    assert!((blade_at(0.0, nose, tail, half) - nose).abs() < 1e-6);
    assert!((blade_at(PI, nose, tail, half) - tail).abs() < 1e-5);
    assert!((blade_at(FRAC_PI_2, nose, tail, half) - half).abs() < 1e-6);
    assert!((blade_at(-FRAC_PI_2, nose, tail, half) - half).abs() < 1e-6);
  }

  fn faders(width: f32, length: f32, height: f32) -> [Fader; 7] {
    [
      Fader::console(width, length, height),
      Fader::terraced(width, length, height),
      Fader::block(width, length, height),
      Fader::barrel(width, length, height),
      Fader::fin(width, length, height),
      Fader::stepped(width, length, height),
      Fader::wedge(width, length, height),
    ]
  }

  #[test]
  fn every_cap_fills_its_size_and_runs_front_to_back() {
    for fader in faders(30.0, 50.0, 24.0) {
      let outline = fader.outline();
      for span in outline.windows(2) {
        // a riser's two ends are the same place worked out twice, so not to the last bit
        assert!(
          span[1].x >= span[0].x - 1e-4,
          "{fader:?} turns around at {}",
          span[1]
        );
      }
      let body = fader.body();
      let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
      for vertex in &body.vertices {
        let position = Vec3::from(vertex.position);
        (min, max) = (min.min(position), max.max(position));
        assert!(
          Vec3::from(vertex.normal).is_normalized(),
          "{fader:?} at {position}"
        );
      }
      assert!(
        (min - Vec3::new(-15.0, -25.0, 0.0)).length() < 0.01,
        "{fader:?} from {min}"
      );
      assert!(
        (max - Vec3::new(15.0, 25.0, 24.0)).length() < 0.01,
        "{fader:?} to {max}"
      );
    }
  }

  #[test]
  fn a_stripe_crosses_the_cap_at_the_height_of_its_middle() {
    for fader in faders(30.0, 50.0, 24.0) {
      let top = fader.middle();
      assert!(
        top > 24.0 * 0.3 && top <= 24.0,
        "{fader:?} is {top} high in the middle"
      );
      let line = fader.line();
      let highest = (line.vertices.iter())
        .map(|vertex| vertex.position[2])
        .fold(f32::MIN, f32::max);
      assert!(
        (highest - top - PAINT * 2.0).abs() < 1e-4,
        "{fader:?} stripe at {highest}"
      );
      assert!(
        line
          .vertices
          .iter()
          .all(|vertex| vertex.position[1].abs() < 50.0 * 0.05)
      );
    }
    // a hollow cap is lowest there and a stepped one highest
    assert!(Fader::console(30.0, 50.0, 24.0).middle() < 24.0 * 0.6);
    assert_eq!(Fader::stepped(30.0, 50.0, 24.0).middle(), 24.0);
  }

  #[test]
  fn a_fader_slides_up_the_panel_as_its_value_rises() {
    let at = |value| fader_transform(100.0, 200.0, 80.0, value).transform_point3(Vec3::ZERO);
    assert_eq!(at(0.5), Vec3::new(100.0, 200.0, 0.0));
    assert_eq!(at(1.0).y, 160.0);
    assert_eq!(at(0.0).y, 240.0);
  }

  #[test]
  fn a_button_s_outline_is_a_rectangle_with_round_corners() {
    let key = Button::key(40.0, 20.0, 8.0);
    // straight out to the middle of each side
    assert!((key.outline_at(0.0) - 20.0).abs() < 1e-4);
    assert!((key.outline_at(FRAC_PI_2) - 10.0).abs() < 1e-4);
    // toward the rectangle's corner it stops on the corner's circle, short of the corner
    let corner = Vec2::new(20.0, 10.0);
    let round = key.corner_radius();
    let hit = corner.normalize() * key.outline_at(corner.to_angle());
    assert!(
      (hit.distance(corner - Vec2::splat(round)) - round).abs() < 1e-3,
      "{hit}"
    );
    assert!(hit.length() < corner.length());
    // a square as round as it can be is a circle
    let round = Button::round(30.0, 8.0);
    for step in 0..64 {
      let out = round.outline_at(step as f32 / 64.0 * TAU);
      assert!((out - 15.0).abs() < 1e-3, "{out}");
    }
  }

  #[test]
  fn every_button_fills_its_size() {
    let buttons = [
      Button::pad(30.0, 10.0),
      Button::round(30.0, 10.0),
      Button::pill(20.0, 44.0, 8.0),
      Button::key(44.0, 20.0, 6.0),
    ];
    for button in buttons {
      let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
      for vertex in &button.body().vertices {
        let position = Vec3::from(vertex.position);
        (min, max) = (min.min(position), max.max(position));
        assert!(
          Vec3::from(vertex.normal).is_normalized(),
          "{button:?} at {position}"
        );
      }
      let half = Vec2::new(button.width, button.length) / 2.0;
      assert!(
        (min.truncate() + half).length() < 0.05,
        "{button:?} from {min}"
      );
      assert!(
        (max.truncate() - half).length() < 0.05,
        "{button:?} to {max}"
      );
      assert_eq!(min.z, 0.0);
      let top = button.height * (1.0 + button.crown);
      assert!((max.z - top).abs() < 1e-3, "{button:?} is {} high", max.z);
      // its seat is that much wider all around and lies flat
      for vertex in &button.seat(2.0).vertices {
        let position = Vec3::from(vertex.position);
        assert!(position.x.abs() <= half.x + 2.01 && position.y.abs() <= half.y + 2.01);
        assert!(position.z <= 0.2);
      }
    }
  }

  #[test]
  fn a_held_button_sinks_and_a_dark_one_throws_no_light() {
    let down = button_transform(10.0, 20.0, 8.0, 1.0).transform_point3(Vec3::ZERO);
    assert_eq!(down, Vec3::new(10.0, 20.0, -2.0));
    let pad = Button::pad(30.0, 10.0);
    assert_eq!(pad.lamp(0.0, 0.0, Vec3::ONE, 0.0).color, Vec3::ZERO);
    assert!(pad.lamp(0.0, 0.0, Vec3::ONE, 1.0).reach > 30.0);
  }

  #[test]
  fn a_meter_lights_from_the_bottom_up_in_green_yellow_and_red() {
    let meter = Meter::ladder(10, 8.0, 100.0);
    assert_eq!(meter.reach(), 100.0);
    let lights = meter.lights(3, 50.0, 200.0, 0.45, None);
    assert_eq!(lights.len(), 10);
    let glows: Vec<f32> = lights.iter().map(|light| light.material.glow).collect();
    // four whole lights and half of the fifth
    assert_eq!(&glows[..4], &[1.0; 4]);
    assert!((glows[4] - 0.5).abs() < 1e-5);
    assert_eq!(&glows[5..], &[0.0; 5]);
    // the first is at the foot and each next one is further up the panel
    let ys: Vec<f32> = (lights.iter())
      .map(|light| light.transform.w_axis.y)
      .collect();
    assert_eq!(ys[0], 195.0);
    assert!(ys.windows(2).all(|pair| pair[1] == pair[0] - 10.0));
    // green, then yellow from six tenths up, then red from eighty-five hundredths
    let greenest = |index: u32| meter.color(index).y > meter.color(index).x;
    assert!(greenest(0) && greenest(5) && !greenest(6));
    assert!(meter.color(8).y > 0.5 && meter.color(9).y < 0.5);
    // a held peak lights one light above the level
    let held = meter.lights(3, 0.0, 0.0, 0.2, Some(0.9));
    assert_eq!(held[8].material.glow, 1.0);
    assert_eq!(held[7].material.glow, 0.0);
    // and a silent meter throws no light
    assert!(meter.lamp(0.0, 0.0, 0.0).is_none());
    assert!(
      meter
        .lamp(0.0, 0.0, 0.95)
        .is_some_and(|lamp| lamp.color.x > lamp.color.y)
    );
  }

  #[test]
  fn a_latching_switch_stays_in_and_a_momentary_one_comes_back() {
    let switch = PushSwitch::square(24.0, 14.0);
    // held, either is all the way in
    assert_eq!(switch.depth(Push::Latching, false, true), 1.0);
    assert_eq!(switch.depth(Push::Momentary, false, true), 1.0);
    // let go, a latching one that is on stays most of the way in and the rest are out
    let latched = switch.depth(Push::Latching, true, false);
    assert!(latched > 0.5 && latched < 1.0);
    assert_eq!(switch.depth(Push::Latching, false, false), 0.0);
    assert_eq!(switch.depth(Push::Momentary, true, false), 0.0);
    // each push turns a latching one over
    assert!(PushSwitch::released(Push::Latching, false));
    assert!(!PushSwitch::released(Push::Latching, true));
    assert!(!PushSwitch::released(Push::Momentary, false));
    // even all the way in, the plunger still stands over its collar
    let top = switch
      .transform(0.0, 0.0, 1.0)
      .transform_point3(Vec3::Z * switch.plunger.height);
    assert!(
      top.z > switch.collar_height,
      "{} under {}",
      top.z,
      switch.collar_height
    );
  }

  #[test]
  fn a_collar_is_a_ring_around_its_plunger() {
    for switch in [
      PushSwitch::square(24.0, 14.0),
      PushSwitch::round(24.0, 14.0),
    ] {
      let outline = |mesh: &Mesh| {
        let reach = |vertex: &crate::Vertex| Vec3::from(vertex.position).truncate().length();
        let reaches: Vec<f32> = mesh.vertices.iter().map(reach).collect();
        (
          reaches.iter().copied().fold(f32::MAX, f32::min),
          reaches.iter().copied().fold(f32::MIN, f32::max),
        )
      };
      let collar = switch.collar();
      let (inside, outside) = outline(&collar);
      // nothing of it is where the plunger's middle is, and it reaches past the plunger
      assert!(inside > 11.9, "{switch:?} collar comes in to {inside}");
      assert!(outside > outline(&switch.plunger()).1 + switch.collar * 0.9);
      for vertex in &collar.vertices {
        assert!(Vec3::from(vertex.normal).is_normalized());
        assert!((0.0..=switch.collar_height + 1e-4).contains(&vertex.position[2]));
      }
    }
  }

  #[test]
  fn a_legend_is_centered_and_as_tall_as_it_is_told() {
    let legend = legend("CUTOFF 12", 12.0, 1.5);
    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    for vertex in &legend.vertices {
      let position = Vec3::from(vertex.position);
      (min, max) = (min.min(position), max.max(position));
    }
    // nine letters of four and eight gaps of two between them, at two to a unit, and
    // then half a stroke more, or a little over that at the end of a slanted one
    let half = (9.0 * 6.0 - 2.0) * 2.0 / 2.0 + 0.75;
    assert!(
      (max.x - half).abs() < 0.35 && (min.x + half).abs() < 0.35,
      "{min} to {max}"
    );
    assert!(
      (max.y - 6.75).abs() < 0.35 && (min.y + 6.75).abs() < 0.35,
      "{min} to {max}"
    );
    // every letter and figure has strokes, each a whole number of points on its grid
    for letter in ('A'..='Z').chain('0'..='9') {
      let runs = strokes(letter);
      assert!(!runs.is_empty(), "{letter} is blank");
      for run in runs {
        assert!(run.len() >= 4 && run.len() % 2 == 0, "{letter}: {run}");
        for pair in run.as_bytes().chunks(2) {
          assert!(
            (b'0'..=b'4').contains(&pair[0]) && (b'0'..=b'6').contains(&pair[1]),
            "{letter}: {run}"
          );
        }
      }
    }
    assert!(super::legend(" ", 12.0, 1.5).vertices.is_empty());
  }

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

  #[test]
  fn a_dimple_is_the_lowest_place_on_its_top() {
    let knob = Knob::dimpled(40.0, 30.0);
    let (center, radius) = knob.dimple();
    assert!(knob.top_at(center) < 30.0 * 0.75);
    assert_eq!(knob.top_at(center + Vec2::X * radius * 1.01), 30.0);
    assert_eq!(knob.top_at(-center), 30.0);
  }
}
