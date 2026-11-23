//! renders a panel to a png without a window: `cargo run --example picture -- out.png`
//! draws every knob style, and `faders`, `lights` or `synth` after the file's name draws
//! every fader, lit buttons and meters, or a whole soft synth's front in an old manner.
//! after those, one number is the pixel size and a second the color steps, for the crude
//! looks

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use skeuo::glam::{Mat4, Vec2, Vec3};
use skeuo::parts::{self, Button, Edge, Fader, Grip, Knob, Meter, Push, PushSwitch, Top};
use skeuo::{Light, Look, Material, Object, Place, Renderer, Scene, wgpu};

const SIZE: [u32; 2] = [1024, 640];

/// what each knob of the gallery is called, in the order they are shown
const NAMES: [&str; 15] = [
  "ROUND",
  "FLUTED",
  "SKIRTED",
  "KNURLED",
  "DISHED",
  "DIMPLED",
  "FACETED",
  "CHICKEN HEAD",
  "BAR",
  "LEVER",
  "RIDGE",
  "CAPPED",
  "HEX",
  "RIBBED",
  "CONICAL",
];

/// how a knob of the gallery is marked
#[derive(Clone, Copy)]
enum Marker {
  Line,
  Dot,
  /// its own shape says where it points
  None,
}

/// a knob of the gallery: its shape, what it is made of, what marks it and in which color,
/// and what its face is inlaid with if anything
struct Shown {
  knob: Knob,
  material: Material,
  marker: Marker,
  mark: Vec3,
  cap: Option<Material>,
}

/// every knob style in rows of five, each over a scale
fn knobs(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let (radius, tall) = (58.0, 42.0);
  let black = Vec3::new(0.035, 0.035, 0.04);
  let white = Vec3::splat(0.86);
  let aluminum = Material::spun(Vec3::new(0.78, 0.79, 0.81));
  let shown = |knob, material, marker| Shown {
    knob,
    material,
    marker,
    mark: white,
    cap: None,
  };
  let knobs = [
    shown(
      Knob::round(radius, tall),
      Material::glossy(black),
      Marker::Line,
    ),
    shown(
      Knob::fluted(radius, tall),
      Material::glossy(black),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: black,
      ..shown(
        Knob::skirted(radius, tall),
        Material::glossy(black),
        Marker::Line,
      )
    },
    Shown {
      mark: black,
      ..shown(Knob::knurled(radius * 0.85, tall), aluminum, Marker::Line)
    },
    shown(
      Knob::dished(radius, tall),
      Material::satin(Vec3::new(0.50, 0.08, 0.07)),
      Marker::Dot,
    ),
    shown(
      Knob::dimpled(radius, tall * 0.55),
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      Marker::None,
    ),
    shown(
      Knob::faceted(radius, tall),
      Material::glossy(Vec3::new(0.06, 0.13, 0.26)),
      Marker::Line,
    ),
    shown(
      Knob::chicken_head(radius, tall),
      Material::glossy(black),
      Marker::Line,
    ),
    shown(
      Knob::bar(radius, tall * 1.2),
      Material::glossy(black),
      Marker::Line,
    ),
    shown(
      Knob::lever(radius, tall * 0.8),
      Material::glossy(Vec3::new(0.70, 0.36, 0.04)),
      Marker::Line,
    ),
    shown(
      Knob::ridge(radius, tall * 1.25),
      Material::glossy(black),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: black,
      ..shown(
        Knob {
          skirt: 0.0,
          top: Top::Flat,
          ..Knob::knurled(radius * 0.8, tall * 1.1)
        },
        Material::glossy(black),
        Marker::Line,
      )
    },
    shown(
      Knob::hex(radius, tall * 1.1),
      Material::glossy(black),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: black,
      ..shown(
        Knob::ribbed(radius * 0.8, tall * 1.1),
        Material::satin(black),
        Marker::Line,
      )
    },
    Shown {
      cap: Some(Material::spun(Vec3::new(0.80, 0.66, 0.36))),
      mark: black,
      ..shown(
        Knob::conical(radius, tall),
        Material::glossy(black),
        Marker::Line,
      )
    },
  ];

  let mut meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(8.0),
    parts::ticks(11, parts::KNOB_SWEEP, radius + 6.0, radius + 13.0, 5),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material::brushed(Vec3::new(0.30, 0.32, 0.36)),
  }];
  for (index, shown) in knobs.into_iter().enumerate() {
    let (x, y) = (
      112.0 + 200.0 * (index % 5) as f32,
      112.0 + 208.0 * (index / 5) as f32,
    );
    let turn = 0.12 + 0.055 * index as f32;
    let transform = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    objects.push(Object {
      mesh: 2,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: Material::matte(Vec3::splat(0.85)),
    });
    let mut part = |mesh: skeuo::Mesh, material: Material| {
      objects.push(Object {
        mesh: meshes.len(),
        transform,
        material,
      });
      meshes.push(mesh);
    };
    part(shown.knob.body(), shown.material);
    if let Some(cap) = shown.cap {
      part(shown.knob.cap(0.86), cap);
    }
    match shown.marker {
      Marker::Line => part(shown.knob.line(), Material::matte(shown.mark)),
      Marker::Dot => part(shown.knob.dot(), Material::matte(shown.mark)),
      Marker::None => {}
    }
    // its name engraved under it, and the ends of its scale numbered
    let engraved = Material::matte(Vec3::splat(0.08));
    let mut letters = |text: &str, x: f32, y: f32, tall: f32| {
      objects.push(Object {
        mesh: meshes.len(),
        transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
        material: engraved,
      });
      meshes.push(parts::legend(text, tall, tall * 0.17));
    };
    letters(NAMES[index], x, y + radius + 34.0, 9.0);
    letters("0", x - radius - 4.0, y + radius + 14.0, 7.0);
    letters("10", x + radius + 6.0, y + radius + 14.0, 7.0);
  }
  let inset = 18.0;
  for (x, y) in [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ] {
    objects.push(Object {
      mesh: 1,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::chrome(Vec3::splat(0.75)),
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    meshes: Arc::from(meshes),
    objects,
    lamps: Vec::new(),
  }
}

/// every fader cap in its slot, the last few a dj mixer's in its colors
fn faders(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let black = Vec3::new(0.035, 0.035, 0.04);
  let (white, dark) = (Vec3::splat(0.86), Vec3::splat(0.05));
  let (wide, long, tall) = (34.0, 62.0, 26.0);
  // a dj mixer's cap is much wider across than it is long
  let block = Fader::block(50.0, 24.0, 22.0);
  let caps = [
    (
      Fader::console(wide, long, tall),
      Material::glossy(black),
      white,
    ),
    (
      Fader::terraced(wide, long, tall),
      Material::glossy(black),
      white,
    ),
    (
      Fader::terraced(wide, long, tall),
      Material::spun(Vec3::new(0.78, 0.79, 0.81)),
      dark,
    ),
    (
      Fader::stepped(wide, long, tall),
      Material::satin(Vec3::splat(0.2)),
      white,
    ),
    (
      Fader::wedge(wide, long, tall * 1.2),
      Material::glossy(black),
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
  let mut meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(8.0),
    parts::fader_slot(travel, 5.0),
    parts::fader_ticks(11, travel, 26.0, 34.0, 5),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.16, 0.17, 0.19)),
  }];
  for (index, (fader, material, stripe)) in caps.into_iter().enumerate() {
    let x = 62.0 + 100.0 * index as f32;
    let track = Mat4::from_translation(Vec3::new(x, middle, 0.0));
    objects.push(Object {
      mesh: 2,
      transform: track,
      material: Material::matte(Vec3::splat(0.02)),
    });
    objects.push(Object {
      mesh: 3,
      transform: track,
      material: Material::matte(Vec3::splat(0.85)),
    });
    // each a little higher than the last, and then around again
    let value = (0.15 + 0.23 * index as f32) % 1.0;
    let transform = parts::fader_transform(x, middle, travel, value);
    objects.push(Object {
      mesh: meshes.len(),
      transform,
      material,
    });
    meshes.push(fader.body());
    objects.push(Object {
      mesh: meshes.len(),
      transform,
      material: Material::matte(stripe),
    });
    meshes.push(fader.line());
    // the channel's number over its slot, and the scale's levels beside every other mark
    let print = Material::matte(Vec3::splat(0.82));
    let mut letters = |text: &str, x: f32, y: f32, tall: f32| {
      objects.push(Object {
        mesh: meshes.len(),
        transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
        material: print,
      });
      meshes.push(parts::legend(text, tall, (tall * 0.17).max(1.0)));
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
  let inset = 18.0;
  for (x, y) in [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ] {
    objects.push(Object {
      mesh: 1,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::chrome(Vec3::splat(0.75)),
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    meshes: Arc::from(meshes),
    objects,
    lamps: Vec::new(),
  }
}

/// rubber buttons, some of them lit, and level meters at a few levels, on a dark panel
/// under a dimmed light so that what glows shows
fn lights(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let rubber = Material::silicone(Vec3::new(0.62, 0.62, 0.60));
  let colors = [
    Vec3::new(1.0, 0.25, 0.05),
    Vec3::new(0.1, 0.9, 0.3),
    Vec3::new(0.1, 0.5, 1.0),
    Vec3::new(1.0, 0.75, 0.1),
    Vec3::new(0.9, 0.1, 0.7),
    Vec3::new(0.95, 0.95, 0.9),
  ];

  let pad = Button::pad(70.0, 12.0);
  let round = Button::round(46.0, 10.0);
  let key = Button::key(70.0, 28.0, 7.0);
  let pill = Button::pill(26.0, 60.0, 8.0);
  let wide = Meter::ladder(16, 18.0, 420.0);
  let thin = Meter::ladder(24, 9.0, 420.0);
  let lying = Meter::ladder(14, 20.0, 250.0);
  let meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(8.0),
    pad.body(),
    pad.seat(3.0),
    round.body(),
    round.seat(2.5),
    key.body(),
    key.seat(2.5),
    pill.body(),
    pill.seat(2.5),
    wide.segment(),
    wide.seat(),
    thin.segment(),
    thin.seat(),
    PushSwitch::square(30.0, 18.0).plunger(),
    PushSwitch::square(30.0, 18.0).collar(),
    PushSwitch::round(30.0, 18.0).plunger(),
    PushSwitch::round(30.0, 18.0).collar(),
    lying.segment(),
    lying.seat(),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.075, 0.08, 0.09)),
  }];
  let mut lamps = Vec::new();
  let seat = Material::matte(Vec3::splat(0.012));

  // each button with the mesh of its body, which its seat's follows, where it is, how far
  // it is lit and in which color, and how far it is held down
  let mut button =
    |shape: Button, mesh: usize, x: f32, y: f32, level: f32, color: Vec3, held: f32| {
      objects.push(Object {
        mesh: mesh + 1,
        transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
        material: seat,
      });
      objects.push(Object {
        mesh,
        transform: parts::button_transform(x, y, shape.height, held),
        material: rubber.lit(color, level, shape.glow_radius()),
      });
      if level > 0.0 {
        lamps.push(shape.lamp(x, y, color, level));
      }
    };
  // a grid of pads, lit here and there, one of them dimly and one held down
  for index in 0..16 {
    let (x, y) = (
      72.0 + 86.0 * (index % 4) as f32,
      82.0 + 86.0 * (index / 4) as f32,
    );
    let level = match index {
      0 | 5 | 6 | 11 | 13 => 1.0,
      9 => 0.35,
      _ => 0.0,
    };
    let held = if index == 6 { 1.0 } else { 0.0 };
    button(pad, 2, x, y, level, colors[index % colors.len()], held);
  }
  for index in 0..5 {
    let level = if index == 1 || index == 4 { 1.0 } else { 0.0 };
    button(
      round,
      4,
      72.0 + 64.5 * index as f32,
      470.0,
      level,
      colors[index + 1],
      0.0,
    );
  }
  for index in 0..4 {
    let level = if index == 2 { 1.0 } else { 0.0 };
    button(
      key,
      6,
      72.0 + 86.0 * index as f32,
      560.0,
      level,
      colors[3],
      0.0,
    );
  }
  for index in 0..12 {
    let (x, y) = (
      440.0 + 44.0 * (index % 6) as f32,
      110.0 + 84.0 * (index / 6) as f32,
    );
    let level = if index % 6 == index / 6 * 3 + 1 {
      1.0
    } else {
      0.0
    };
    button(
      pill,
      8,
      x,
      y,
      level,
      colors[(index + 2) % colors.len()],
      0.0,
    );
  }

  // push switches: out, latched in and lit, and held all the way in, square and then round
  let black = Material::glossy(Vec3::new(0.05, 0.05, 0.055));
  let collar = Material::chrome(Vec3::splat(0.7));
  let switches = [
    (PushSwitch::square(30.0, 18.0), Push::Latching, false, false),
    (PushSwitch::square(30.0, 18.0), Push::Latching, true, false),
    (PushSwitch::square(30.0, 18.0), Push::Momentary, true, true),
    (PushSwitch::round(30.0, 18.0), Push::Latching, false, false),
    (PushSwitch::round(30.0, 18.0), Push::Latching, true, false),
    (PushSwitch::round(30.0, 18.0), Push::Momentary, true, true),
  ];
  for (index, (switch, push, on, held)) in switches.into_iter().enumerate() {
    let (x, y) = (452.0 + 48.0 * index as f32, 300.0);
    let round = index >= 3;
    let (plunger, ring) = if round { (16, 17) } else { (14, 15) };
    objects.push(Object {
      mesh: ring,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: collar,
    });
    let amber = Vec3::new(1.0, 0.55, 0.05);
    let level = if on { 1.0 } else { 0.0 };
    objects.push(Object {
      mesh: plunger,
      transform: switch.transform(x, y, switch.depth(push, on, held)),
      material: if round {
        black
      } else {
        rubber.lit(amber, level, switch.plunger.glow_radius())
      },
    });
    if on && !round {
      lamps.push(switch.plunger.lamp(x, y, amber, 0.6));
    }
  }

  // two pairs of meters, each at its own level, the first pair holding a peak
  let foot = 540.0;
  let meters = [
    (wide, 10, 760.0, 0.78, Some(0.92)),
    (wide, 10, 792.0, 0.64, Some(0.80)),
    (thin, 12, 870.0, 0.42, None),
    (thin, 12, 888.0, 1.0, None),
    (thin, 12, 940.0, 0.0, None),
    (thin, 12, 958.0, 0.18, None),
  ];
  for (meter, mesh, x, level, peak) in meters {
    objects.extend(meter.seats(mesh + 1, x, foot));
    objects.extend(meter.lights(mesh, x, foot, level, peak));
    lamps.extend(meter.lamp(x, foot, level));
  }
  // and a row of lights lying on its side, which is a meter turned a quarter
  let across =
    Mat4::from_translation(Vec3::new(440.0, 420.0, 0.0)) * Mat4::from_rotation_z(FRAC_PI_2);
  let turned = |object: Object| Object {
    transform: across * object.transform,
    ..object
  };
  objects.extend(lying.seats(19, 0.0, 0.0).into_iter().map(turned));
  objects.extend(
    lying
      .lights(18, 0.0, 0.0, 0.7, None)
      .into_iter()
      .map(turned),
  );

  let inset = 18.0;
  for (x, y) in [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ] {
    objects.push(Object {
      mesh: 1,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::chrome(Vec3::splat(0.75)),
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light {
      color: Vec3::splat(0.6),
      ambient: Vec3::splat(0.2),
      ..Light::default()
    },
    look,
    meshes: Arc::from(meshes),
    objects,
    lamps,
  }
}

/// a panel being put together: its meshes, what stands where, and what is lit
#[derive(Default)]
struct Build {
  meshes: Vec<skeuo::Mesh>,
  objects: Vec<Object>,
  lamps: Vec<skeuo::Lamp>,
}

impl Build {
  /// takes a mesh in and says which it is, to place as often as wanted
  fn mesh(&mut self, mesh: skeuo::Mesh) -> usize {
    self.meshes.push(mesh);
    self.meshes.len() - 1
  }

  fn place(&mut self, mesh: usize, transform: Mat4, material: Material) {
    self.objects.push(Object {
      mesh,
      transform,
      material,
    });
  }

  /// a mesh that goes in one place only
  fn put(&mut self, mesh: skeuo::Mesh, transform: Mat4, material: Material) {
    let mesh = self.mesh(mesh);
    self.place(mesh, transform, material);
  }

  /// lettering centered on a point at a height off the panel
  fn letters(&mut self, text: &str, at: Vec3, tall: f32, material: Material) {
    let weight = (tall * 0.17).max(1.0);
    self.put(
      parts::legend(text, tall, weight),
      Mat4::from_translation(at),
      material,
    );
  }
}

/// a soft synth's front in the manner of the mid 2000s: a saturated plastic panel in a rim
/// with raised and titled sections, soft-touch knobs in dashed rings seen from in front,
/// small round lights, a lit screen, lettering everywhere, and a wide soft light on it all
fn synth(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let blue = Vec3::new(0.13, 0.4, 0.74);
  let mut build = Build::default();
  let at = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
  // how high a section stands, which everything on one stands on
  const UP: f32 = 3.0;

  let plastic = |color: Vec3| Material {
    grain: 0.22,
    ..Material::satin(color)
  };
  let ink = Material::matte(Vec3::splat(0.92));
  let dark_ink = Material::matte(Vec3::new(0.04, 0.07, 0.16));
  let hole = Material::matte(Vec3::splat(0.012));

  // the panel, the rim around it, and a darker strip along the bottom under the rim
  build.put(
    parts::plate(width, height, 6.0),
    Mat4::IDENTITY,
    plastic(blue * 0.72),
  );
  build.put(
    parts::frame(width, height, 9.0, 9.0, 16.0),
    at(width / 2.0, height / 2.0, 0.0),
    Material::glossy(blue * 0.42),
  );

  // the sections, each with a white banner carrying its name and slanted bars after it
  let tall = build.mesh(parts::slab(300.0, 318.0, UP, 12.0));
  let low = build.mesh(parts::slab(300.0, 196.0, UP, 12.0));
  let wide = build.mesh(parts::slab(336.0, 196.0, UP, 12.0));
  let banner = build.mesh(parts::slab(150.0, 15.0, 1.2, 3.0));
  let slant = build.mesh(skeuo::Mesh::cuboid(
    Vec3::new(-1.3, -6.0, 0.0),
    Vec3::new(1.3, 6.0, 0.4),
  ));
  let sections = [
    (tall, 172.0, 196.0, 159.0, "FILTER MOD"),
    (tall, 852.0, 196.0, 159.0, "AMP MOD"),
    (low, 172.0, 508.0, 98.0, "MASTER FILTER"),
    (wide, 512.0, 508.0, 98.0, "DELAY"),
    (low, 852.0, 508.0, 98.0, "OUTPUT"),
  ];
  for (mesh, x, y, half, name) in sections {
    build.place(mesh, at(x, y, 0.0), plastic(blue));
    let (left, top) = (x - 62.0, y - half + 15.0);
    build.place(banner, at(left, top, UP), Material::satin(Vec3::splat(0.9)));
    build.letters(name, Vec3::new(left - 6.0, top, UP + 1.2), 7.0, dark_ink);
    for bar in 0..4 {
      let lean = Mat4::from_rotation_z(0.5);
      build.place(
        slant,
        at(left + 86.0 + 7.0 * bar as f32, top, UP) * lean,
        ink,
      );
    }
    // a line cut across the section under its banner, with its lower lip catching light
    build.put(
      skeuo::Mesh::cuboid(Vec3::new(-136.0, -0.6, 0.0), Vec3::new(136.0, 0.6, 0.25)),
      at(x, top + 13.0, UP),
      Material::matte(blue * 0.35),
    );
    build.put(
      skeuo::Mesh::cuboid(Vec3::new(-136.0, 0.6, 0.0), Vec3::new(136.0, 1.5, 0.25)),
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
  let body = build.mesh(knob.body());
  let face = build.mesh(knob.cap(0.82));
  let line = build.mesh(knob.line());
  let ring = build.mesh(parts::dashes(17, parts::KNOB_SWEEP, 27.0, 3.0, 0.62));
  let dial = |build: &mut Build, x: f32, y: f32, turn: f32, name: &str| {
    let turned = at(0.0, 0.0, UP) * parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    build.place(ring, at(x, y, UP), ink);
    build.place(body, turned, Material::satin(Vec3::splat(0.2)));
    build.place(face, turned, Material::satin(Vec3::splat(0.36)));
    build.place(line, turned, ink);
    build.letters(name, Vec3::new(x, y + 36.0, UP), 6.5, ink);
  };
  let names = [
    ["", "ENV", "CUTOFF", "RES", "ATK", "DEC", "SUS", "REL"],
    ["", "PAN", "SPIKE", "SPREAD", "ATK", "DEC", "SUS", "REL"],
  ];
  for (section, left) in [(0, 117.0), (1, 797.0)] {
    for (place, name) in names[section].into_iter().enumerate().skip(1) {
      let (x, y) = (
        left + 110.0 * (place % 2) as f32,
        104.0 + 72.0 * (place / 2) as f32,
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
  for (section, left, count) in [(0, 146.0, 2), (1, 398.0, 4), (2, 762.0, 2)] {
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

  // the switches, each a low key in its seat with a small light over it and a name under
  let key = Button::key(38.0, 38.0, 5.0);
  let (key_body, key_seat) = (build.mesh(key.body()), build.mesh(key.seat(2.0)));
  let (light, light_seat) = (
    build.mesh(parts::led(11.0)),
    build.mesh(parts::led_seat(11.0)),
  );
  let green = Vec3::new(0.55, 1.0, 0.2);
  let switches = [
    (117.0, 110.0, true, "ON"),
    (797.0, 110.0, true, "ON"),
    (82.0, 516.0, false, "LINK"),
    (398.0, 484.0, true, "ON"),
    (398.0, 556.0, false, "SYNC"),
  ];
  for (x, y, lit, name) in switches {
    build.place(key_seat, at(x, y, UP), hole);
    build.place(key_body, at(x, y, UP), Material::satin(Vec3::splat(0.22)));
    build.letters(name, Vec3::new(x, y + 30.0, UP), 6.5, ink);
    let level = if lit { 1.0 } else { 0.0 };
    build.place(light_seat, at(x, y - 31.0, UP), hole);
    build.place(
      light,
      at(x, y - 31.0, UP),
      parts::led_material(green, level, 11.0),
    );
    if lit {
      build.lamps.push(skeuo::Lamp {
        at: Vec3::new(x, y - 31.0, 12.0),
        color: green * 2.5,
        reach: 40.0,
      });
    }
  }

  // the output's two columns of lights with a scale beside them
  let dots = Meter::dots(8, 9.0, 96.0);
  let (dot, dot_seat) = (build.mesh(dots.segment()), build.mesh(dots.seat()));
  let raised = |object: Object| Object {
    transform: at(0.0, 0.0, UP) * object.transform,
    ..object
  };
  for (x, level) in [(928.0, 0.42), (941.0, 0.55)] {
    build
      .objects
      .extend(dots.seats(dot_seat, x, 594.0).into_iter().map(raised));
    build.objects.extend(
      dots
        .lights(dot, x, 594.0, level, None)
        .into_iter()
        .map(raised),
    );
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
    skeuo::Mesh::cuboid(Vec3::new(-138.0, -106.0, 0.0), Vec3::new(138.0, 106.0, 1.0)),
    at(middle, top + 26.0, 5.0),
    Material::pane(glass, 300.0).lit(Vec3::new(0.1, 0.26, 0.56), 0.16, 0.0),
  );
  let glow = Material::matte(shown).lit(shown, 0.6, 0.0);
  let dim = Material::matte(shown * 0.5).lit(shown * 0.6, 0.3, 0.0);
  let on_glass = 6.1;
  // a row of tabs, two rows of settings, a grid of steps, and the patch's name in large
  for (tab, name) in ["ARP", "MOD", "LIB", "SYS"].into_iter().enumerate() {
    let x = middle - 102.0 + 68.0 * tab as f32;
    let shade = if tab == 0 { glow } else { dim };
    build.letters(name, Vec3::new(x, top - 62.0, on_glass), 7.0, shade);
  }
  build.put(
    skeuo::Mesh::cuboid(Vec3::new(-128.0, -0.5, 0.0), Vec3::new(128.0, 0.5, 0.1)),
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
      skeuo::Mesh::cuboid(Vec3::new(0.0, -3.5, 0.0), Vec3::new(88.0, 3.5, 0.1)),
      at(middle + 36.0, y, on_glass),
      Material::matte(shown * 0.2).lit(shown * 0.3, 0.15, 0.0),
    );
    build.put(
      skeuo::Mesh::cuboid(Vec3::new(0.0, -3.5, 0.0), Vec3::new(filled, 3.5, 0.15)),
      at(middle + 36.0, y, on_glass),
      glow,
    );
  }
  let step = build.mesh(skeuo::Mesh::cuboid(
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
      build.place(step, at(x, y, on_glass), shade);
    }
  }
  build.put(
    skeuo::Mesh::cuboid(Vec3::new(-128.0, -0.5, 0.0), Vec3::new(128.0, 0.5, 0.1)),
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
  let screw = build.mesh(parts::screw(7.0));
  let screw_seat = build.mesh(Button::round(14.0, 1.0).seat(2.5));
  for (x, y) in [
    (20.0, 130.0),
    (20.0, 500.0),
    (width - 20.0, 130.0),
    (width - 20.0, 500.0),
  ] {
    build.place(screw_seat, at(x, y, 0.0), hole);
    build.place(
      screw,
      at(x, y, 0.0) * Mat4::from_rotation_z(x + y),
      Material::chrome(Vec3::splat(0.55)),
    );
  }
  let vent = build.mesh(skeuo::Mesh::cuboid(
    Vec3::new(-9.0, -1.3, 0.0),
    Vec3::new(9.0, 1.3, 0.2),
  ));
  for slot in 0..14 {
    for side in [338.0, 686.0] {
      let x = side + (slot % 7) as f32 * 0.0;
      let y = 372.0 + 5.0 * (slot % 7) as f32;
      let aside = if slot < 7 { -11.0 } else { 11.0 };
      build.place(vent, at(x + aside, y, 0.0), hole);
    }
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.5),
    light: Light {
      toward: Vec3::new(-0.3, -0.5, 1.0),
      color: Vec3::splat(0.8),
      ambient: Vec3::splat(0.42),
      falloff: 0.35,
      softness: 7.0,
      ..Light::default()
    },
    look: Look {
      soften: 0.5,
      bloom: 1.0,
      ..look
    },
    meshes: Arc::from(build.meshes),
    objects: build.objects,
    lamps: build.lamps,
  }
}

fn main() {
  let mut arguments = std::env::args().skip(1).peekable();
  let path = arguments.next().unwrap_or_else(|| "picture.png".to_owned());
  let scene: fn(Look) -> Scene = match arguments.next_if(|panel| panel.parse::<f32>().is_err()) {
    None => knobs,
    Some(panel) if panel == "knobs" => knobs,
    Some(panel) if panel == "faders" => faders,
    Some(panel) if panel == "lights" => lights,
    Some(panel) if panel == "synth" => synth,
    Some(panel) => panic!("no panel called {panel}, only knobs, faders, lights and synth"),
  };
  let look = Look {
    pixel_size: arguments
      .next()
      .map_or(1.0, |size| size.parse().expect("a pixel size")),
    color_steps: arguments
      .next()
      .map_or(0, |steps| steps.parse().expect("a count of steps")),
    ..Look::default()
  };

  let instance = wgpu::Instance::default();
  let adapter =
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
      .expect("no graphics adapter");
  eprintln!("drawing on {}", adapter.get_info().name);
  let (device, queue) =
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
      .expect("no graphics device");

  let format = wgpu::TextureFormat::Rgba8UnormSrgb;
  let target = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d {
      width: SIZE[0],
      height: SIZE[1],
      depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    view_formats: &[],
  });

  let mut renderer = Renderer::new(&device, format);
  renderer.prepare(&device, &queue, &scene(look), SIZE);
  let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
  renderer.render(
    &mut encoder,
    &target.create_view(&wgpu::TextureViewDescriptor::default()),
    Place {
      x: 0.0,
      y: 0.0,
      width: SIZE[0] as f32,
      height: SIZE[1] as f32,
    },
    [0, 0, SIZE[0], SIZE[1]],
  );

  // a row of 1024 rgba pixels is already a multiple of the 256 bytes wgpu copies rows by
  let row = SIZE[0] * 4;
  let read = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: u64::from(row * SIZE[1]),
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  encoder.copy_texture_to_buffer(
    target.as_image_copy(),
    wgpu::TexelCopyBufferInfo {
      buffer: &read,
      layout: wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(row),
        rows_per_image: None,
      },
    },
    target.size(),
  );
  queue.submit([encoder.finish()]);
  read.slice(..).map_async(wgpu::MapMode::Read, |result| {
    result.expect("the picture to map")
  });
  device
    .poll(wgpu::PollType::wait_indefinitely())
    .expect("the device to finish");

  let file = std::fs::File::create(&path).expect("the png to open");
  let mut png = png::Encoder::new(std::io::BufWriter::new(file), SIZE[0], SIZE[1]);
  png.set_color(png::ColorType::Rgba);
  let mut writer = png.write_header().expect("the png to start");
  writer
    .write_image_data(&read.slice(..).get_mapped_range())
    .expect("the png to write");
  eprintln!("wrote {path}");
}
