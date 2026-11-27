//! Needle meters with a cream face behind glass.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::{INK, dashes, frame, legend, ticks};
use crate::material::Material;
use crate::mesh::Mesh;
use crate::scene::Object;

/// The angle the needle sweeps from its rest to full scale, in radians.
const SWEEP: f32 = 1.6;
/// The marks of the scale, the first and every second one long and labelled.
const MARKS: u32 = 13;
/// The labels of the long marks, from the left.
const LABELS: [&str; 7] = ["-20", "-10", "-7", "-5", "-3", "0", "+3"];
/// The share of the sweep, from the left, where the red part of the scale starts.
const RED_FROM: f32 = 5.0 / 6.0;
/// The height of the face over the panel, under the bezel's top.
const FACE_UP: f32 = 0.5;
/// The color of the lamp behind the face.
const LAMP: Vec3 = Vec3::new(1.0, 0.7, 0.32);

/// A VU meter: a needle over a printed cream face, behind glass in a bezel.
///
/// The meter is centered on the origin. The needle turns on a pivot near the face's foot, and a
/// lamp behind the face lights it warmly from its middle. The glass is drawn as a clear coat on
/// everything under it.
#[derive(Clone, Debug)]
pub struct Vu {
  face: Arc<Mesh>,
  bezel: Arc<Mesh>,
  scale: Arc<Mesh>,
  red: Arc<Mesh>,
  needle: Arc<Mesh>,
  cover: Arc<Mesh>,
  /// How far the pivot is down the face from its middle, in points.
  pivot: f32,
  /// The width of the face, in points.
  width: f32,
}

impl Vu {
  /// Creates a meter whose face is `width` by `height` points.
  pub fn new(width: f32, height: f32) -> Self {
    let border = width.min(height) * 0.09;
    let depth = border * 0.6;
    let pivot = height * 0.42;
    let reach = height * 0.72;
    // the scale lies over the face and turns on the pivot
    let at_pivot = Mat4::from_translation(Vec3::new(0.0, pivot, FACE_UP));
    let mut scale = ticks(MARKS, SWEEP, reach, reach + height * 0.05, 2);
    let label_reach = reach + height * 0.15;
    for (index, label) in LABELS.iter().enumerate() {
      let turn = index as f32 / (LABELS.len() - 1) as f32;
      let angle = (turn - 0.5) * SWEEP;
      let place = Vec3::new(angle.sin(), -angle.cos(), 0.0) * label_reach;
      let text = legend(label, height * 0.07, height * 0.011);
      scale = scale.merged(&text.transformed(Mat4::from_translation(place)));
    }
    let name = legend("VU", height * 0.13, height * 0.018);
    scale =
      scale.merged(&name.transformed(Mat4::from_translation(Vec3::new(0.0, -reach * 0.45, 0.0))));
    // a thin printed arc under the marks, up to where the red takes over, each a run of short
    // straight bars
    let arc = dashes(40, SWEEP * RED_FROM, reach, height * 0.012, 1.0)
      .transformed(Mat4::from_rotation_z((RED_FROM / 2.0 - 0.5) * SWEEP));
    scale = scale.merged(&arc);
    let red_share = 1.0 - RED_FROM;
    let red = dashes(
      8,
      SWEEP * red_share,
      reach + height * 0.02,
      height * 0.04,
      1.0,
    )
    .transformed(Mat4::from_rotation_z(
      (RED_FROM + red_share / 2.0 - 0.5) * SWEEP,
    ));
    let needle = Mesh::cuboid(
      Vec3::new(-height * 0.006, -reach * 1.06, 0.0),
      Vec3::new(height * 0.006, 0.0, INK),
    );
    // a dark half dome over the pivot
    let dome = height * 0.12;
    let profile: Vec<[f32; 2]> = (0..=8)
      .map(|step| {
        let (up, out) = (step as f32 / 8.0 * FRAC_PI_2).sin_cos();
        [dome * out, dome * 0.5 * up]
      })
      .collect();
    Self {
      face: Arc::new(Mesh::cuboid(
        Vec3::new(-width / 2.0, -height / 2.0, 0.0),
        Vec3::new(width / 2.0, height / 2.0, FACE_UP),
      )),
      bezel: Arc::new(frame(
        width + border * 2.0,
        height + border * 2.0,
        border,
        depth,
        border,
      )),
      scale: Arc::new(scale.transformed(at_pivot)),
      red: Arc::new(red.transformed(at_pivot)),
      needle: Arc::new(needle),
      cover: Arc::new(Mesh::lathe(&profile, 32, true)),
      pivot,
      width,
    }
  }

  /// Returns the meter at (`x`, `y`) as objects.
  ///
  /// `value` runs from zero at rest to one at full scale, and the needle stands there; the
  /// caller gives it its ballistics. `lamp` runs from zero for the lamp off to one for on.
  pub fn objects(&self, x: f32, y: f32, value: f32, lamp: f32) -> [Object; 6] {
    let at = Mat4::from_translation(Vec3::new(x, y, 0.0));
    let glass = |material: Material| Material {
      clearcoat: 1.0,
      clearcoat_roughness: 0.05,
      ..material
    };
    let lamp = lamp.clamp(0.0, 1.0);
    let face = glass(Material::vu_face()).lit(LAMP, lamp * 0.22, self.width * 0.5);
    let ink = glass(Material::ink(Vec3::splat(0.04)));
    let red = glass(Material::ink(Vec3::new(0.62, 0.05, 0.04)));
    let turn = (value.clamp(0.0, 1.0) - 0.5) * SWEEP;
    let pivot = at * Mat4::from_translation(Vec3::new(0.0, self.pivot, FACE_UP + INK));
    let object = |mesh: &Arc<Mesh>, transform: Mat4, material: Material| Object {
      mesh: Arc::clone(mesh),
      transform,
      material,
    };
    [
      object(&self.face, at, face),
      object(&self.scale, at, ink),
      object(&self.red, at, red),
      object(&self.needle, pivot * Mat4::from_rotation_z(turn), ink),
      object(
        &self.cover,
        pivot,
        glass(Material::bakelite(Vec3::splat(0.03))),
      ),
      object(&self.bezel, at, Material::bakelite(Vec3::splat(0.025))),
    ]
  }
}
