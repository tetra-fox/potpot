//! Small lamps beside what shows their light.
//!
//! Their light shows on polished and glossy knobs, posts that stand in the way, and painted
//! blocks.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Button, Knob};
use potpot::{Lamp, Light, Look, Material, Object, Scene};

use super::{ALUMINUM, BLACK, SEAT, SIZE, framed};

/// Returns two rows on a pale plate under a dimmed light.
///
/// The first row is a red lamp between a chrome knob and a glossy one, with posts below it.
/// The second is a lit pad and a blue lamp among knobs and blocks.
pub(super) fn lamp(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let mut objects = vec![Object {
    mesh: Arc::new(parts::plate(width, height, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::satin(Vec3::new(0.55, 0.56, 0.58)),
  }];
  let mut lamps = Vec::new();
  let knob = Knob::round(52.0, 40.0);
  let chrome = Material::chrome(Vec3::splat(0.8));

  // a red lamp with a knob to each side, another above it and three posts below it
  let (light, lamp) = small_lamp(512.0, 170.0, Vec3::new(1.0, 0.12, 0.06));
  objects.extend(light);
  lamps.push(lamp);
  objects.extend([
    put(knob.body(), 400.0, 170.0, chrome),
    put(knob.body(), 624.0, 170.0, Material::glossy(BLACK)),
    put(
      Knob::fluted(40.0, 34.0).body(),
      512.0,
      70.0,
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
    ),
    // and one more of each further off, past the lamp's reach
    put(knob.body(), 250.0, 170.0, chrome),
    put(knob.body(), 774.0, 170.0, Material::glossy(BLACK)),
  ]);
  let posts = [
    (462.0, chrome),
    (512.0, Material::matte(Vec3::splat(0.8))),
    (562.0, Material::glossy(BLACK)),
  ];
  for (x, material) in posts {
    objects.push(put(parts::slab(12.0, 12.0, 26.0, 6.0), x, 232.0, material));
  }

  // a lit pad beside painted blocks, and a blue lamp between a spun knob and a cream one
  let pad = Button::pad(70.0, 12.0);
  let green = Vec3::new(0.1, 0.9, 0.3);
  objects.push(put(pad.seat(3.0), 200.0, 450.0, Material::matte(SEAT)));
  objects.push(Object {
    mesh: Arc::new(pad.body()),
    transform: pad.transform(200.0, 450.0, 0.0),
    material: Material::silicone(Vec3::splat(0.62)).lit(green, 1.0, pad.emission_radius()),
  });
  lamps.push(pad.lamp(200.0, 450.0, green, 1.0));
  let blocks = [
    (300.0, Vec3::splat(0.85)),
    (420.0, Vec3::new(0.9, 0.35, 0.05)),
    (540.0, Vec3::new(0.08, 0.3, 0.85)),
  ];
  for (x, color) in blocks {
    let block = parts::slab(60.0, 120.0, 30.0, 6.0);
    objects.push(put(block, x, 450.0, Material::matte(color)));
  }
  let (light, lamp) = small_lamp(760.0, 450.0, Vec3::new(0.1, 0.4, 1.0));
  objects.extend(light);
  lamps.push(lamp);
  objects.extend([
    put(knob.body(), 660.0, 450.0, Material::spun(ALUMINUM)),
    put(
      knob.body(),
      860.0,
      450.0,
      Material::glossy(Vec3::new(0.74, 0.72, 0.66)),
    ),
  ]);

  Scene {
    light: Light {
      color: Vec3::splat(0.6),
      bounce: 3.0,
      ..Light::default()
    },
    lamps,
    ..framed(look, objects)
  }
}

fn put(mesh: potpot::Mesh, x: f32, y: f32, material: Material) -> Object {
  Object {
    mesh: Arc::new(mesh),
    transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
    material,
  }
}

/// Returns a small round light in its seat and its lamp.
///
/// The light is lit in `color`, and the lamp is its light.
fn small_lamp(x: f32, y: f32, color: Vec3) -> ([Object; 2], Lamp) {
  let size = 20.0;
  let seat = Material::matte(SEAT);
  let objects = [
    put(parts::led_seat(size), x, y, seat),
    put(
      parts::led(size),
      x,
      y,
      parts::led_material(color, 1.0, size),
    ),
  ];
  let lamp = parts::led_lamp(Vec3::new(x, y, size), color, 1.0, size, 190.0);
  (objects, lamp)
}
