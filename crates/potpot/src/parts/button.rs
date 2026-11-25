//! push buttons

use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Vec2, Vec3};

use super::outline::Outline;
use super::{CREASE, ROLL, TOP_RINGS};
use crate::mesh::Mesh;
use crate::scene::Lamp;

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

  /// the button's shape seen from above
  pub(super) fn outline(&self) -> Outline {
    let shorter = self.width.min(self.length) / 2.0;
    Outline::new(
      self.width,
      self.length,
      shorter * self.corner.clamp(0.0, 1.0),
    )
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
    let (rings, outline) = (self.rings(), self.outline());
    Mesh::rings(rings.len(), 96, CREASE, |ring, around| {
      let ring = rings[ring];
      (outline.point(around) * ring.x).extend(ring.y)
    })
  }

  /// the gap around the button, as a dark strip lying on the panel `gap` wider than the
  /// button all around
  pub fn seat(&self, gap: f32) -> Mesh {
    self.outline().grown(gap).strip()
  }

  /// the light a lit button throws on what is around it, for a button at (x, y) lit in
  /// `color` at `level` from zero to one
  pub fn lamp(&self, x: f32, y: f32, color: Vec3, level: f32) -> Lamp {
    Lamp {
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

#[cfg(test)]
mod tests {
  use super::*;

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
}
