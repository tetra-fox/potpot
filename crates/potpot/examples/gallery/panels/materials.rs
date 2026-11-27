//! Every material constructor, each on a knob, a strip and a strip under a lamp.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Knob};
use potpot::{Lamp, Light, Look, Material, Scene};

use super::{Build, SIZE, framed};

/// Returns a grid of every material constructor, each named under its samples.
///
/// Each cell holds a round knob of the material, a strip of it lying on the panel, and a second
/// strip under a small white lamp. The panel is plain grey paint.
pub(super) fn materials(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let red = Vec3::new(0.55, 0.06, 0.05);
  let samples = [
    ("matte", Material::matte(red)),
    ("glossy", Material::glossy(red)),
    ("satin", Material::satin(red)),
    ("chrome", Material::chrome(Vec3::splat(0.8))),
    ("spun", Material::spun(Vec3::splat(0.78))),
    ("pane", Material::pane(Vec3::new(0.02, 0.04, 0.06))),
    (
      "display_glass",
      Material::display_glass(Vec3::new(0.01, 0.015, 0.02), 1.5).lit(
        Vec3::new(0.3, 0.9, 1.0),
        0.8,
        0.0,
      ),
    ),
    ("pipe", Material::pipe()),
    ("brushed", Material::brushed(Vec3::splat(0.72))),
    ("powder_coat", Material::powder_coat(red)),
    ("anodized", Material::anodized(Vec3::new(0.2, 0.21, 0.23))),
    (
      "hammertone",
      Material::hammertone(Vec3::new(0.3, 0.42, 0.5)),
    ),
    ("soft_touch", Material::soft_touch(Vec3::splat(0.05))),
    ("bakelite", Material::bakelite(Vec3::splat(0.03))),
    ("walnut", Material::walnut()),
    ("vu_face", Material::vu_face()),
    ("silicone", Material::silicone(Vec3::splat(0.6))),
    ("ink", Material::ink(Vec3::splat(0.9))),
    ("lamp", Material::lamp(Vec3::new(1.0, 0.6, 0.15))),
  ];
  let mut build = Build::default();
  build.put(
    parts::plate(width, height, 6.0),
    Mat4::IDENTITY,
    Material::satin(Vec3::splat(0.32)),
  );
  let knob = Arc::new(Knob::round(17.0, 14.0).body());
  let strip = Arc::new(parts::slab(78.0, 22.0, 3.0, 3.0));
  let label = Material::ink(Vec3::splat(0.04));
  let (columns, rows) = (5, 4);
  let (cell_width, cell_height) = (width / columns as f32, height / rows as f32);
  for (index, (name, material)) in samples.into_iter().enumerate() {
    let (x, y) = (
      cell_width * ((index % columns) as f32 + 0.5),
      cell_height * ((index / columns) as f32 + 0.5),
    );
    let at = |dx: f32, dy: f32| Mat4::from_translation(Vec3::new(x + dx, y + dy, 0.0));
    build.place(&knob, at(-56.0, -18.0), material);
    build.place(&strip, at(28.0, -38.0), material);
    build.place(&strip, at(28.0, 0.0), material);
    build.lamps.push(Lamp {
      at: Vec3::new(x + 28.0, y, 16.0),
      color: Vec3::splat(2.0),
      reach: 64.0,
      radius: 3.0,
    });
    build.letters(
      &name.to_uppercase(),
      Vec3::new(x, y + 42.0, 0.0),
      9.0,
      label,
    );
  }
  Scene {
    light: Light {
      color: Vec3::splat(0.8),
      ..Light::default()
    },
    lamps: build.lamps,
    ..framed(look, build.objects)
  }
}
