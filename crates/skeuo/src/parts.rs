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

  #[test]
  fn a_dimple_is_the_lowest_place_on_its_top() {
    let knob = Knob::dimpled(40.0, 30.0);
    let (center, radius) = knob.dimple();
    assert!(knob.top_at(center) < 30.0 * 0.75);
    assert_eq!(knob.top_at(center + Vec2::X * radius * 1.01), 30.0);
    assert_eq!(knob.top_at(-center), 30.0);
  }
}
