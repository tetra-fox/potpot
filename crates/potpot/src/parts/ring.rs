//! Rings of lights around a knob.

use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::KNOB_SWEEP;
use super::button::Button;
use crate::material::Material;
use crate::mesh::Mesh;
use crate::scene::Object;

/// A ring of lights around a knob, each the end of a light pipe.
///
/// The lights lie end to end around the origin over the sweep that
/// [`knob_transform`](super::knob_transform) turns a knob through, so each light stands for a
/// part of the knob's turn. [`LedRing::lights`] lights the run between two places of the turn.
/// A run from the start shows a level, a run from the middle a level either way, and a run from
/// where the knob points how far another value is from the knob's.
#[derive(Clone, Copy, Debug)]
pub struct LedRing {
  /// The count of lights.
  pub segments: u32,
  /// The angle the lights spread over, in radians.
  pub sweep: f32,
  /// The distance from the origin to the middle of each light, in points.
  pub radius: f32,
  /// The width of each light out from the origin, in points.
  pub width: f32,
  /// The share of its part of the ring a light takes.
  ///
  /// The gap to the next light takes what is left.
  pub fill: f32,
  /// The width of the dark gap around each light, in points.
  pub margin: f32,
}

impl LedRing {
  /// Creates a ring of lights over the turn of a knob.
  ///
  /// The ring has `segments` lights over [`KNOB_SWEEP`], with their middles `radius` from the
  /// origin, each `width` across.
  pub fn new(segments: u32, radius: f32, width: f32) -> Self {
    Self {
      segments,
      sweep: KNOB_SWEEP,
      radius,
      width,
      fill: 0.7,
      margin: 0.6,
    }
  }

  /// Returns the shape of one light pipe's end, lying across the ring.
  fn lens(&self) -> Button {
    let share = self.sweep / self.segments.max(1) as f32;
    let along = 2.0 * self.radius * (share * self.fill.clamp(0.0, 1.0) / 2.0).sin();
    Button {
      corner: 0.3,
      roll: 0.6,
      crown: 0.1,
      ..Button::key(along, self.width, self.width.min(along) * 0.22)
    }
  }

  /// Returns one light, lying on the origin.
  ///
  /// Every light of the ring is this mesh.
  pub fn segment(&self) -> Mesh {
    self.lens().body()
  }

  /// Returns the dark gap around one light set into the panel.
  pub fn seat(&self) -> Mesh {
    self.lens().seat(self.margin)
  }

  /// Returns the transform of the light at `index` of a ring around (`x`, `y`).
  fn place(&self, x: f32, y: f32, index: u32) -> Mat4 {
    let turn = (index as f32 + 0.5) / self.segments as f32;
    Mat4::from_translation(Vec3::new(x, y, 0.0))
      * Mat4::from_rotation_z((turn - 0.5) * self.sweep)
      * Mat4::from_translation(Vec3::new(0.0, -self.radius, 0.0))
  }

  /// Returns the gaps around every light as objects.
  ///
  /// Each [`Object`] has the mesh [`LedRing::seat`] makes, for a ring around (`x`, `y`).
  pub fn seats(&self, seat: &Arc<Mesh>, x: f32, y: f32) -> impl Iterator<Item = Object> {
    (0..self.segments).map(move |index| Object {
      mesh: Arc::clone(seat),
      transform: self.place(x, y, index),
      material: Material::matte(Vec3::splat(0.008)),
    })
  }

  /// Returns the ring's lights as objects.
  ///
  /// Each [`Object`] has the mesh [`LedRing::segment`] makes, for a ring around (`x`, `y`). The
  /// lights are lit in `color` over the run between the two places of `run`, in either order.
  /// Each place is a part of the sweep, from zero at its start to one at its end, and one past
  /// either end is at that end. A light the run covers in part is as bright as the part it
  /// covers, and a run of no length lights nothing.
  pub fn lights(
    &self,
    segment: &Arc<Mesh>,
    x: f32,
    y: f32,
    run: [f32; 2],
    color: Vec3,
  ) -> impl Iterator<Item = Object> {
    let count = self.segments as f32;
    let [from, to] = run.map(|place| place.clamp(0.0, 1.0) * count);
    let (low, high) = (from.min(to), from.max(to));
    (0..self.segments).map(move |index| {
      let start = index as f32;
      let lit = (high.min(start + 1.0) - low.max(start)).max(0.0);
      // smoky and barely tinted when dark, and when lit brightest over the light under it
      let dark = Material::pipe();
      let dark = Material {
        color: dark.color + color * 0.03,
        ..dark
      };
      Object {
        mesh: Arc::clone(segment),
        transform: self.place(x, y, index),
        material: dark.lit(color, lit, self.width * 0.6),
      }
    })
  }
}

#[cfg(test)]
mod tests {
  use std::f32::consts::FRAC_PI_2;

  use super::*;

  #[test]
  fn a_ring_lights_the_run_between_two_places_either_way_round() {
    let ring = LedRing {
      sweep: 2.0 * FRAC_PI_2,
      ..LedRing::new(10, 40.0, 4.0)
    };
    let segment = Arc::new(ring.segment());
    let glows = |run| -> Vec<f32> {
      (ring.lights(&segment, 0.0, 0.0, run, Vec3::ONE))
        .map(|light| light.material.glow)
        .collect()
    };
    // from the middle down to a fifth, which is three whole lights
    let down = glows([0.5, 0.2]);
    assert_eq!(down, glows([0.2, 0.5]));
    assert_eq!(&down[..2], &[0.0; 2]);
    assert!((down[2] - 1.0).abs() < 1e-5 && (down[4] - 1.0).abs() < 1e-5);
    assert_eq!(&down[5..], &[0.0; 5]);
    // and down to a quarter, which covers half the third
    let part = glows([0.5, 0.25]);
    assert!((part[2] - 0.5).abs() < 1e-5, "{}", part[2]);
    // a run past an end stops there, and a run of no length lights nothing
    assert_eq!(glows([0.95, 1.6])[9], 0.5);
    assert!(glows([0.37, 0.37]).iter().all(|&glow| glow == 0.0));
    // the first light is a little way into the sweep from its start, which is to the left
    // of the middle here, and every one is the radius out
    let lights: Vec<Object> = ring
      .lights(&segment, 0.0, 0.0, [0.0, 0.0], Vec3::ONE)
      .collect();
    let first = lights[0].transform.transform_point3(Vec3::ZERO);
    let angle = (0.05 - 0.5) * ring.sweep;
    assert!((first - Vec3::new(40.0 * angle.sin(), -40.0 * angle.cos(), 0.0)).length() < 1e-4);
    assert!(first.x < 0.0);
    for light in &lights {
      let middle = light.transform.transform_point3(Vec3::ZERO);
      assert!((middle.length() - 40.0).abs() < 1e-4);
    }
  }
}
