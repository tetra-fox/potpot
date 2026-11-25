//! Fader caps and the slot they run in.

use glam::{Mat4, Vec2, Vec3};

use super::{CREASE, PAINT};
use crate::mesh::Mesh;

/// A fader's cap, described by its parts.
///
/// The cap stands with its middle on the origin and slides along y, and its stripe runs across it.
#[derive(Clone, Copy, Debug)]
pub struct Fader {
  /// The cap's width across the way it slides, in points.
  pub width: f32,
  /// The cap's length along the way it slides, in points.
  pub length: f32,
  /// The height of its highest point along z, in points.
  pub height: f32,
  /// The cap's shape from the side.
  pub cap: Cap,
  /// The cap's narrowing from foot to top, as a share of its width.
  pub taper: f32,
}

/// The shape of a fader's cap, seen from its side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cap {
  /// A cap hollowed along its length for a fingertip.
  ///
  /// A mixing console has this cap. With `terraces` above zero the hollow is cut in that many flat
  /// steps down each side.
  Saddle {
    /// The count of steps down each side, or zero for one curve.
    terraces: u32,
  },
  /// A block with slanted ends and a flat top.
  ///
  /// A DJ mixer has this cap, which is made much wider across than it is long.
  Block,
  /// A cap round over the top like half a barrel.
  Barrel,
  /// A low flat body with one fin standing across its middle.
  ///
  /// A synthesizer's slider has this cap.
  Fin,
  /// Flat steps rising to the middle.
  Stepped {
    /// The count of steps to each side of the middle.
    steps: u32,
  },
  /// A ramp rising in a hollow curve from the front to a tall back.
  ///
  /// A thumb pushes it.
  Wedge,
}

impl Fader {
  /// Creates the wide hollow cap of a mixing console.
  pub fn console(width: f32, length: f32, height: f32) -> Self {
    Self {
      width,
      length,
      height,
      cap: Cap::Saddle { terraces: 0 },
      taper: 0.12,
    }
  }

  /// Creates a console cap whose hollow is cut in steps.
  pub fn terraced(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Saddle { terraces: 5 },
      ..Self::console(width, length, height)
    }
  }

  /// Creates the slanted block of a DJ mixer.
  ///
  /// The cap is to be made about twice as wide as it is long.
  pub fn block(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Block,
      taper: 0.1,
      ..Self::console(width, length, height)
    }
  }

  /// Creates a round-topped cap.
  pub fn barrel(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Barrel,
      taper: 0.06,
      ..Self::console(width, length, height)
    }
  }

  /// Creates a low cap with a fin, like a synthesizer's sliders.
  pub fn fin(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Fin,
      taper: 0.0,
      ..Self::console(width, length, height)
    }
  }

  /// Creates a cap of steps up to its middle.
  pub fn stepped(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Stepped { steps: 3 },
      taper: 0.08,
      ..Self::console(width, length, height)
    }
  }

  /// Creates a thumb ramp.
  pub fn wedge(width: f32, length: f32, height: f32) -> Self {
    Self {
      cap: Cap::Wedge,
      ..Self::console(width, length, height)
    }
  }

  /// Returns the cap from its side, front to back.
  ///
  /// Each point is how far along and how high.
  fn outline(&self) -> Vec<Vec2> {
    let (half, tall) = (self.length / 2.0, self.height);
    // a small cut across the corner at each end of a top that has no steps, so that it is
    // not a knife's edge
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

  /// Returns half the cap's width at a height.
  fn half_width(&self, height: f32) -> f32 {
    self.width / 2.0 * (1.0 - self.taper * (height / self.height).clamp(0.0, 1.0))
  }

  /// Returns the height of the cap's top `along` from its middle.
  fn top_at(&self, along: f32) -> f32 {
    let outline = self.outline();
    (outline.windows(2))
      .find(|span| span[0].x <= along && span[1].x >= along && span[1].x > span[0].x)
      .map_or(self.height, |span| {
        span[0].y + (span[1].y - span[0].y) * (along - span[0].x) / (span[1].x - span[0].x)
      })
  }

  /// Returns the mesh of the cap itself, without its stripe.
  pub fn body(&self) -> Mesh {
    Mesh::extruded(&self.outline(), |height| self.half_width(height), CREASE)
  }

  /// Returns the stripe painted across the cap's middle.
  ///
  /// The stripe runs over the top and down both sides, sloped the way the top is under it.
  pub fn line(&self) -> Mesh {
    let half_stripe = (self.length * 0.035).max(0.8);
    let (front, back) = (self.top_at(-half_stripe), self.top_at(half_stripe));
    let lift = PAINT * 2.0;
    // the stripe's front and back corners on one side of the cap, `out` past its edge, each
    // as high as the top is under it
    let corners = |side: f32, out: f32| {
      [(-half_stripe, front), (half_stripe, back)]
        .map(|(along, top)| Vec3::new(side * (self.half_width(top) + out), along, top + lift))
    };
    let mut line = Mesh::default();
    let ([left_front, left_back], [right_front, right_back]) =
      (corners(-1.0, 0.0), corners(1.0, 0.0));
    line.quad(
      [left_front, right_front, right_back, left_back],
      Vec3::new(0.0, front - back, half_stripe * 2.0).normalize(),
    );
    let foot = self.half_width(0.0);
    // a side leans in by the same slope at any height
    let lean = (foot - self.half_width(front)) / front.max(f32::EPSILON);
    for side in [-1.0, 1.0] {
      let [top_front, top_back] = corners(side, lift);
      line.quad(
        [
          Vec3::new(side * (foot + lift), -half_stripe, 0.0),
          top_front,
          top_back,
          Vec3::new(side * (foot + lift), half_stripe, 0.0),
        ],
        Vec3::new(side, 0.0, lean).normalize(),
      );
    }
    line
  }
}

/// Returns the transform of a fader's cap on its track.
///
/// The track is `travel` long and centered on (`x`, `y`). The cap sits at `value` of the way from
/// the track's bottom end, down the panel, to its top.
pub fn fader_transform(x: f32, y: f32, travel: f32, value: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y + travel * (0.5 - value), 0.0))
}

/// Returns the slot a fader runs in, as a dark strip.
///
/// The strip lies on the panel, `travel` long between the ends the cap's middle reaches and `width`
/// wide, with round ends.
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

#[cfg(test)]
mod tests {
  use super::*;

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
          "{fader:?} runs front to back at {}",
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
  fn a_stripe_crosses_the_middle_of_the_cap_and_lies_on_its_top() {
    for fader in faders(30.0, 50.0, 24.0) {
      let top = fader.top_at(0.0);
      assert!(
        top > 24.0 * 0.3 && top <= 24.0,
        "{fader:?} is {top} high in the middle"
      );
      let line = fader.line();
      assert!(
        line
          .vertices
          .iter()
          .all(|vertex| vertex.position[1].abs() < 50.0 * 0.05)
      );
      // its top runs straight from its front edge to its back one, just off the cap's top
      // at both and nowhere under it between them
      let edge = |back: bool| {
        (line.vertices.iter())
          .filter(|vertex| (vertex.position[1] > 0.0) == back)
          .map(|vertex| Vec2::new(vertex.position[1], vertex.position[2]))
          .fold(
            Vec2::MIN,
            |highest, at| {
              if at.y > highest.y { at } else { highest }
            },
          )
      };
      let (front, back) = (edge(false), edge(true));
      for at in [front, back] {
        assert!(
          (at.y - fader.top_at(at.x) - PAINT * 2.0).abs() < 1e-4,
          "{fader:?} stripe at {at}"
        );
      }
      for step in 0..=16 {
        let at = front.lerp(back, step as f32 / 16.0);
        assert!(
          at.y >= fader.top_at(at.x),
          "{fader:?} stripe at {at} is under {}",
          fader.top_at(at.x)
        );
      }
    }
    // a hollow cap is lowest in the middle and a stepped one highest
    assert!(Fader::console(30.0, 50.0, 24.0).top_at(0.0) < 24.0 * 0.6);
    assert_eq!(Fader::stepped(30.0, 50.0, 24.0).top_at(0.0), 24.0);
  }

  #[test]
  fn a_fader_slides_up_the_panel_as_its_value_rises() {
    let at = |value| fader_transform(100.0, 200.0, 80.0, value).transform_point3(Vec3::ZERO);
    assert_eq!(at(0.5), Vec3::new(100.0, 200.0, 0.0));
    assert_eq!(at(1.0).y, 160.0);
    assert_eq!(at(0.0).y, 240.0);
  }
}
