//! fader caps and the slot they run in

use glam::{Mat4, Vec2, Vec3};

use super::{CREASE, PAINT};
use crate::mesh::Mesh;

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
}
