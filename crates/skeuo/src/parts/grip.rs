//! the outline of a knob's side

use std::f32::consts::{FRAC_PI_2, PI};

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

impl Grip {
  /// how far out the grip is at an angle, against its plain radius. `level` is how far up the
  /// grip, from zero at its foot to one at its top, for the grips that change on the way
  pub(super) fn at(self, angle: f32, level: f32) -> f32 {
    match self {
      Grip::Smooth | Grip::Faceted(_) | Grip::Knurled(_) => 1.0,
      Grip::Ribbed { ribs, depth } => {
        // turned so that a rib is where the knob points
        let turned = (angle + FRAC_PI_2) * ribs as f32;
        1.0 - depth * (0.5 - 0.5 * turned.cos())
      }
      Grip::Fluted { flutes, depth } => {
        // a scoop takes up most of its share of the turn, and between two scoops a strip
        // of the round side is left standing. the middle of a strip is turned to where the
        // knob points, for its line
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

  /// how many points go around a knob with this grip
  pub(super) fn segments(self) -> u32 {
    match self {
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
}

#[cfg(test)]
mod tests {
  use super::*;

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
}
