//! A whole soft synth's front.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Edge, Grip, Knob, Meter, Top};
use potpot::{Light, Look, Material, Object, Scene};

use super::{Build, PAGE, SEAT, framed};

/// Returns a soft synth's front in the manner of the mid 2000s.
///
/// - a saturated plastic panel in a rim, with raised and titled sections
/// - soft-touch knobs in dashed rings, seen from in front
/// - small round lights
/// - a lit screen
/// - lettering everywhere
/// - a wide soft light on it all
pub(super) fn retro(look: Look) -> Scene {
  let (width, height) = (PAGE.x, PAGE.y);
  let blue = Vec3::new(0.13, 0.4, 0.74);
  let mut build = Build::default();
  let at = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
  // how high a section stands, which everything on one stands on
  const UP: f32 = 3.0;

  let plastic = |color: Vec3| Material {
    grain: 0.22,
    ..Material::satin(color)
  };
  let ink = Material::ink(Vec3::splat(0.92));
  let dark_ink = Material::ink(Vec3::new(0.04, 0.07, 0.16));
  let hole = Material::matte(SEAT);

  // the panel and a walnut rim around it
  build.put(
    parts::plate(width, height, 6.0),
    Mat4::IDENTITY,
    plastic(blue * 0.72),
  );
  build.put(
    parts::frame(width, height, 9.0, 9.0, 16.0),
    at(width / 2.0, height / 2.0, 0.0),
    Material::walnut(),
  );

  // the sections, each with a white banner carrying its name and slanted bars after it
  let tall = Arc::new(parts::slab(300.0, 318.0, UP, 12.0));
  let low = Arc::new(parts::slab(300.0, 196.0, UP, 12.0));
  let wide = Arc::new(parts::slab(336.0, 196.0, UP, 12.0));
  let banner = Arc::new(parts::slab(150.0, 15.0, 1.2, 3.0));
  let slant = Arc::new(potpot::Mesh::cuboid(
    Vec3::new(-1.3, -6.0, 0.0),
    Vec3::new(1.3, 6.0, 0.4),
  ));
  let sections = [
    (&tall, 172.0, 196.0, 159.0, "FILTER MOD"),
    (&tall, 852.0, 196.0, 159.0, "AMP MOD"),
    (&low, 172.0, 508.0, 98.0, "MASTER FILTER"),
    (&wide, 512.0, 508.0, 98.0, "DELAY"),
    (&low, 852.0, 508.0, 98.0, "OUTPUT"),
  ];
  for (mesh, x, y, half, name) in sections {
    build.place(mesh, at(x, y, 0.0), plastic(blue));
    let (left, top) = (x - 62.0, y - half + 15.0);
    build.place(
      &banner,
      at(left, top, UP),
      Material::satin(Vec3::splat(0.9)),
    );
    build.letters(name, Vec3::new(left - 6.0, top, UP + 1.2), 7.0, dark_ink);
    for bar in 0..4 {
      let lean = Mat4::from_rotation_z(0.5);
      build.place(
        &slant,
        at(left + 86.0 + 7.0 * bar as f32, top, UP) * lean,
        ink,
      );
    }
    // a line cut across the section under its banner, with its lower lip catching light
    build.put(
      potpot::Mesh::cuboid(Vec3::new(-136.0, -0.6, 0.0), Vec3::new(136.0, 0.6, 0.25)),
      at(x, top + 13.0, UP),
      Material::matte(blue * 0.35),
    );
    build.put(
      potpot::Mesh::cuboid(Vec3::new(-136.0, 0.6, 0.0), Vec3::new(136.0, 1.5, 0.25)),
      at(x, top + 13.0, UP),
      Material::matte((blue * 1.5).min(Vec3::ONE)),
    );
  }

  // the knobs: a soft body with a paler face let into it and a painted line, in a ring of
  // dashes, with its name under it
  let knob = Knob {
    skirt: 0.0,
    grip: Grip::Ribbed {
      ribs: 26,
      depth: 0.03,
    },
    top: Top::Domed { rise: 0.04 },
    taper: 0.1,
    edge: Edge::Rolled(0.2),
    ..Knob::round(20.0, 23.0)
  };
  let body = Arc::new(knob.body());
  let face = Arc::new(knob.cap(0.82));
  let line = Arc::new(knob.line());
  let ring = Arc::new(parts::dashes(17, parts::KNOB_SWEEP, 27.0, 3.0, 0.62));
  let dial = |build: &mut Build, x: f32, y: f32, turn: f32, name: &str| {
    let turned = at(0.0, 0.0, UP) * parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    build.place(&ring, at(x, y, UP), ink);
    build.place(&body, turned, Material::soft_touch(Vec3::splat(0.12)));
    build.place(&face, turned, Material::soft_touch(Vec3::splat(0.3)));
    build.place(&line, turned, ink);
    build.letters(name, Vec3::new(x, y + 34.0, UP), 6.5, ink);
  };
  let names = [
    ["", "ENV", "CUTOFF", "RES", "ATK", "DEC", "SUS", "REL"],
    ["", "PAN", "SPIKE", "SPREAD", "ATK", "DEC", "SUS", "REL"],
  ];
  for (section, left) in [(0, 117.0), (1, 797.0)] {
    for (place, name) in names[section].into_iter().enumerate().skip(1) {
      let (x, y) = (
        left + 110.0 * (place % 2) as f32,
        98.0 + 72.0 * (place / 2) as f32,
      );
      let turn = 0.12 + 0.11 * ((place * 3 + section * 2) % 8) as f32;
      dial(&mut build, x, y, turn, name);
    }
  }
  let low_names = [
    vec!["CUTOFF", "RES", "TYPE", "SLOPE"],
    vec!["", "MIX", "MOD", "TIME", "", "FBACK", "LOCUT", "HICUT"],
    vec!["VOL", "WIDTH", "DRIVE", "TONE"],
  ];
  for (section, left, count) in [(0, 171.0, 2), (1, 402.0, 4), (2, 762.0, 2)] {
    for (place, name) in low_names[section].iter().copied().enumerate() {
      if name.is_empty() {
        continue;
      }
      let (x, y) = (
        left + 76.0 * (place % count) as f32,
        480.0 + 72.0 * (place / count) as f32,
      );
      let turn = 0.2 + 0.09 * ((place * 5 + section) % 8) as f32;
      dial(&mut build, x, y, turn, name);
    }
  }

  // the switches, each a low key in its seat with a small light to its left and a name under
  // it, level with the knobs and their names
  let key = Button::key(38.0, 38.0, 5.0);
  let (key_body, key_seat) = (Arc::new(key.body()), Arc::new(key.seat(2.0)));
  let light_size = 11.0;
  let (light, light_seat) = (
    Arc::new(parts::led(light_size)),
    Arc::new(parts::led_seat(light_size)),
  );
  let green = Vec3::new(0.55, 1.0, 0.2);
  let switches = [
    (117.0, 98.0, true, "ON"),
    (797.0, 98.0, true, "ON"),
    (107.0, 516.0, false, "LINK"),
    (402.0, 480.0, true, "ON"),
    (402.0, 552.0, false, "SYNC"),
  ];
  for (x, y, lit, name) in switches {
    build.place(&key_seat, at(x, y, UP), hole);
    build.place(
      &key_body,
      at(x, y, UP),
      Material::soft_touch(Vec3::splat(0.2)),
    );
    build.letters(name, Vec3::new(x, y + 34.0, UP), 6.5, ink);
    let level = if lit { 1.0 } else { 0.0 };
    build.place(&light_seat, at(x - 31.0, y, UP), hole);
    build.place(
      &light,
      at(x - 31.0, y, UP),
      parts::led_material(green, level, light_size),
    );
    if lit {
      let over = Vec3::new(x - 31.0, y, 12.0);
      (build.lamps).push(parts::led_lamp(over, green, 1.0, light_size, 40.0));
    }
  }

  // the output's two columns of lights with a scale beside them
  let dots = Meter::dots(8, 9.0, 96.0);
  let (dot, dot_seat) = (Arc::new(dots.segment()), Arc::new(dots.seat()));
  let raised = |object: Object| Object {
    transform: at(0.0, 0.0, UP) * object.transform,
    ..object
  };
  for (x, level) in [(928.0, 0.42), (941.0, 0.55)] {
    build
      .objects
      .extend(dots.seats(&dot_seat, x, 594.0).map(raised));
    build
      .objects
      .extend(dots.lights(&dot, x, 594.0, level, None).map(raised));
    build.lamps.extend(dots.lamp(x, 594.0, level));
  }
  for (mark, up) in [("0", 0.94), ("6", 0.69), ("12", 0.44), ("24", 0.19)] {
    build.letters(mark, Vec3::new(962.0, 594.0 - 96.0 * up, UP), 5.5, ink);
  }

  // the screen: a deep dark bezel with the maker's name on its brow, glass set down inside
  // it, and what the screen shows lit on the glass
  let (middle, top) = (512.0, 196.0);
  build.put(
    parts::slab(336.0, 318.0, 5.0, 16.0),
    at(middle, top, 0.0),
    Material::glossy(blue * 0.36),
  );
  build.put(
    parts::frame(300.0, 236.0, 12.0, 8.0, 10.0),
    at(middle, top + 26.0, 5.0),
    Material::glossy(Vec3::new(0.02, 0.03, 0.06)),
  );
  build.letters("CANWETALK", Vec3::new(middle, top - 128.0, 5.0), 13.0, ink);
  build.letters("2", Vec3::new(middle + 96.0, top - 135.0, 5.0), 7.0, ink);
  let glass = Vec3::new(0.05, 0.12, 0.26);
  let shown = Vec3::new(0.55, 0.8, 1.0);
  build.put(
    potpot::Mesh::cuboid(Vec3::new(-138.0, -106.0, 0.0), Vec3::new(138.0, 106.0, 1.0)),
    at(middle, top + 26.0, 5.0),
    // an LCD, its cells a point and a half apart
    Material::display_glass(glass, 1.5).lit(Vec3::new(0.1, 0.26, 0.56), 0.16, 0.0),
  );
  let glow = Material::matte(shown).lit(shown, 0.6, 0.0);
  let dim = Material::matte(shown * 0.5).lit(shown * 0.6, 0.3, 0.0);
  let on_glass = 6.1;
  // a row of tabs, three rows of settings, a grid of steps, and the patch's name in large
  for (tab, name) in ["ARP", "MOD", "LIB", "SYS"].into_iter().enumerate() {
    let x = middle - 102.0 + 68.0 * tab as f32;
    let shade = if tab == 0 { glow } else { dim };
    build.letters(name, Vec3::new(x, top - 62.0, on_glass), 7.0, shade);
  }
  build.put(
    potpot::Mesh::cuboid(Vec3::new(-128.0, -0.5, 0.0), Vec3::new(128.0, 0.5, 0.1)),
    at(middle, top - 52.0, on_glass),
    dim,
  );
  for (row, (name, value)) in [("MODE", "UP"), ("SPEED", "1/16"), ("GATE", "110")]
    .into_iter()
    .enumerate()
  {
    let y = top - 36.0 + 16.0 * row as f32;
    build.letters(name, Vec3::new(middle - 96.0, y, on_glass), 6.5, dim);
    build.letters(value, Vec3::new(middle - 20.0, y, on_glass), 6.5, glow);
    // a bar filled as far as the setting goes
    let filled = 26.0 + 22.0 * row as f32;
    build.put(
      potpot::Mesh::cuboid(Vec3::new(0.0, -3.5, 0.0), Vec3::new(88.0, 3.5, 0.1)),
      at(middle + 36.0, y, on_glass),
      Material::matte(shown * 0.2).lit(shown * 0.3, 0.15, 0.0),
    );
    build.put(
      potpot::Mesh::cuboid(Vec3::new(0.0, -3.5, 0.0), Vec3::new(filled, 3.5, 0.15)),
      at(middle + 36.0, y, on_glass),
      glow,
    );
  }
  let step = Arc::new(potpot::Mesh::cuboid(
    Vec3::new(-6.5, -5.0, 0.0),
    Vec3::new(6.5, 5.0, 0.1),
  ));
  for column in 0..16 {
    for row in 0..4 {
      let on = (column * 7 + row * 5 + column / 3) % 6 == row || (column + row) % 9 == 0;
      let x = middle - 120.0 + 16.0 * column as f32;
      let y = top + 22.0 + 12.5 * row as f32;
      let shade = if on {
        glow
      } else {
        Material::matte(shown * 0.18).lit(shown * 0.28, 0.15, 0.0)
      };
      build.place(&step, at(x, y, on_glass), shade);
    }
  }
  build.put(
    potpot::Mesh::cuboid(Vec3::new(-128.0, -0.5, 0.0), Vec3::new(128.0, 0.5, 0.1)),
    at(middle, top + 78.0, on_glass),
    dim,
  );
  build.letters(
    "SQ DEL MAR",
    Vec3::new(middle - 30.0, top + 100.0, on_glass),
    13.0,
    glow,
  );
  build.letters(
    "19 %",
    Vec3::new(middle + 102.0, top + 94.0, on_glass),
    6.0,
    dim,
  );
  build.letters(
    "27 VC",
    Vec3::new(middle + 100.0, top + 108.0, on_glass),
    6.0,
    dim,
  );

  // screws in dark seats at the sides, and vents cut in the strip between the rows
  let screw = Arc::new(parts::screw(7.0));
  let screw_seat = Arc::new(parts::screw_seat(7.0, 2.5));
  for (x, y) in [
    (20.0, 130.0),
    (20.0, 500.0),
    (width - 20.0, 130.0),
    (width - 20.0, 500.0),
  ] {
    build.place(&screw_seat, at(x, y, 0.0), hole);
    build.place(
      &screw,
      at(x, y, 0.0) * Mat4::from_rotation_z(x + y),
      Material::chrome(Vec3::splat(0.55)),
    );
  }
  let vent = Arc::new(potpot::Mesh::cuboid(
    Vec3::new(-9.0, -1.3, 0.0),
    Vec3::new(9.0, 1.3, 0.2),
  ));
  for slot in 0..14 {
    for side in [338.0, 686.0] {
      let y = 372.0 + 5.0 * (slot % 7) as f32;
      let aside = if slot < 7 { -11.0 } else { 11.0 };
      build.place(&vent, at(side + aside, y, 0.0), hole);
    }
  }

  let look = Look {
    soften: 0.5,
    bloom: 1.0,
    ..look
  };
  Scene {
    lean: Vec2::new(0.0, -0.5),
    light: Light {
      toward: Vec3::new(-0.3, -0.5, 1.0),
      color: Vec3::splat(0.8),
      softness: 7.0,
      ..Light::default()
    },
    lamps: build.lamps,
    ..framed(look, build.objects)
  }
}
