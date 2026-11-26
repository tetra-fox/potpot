//! The rounded rectangle that buttons and their seats are cut to.
//!
//! Collars and frames are cut to it too.

use std::f32::consts::FRAC_PI_2;

use glam::Vec2;

use super::CREASE;
use crate::mesh::Mesh;

/// The height of a strip's top over the panel.
pub(super) const STRIP: f32 = 0.2;

/// A rectangle with round corners, centered on the origin.
#[derive(Clone, Copy, Debug)]
pub(super) struct Outline {
  /// The width along x, in points.
  width: f32,
  /// The length along y, in points.
  length: f32,
  /// The radius of its corners in points, at most half its shorter side.
  round: f32,
}

impl Outline {
  /// Creates an outline with corners of radius `corner`.
  ///
  /// `corner` is kept to what the shorter side has room for.
  pub(super) fn new(width: f32, length: f32, corner: f32) -> Self {
    Self {
      width,
      length,
      round: corner.min(width.min(length) / 2.0).max(0.0),
    }
  }

  /// Returns how far out the outline is at an angle.
  ///
  /// The angle is around the outline's middle.
  pub(super) fn at(&self, angle: f32) -> f32 {
    let half = Vec2::new(self.width, self.length) / 2.0;
    let toward = Vec2::from_angle(angle).abs();
    // out to the nearer of the two straight sides
    let out = (half / toward.max(Vec2::splat(f32::EPSILON))).min_element();
    let center = half - Vec2::splat(self.round);
    let hit = toward * out;
    if hit.x <= center.x || hit.y <= center.y {
      return out;
    }
    // past where both sides have begun to curve, it is out to the corner's circle instead
    let along = toward.dot(center);
    along
      + (along * along - center.length_squared() + self.round * self.round)
        .max(0.0)
        .sqrt()
  }

  /// Returns the angle a step around the outline lands on.
  ///
  /// The angle is around the middle, for steps that are even in `around`. Steps even in angle would
  /// put next to none on the corners of a long outline, which are a sliver of the turn as its
  /// middle sees them. Each stretch gets steps by its length, and a corner by several times its
  /// own.
  pub(super) fn spread(&self, around: f32) -> f32 {
    /// The multiple of its length a corner counts for.
    const CORNERS: f32 = 6.0;
    let half = Vec2::new(self.width, self.length) / 2.0;
    let straight = half - Vec2::splat(self.round);
    // the first quarter of the turn runs up the side at x, around the corner and along
    // the side at y, and the quarter after it is the same thing backwards
    let quarter = (around / FRAC_PI_2).floor();
    let backwards = quarter.rem_euclid(2.0) == 1.0;
    let (first, last) = if backwards {
      (straight.x, straight.y)
    } else {
      (straight.y, straight.x)
    };
    let corner = self.round * FRAC_PI_2 * CORNERS;
    let all = first + corner + last;
    let within = around / FRAC_PI_2 - quarter;
    if all <= f32::EPSILON {
      return around;
    }
    // where the corner begins and ends, as the middle sees it from the side it comes to
    // first
    let begins = if backwards {
      straight.x.atan2(half.y)
    } else {
      straight.y.atan2(half.x)
    };
    let ends = FRAC_PI_2
      - if backwards {
        straight.y.atan2(half.x)
      } else {
        straight.x.atan2(half.y)
      };
    let (to_corner, past_corner) = (first / all, (first + corner) / all);
    let angle = if within < to_corner {
      begins * within / to_corner
    } else if within < past_corner {
      begins + (ends - begins) * (within - to_corner) / (past_corner - to_corner)
    } else {
      ends + (FRAC_PI_2 - ends) * (within - past_corner) / (1.0 - past_corner)
    };
    quarter * FRAC_PI_2 + angle
  }

  /// Returns the point of the outline a step lands on.
  ///
  /// Steps are even in `around`.
  pub(super) fn point(&self, around: f32) -> Vec2 {
    let angle = self.spread(around);
    Vec2::from_angle(angle) * self.at(angle)
  }

  /// Returns the outline pushed out by a distance all around.
  ///
  /// Its corners are rounder by as much.
  pub(super) fn grown(&self, by: f32) -> Self {
    Self::new(
      self.width + by * 2.0,
      self.length + by * 2.0,
      self.round + by,
    )
  }

  /// Returns the outline filled in as a low strip on the panel.
  ///
  /// The strip is the dark gap around something set into the panel.
  pub(super) fn strip(&self) -> Mesh {
    let rings = [
      Vec2::new(1.0, 0.0),
      Vec2::new(1.0, STRIP),
      Vec2::new(0.0, STRIP),
    ];
    Mesh::rings(rings.len(), 96, CREASE, |ring, around| {
      let ring = rings[ring];
      (self.point(around) * ring.x).extend(ring.y)
    })
  }
}

/// Returns a raised band between two outlines.
///
/// The band has a hole in its middle and stands `tall` above the panel. The outer and the inner
/// corner of its top are cut as wide as they are tall, by the two of `cuts` in that order.
pub(super) fn band(outer: Outline, inner: Outline, tall: f32, [outward, inward]: [f32; 2]) -> Mesh {
  // each cut's edge on the top is its outline drawn in or out by the cut, which keeps the cut
  // as wide all along a side
  let rings = [
    (outer, 0.0),
    (outer, tall - outward),
    (outer.grown(-outward), tall),
    (inner.grown(inward), tall),
    (inner, tall - inward),
    (inner, 0.0),
  ];
  Mesh::rings(rings.len(), 144, CREASE, |ring, around| {
    let (outline, height) = rings[ring];
    // every outline by the outer one's steps, so that the band runs straight across
    let angle = outer.spread(around);
    (Vec2::from_angle(angle) * outline.at(angle)).extend(height)
  })
}

#[cfg(test)]
mod tests {
  use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};

  use super::*;

  #[test]
  fn an_outline_is_a_rectangle_with_round_corners() {
    let key = Outline::new(40.0, 20.0, 2.5);
    // straight out to the middle of each side
    assert!((key.at(0.0) - 20.0).abs() < 1e-4);
    assert!((key.at(FRAC_PI_2) - 10.0).abs() < 1e-4);
    // toward the rectangle's corner it stops on the corner's circle, short of the corner
    let corner = Vec2::new(20.0, 10.0);
    let hit = corner.normalize() * key.at(corner.to_angle());
    assert!(
      (hit.distance(corner - Vec2::splat(2.5)) - 2.5).abs() < 1e-3,
      "{hit}"
    );
    assert!(hit.length() < corner.length());
    // a square as round as it can be is a circle, however much rounder it is asked to be
    let round = Outline::new(30.0, 30.0, 40.0);
    for step in 0..64 {
      let out = round.at(step as f32 / 64.0 * TAU);
      assert!((out - 15.0).abs() < 1e-3, "{out}");
    }
  }

  #[test]
  fn steps_around_an_outline_crowd_into_its_corners_and_miss_none_of_the_turn() {
    // a long frame's corner is under three degrees of the turn as its middle sees it
    let rim = Outline::new(500.0, 250.0, 10.0);
    let steps = 144;
    let angles: Vec<f32> = (0..=steps)
      .map(|step| rim.spread(step as f32 / steps as f32 * TAU))
      .collect();
    assert!(angles[0].abs() < 1e-5 && (angles[steps] - TAU).abs() < 1e-4);
    assert!(
      angles.windows(2).all(|pair| pair[1] > pair[0]),
      "{angles:?}"
    );
    for quarter in 0..4 {
      // the corner's own middle, which is where the diagonal of its square points
      let toward = Vec2::new(240.0, 115.0) + Vec2::splat(10.0 * FRAC_1_SQRT_2);
      let mirrored = match quarter {
        0 => toward,
        1 => Vec2::new(-toward.x, toward.y),
        2 => -toward,
        _ => Vec2::new(toward.x, -toward.y),
      };
      let middle = mirrored.to_angle().rem_euclid(TAU);
      let near = (angles.iter()).filter(|angle| (**angle - middle).abs() < 0.03);
      assert!(
        near.count() >= 5,
        "five steps or more lie near the middle of corner {quarter}"
      );
    }
    // a circle is all corner, and its steps stay even
    let round = Outline::new(30.0, 30.0, 15.0);
    for step in 0..64 {
      let around = step as f32 / 64.0 * TAU;
      assert!((round.spread(around) - around).abs() < 1e-4, "{around}");
    }
  }

  #[test]
  fn a_grown_outline_is_as_much_further_out_all_around() {
    let key = Outline::new(40.0, 20.0, 2.5);
    let grown = key.grown(3.0);
    assert!((grown.at(0.0) - 23.0).abs() < 1e-4);
    assert!((grown.at(FRAC_PI_2) - 13.0).abs() < 1e-4);
    // its corners turn around the same middles, on circles wider by as much
    let middle = Vec2::new(20.0, 10.0) - Vec2::splat(2.5);
    for step in 0..=8 {
      let toward = Vec2::from_angle(step as f32 / 8.0 * FRAC_PI_2);
      let angle = (middle + toward * 5.5).to_angle();
      let hit = Vec2::from_angle(angle) * grown.at(angle);
      assert!((hit.distance(middle) - 5.5).abs() < 1e-3, "{hit}");
    }
  }
}
