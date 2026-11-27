//! Every fader cap, each in its slot.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Fader};
use potpot::{Light, Look, Material, Scene};

use super::{ALUMINUM, BLACK, Catalog};

/// Returns every fader cap in its slot beside a scale.
///
/// Each cap stands a little higher than the last.
pub(super) fn faders(look: Look) -> Scene {
  let (white, dark) = (Vec3::splat(0.86), Vec3::splat(0.05));
  let (wide, long, tall) = (34.0, 62.0, 26.0);
  let caps = [
    (
      "CONSOLE",
      Fader::console(wide, long, tall),
      Material::glossy(BLACK),
      white,
    ),
    (
      "TERRACED",
      Fader::terraced(wide, long, tall),
      Material::spun(ALUMINUM),
      dark,
    ),
    (
      "STEPPED",
      Fader::stepped(wide, long, tall),
      Material::satin(Vec3::splat(0.2)),
      white,
    ),
    (
      "WEDGE",
      Fader::wedge(wide, long, tall * 1.2),
      Material::glossy(BLACK),
      white,
    ),
    (
      "BARREL",
      Fader::barrel(wide * 0.8, long * 0.8, tall),
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      dark,
    ),
    (
      "FIN",
      Fader::fin(wide * 0.8, long * 0.7, tall),
      Material::satin(Vec3::new(0.62, 0.10, 0.08)),
      white,
    ),
    // a dj mixer's cap is much wider across than it is long
    (
      "BLOCK",
      Fader::block(50.0, 24.0, 22.0),
      Material::glossy(Vec3::new(0.86, 0.72, 0.05)),
      dark,
    ),
  ];

  let print = Material::ink(Vec3::splat(0.82));
  let plate = Material::powder_coat(Vec3::new(0.16, 0.17, 0.19));
  let mut catalog = Catalog::new("FADERS", plate, print, [caps.len(), 1], 524.0);
  let travel = 420.0;
  let slot = Arc::new(parts::fader_slot(travel, 5.0));
  let scale = Arc::new(parts::fader_ticks(11, travel, 26.0, 34.0, 5));
  for (index, ((name, fader, material, stripe), cell)) in
    caps.into_iter().zip(catalog.cells.clone()).enumerate()
  {
    let (x, middle) = (cell.middle.x, cell.middle.y);
    let track = Mat4::from_translation(Vec3::new(x, middle, 0.0));
    let build = &mut catalog.build;
    build.place(&slot, track, Material::matte(Vec3::splat(0.02)));
    build.place(&scale, track, Material::ink(Vec3::splat(0.85)));
    let value = 0.15 + 0.11 * index as f32;
    let transform = parts::fader_transform(x, middle, travel, value);
    build.put(fader.body(), transform, material);
    build.put(fader.line(), transform, Material::matte(stripe));
    // the scale's levels beside every other mark
    for (mark, level) in ["10", "5", "0", "5", "10", "20"].into_iter().enumerate() {
      let y = middle - travel / 2.0 + travel * mark as f32 / 5.0;
      build.letters(level, Vec3::new(x - 42.0, y, 0.0), 6.0, print);
    }
    catalog.label(cell, name);
  }
  catalog.finish(look, Light::default())
}
