//! A small instrument's face and its settings.
//!
//! `just example demo` opens them in a window. `just render demo out.png` draws the face to a
//! file in place of the window, and `just render demo out.png 2 --page 1` draws the settings.
//!
//! The face has one of each kind of control, and each changes something on the panel. A thread
//! standing in for a plugin's audio thread writes a sound into a [`Feed`], which the needle
//! meter and the scope show.
//!
//! - `LAMP`: a knob in a ring of lights sets how bright the lamp between the knobs is, a second
//!   knob its color, a switch turns it on, and the pad flashes it while it is held
//! - `LEVEL`: a fader sets how loud the sound is, and the needle meter and the scope show it
//! - `LIGHT`: the puck of the xy pad sets where the light over the panel comes from, and the
//!   knob under it, which clicks from stop to stop, how much of it bounces between neighbors
//!
//! `SETUP` shows the settings of the plugin the demo stands in for, and `DONE` the face again:
//! oversampling on a selector, the number of voices on a stepper, and a switch.

mod shared;

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{
  self, Arrow, Button, Fader, Knob, LedRing, LedStops, Pad, PushSwitch, Screen, Selector, Stepper,
  Vu,
};
use potpot::{Light, Look, Material, Mesh, Renderer, Scene};
use potpot_window::controls::flat::Flat;
use potpot_window::controls::interaction::{Control, Dial, Drag, Push, Readout, Target, Touch};
use potpot_window::{Feed, Gpu, Settings, Skin, View, wgpu};

use crate::shared::{ALUMINUM, BLACK, Drawn, Picture, Plot, SEAT, key, key_ink, rubber};

const SIZE: [f32; 2] = [900.0, 480.0];
// the pages: the face, and the settings behind it
const FACE: usize = 0;
const SETUP: usize = 1;
// the parameters. the light across and up are the xy pad's, and the last three the settings'
const BRIGHT: usize = 0;
const HUE: usize = 1;
const BOUNCE: usize = 2;
const LEVEL: usize = 3;
const ON: usize = 4;
const FLASH: usize = 5;
const LIGHT_ACROSS: usize = 6;
const LIGHT_UP: usize = 7;
const OVERSAMPLING: usize = 8;
const VOICES: usize = 9;
const LOW_LATENCY: usize = 10;
// the controls. those up to the xy pad are on the parameter at their own place
const XY_CONTROL: usize = 6;
const SELECTOR_CONTROL: usize = 7;
const STEPPER_CONTROL: usize = 8;
const LATENCY_CONTROL: usize = 9;
// the slots of the feed: how loud the sound is on the left and on the right, and then the
// last stretch of the wave, which the scope draws
const LEFT: usize = 0;
const RIGHT: usize = 1;
const WAVE: usize = 2;
/// The count of the wave's samples in the feed.
const SAMPLES: usize = 64;
/// The time between two blocks of the sound the meters show.
const BLOCK: Duration = Duration::from_millis(10);
/// The count of blocks of the sound that a picture is drawn after.
const PICTURED_AFTER: u32 = 60;
/// The light that bounces with its knob at the last stop.
const MOST_BOUNCE: f32 = 6.0;
/// The light's reach to a side, for each unit of its height.
///
/// The first is with the puck of the xy pad at the left or the foot of its room, and the second
/// with it at the right or the top.
const LIGHT_REACH: [f32; 2] = [-1.0, 1.0];
/// The color of what is lit that has no color of its own.
const AMBER: Vec3 = Vec3::new(1.0, 0.55, 0.05);
/// The color of the panel's powder coat, on both pages.
const PLATE: Vec3 = Vec3::new(0.30, 0.31, 0.33);

// the face's three sections, their left and right edges and the line under their names
const SECTIONS: [(&str, [f32; 2]); 3] = [
  ("LAMP", [40.0, 320.0]),
  ("LEVEL", [340.0, 620.0]),
  ("LIGHT", [640.0, 860.0]),
];
const SECTION_Y: f32 = 72.0;
// the middles of the two rows the face's parts stand in, each part's name the same distance
// under it, and the readouts of the knobs in the upper row
const UPPER: f32 = 158.0;
const LOWER: f32 = 341.0;
const UPPER_NAMES: f32 = UPPER + 62.0;
const LOWER_NAMES: f32 = LOWER + 66.0;
const READOUTS: f32 = UPPER + 88.0;
/// Half a readout's width and height.
const READOUT: Vec2 = Vec2::new(30.0, 10.0);
const LETTERS: f32 = 8.0;
// the knobs and the lamp between two of them, and its size
const BRIGHT_AT: Vec2 = Vec2::new(105.0, UPPER);
const LAMP: Vec2 = Vec2::new(180.0, UPPER);
const HUE_AT: Vec2 = Vec2::new(255.0, UPPER);
const BOUNCE_AT: Vec2 = Vec2::new(750.0, LOWER);
const LAMP_SIZE: f32 = 16.0;
// the switch that turns the lamp on and the pad that flashes it
const SWITCH_AT: Vec2 = Vec2::new(105.0, LOWER);
const FLASH_AT: Vec2 = Vec2::new(255.0, LOWER);
// the needle meter, the fader and how far it slides, and the scope
const VU_AT: Vec2 = Vec2::new(480.0, UPPER);
const VU_FACE: Vec2 = Vec2::new(140.0, 70.0);
const FADER: Vec2 = Vec2::new(395.0, LOWER);
const TRAVEL: f32 = 72.0;
const SCOPE: Vec2 = Vec2::new(530.0, LOWER);
const SCOPE_SIZE: Vec2 = Vec2::new(130.0, 88.0);
/// The middle of the xy pad, and its width and height.
const XY: Vec2 = Vec2::new(750.0, UPPER);
const XY_SIZE: Vec2 = Vec2::new(150.0, 88.0);
/// The middle of the key that shows the other page.
///
/// It is the same on both pages.
const PAGE_KEY: Vec2 = Vec2::new(826.0, 36.0);

// the settings: the field they are set out on, the middles of its rows, and where the names
// and the parts of each row go across it
const FIELD: Vec2 = Vec2::new(450.0, 260.0);
const FIELD_SIZE: Vec2 = Vec2::new(600.0, 300.0);
const ROWS: [f32; 3] = [170.0, 260.0, 350.0];
const NAMES_X: f32 = 300.0;
const PARTS_X: f32 = 590.0;
/// What each choice of the oversampling is called.
const FACTORS: [&str; 4] = ["1X", "2X", "4X", "8X"];

/// Returns the knob that sets the lamp's brightness.
fn bright_knob() -> Knob {
  Knob::round(26.0, 24.0)
}

/// Returns the ring of lights around the brightness knob.
fn ring() -> LedRing {
  LedRing::new(21, bright_knob().radius + 16.0, 5.0)
}

/// Returns the knob that sets the lamp's color.
fn hue_knob() -> Knob {
  Knob::fluted(26.0, 24.0)
}

/// Returns the knob that sets how much light bounces.
fn bounce_knob() -> Knob {
  Knob::knurled(24.0, 24.0)
}

/// Returns the lights at the stops of the bounce knob.
fn stops() -> LedStops {
  LedStops::new(5, bounce_knob().radius + 16.0, 6.0)
}

/// Returns the switch that turns the lamp on.
fn switch() -> PushSwitch {
  PushSwitch::square(44.0, 22.0)
}

/// Returns the switch of the low latency setting.
fn latency_switch() -> PushSwitch {
  PushSwitch::square(30.0, 16.0)
}

/// Returns the key that shows the other page.
fn page_key() -> Button {
  Button::key(64.0, 24.0, 6.0)
}

fn selector() -> Selector {
  Selector::new(FACTORS.len(), 56.0, 28.0, 6.0)
}

/// Returns the stepper of the number of voices.
fn stepper() -> Stepper {
  Stepper::new(56.0, 22.0, 7.0)
}

/// Returns every parameter, and every control with the parameter it is on.
fn wiring() -> (Vec<Dial>, Vec<Control>) {
  let switch = |default| Dial::linear([0.0, 1.0], default, Some(1.0), 0, "");
  // the light starts where it is by default, which is up the panel and to the left
  let toward = Light::default().toward;
  let factors = FACTORS.len() as f32 - 1.0;
  let mut oversampling = Dial::new(
    1.0 / factors,
    move |value| value * factors,
    move |plain| (plain / factors).clamp(0.0, 1.0),
    move |value| format!("{}X", 1_u32 << (value * factors).round() as u32),
    move |text| {
      let times: f32 = text.trim().trim_end_matches(['x', 'X']).parse().ok()?;
      Some((times.log2() / factors).clamp(0.0, 1.0))
    },
  );
  oversampling.step = Some(1.0);
  let dials = vec![
    Dial::linear([0.0, 100.0], 80.0, None, 0, "%"),
    Dial::linear([0.0, 360.0], 8.0, None, 0, ""),
    Dial::linear([0.0, 4.0], 2.0, Some(1.0), 0, ""),
    Dial::linear([0.0, 100.0], 70.0, None, 0, "%"),
    switch(1.0),
    switch(0.0),
    Dial::linear(LIGHT_REACH, toward.x, None, 2, ""),
    Dial::linear(LIGHT_REACH, -toward.y, None, 2, ""),
    oversampling,
    Dial::linear([1.0, 16.0], 8.0, Some(1.0), 0, ""),
    switch(0.0),
  ];

  let readout = Readout::new(5);
  let on = |parameter, page| {
    let mut control = Control::new(parameter, readout);
    control.page = page;
    control
  };
  let pressed = |parameter, push, page| {
    let mut control = on(parameter, page);
    control.touch = Touch::Press(push);
    control
  };
  let mut fader = on(LEVEL, FACE);
  // the cap goes as far as the pointer does, along the way it slides
  fader.feel.drag = Drag::Along {
    way: [0.0, -1.0],
    turn: 1.0 / TRAVEL,
  };
  // dragged over the room the puck's middle has
  let room = xy().travel().into();
  let controls = vec![
    on(BRIGHT, FACE),
    on(HUE, FACE),
    on(BOUNCE, FACE),
    fader,
    pressed(ON, Push::Latching, FACE),
    pressed(FLASH, Push::Momentary, FACE),
    Control::pad(LIGHT_ACROSS, LIGHT_UP, room, readout),
    on(OVERSAMPLING, SETUP),
    on(VOICES, SETUP),
    pressed(LOW_LATENCY, Push::Latching, SETUP),
  ];
  (dials, controls)
}

fn xy() -> Pad {
  Pad::new(XY_SIZE.x, XY_SIZE.y)
}

/// Returns the color a part of the way around the color wheel.
///
/// A whole `turn` goes from red through green and blue back to red. The color is as bright
/// as a color gets.
fn hue(turn: f32) -> Vec3 {
  let around = turn * 6.0;
  let color = Vec3::new(
    (around - 3.0).abs() - 1.0,
    2.0 - (around - 2.0).abs(),
    2.0 - (around - 4.0).abs(),
  );
  color.clamp(Vec3::splat(0.04), Vec3::ONE)
}

/// Returns the transform of a place on the panel.
fn at(place: Vec2) -> Mat4 {
  Mat4::from_translation(place.extend(0.0))
}

/// Returns the trace of a wave on the scope.
///
/// The wave's samples run from minus one to one, across the scope from the left.
fn traced(scope: &Screen, wave: &[f32; SAMPLES]) -> Flat {
  let plot = Plot::of(scope, SCOPE, 8.0);
  let mut flat = Flat::default();
  plot.divide(&mut flat, 8, 4);
  let last = (SAMPLES - 1) as f32;
  let trace: Vec<[f32; 2]> = (wave.iter().enumerate())
    .map(|(sample, height)| plot.at([sample as f32 / last, 0.5 + 0.45 * height]))
    .collect();
  flat.curve(&trace, 1.8, [0.4, 1.0, 0.5, 1.0]);
  flat
}

/// The meshes of the face that are placed anew each frame.
struct Face {
  /// What never moves or changes.
  fixed: Drawn,
  /// Each knob's body and line, in the order of the controls.
  knobs: [[Arc<Mesh>; 2]; 3],
  ring: LedRing,
  ring_light: Arc<Mesh>,
  stops: LedStops,
  stop_light: Arc<Mesh>,
  lamp: Arc<Mesh>,
  plunger: Arc<Mesh>,
  flash: Button,
  pad: Arc<Mesh>,
  /// The fader's cap and its stripe.
  cap: [Arc<Mesh>; 2],
  vu: Vu,
  scope: Screen,
  /// The place of the scope's face among the objects that never move.
  scope_face: usize,
  /// What the scope shows.
  ///
  /// It is made again when the wave it is drawn from differs.
  wave: Picture<[f32; SAMPLES]>,
  xy: Pad,
  puck: Arc<Mesh>,
}

/// The meshes of the settings that are placed anew each frame.
struct Setup {
  /// What never moves or changes.
  fixed: Drawn,
  selector: Selector,
  segment: Arc<Mesh>,
  /// The name of each choice of the selector.
  factors: Vec<Arc<Mesh>>,
  stepper: Stepper,
  arrow: Arc<Mesh>,
  /// The minus and the plus on the stepper's keys.
  signs: [Arc<Mesh>; 2],
  plunger: Arc<Mesh>,
}

/// The key that shows the other page, and its names.
struct PageKey {
  key: Button,
  body: Arc<Mesh>,
  /// `SETUP` on the face and `DONE` on the settings.
  names: [Arc<Mesh>; 2],
}

/// The panel as potpot draws it.
struct Demo {
  /// The renderer, which also finds the pointer on the scene.
  renderer: Renderer,
  /// What each of that scene's objects is, in the same order.
  targets: Vec<Option<Target>>,
  face: Face,
  setup: Setup,
  page_key: PageKey,
}

/// Returns a page with its plate, title and screws alone.
fn plate(title: &str) -> Drawn {
  let size = Vec2::from(SIZE);
  let mut drawn = Drawn::default();
  let plate = parts::plate(size.x, size.y, 6.0);
  drawn.put(plate, Vec2::ZERO, Material::powder_coat(PLATE));
  drawn.put(
    parts::legend(title, 11.0, 2.0),
    Vec2::new(size.x / 2.0, 36.0),
    ink(),
  );
  drawn.screws(size, 18.0, 7.0);
  let seat = page_key().seat(2.5);
  drawn.put(seat, PAGE_KEY, Material::matte(SEAT));
  drawn
}

/// Returns the ink the panel is printed in.
fn ink() -> Material {
  Material::ink(Vec3::splat(0.9))
}

/// Returns a line printed across the panel at `y`.
///
/// It runs from `from` to `to` points across.
fn rule(from: f32, to: f32, y: f32) -> Mesh {
  Mesh::cuboid(Vec3::new(from, y - 0.5, 0.0), Vec3::new(to, y + 0.5, 0.02))
}

impl Face {
  fn new() -> Self {
    let mut fixed = plate("POTPOT");
    let hole = Material::matte(SEAT);
    let chrome = Material::chrome(Vec3::splat(0.8));
    for (name, [left, right]) in SECTIONS {
      let middle = (left + right) / 2.0;
      fixed.letters(name, Vec2::new(middle, SECTION_Y), 7.0, ink());
      fixed.put(
        rule(left + 10.0, right - 10.0, SECTION_Y + 10.0),
        Vec2::ZERO,
        ink(),
      );
    }

    // the knobs' rings, scale and stops, their names, and the windows of their values
    let ring = ring();
    fixed.extend(ring.seats(&Arc::new(ring.seat()), BRIGHT_AT.x, BRIGHT_AT.y));
    let radius = hue_knob().radius;
    let scale = parts::ticks(11, parts::KNOB_SWEEP, radius + 5.0, radius + 10.0, 5);
    fixed.put(scale, HUE_AT, ink());
    let stops = stops();
    fixed.extend(stops.seats(&Arc::new(stops.seat()), BOUNCE_AT.x, BOUNCE_AT.y));
    for (control, middle) in [(BRIGHT, BRIGHT_AT), (HUE, HUE_AT)] {
      fixed.readout(Vec2::new(middle.x, READOUTS), READOUT, control);
    }

    // the lamp in its bezel, the switch's collar and the pad's seat, and their names
    fixed.put(parts::led_seat(LAMP_SIZE), LAMP, hole);
    let bezel = parts::bezel(LAMP_SIZE * 0.56 + 0.4, LAMP_SIZE * 0.56 + 3.0, 2.0);
    fixed.put(bezel, LAMP, chrome);
    let switch = switch();
    fixed.put(switch.collar(), SWITCH_AT, chrome);
    let flash = Button::pad(64.0, 13.0);
    fixed.put(flash.seat(2.5), FLASH_AT, hole);

    // the fader's slot and scale, and the scope's dark face in its bezel
    fixed.put(parts::fader_slot(TRAVEL, 5.0), FADER, hole);
    fixed.put(parts::fader_ticks(11, TRAVEL, 16.0, 22.0, 5), FADER, ink());
    let scope = Screen::new(SCOPE_SIZE.x, SCOPE_SIZE.y);
    let scope_face = fixed.objects.len();
    fixed.put(scope.face(), SCOPE, shared::glass());
    fixed.put(scope.bezel(), SCOPE, Material::glossy(BLACK));

    // the xy pad's surface in its rim
    let xy = xy();
    fixed.put(xy.surface(), XY, Material::glossy(Vec3::splat(0.02)));
    fixed.put(xy.rim(), XY, chrome);

    // each part's name under it, on one line for each row
    let names = [
      ("BRIGHT", BRIGHT_AT.x, UPPER_NAMES),
      ("HUE", HUE_AT.x, UPPER_NAMES),
      ("ANGLE", XY.x, UPPER_NAMES),
      ("ON", SWITCH_AT.x, LOWER_NAMES),
      ("FLASH", FLASH_AT.x, LOWER_NAMES),
      ("LEVEL", FADER.x, LOWER_NAMES),
      ("SCOPE", SCOPE.x, LOWER_NAMES),
      ("BOUNCE", BOUNCE_AT.x, LOWER_NAMES),
    ];
    for (name, x, y) in names {
      fixed.letters(name, Vec2::new(x, y), 7.0, ink());
    }

    let fader = Fader::console(24.0, 38.0, 14.0);
    Self {
      fixed,
      knobs: [bright_knob(), hue_knob(), bounce_knob()]
        .map(|knob| [knob.body(), knob.line()].map(Arc::new)),
      ring_light: Arc::new(ring.segment()),
      ring,
      stop_light: Arc::new(stops.light()),
      stops,
      lamp: Arc::new(parts::led(LAMP_SIZE)),
      plunger: Arc::new(switch.plunger()),
      pad: Arc::new(flash.body()),
      flash,
      cap: [fader.body(), fader.line()].map(Arc::new),
      vu: Vu::new(VU_FACE.x, VU_FACE.y),
      scope,
      scope_face,
      wave: Picture::new(),
      puck: Arc::new(xy.puck()),
      xy,
    }
  }

  /// Shows the wave of `feed` on the scope.
  fn show(&mut self, feed: &[f32]) {
    let wave = feed[WAVE..WAVE + SAMPLES]
      .try_into()
      .expect("the feed has every sample");
    (self.wave).show(wave, |wave| traced(&self.scope, wave));
  }

  /// Adds the face as `values`, `feed` and `held` have it.
  ///
  /// `feed` is the sound, and `held` is the control held down. The scope shows what
  /// [`Face::show`] was last given.
  fn draw(&self, drawn: &mut Drawn, values: &[f32], feed: &[f32], held: Option<usize>) {
    let knob_materials = [
      [Material::glossy(BLACK), Material::matte(Vec3::splat(0.9))],
      [Material::glossy(BLACK), Material::matte(Vec3::splat(0.9))],
      [Material::spun(ALUMINUM), Material::matte(BLACK)],
    ];
    let middles = [BRIGHT_AT, HUE_AT, BOUNCE_AT];
    for (control, ((meshes, materials), middle)) in
      (self.knobs.iter().zip(knob_materials).zip(middles)).enumerate()
    {
      drawn.knob(meshes, materials, middle, values[control], control);
    }
    let lit = |turn: f32| [0.0, turn];
    let ring = self.ring.lights(
      &self.ring_light,
      BRIGHT_AT.x,
      BRIGHT_AT.y,
      lit(values[BRIGHT]),
      AMBER,
    );
    drawn.extend(ring);
    let stops = self.stops.lights(
      &self.stop_light,
      BOUNCE_AT.x,
      BOUNCE_AT.y,
      values[BOUNCE],
      AMBER,
    );
    drawn.extend(stops);

    // the lamp, as bright as its knob has it while it is on and all the way while the pad is
    // held, and the switch that is in and lit while it is on
    let on = values[ON] > 0.5;
    let level = match (values[FLASH] > 0.5, on) {
      (true, _) => 1.0,
      (false, true) => values[BRIGHT],
      (false, false) => 0.0,
    };
    let color = hue(values[HUE]);
    drawn.place(
      &self.lamp,
      at(LAMP),
      parts::led_material(color, level, LAMP_SIZE),
      None,
    );
    drawn.lamps.push(parts::led_lamp(
      LAMP.extend(LAMP_SIZE),
      color,
      level,
      LAMP_SIZE,
      150.0,
    ));
    let switch = switch();
    let depth = Push::Latching.depth(on, held == Some(ON));
    let lit_switch = rubber().lit(color, level, switch.plunger.emission_radius());
    let pushed = switch.transform(SWITCH_AT.x, SWITCH_AT.y, depth);
    drawn.place(&self.plunger, pushed, lit_switch, Some(Target::Control(ON)));
    let flashing = values[FLASH] > 0.5;
    let depth = Push::Momentary.depth(flashing, held == Some(FLASH));
    let flash = rubber().lit(
      color,
      f32::from(u8::from(flashing)),
      self.flash.emission_radius(),
    );
    let pressed = self.flash.transform(FLASH_AT.x, FLASH_AT.y, depth);
    drawn.place(&self.pad, pressed, flash, Some(Target::Control(FLASH)));
    if flashing {
      drawn
        .lamps
        .push(self.flash.lamp(FLASH_AT.x, FLASH_AT.y, color, 1.0));
    }

    // the fader stands where the level is, and the needle meter and the scope show the sound
    drawn.fader(&self.cap, FADER, TRAVEL, values[LEVEL], LEVEL);
    let loudest = feed[LEFT].max(feed[RIGHT]);
    drawn.extend(self.vu.objects(VU_AT.x, VU_AT.y, loudest, 1.0));
    drawn.objects[self.scope_face].picture = vec![self.wave.layer()];
    let green = Vec3::new(0.3, 1.0, 0.45);
    drawn
      .lamps
      .extend(self.scope.lamps(SCOPE.x, SCOPE.y, green, 1.0));

    // the puck of the xy pad
    let [across, up] = [LIGHT_ACROSS, LIGHT_UP].map(|light| values[light]);
    let puck = self.xy.transform(XY.x, XY.y, across, up);
    drawn.place(
      &self.puck,
      puck,
      rubber(),
      Some(Target::Control(XY_CONTROL)),
    );
  }
}

impl Setup {
  fn new() -> Self {
    let mut fixed = plate("SETUP");
    let inset = parts::inset(FIELD_SIZE.x, FIELD_SIZE.y, 10.0, 3.0, 8.0);
    fixed.put(inset, FIELD, Material::powder_coat(PLATE));
    let names = ["OVERSAMPLING", "POLYPHONY", "LOW LATENCY"];
    for (name, y) in names.into_iter().zip(ROWS) {
      fixed.letters(name, Vec2::new(NAMES_X, y), 8.0, ink());
    }
    // a line between each two rows, inside the field's rim
    let [left, right] = [
      FIELD.x - FIELD_SIZE.x / 2.0 + 30.0,
      FIELD.x + FIELD_SIZE.x / 2.0 - 30.0,
    ];
    for pair in ROWS.windows(2) {
      fixed.put(
        rule(left, right, (pair[0] + pair[1]) / 2.0),
        Vec2::ZERO,
        ink(),
      );
    }

    let selector = selector();
    let row = Vec2::new(PARTS_X, ROWS[0]);
    fixed.put(selector.frame(), row, Material::chrome(Vec3::splat(0.7)));
    let stepper = stepper();
    let row = Vec2::new(PARTS_X, ROWS[1]);
    fixed.put(stepper.frame(), row, Material::glossy(BLACK));
    let glass = Arc::new(stepper.glass());
    let pane = Material::pane(Vec3::splat(0.01));
    fixed.place(
      &glass,
      at(row),
      pane,
      Some(Target::Readout(STEPPER_CONTROL)),
    );
    let seat = Arc::new(stepper.seat(2.0));
    for arrow in [Arrow::Back, Arrow::On] {
      fixed.place(
        &seat,
        at(row + stepper.middle(arrow)),
        Material::matte(SEAT),
        None,
      );
    }
    let switch = latency_switch();
    let row = Vec2::new(PARTS_X, ROWS[2]);
    fixed.put(switch.collar(), row, Material::chrome(Vec3::splat(0.8)));

    Self {
      fixed,
      segment: Arc::new(selector.row.body()),
      factors: (FACTORS.iter())
        .map(|factor| Arc::new(shared::lettering(factor, 7.0)))
        .collect(),
      selector,
      arrow: Arc::new(stepper.body()),
      signs: stepper.signs().map(Arc::new),
      stepper,
      plunger: Arc::new(switch.plunger()),
    }
  }

  /// Adds the settings as `values`, `held` and `arrow` have them.
  ///
  /// `arrow` is the stepper's key the left button holds down, if one is.
  fn draw(&self, drawn: &mut Drawn, values: &[f32], held: Option<usize>, arrow: Option<Target>) {
    // the choice the oversampling stands at is in and lit
    let row = self.selector.row;
    let [x, y] = [PARTS_X, ROWS[0]];
    let chosen = (values[OVERSAMPLING] * (FACTORS.len() - 1) as f32).round() as usize;
    for (choice, name) in self.factors.iter().enumerate() {
      let target = Some(Target::Choice {
        control: SELECTOR_CONTROL,
        choice,
      });
      let lit = choice == chosen;
      let segment = row.transform(x, y, choice, lit);
      drawn.place(&self.segment, segment, key(lit), target);
      drawn.place(name, row.name(x, y, choice, lit), key_ink(), target);
    }

    // the stepper's keys, which move the voices a step back and on
    let [x, y] = [PARTS_X, ROWS[1]];
    let arrows = [
      (Arrow::Back, Target::Decrement(STEPPER_CONTROL)),
      (Arrow::On, Target::Increment(STEPPER_CONTROL)),
    ];
    for ((side, target), sign) in arrows.into_iter().zip(&self.signs) {
      let held = arrow == Some(target);
      let pressed = self.stepper.transform(x, y, side, held);
      drawn.place(&self.arrow, pressed, rubber(), Some(target));
      let printed = self.stepper.sign(x, y, side, held);
      drawn.place(sign, printed, key_ink(), Some(target));
    }

    // the switch, in and lit while it is on
    let on = values[LOW_LATENCY] > 0.5;
    let switch = latency_switch();
    let depth = Push::Latching.depth(on, held == Some(LATENCY_CONTROL));
    let lit = rubber().lit(
      AMBER,
      f32::from(u8::from(on)),
      switch.plunger.emission_radius(),
    );
    let pushed = switch.transform(PARTS_X, ROWS[2], depth);
    drawn.place(
      &self.plunger,
      pushed,
      lit,
      Some(Target::Control(LATENCY_CONTROL)),
    );
  }
}

impl PageKey {
  fn new() -> Self {
    let key = page_key();
    Self {
      body: Arc::new(key.body()),
      names: ["SETUP", "DONE"].map(|name| Arc::new(shared::lettering(name, 7.0))),
      key,
    }
  }

  /// Adds the key as it is on `page`.
  ///
  /// A press on it shows the other page.
  fn draw(&self, drawn: &mut Drawn, page: usize) {
    let other = Some(Target::Page(if page == FACE { SETUP } else { FACE }));
    let transform = self.key.transform(PAGE_KEY.x, PAGE_KEY.y, 0.0);
    drawn.place(&self.body, transform, rubber(), other);
    let name = transform * Mat4::from_translation(Vec3::Z * self.key.top());
    drawn.place(&self.names[page], name, key_ink(), other);
  }
}

impl Demo {
  fn new(gpu: &Gpu) -> Self {
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      targets: Vec::new(),
      face: Face::new(),
      setup: Setup::new(),
      page_key: PageKey::new(),
    }
  }

  /// Returns the scene of a page and its objects' targets.
  fn scene(&self, view: &View) -> (Scene, Vec<Option<Target>>) {
    let values = view.values;
    // the scene owns this frame's objects, so the ones that never move are copied into it
    let mut drawn = match view.page {
      FACE => {
        let mut drawn = self.face.fixed.clone();
        self.face.draw(&mut drawn, values, view.feed, view.held);
        drawn
      }
      _ => {
        let mut drawn = self.setup.fixed.clone();
        self.setup.draw(&mut drawn, values, view.held, view.arrow);
        drawn
      }
    };
    self.page_key.draw(&mut drawn, view.page);

    // the light comes from the side of the panel the puck is on, on either page
    let [across, up] = [LIGHT_ACROSS, LIGHT_UP].map(|light| values[light]);
    let [left, right] = LIGHT_REACH;
    let reach = |turn: f32| left + (right - left) * turn;
    let scene = Scene {
      size: SIZE.into(),
      heights: [-8.0, 48.0],
      lean: Vec2::new(0.0, -0.25),
      // dimmed, so that what the lamps do shows
      light: Light {
        toward: Vec3::new(reach(across), -reach(up), 1.0),
        color: Vec3::splat(0.6),
        bounce: values[BOUNCE] * MOST_BOUNCE,
        ..Light::default()
      },
      look: Look::default(),
      objects: drawn.objects,
      lamps: drawn.lamps,
    };
    (scene, drawn.targets)
  }
}

impl Skin for Demo {
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat) {
    self.face.show(view.feed);
    let (scene, targets) = self.scene(view);
    (self.renderer).prepare(gpu.device, gpu.queue, &scene, view.pixels);
    self.targets = targets;

    // each value, written flat over the glass of its window
    let windows = match view.page {
      FACE => vec![
        (BRIGHT, Vec2::new(BRIGHT_AT.x, READOUTS)),
        (HUE, Vec2::new(HUE_AT.x, READOUTS)),
      ],
      _ => vec![(STEPPER_CONTROL, Vec2::new(PARTS_X, ROWS[1]))],
    };
    for (control, middle) in windows {
      shared::written(over, view, control, middle, LETTERS);
    }
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    shared::render(&self.renderer, encoder, target, place);
  }

  fn changed(&self) -> Option<&[[u32; 4]]> {
    self.renderer.changed()
  }

  fn settled(&self) -> bool {
    self.renderer.settled()
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    let object = self.renderer.pick(at.into())?;
    self.targets[object]
  }
}

/// Returns the function that writes one block of the sound.
///
/// It writes how loud a made-up sound is on each side into `feed`, and the last stretch of its
/// wave. The sound swells and dies away, a little later on the right, and is as loud at most as
/// the level among `values` has it. This is what a plugin's audio thread does at the end of each
/// block.
fn sound(feed: Feed, values: Arc<[AtomicU32]>) -> impl FnMut() + Send {
  let mut blocks = 0.0_f32;
  move || {
    let level = shared::value(&values, LEVEL);
    let swell = |behind: f32| ((blocks - behind) * 0.02).sin().max(0.0);
    feed.set(LEFT, level * swell(0.0));
    feed.set(RIGHT, level * swell(20.0));
    // three turns of a wave with a third of its own third harmonic, drifting along each block
    for sample in 0..SAMPLES {
      let turn = std::f32::consts::TAU * (sample as f32 / SAMPLES as f32 * 3.0 + blocks * 0.05);
      let wave = (turn.sin() + (turn * 3.0).sin() / 3.0) * 0.75;
      feed.set(WAVE + sample, level * swell(0.0) * wave);
    }
    blocks += 1.0;
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot", SIZE);
  settings.pages = 2;
  let feed = Feed::new(WAVE + SAMPLES);
  settings.feed = feed.clone();
  let (dials, controls) = wiring();
  let (params, values) = shared::params(dials);
  let sound = shared::window::Sound {
    block: Box::new(sound(feed, values)),
    every: BLOCK,
    pictured_after: PICTURED_AFTER,
  };
  shared::window::run(settings, params, controls, Some(sound), Demo::new)
}
