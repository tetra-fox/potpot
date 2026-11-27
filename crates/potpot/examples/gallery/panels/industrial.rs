//! Dark metal hardware lit from within.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Handle, Knob, LedArc, LedRing, LedStops, PushSwitch, Vent};
use potpot::{Light, Look, Material, Scene};
use potpot_controls::interaction::Push;

use super::{BLACK, Build, SEAT, SIZE, framed};

/// The color of the light built into the hardware.
const CYAN: Vec3 = Vec3::new(0.2, 0.85, 1.0);
/// The color of the panel's dark metal.
const GUNMETAL: Vec3 = Vec3::new(0.41, 0.44, 0.46);

/// Returns the place (`x`, `y`) on the panel.
fn on_panel(x: f32, y: f32) -> Mat4 {
  Mat4::from_translation(Vec3::new(x, y, 0.0))
}

impl Build {
  /// Adds thin lit lettering centered on `at`.
  fn lit_letters(&mut self, text: &str, at: Vec3, tall: f32) {
    let lettering = parts::legend(text, tall, (tall * 0.1).max(0.8));
    let at = Mat4::from_translation(at);
    self.put(lettering, at, Material::lamp(CYAN * 0.8));
  }

  /// Adds a line of light in its groove, through the points of `path`.
  fn light_line(&mut self, path: &[Vec2], level: f32) {
    let seat = Material::matte(SEAT);
    self.put(parts::light_line_seat(path, 1.4, 0.7), Mat4::IDENTITY, seat);
    let light = Material::lamp(CYAN * level);
    self.put(parts::light_line(path, 1.4), Mat4::IDENTITY, light);
  }

  /// Adds a glossy black knob on a chrome skirt at (`x`, `y`), turned to `turn`.
  ///
  /// Its line is lit.
  fn knob(&mut self, knob: Knob, x: f32, y: f32, turn: f32) {
    let skirt = parts::bezel(knob.radius * 0.8, knob.radius * 1.22, knob.height * 0.18);
    self.put(skirt, on_panel(x, y), Material::chrome(Vec3::splat(0.75)));
    let turned = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    self.put(knob.body(), turned, Material::bakelite(BLACK));
    self.put(knob.line(), turned, Material::lamp(CYAN));
  }
}

/// Returns industrial hardware on a dark metal panel.
///
/// - rack handles on blocky mounts, and hex socket screws
/// - inset fields with a cut rim, and a chamfered plate
/// - a big knob in a ring of lights, a small one in an unbroken arc, and one with a light at
///   each stop
/// - backlit push switches out, latched in and held
/// - lines of light inlaid in the panel
/// - slotted vents with light coming through the sheet behind them
/// - a few lit shapes alone, which show the glow on its own
///
/// The light is dimmed so that what glows shows.
pub(super) fn industrial(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let mut build = Build::default();
  let metal = Material::brushed(GUNMETAL);
  build.put(parts::plate(width, height, 6.0), Mat4::IDENTITY, metal);

  // a rack ear at each side, with a handle between two screws
  let handle = Handle::new(250.0, 26.0, 7.0);
  let (bar, mounts) = (Arc::new(handle.bar()), Arc::new(handle.mounts()));
  let screw = [
    Arc::new(parts::cap_screw(7.0)),
    Arc::new(parts::hex_socket(7.0)),
  ];
  let screw_seat = Arc::new(parts::screw_seat(7.0, 1.2));
  for x in [40.0, width - 40.0] {
    let at = Mat4::from_translation(Vec3::new(x, height / 2.0, 0.0));
    build.place(&mounts, at, Material::brushed(Vec3::splat(0.5)));
    build.place(&bar, at, Material::chrome(Vec3::splat(0.7)));
    for y in [60.0, height - 60.0] {
      let at = Mat4::from_translation(Vec3::new(x, y, 0.0));
      build.place(&screw_seat, at, Material::matte(SEAT));
      // turned its own way, which only the socket shows
      let turned = at * Mat4::from_rotation_z(x * 0.3 + y);
      build.place(&screw[0], turned, Material::chrome(Vec3::splat(0.55)));
      build.place(&screw[1], turned, Material::matte(Vec3::splat(0.03)));
    }
  }

  // three fields set into the panel, each named and underlined in light
  let fields = [
    (200.0, 220.0, 240.0, 300.0, "SHIFT"),
    (440.0, 220.0, 200.0, 300.0, "LEVEL"),
    (660.0, 220.0, 200.0, 300.0, "MODE"),
  ];
  for (x, y, wide, tall, name) in fields {
    build.put(
      parts::inset(wide, tall, 10.0, 3.0, 8.0),
      on_panel(x, y),
      metal,
    );
    let top = y - tall / 2.0 + 30.0;
    build.lit_letters(name, Vec3::new(x, top, 0.0), 10.0);
    let line = [
      Vec2::new(x - wide / 2.0 + 24.0, top + 14.0),
      Vec2::new(x + wide / 2.0 - 24.0, top + 14.0),
    ];
    build.light_line(&line, 0.7);
  }

  // a big knob in a ring of lights lit from the middle, a small one in an arc lit from its
  // start, and one with a light at each of its stops
  let (x, y) = (200.0, 240.0);
  let big = Knob {
    skirt: 0.0,
    ..Knob::round(46.0, 30.0)
  };
  let ring = LedRing::new(24, 80.0, 6.0);
  let (segment, seat) = (Arc::new(ring.segment()), Arc::new(ring.seat()));
  build.objects.extend(ring.seats(&seat, x, y));
  build
    .objects
    .extend(ring.lights(&segment, x, y, [0.5, 0.79], CYAN));
  build.knob(big, x, y, 0.79);

  let (x, y) = (440.0, 240.0);
  let small = Knob {
    skirt: 0.0,
    ..Knob::round(32.0, 24.0)
  };
  let arc = LedArc::new(54.0, 3.0);
  let meshes = [Arc::new(arc.seat()), Arc::new(arc.lens())];
  build
    .objects
    .extend(arc.lights(&meshes, x, y, [0.0, 0.62], CYAN));
  build.knob(small, x, y, 0.62);

  let (x, y) = (660.0, 240.0);
  let stops = LedStops::new(5, 54.0, 7.0);
  let (light, seat) = (Arc::new(stops.light()), Arc::new(stops.seat()));
  build.objects.extend(stops.seats(&seat, x, y));
  build
    .objects
    .extend(stops.lights(&light, x, y, stops.turn(1), CYAN));
  build.knob(small, x, y, stops.turn(1));

  // backlit switches out and dark, latched in and lit, and held all the way in
  let switch = PushSwitch::backlit(56.0, 26.0, 8.0);
  let (plunger, collar) = (Arc::new(switch.plunger()), Arc::new(switch.collar()));
  let switches = [
    (Push::Latching, false, false, "OFF"),
    (Push::Latching, true, false, "ON"),
    (Push::Momentary, true, true, "HELD"),
  ];
  for (index, (push, on, held, name)) in switches.into_iter().enumerate() {
    let (x, y) = (870.0, 110.0 + 80.0 * index as f32);
    let at = Mat4::from_translation(Vec3::new(x, y, 0.0));
    build.place(&collar, at, Material::satin(Vec3::splat(0.04)));
    let level = if on { 1.0 } else { 0.0 };
    let pressed = switch.transform(x, y, push.depth(on, held));
    build.place(&plunger, pressed, switch.backlight(CYAN, level));
    if on {
      build.lamps.push(switch.plunger.lamp(x, y, CYAN, 0.7));
    }
    build.lit_letters(name, Vec3::new(x, y + 28.0, 0.0), 7.0);
  }

  // the bottom strip: a vent either side of a chamfered plate with lit lettering, under a
  // line of light that turns down at both ends
  let strip = 470.0;
  build.light_line(
    &[
      Vec2::new(90.0, strip - 40.0),
      Vec2::new(100.0, strip - 30.0),
      Vec2::new(924.0, strip - 30.0),
      Vec2::new(934.0, strip - 40.0),
    ],
    0.5,
  );
  let vent = Vent::new(8, 8.0, 52.0);
  for x in [240.0, 620.0] {
    build.put(
      vent.slots(),
      on_panel(x, strip + 30.0),
      Material::matte(Vec3::splat(0.02)),
    );
    build.put(
      vent.holes(),
      on_panel(x, strip + 30.0),
      Material::lamp(CYAN * 0.5),
    );
  }
  let plate = parts::chamfered_slab(160.0, 44.0, 3.0, 1.5, 3.0);
  build.put(
    plate,
    on_panel(430.0, strip + 30.0),
    Material::brushed(Vec3::splat(0.5)),
  );
  // on the plate's top
  build.lit_letters("POTPOT", Vec3::new(430.0, strip + 30.0, 3.0), 12.0);

  // lit shapes alone on the bare panel, where nothing else catches their glow
  let alone = 790.0;
  build.put(
    parts::led(10.0),
    on_panel(alone, strip + 30.0),
    Material::lamp(CYAN),
  );
  build.light_line(
    &[
      Vec2::new(alone + 30.0, strip + 30.0),
      Vec2::new(alone + 110.0, strip + 30.0),
    ],
    1.0,
  );
  build.lit_letters("GLOW", Vec3::new(alone + 70.0, strip + 52.0, 0.0), 7.0);

  Scene {
    light: Light {
      color: Vec3::splat(0.55),
      ..Light::default()
    },
    lamps: build.lamps,
    ..framed(look, build.objects)
  }
}
