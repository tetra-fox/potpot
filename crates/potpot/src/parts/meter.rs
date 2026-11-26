//! Level meters, each a ladder of light pipe ends.

use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::button::Button;
use super::{pipe_end, pipe_seat};
use crate::mesh::Mesh;
use crate::scene::{Lamp, Object};

/// A level meter made of a ladder of lights.
///
/// The lights are in three colors from the bottom, green, yellow and red unless set otherwise, each
/// the end of its own light pipe set nearly flush in the panel. The meter runs up the panel from
/// its foot at the origin.
#[derive(Clone, Copy, Debug)]
pub struct Meter {
  /// The count of lights.
  pub segments: u32,
  /// The width of each light across the meter, in points.
  pub width: f32,
  /// The length of each light along the meter, in points.
  pub length: f32,
  /// The distance from one light to the next, in points.
  pub pitch: f32,
  /// The width of the dark gap around each window, in points.
  pub margin: f32,
  /// The roundness of each light, from zero to one.
  ///
  /// Zero is square corners and one is round.
  pub corner: f32,
  /// The share of the lights, from the bottom, where they turn the second color.
  pub yellow_from: f32,
  /// The share of the lights, from the bottom, where they turn the third color.
  pub red_from: f32,
  /// The colors of the lights, from the bottom.
  pub colors: [Vec3; 3],
}

impl Meter {
  /// Creates a ladder of lights filling a length of the panel.
  ///
  /// The ladder has `segments` lights, each `width` across, and fills `length`.
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
      colors: [
        Vec3::new(0.08, 0.95, 0.2),
        Vec3::new(1.0, 0.80, 0.05),
        Vec3::new(1.0, 0.10, 0.06),
      ],
    }
  }

  /// Creates a ladder of round lights filling a length.
  ///
  /// The ladder has `segments` lights, each `diameter` across, and fills `length`.
  pub fn dots(segments: u32, diameter: f32, length: f32) -> Self {
    Self {
      length: diameter,
      corner: 1.0,
      ..Self::ladder(segments, diameter, length)
    }
  }

  /// Returns the shape of one light pipe's end.
  ///
  /// The end is a low pad with corners as round as [`Meter::corner`] and a tight rolled edge, which
  /// is what catches a glint and where its light comes out.
  fn lens(&self) -> Button {
    Button {
      corner: self.corner,
      roll: 0.5 + 0.4 * self.corner,
      crown: 0.03 + 0.2 * self.corner,
      ..Button::key(self.width, self.length, self.length.min(self.width) * 0.22)
    }
  }

  /// Returns one light's window, lying on the origin.
  ///
  /// Every light of the meter is this mesh.
  pub fn segment(&self) -> Mesh {
    self.lens().body()
  }

  /// Returns the dark gap around one window set into the panel.
  pub fn seat(&self) -> Mesh {
    self.lens().seat(self.margin)
  }

  /// Returns the gaps around every window as objects.
  ///
  /// Each [`Object`] has the mesh [`Meter::seat`] makes, for a meter whose foot is at (`x`, `y`).
  pub fn seats(&self, seat: &Arc<Mesh>, x: f32, y: f32) -> impl Iterator<Item = Object> {
    (0..self.segments).map(move |index| Object {
      mesh: Arc::clone(seat),
      transform: self.place(x, y, index),
      material: pipe_seat(),
    })
  }

  /// Returns the transform of the light at `index`.
  ///
  /// The meter's foot is at (`x`, `y`).
  fn place(&self, x: f32, y: f32, index: u32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y - self.pitch * (index as f32 + 0.5), 0.0))
  }

  /// Returns the color the light at `index` shows when lit.
  ///
  /// `index` counts from the bottom.
  pub fn color(&self, index: u32) -> Vec3 {
    let up = index as f32 / self.segments.max(1) as f32;
    let [low, middle, high] = self.colors;
    if up >= self.red_from {
      high
    } else if up >= self.yellow_from {
      middle
    } else {
      low
    }
  }

  /// Returns the meter's lights as objects.
  ///
  /// Each [`Object`] has the mesh [`Meter::segment`] makes, for a meter whose foot is at (`x`, `y`)
  /// showing `level` from zero to one. The topmost lit light is as bright as the part of it the
  /// level covers. `peak`, if there is one, lights the one light at that level too, the way a meter
  /// holds its highest reading.
  pub fn lights(
    &self,
    segment: &Arc<Mesh>,
    x: f32,
    y: f32,
    level: f32,
    peak: Option<f32>,
  ) -> impl Iterator<Item = Object> {
    let count = self.segments as f32;
    let held = peak
      .filter(|&peak| peak > 0.0)
      .map(|peak| ((peak.clamp(0.0, 1.0) * count).ceil() as u32).saturating_sub(1));
    (0..self.segments).map(move |index| {
      let color = self.color(index);
      let covered = (level.clamp(0.0, 1.0) * count - index as f32).clamp(0.0, 1.0);
      let lit = if held == Some(index) { 1.0 } else { covered };
      Object {
        mesh: Arc::clone(segment),
        transform: self.place(x, y, index),
        material: pipe_end(color, lit, self.width.min(self.length) * 0.5),
      }
    })
  }

  /// Returns the light the meter throws around it at a level.
  ///
  /// The [`Lamp`] is at the top of the meter's lit part, in the color it has reached.
  pub fn lamp(&self, x: f32, y: f32, level: f32) -> Option<Lamp> {
    let lit = (level.clamp(0.0, 1.0) * self.segments as f32).ceil() as u32;
    let top = lit.checked_sub(1)?;
    Some(Lamp {
      at: Vec3::new(x, y - self.pitch * (top as f32 + 0.5), self.width),
      color: self.color(top) * 0.6,
      reach: self.width * 3.0,
      radius: self.width.hypot(self.length) / 2.0,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_meter_lights_from_the_bottom_up_in_green_yellow_and_red() {
    let meter = Meter::ladder(10, 8.0, 100.0);
    assert_eq!(meter.pitch * meter.segments as f32, 100.0);
    let segment = Arc::new(meter.segment());
    let lights: Vec<Object> = (meter.lights(&segment, 50.0, 200.0, 0.45, None)).collect();
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
    let held: Vec<Object> = (meter.lights(&segment, 0.0, 0.0, 0.2, Some(0.9))).collect();
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
}
