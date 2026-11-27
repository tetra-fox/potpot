//! Small lights, lines of light and lamps.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Knob};
use potpot::{Lamp, Light, Look, Material, Object, Scene};

use super::shared::rubber;
use super::{BLACK, Build, Catalog, SEAT, at, dim};

/// The colors the lights are lit in.
const COLORS: [Vec3; 5] = [
  Vec3::new(1.0, 0.12, 0.06),
  Vec3::new(1.0, 0.75, 0.1),
  Vec3::new(0.1, 0.9, 0.3),
  Vec3::new(0.1, 0.4, 1.0),
  Vec3::new(0.95, 0.95, 0.9),
];

/// Returns small lights, light lines and lamps.
///
/// They are on a pale plate. The top row is small lights of two sizes, the last of each row dark,
/// and lines of light. The bottom row is a lamp's light on chrome and gloss, on paint, and from a
/// lit button. The light is dimmed so that what glows shows.
pub(super) fn lights(look: Look) -> Scene {
  let ink = Material::ink(Vec3::splat(0.08));
  let plate = Material::satin(Vec3::new(0.55, 0.56, 0.58));
  let mut catalog = Catalog::new("LIGHTS", plate, ink, [3, 2], 180.0);
  let cells = catalog.cells.clone();

  // small lights of two sizes, each row ending in a dark one
  for (cell, size, count) in [(cells[0], 10.0, 6), (cells[1], 20.0, 4)] {
    let apart = size * 2.8;
    let first = cell.middle.x - apart * (count - 1) as f32 / 2.0;
    for index in 0..count {
      let at = Vec2::new(first + apart * index as f32, cell.middle.y);
      let level = if index + 1 < count { 1.0 } else { 0.0 };
      let color = COLORS[index % COLORS.len()];
      let (light, lamp) = small_lamp(at, color, level, size, size * 4.0);
      catalog.build.objects.extend(light);
      catalog.build.lamps.extend(lamp);
    }
  }
  catalog.label(cells[0], "LED");
  catalog.label(cells[1], "LARGE LED");

  // a straight line of light, and one that turns down at both ends at half its level
  let cyan = Vec3::new(0.2, 0.85, 1.0);
  let middle = cells[2].middle;
  let lines = [
    (vec![Vec2::new(-110.0, -30.0), Vec2::new(110.0, -30.0)], 1.0),
    (
      vec![
        Vec2::new(-110.0, 20.0),
        Vec2::new(-100.0, 30.0),
        Vec2::new(100.0, 30.0),
        Vec2::new(110.0, 20.0),
      ],
      0.5,
    ),
  ];
  for (path, level) in lines {
    let path: Vec<Vec2> = path.into_iter().map(|at| middle + at).collect();
    let build = &mut catalog.build;
    build.put(
      parts::light_line_seat(&path, 1.4, 0.7),
      Mat4::IDENTITY,
      Material::matte(SEAT),
    );
    build.put(
      parts::light_line(&path, 1.4),
      Mat4::IDENTITY,
      Material::lamp(cyan * level),
    );
  }
  catalog.label(cells[2], "LIGHT LINE");

  // a red lamp between a chrome knob and a glossy one, with posts below it
  let knob = Arc::new(Knob::round(40.0, 32.0).body());
  let chrome = Material::chrome(Vec3::splat(0.8));
  let middle = cells[3].middle;
  lamp_between(
    &mut catalog.build,
    middle,
    COLORS[0],
    &knob,
    [chrome, Material::glossy(BLACK)],
  );
  let posts = [
    (-50.0, chrome),
    (0.0, Material::matte(Vec3::splat(0.8))),
    (50.0, Material::glossy(BLACK)),
  ];
  for (x, material) in posts {
    let post = parts::slab(12.0, 12.0, 26.0, 6.0);
    catalog
      .build
      .put(post, at(middle + Vec2::new(x, 64.0)), material);
  }
  catalog.label(cells[3], "LAMP ON METAL");

  // a blue lamp between a spun knob and a cream one, over painted blocks
  let middle = cells[4].middle;
  let cream = Material::glossy(Vec3::new(0.74, 0.72, 0.66));
  let spun = Material::spun(super::ALUMINUM);
  lamp_between(&mut catalog.build, middle, COLORS[3], &knob, [spun, cream]);
  let blocks = [
    (-50.0, Vec3::splat(0.85)),
    (0.0, Vec3::new(0.9, 0.35, 0.05)),
    (50.0, Vec3::new(0.08, 0.3, 0.85)),
  ];
  for (x, color) in blocks {
    let block = parts::slab(36.0, 28.0, 20.0, 4.0);
    catalog.build.put(
      block,
      at(middle + Vec2::new(x, 70.0)),
      Material::matte(color),
    );
  }
  catalog.label(cells[4], "LAMP ON PAINT");

  // a lit pad beside painted blocks, which its light falls on
  let middle = cells[5].middle;
  let pad = Button::pad(64.0, 12.0);
  let green = COLORS[2];
  let build = &mut catalog.build;
  build.put(pad.seat(3.0), at(middle), Material::matte(SEAT));
  let lit = rubber().lit(green, 1.0, pad.emission_radius());
  build.put(pad.body(), pad.transform(middle.x, middle.y, 0.0), lit);
  build.lamps.push(pad.lamp(middle.x, middle.y, green, 1.0));
  for (x, color) in [
    (-80.0, Vec3::splat(0.85)),
    (80.0, Vec3::new(0.9, 0.35, 0.05)),
  ] {
    let block = parts::slab(40.0, 90.0, 26.0, 6.0);
    build.put(
      block,
      at(middle + Vec2::new(x, 0.0)),
      Material::matte(color),
    );
  }
  catalog.label(cells[5], "BUTTON LAMP");

  let light = Light {
    bounce: 3.0,
    ..dim()
  };
  catalog.finish(look, light)
}

/// Adds a lamp at `middle` with a knob to each side.
///
/// The knobs are of `materials`, the left one first, and the lamp is lit in `color`.
fn lamp_between(
  build: &mut Build,
  middle: Vec2,
  color: Vec3,
  knob: &Arc<potpot::Mesh>,
  materials: [Material; 2],
) {
  let (light, lamp) = small_lamp(middle, color, 1.0, 20.0, 150.0);
  build.objects.extend(light);
  build.lamps.extend(lamp);
  for (x, material) in [-90.0, 90.0].into_iter().zip(materials) {
    build.place(knob, at(middle + Vec2::new(x, 0.0)), material);
  }
}

/// Returns a small light in its seat, and its lamp.
///
/// The light is `size` across and lit in `color` at `level`. Its lamp reaches `reach`, and is
/// there only while the light is lit.
fn small_lamp(
  middle: Vec2,
  color: Vec3,
  level: f32,
  size: f32,
  reach: f32,
) -> ([Object; 2], Option<Lamp>) {
  let objects = [
    Object {
      mesh: Arc::new(parts::led_seat(size)),
      transform: at(middle),
      material: Material::matte(SEAT),
      picture: Vec::new(),
    },
    Object {
      mesh: Arc::new(parts::led(size)),
      transform: at(middle),
      material: parts::led_material(color, level, size),
      picture: Vec::new(),
    },
  ];
  let over = middle.extend(size);
  let lamp = (level > 0.0).then(|| parts::led_lamp(over, color, level, size, reach));
  (objects, lamp)
}
