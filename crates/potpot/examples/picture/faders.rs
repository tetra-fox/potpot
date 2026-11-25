//! the gallery of fader caps

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Fader};
use potpot::{Look, Material, Object, Scene};

use crate::{ALUMINUM, BLACK, SIZE, framed, lettering, screws};

/// every fader cap in its slot, the last few a dj mixer's in its colors
pub(super) fn faders(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let (white, dark) = (Vec3::splat(0.86), Vec3::splat(0.05));
  let (wide, long, tall) = (34.0, 62.0, 26.0);
  // a dj mixer's cap is much wider across than it is long
  let block = Fader::block(50.0, 24.0, 22.0);
  let caps = [
    (
      Fader::console(wide, long, tall),
      Material::glossy(BLACK),
      white,
    ),
    (
      Fader::terraced(wide, long, tall),
      Material::glossy(BLACK),
      white,
    ),
    (
      Fader::terraced(wide, long, tall),
      Material::spun(ALUMINUM),
      dark,
    ),
    (
      Fader::stepped(wide, long, tall),
      Material::satin(Vec3::splat(0.2)),
      white,
    ),
    (
      Fader::wedge(wide, long, tall * 1.2),
      Material::glossy(BLACK),
      white,
    ),
    (
      Fader::barrel(wide * 0.8, long * 0.8, tall),
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      dark,
    ),
    (
      Fader::fin(wide * 0.8, long * 0.7, tall),
      Material::satin(Vec3::new(0.62, 0.10, 0.08)),
      white,
    ),
    (block, Material::glossy(Vec3::new(0.86, 0.72, 0.05)), dark),
    (block, Material::glossy(Vec3::new(0.70, 0.06, 0.07)), white),
    (block, Material::glossy(Vec3::new(0.10, 0.55, 0.62)), dark),
  ];

  let (travel, middle) = (380.0, height / 2.0);
  let slot = Arc::new(parts::fader_slot(travel, 5.0));
  let scale = Arc::new(parts::fader_ticks(11, travel, 26.0, 34.0, 5));
  let mut objects = vec![Object {
    mesh: Arc::new(parts::plate(width, height, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.16, 0.17, 0.19)),
  }];
  for (index, (fader, material, stripe)) in caps.into_iter().enumerate() {
    let x = 62.0 + 100.0 * index as f32;
    let track = Mat4::from_translation(Vec3::new(x, middle, 0.0));
    objects.push(Object {
      mesh: Arc::clone(&slot),
      transform: track,
      material: Material::matte(Vec3::splat(0.02)),
    });
    objects.push(Object {
      mesh: Arc::clone(&scale),
      transform: track,
      material: Material::matte(Vec3::splat(0.85)),
    });
    // each a little higher than the last, and then around again
    let value = (0.15 + 0.23 * index as f32) % 1.0;
    let transform = parts::fader_transform(x, middle, travel, value);
    objects.push(Object {
      mesh: Arc::new(fader.body()),
      transform,
      material,
    });
    objects.push(Object {
      mesh: Arc::new(fader.line()),
      transform,
      material: Material::matte(stripe),
    });
    // the channel's number over its slot, and the scale's levels beside every other mark
    let print = Material::matte(Vec3::splat(0.82));
    let mut letters = |text: &str, x: f32, y: f32, tall: f32| {
      objects.push(Object {
        mesh: Arc::new(lettering(text, tall)),
        transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
        material: print,
      });
    };
    letters(
      &format!("{}", index + 1),
      x,
      middle - travel / 2.0 - 34.0,
      12.0,
    );
    for (mark, level) in ["10", "5", "0", "5", "10", "20"].into_iter().enumerate() {
      let y = middle - travel / 2.0 + travel * mark as f32 / 5.0;
      letters(level, x - 40.0, y, 6.0);
    }
  }
  objects.extend(screws());
  framed(look, objects)
}
