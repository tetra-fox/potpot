//! Push buttons.

use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Vec2, Vec3};

use super::outline::Outline;
use super::{CREASE, ROLL, TOP_RINGS};
use crate::mesh::Mesh;
use crate::scene::Lamp;

/// A push button, described by its parts.
///
/// The button is a pad with rounded corners that stands on the origin.
#[derive(Clone, Copy, Debug)]
pub struct Button {
  /// The button's width along x, in points.
  pub width: f32,
  /// The button's length along y, in points.
  pub length: f32,
  /// The height of the rim of its top along z, in points.
  pub height: f32,
  /// The roundness of its corners, from zero to one.
  ///
  /// Zero is square and one is as round as the shorter side allows, which makes a square button a
  /// circle.
  pub corner: f32,
  /// The roll of its top's edge, as a share of its height.
  pub roll: f32,
  /// The rise of its top's middle over the rim, as a share of its height.
  ///
  /// Below zero the top is hollow.
  pub crown: f32,
}

impl Button {
  /// Creates a square rubber pad with soft corners.
  ///
  /// It is like a drum pad or a clip launcher's.
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

  /// Creates a round button with a crowned top.
  pub fn round(diameter: f32, height: f32) -> Self {
    Self {
      corner: 1.0,
      roll: 0.4,
      crown: 0.15,
      ..Self::pad(diameter, height)
    }
  }

  /// Creates a long button with round ends.
  pub fn pill(width: f32, length: f32, height: f32) -> Self {
    Self {
      width,
      length,
      corner: 1.0,
      ..Self::pad(width, height)
    }
  }

  /// Creates a low wide key with small corners.
  ///
  /// Its top is nearly flat, like a function button's.
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

  /// Returns the button's shape seen from above.
  pub(super) fn outline(&self) -> Outline {
    let shorter = self.width.min(self.length) / 2.0;
    Outline::new(
      self.width,
      self.length,
      shorter * self.corner.clamp(0.0, 1.0),
    )
  }

  /// Returns the rings of the button's surface.
  ///
  /// Each ring is how far out it is, against the button's outline, and how high.
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

  /// Returns the mesh of the button itself.
  pub fn body(&self) -> Mesh {
    let (rings, outline) = (self.rings(), self.outline());
    Mesh::rings(rings.len(), 96, CREASE, |ring, around| {
      let ring = rings[ring];
      (outline.point(around) * ring.x).extend(ring.y)
    })
  }

  /// Returns the gap around the button as a dark strip.
  ///
  /// The strip lies on the panel, `gap` wider than the button all around.
  pub fn seat(&self, gap: f32) -> Mesh {
    self.outline().grown(gap).strip()
  }

  /// Returns the light a lit button throws on what is around it.
  ///
  /// The button is at (`x`, `y`), lit in `color` at `level` from zero to one.
  pub fn lamp(&self, x: f32, y: f32, color: Vec3, level: f32) -> Lamp {
    Lamp {
      at: Vec3::new(x, y, self.height * 1.5),
      // about the light the lit top and walls give off toward a neighbour a button's width
      // away, so that what stands next to it is lit no brighter than the button itself
      color: color * level.clamp(0.0, 1.0) * 1.6,
      reach: self.width.max(self.length) * 1.5,
      radius: self.width.hypot(self.length) / 2.0,
    }
  }

  /// Returns how far from its middle a lit button's emission fades.
  ///
  /// The distance is the radius for [`Material::lit`](crate::Material::lit).
  pub fn emission_radius(&self) -> f32 {
    self.width.min(self.length) * 0.55
  }

  /// Returns the transform of a button at (`x`, `y`) held down.
  ///
  /// `pressed` runs from zero for up to one for all the way down, which sinks the button a quarter
  /// of its height.
  pub fn transform(&self, x: f32, y: f32, pressed: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(
      x,
      y,
      -self.height * 0.25 * pressed.clamp(0.0, 1.0),
    ))
  }
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
    let pad = Button::pad(30.0, 8.0);
    let down = pad.transform(10.0, 20.0, 1.0).transform_point3(Vec3::ZERO);
    assert_eq!(down, Vec3::new(10.0, 20.0, -2.0));
    assert_eq!(pad.lamp(0.0, 0.0, Vec3::ONE, 0.0).color, Vec3::ZERO);
    assert!(pad.lamp(0.0, 0.0, Vec3::ONE, 1.0).reach > 30.0);
  }
}
