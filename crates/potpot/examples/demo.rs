//! A window with every kind of control on it, opened by `just example demo`.
//!
//! `just render demo out.png` draws it to a file in place of the window.
//!
//! Lamps follow the controls, and the two meters beside the fader follow a [`Feed`] that a
//! thread standing in for a plugin's audio thread writes. Each control answers the pointer the
//! way its kind does: knobs and faders are dragged, buttons are pressed, and the row of keys is
//! one switch that each click moves a step on. The puck of the pad in the corner is dragged both
//! ways at once, and the light over the whole panel comes from where it stands.
//!
//! The six knobs along the top, from the left:
//!
//! - `BRIGHT`: how bright the lamp between the first two knobs is
//! - `HUE`: the lamp's color
//! - `LAMP`: whether the lamp is on
//! - `BOUNCE`: how much light bounces between neighbors
//! - `LEVEL`: where the fader stands, which is how loud the meters beside it show the sound
//! - `DOTS`: how many of a column of dots are lit

mod shared;

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Fader, Knob, Meter, Pad, PushSwitch};
use potpot::{Light, Look, Material, Mesh, Renderer, Scene};
use potpot_window::controls::flat::Flat;
use potpot_window::controls::interaction::{Control, Dial, Drag, Push, Readout, Target, Touch};
use potpot_window::{Feed, Gpu, Settings, Skin, View, wgpu};

use crate::shared::{ALUMINUM, BLACK, Drawn, SEAT};

const SIZE: [f32; 2] = [900.0, 520.0];
// the knobs that drive something, in the order of `knobs`
const BRIGHT: usize = 0;
const HUE: usize = 1;
const ON: usize = 2;
const BOUNCE: usize = 3;
const LEVEL: usize = 4;
const DOTS: usize = 5;
/// The count of knobs that drive something, each with a readout.
const WIRED: usize = 6;
/// The count of knobs after those, one of every other style and a second round one.
const REST: usize = 9;
/// The count of knobs, each a control on the parameter at its own place.
const KNOBS: usize = WIRED + REST;
// the parameters after the knobs' own: the pad that flashes the lamp, the switch the keys are,
// the fader on its side, and how far across and up the light over the panel is. each button of
// `buttons` on none of these has its own after them
const FLASH: usize = KNOBS;
const KEYS: usize = KNOBS + 1;
const SLIDE: usize = KNOBS + 2;
const LIGHT_ACROSS: usize = KNOBS + 3;
const LIGHT_UP: usize = KNOBS + 4;
const BUTTONS: usize = KNOBS + 5;
// the controls after the knobs: the fader on the level, the switch that turns the lamp on, the
// fader on its side, the xy pad, and then one for each of `buttons`
const FADER_CONTROL: usize = KNOBS;
const SWITCH_CONTROL: usize = KNOBS + 1;
const SLIDE_CONTROL: usize = KNOBS + 2;
const XY_CONTROL: usize = KNOBS + 3;
const BUTTON_CONTROLS: usize = KNOBS + 4;
// the slots of the feed: how loud the sound is on the left and on the right
const LEFT: usize = 0;
const RIGHT: usize = 1;
/// The time between two blocks of the sound the meters show.
const BLOCK: Duration = Duration::from_millis(10);
/// The count of blocks of the sound that a picture is drawn after.
const PICTURED_AFTER: u32 = 60;
/// The light that bounces with its knob all the way up.
const MOST_BOUNCE: f32 = 6.0;
/// The light's reach to a side, for each unit of its height.
///
/// The first is with the puck of the xy pad at the left or the foot of its room, and the second
/// with it at the right or the top.
const LIGHT_REACH: [f32; 2] = [-1.0, 1.0];

// the wired knobs' middles down the panel, and the names and readouts under them
const WIRED_Y: f32 = 100.0;
const NAME_UNDER: f32 = 52.0;
const READOUT_UNDER: f32 = 76.0;
/// Half a readout's width and height.
const READOUT: Vec2 = Vec2::new(32.0, 10.0);
const LETTERS: f32 = 8.0;
/// The place of the lamp between the first two knobs.
const LAMP: Vec2 = Vec2::new(150.0, WIRED_Y);
const LAMP_SIZE: f32 = 16.0;
const SWITCH: Vec2 = Vec2::new(398.0, WIRED_Y);
// the fader's middle and how far it slides, and the foot of the meters beside it
const FADER: Vec2 = Vec2::new(690.0, WIRED_Y + 5.0);
const TRAVEL: f32 = 110.0;
const METER_FOOT: f32 = FADER.y + TRAVEL / 2.0 + 10.0;
// how far across the two ladders of lights and the column of dots are
const LADDERS: [f32; 2] = [736.0, 752.0];
const DOTS_X: f32 = 872.0;
/// The distance down the panel to the buttons along the bottom.
const BUTTONS_Y: f32 = 440.0;
// the middle of the fader that lies on its side and how far it slides, and how far down
// the row of small lights over it is
const SLIDE_AT: Vec2 = Vec2::new(678.0, 466.0);
const SLIDE_TRAVEL: f32 = 100.0;
const LIGHTS_Y: f32 = 418.0;
// the middle of the xy pad and the side of its square surface
const XY: Vec2 = Vec2::new(806.0, 446.0);
const XY_SIDE: f32 = 70.0;
const LIGHT_SIZE: f32 = 11.0;
/// The color of each light of the row, from the left.
const LIGHT_COLORS: [Vec3; 4] = [
  Vec3::new(0.1, 0.9, 0.3),
  Vec3::new(0.1, 0.9, 0.3),
  Vec3::new(1.0, 0.75, 0.1),
  Vec3::new(1.0, 0.12, 0.06),
];
const AMBER: Vec3 = Vec3::new(1.0, 0.55, 0.05);

/// A knob of the panel.
struct Shown {
  name: &'static str,
  at: Vec2,
  knob: Knob,
  material: Material,
  /// The color of the line painted on it.
  mark: Vec3,
  /// The parameter it turns.
  dial: Dial,
}

/// Returns the six wired knobs along the top and the rest under them.
///
/// The rest are one of every other style and a second round one.
fn knobs() -> Vec<Shown> {
  let white = Vec3::splat(0.9);
  let percent = || Dial::linear([0.0, 100.0], 50.0, None, 0, "%");
  let wired = |name, x, knob, material, mark, dial| Shown {
    name,
    at: Vec2::new(x, WIRED_Y),
    knob,
    material,
    mark,
    dial,
  };
  let first: [_; WIRED] = [
    wired(
      "BRIGHT",
      80.0,
      Knob::round(30.0, 26.0),
      Material::chrome(Vec3::splat(0.8)),
      BLACK,
      Dial::linear([0.0, 100.0], 80.0, None, 0, "%"),
    ),
    wired(
      "HUE",
      220.0,
      Knob::fluted(30.0, 26.0),
      Material::glossy(BLACK),
      white,
      Dial::linear([0.0, 360.0], 8.0, None, 0, ""),
    ),
    wired(
      "LAMP",
      330.0,
      Knob::skirted(30.0, 26.0),
      Material::glossy(BLACK),
      white,
      Dial::linear([0.0, 1.0], 1.0, Some(1.0), 0, ""),
    ),
    wired(
      "BOUNCE",
      490.0,
      Knob::knurled(26.0, 26.0),
      Material::spun(ALUMINUM),
      BLACK,
      Dial::linear([0.0, MOST_BOUNCE], 3.0, None, 1, ""),
    ),
    wired(
      "LEVEL",
      600.0,
      Knob::dished(30.0, 26.0),
      Material::satin(Vec3::new(0.50, 0.08, 0.07)),
      white,
      Dial::linear([0.0, 100.0], 70.0, None, 0, "%"),
    ),
    wired(
      "DOTS",
      820.0,
      Knob::faceted(30.0, 26.0),
      Material::glossy(Vec3::new(0.06, 0.13, 0.26)),
      white,
      Dial::linear([0.0, 8.0], 5.0, Some(1.0), 0, ""),
    ),
  ];
  let mut knobs = Vec::from(first);
  let rest: [_; REST] = [
    ("DIMPLED", Knob::dimpled(26.0, 14.0)),
    ("CHICKEN", Knob::chicken_head(26.0, 22.0)),
    ("BAR", Knob::bar(26.0, 26.0)),
    ("WALKER", Knob::big_walker(26.0, 18.0)),
    ("RIDGE", Knob::ridge(26.0, 28.0)),
    ("HEX", Knob::hex(26.0, 24.0)),
    ("RIBBED", Knob::ribbed(22.0, 24.0)),
    ("CONICAL", Knob::conical(26.0, 22.0)),
    ("ROUND", Knob::round(26.0, 22.0)),
  ];
  for (index, (name, knob)) in rest.into_iter().enumerate() {
    let material = match index % 3 {
      0 => Material::glossy(BLACK),
      1 => Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      _ => Material::glossy(Vec3::new(0.70, 0.36, 0.04)),
    };
    knobs.push(Shown {
      name,
      at: Vec2::new(70.0 + 95.0 * index as f32, 290.0),
      knob,
      material,
      mark: if index % 3 == 1 { BLACK } else { white },
      dial: percent(),
    });
  }
  knobs
}

/// A button along the bottom.
struct Pressed {
  shape: Button,
  /// The distance across the panel to it.
  x: f32,
  /// The parameter it sets.
  parameter: usize,
  /// Where the parameter stands while the button is lit.
  lit_at: f32,
  /// What pressing it does.
  touch: Touch,
  /// The color it lights in, or `None` for the lamp's.
  color: Option<Vec3>,
}

/// Returns the buttons along the bottom.
///
/// - a pad that flashes the lamp while it is held
/// - three keys that are one switch of three places
/// - two pills that stay in
/// - two round buttons that come back
fn buttons() -> Vec<Pressed> {
  let on = |shape, x, parameter, push| Pressed {
    shape,
    x,
    parameter,
    lit_at: 1.0,
    touch: Touch::Press(push),
    color: Some(AMBER),
  };
  let key = |x, lit_at| Pressed {
    shape: Button::key(60.0, 26.0, 7.0),
    x,
    parameter: KEYS,
    lit_at,
    touch: Touch::Step,
    color: Some(AMBER),
  };
  let pill = Button::pill(24.0, 54.0, 8.0);
  let round = Button::round(40.0, 10.0);
  vec![
    Pressed {
      color: None,
      ..on(Button::pad(60.0, 11.0), 80.0, FLASH, Push::Momentary)
    },
    key(170.0, 0.0),
    key(245.0, 0.5),
    key(320.0, 1.0),
    on(pill, 390.0, BUTTONS, Push::Latching),
    on(pill, 430.0, BUTTONS + 1, Push::Latching),
    on(round, 495.0, BUTTONS + 2, Push::Momentary),
    on(round, 555.0, BUTTONS + 3, Push::Momentary),
  ]
}

/// Returns the xy pad in the corner.
fn xy() -> Pad {
  Pad::new(XY_SIDE, XY_SIDE)
}

/// Returns every parameter, and every control with the parameter it is on.
///
/// The controls are a knob for each of the first parameters, then the faders and the switch,
/// then the xy pad, then the buttons.
fn wiring() -> (Vec<Dial>, Vec<Control>) {
  let switch = || Dial::linear([0.0, 1.0], 0.0, Some(1.0), 0, "");
  let mut dials: Vec<Dial> = knobs().into_iter().map(|shown| shown.dial).collect();
  dials.push(switch());
  dials.push(Dial::linear([0.0, 2.0], 0.0, Some(1.0), 0, ""));
  dials.push(Dial::linear([0.0, 100.0], 60.0, None, 0, "%"));
  // the light starts where it is by default, which is up the panel and to the left
  let toward = Light::default().toward;
  dials.push(Dial::linear(LIGHT_REACH, toward.x, None, 2, ""));
  dials.push(Dial::linear(LIGHT_REACH, -toward.y, None, 2, ""));
  let own = buttons()
    .into_iter()
    .filter(|button| button.parameter >= BUTTONS);
  dials.extend(own.map(|_| switch()));

  let readout = Readout::new(5);
  let mut controls: Vec<Control> = (0..KNOBS)
    .map(|parameter| Control::new(parameter, readout))
    .collect();
  // a fader's cap goes as far as the pointer does, along the way it slides
  let slides = |parameter, way, travel: f32| {
    let mut control = Control::new(parameter, readout);
    control.feel.drag = Drag::Along {
      way,
      turn: 1.0 / travel,
    };
    control
  };
  let pressed = |parameter, touch| {
    let mut control = Control::new(parameter, readout);
    control.touch = touch;
    control
  };
  controls.push(slides(LEVEL, [0.0, -1.0], TRAVEL));
  controls.push(pressed(ON, Touch::Press(Push::Latching)));
  controls.push(slides(SLIDE, [1.0, 0.0], SLIDE_TRAVEL));
  // dragged over the room the puck's middle has
  let room = xy().travel().into();
  controls.push(Control::pad(LIGHT_ACROSS, LIGHT_UP, room, readout));
  controls.extend(
    buttons()
      .iter()
      .map(|button| pressed(button.parameter, button.touch)),
  );
  (dials, controls)
}

/// Returns where a light of the row over the fader on its side is.
///
/// `place` counts the lights from the left.
fn light_at(place: usize) -> Vec2 {
  Vec2::new(630.0 + 32.0 * place as f32, LIGHTS_Y)
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

/// The panel as potpot draws it.
///
/// It holds its meshes, each made once.
struct Demo {
  /// The renderer, which also finds the pointer on the scene.
  renderer: Renderer,
  /// What each of that scene's objects is, in the same order.
  targets: Vec<Option<Target>>,
  /// Everything that never moves or changes.
  fixed: Drawn,
  knobs: Vec<Shown>,
  /// Each knob's body and the line painted on it.
  turned: Vec<[Arc<Mesh>; 2]>,
  lamp: Arc<Mesh>,
  switch: PushSwitch,
  plunger: Arc<Mesh>,
  /// The fader's cap and its stripe.
  cap: [Arc<Mesh>; 2],
  /// The same cap and stripe turned to lie on their side.
  lying: [Arc<Mesh>; 2],
  light: Arc<Mesh>,
  ladder: Meter,
  dots: Meter,
  /// One light of the ladder and one of the dots.
  segments: [Arc<Mesh>; 2],
  /// The buttons along the bottom, each with its body.
  buttons: Vec<(Pressed, Arc<Mesh>)>,
  xy: Pad,
  puck: Arc<Mesh>,
}

impl Demo {
  fn new(gpu: &Gpu) -> Self {
    let knobs = knobs();
    let mut fixed = Drawn::default();
    let ink = Material::matte(Vec3::splat(0.9));
    let hole = Material::matte(SEAT);
    let chrome = Material::chrome(Vec3::splat(0.8));
    let size = Vec2::from(SIZE);

    fixed.put(
      parts::plate(size.x, size.y, 6.0),
      Vec2::ZERO,
      Material::textured(Vec3::new(0.30, 0.31, 0.33)),
    );
    fixed.put(
      parts::legend("POTPOT", 11.0, 2.0),
      Vec2::new(size.x / 2.0, 24.0),
      ink,
    );
    fixed.screws(size, 18.0, 7.0);

    for (index, shown) in knobs.iter().enumerate() {
      let radius = shown.knob.radius;
      let scale = parts::ticks(11, parts::KNOB_SWEEP, radius + 5.0, radius + 10.0, 5);
      fixed.put(scale, shown.at, ink);
      let wired = index < WIRED;
      let under = if wired { NAME_UNDER } else { radius + 26.0 };
      fixed.put(
        parts::legend(shown.name, 7.0, 1.3),
        shown.at + Vec2::Y * under,
        ink,
      );
      if wired {
        fixed.readout(shown.at + Vec2::Y * READOUT_UNDER, READOUT, index);
      }
    }

    // what the lamp and its switch sit in
    fixed.put(parts::led_seat(LAMP_SIZE), LAMP, hole);
    fixed.put(
      parts::bezel(LAMP_SIZE * 0.56 + 0.4, LAMP_SIZE * 0.56 + 3.0, 2.0),
      LAMP,
      chrome,
    );
    let switch = PushSwitch::square(24.0, 14.0);
    fixed.put(switch.collar(), SWITCH, chrome);

    // the fader's slot and scale, and the gaps the meters' lights sit in
    fixed.put(parts::fader_slot(TRAVEL, 5.0), FADER, hole);
    fixed.put(parts::fader_ticks(11, TRAVEL, 16.0, 22.0, 5), FADER, ink);
    let ladder = Meter::ladder(12, 11.0, TRAVEL + 20.0);
    let dots = Meter::dots(8, 9.0, TRAVEL + 20.0);
    let seat = Arc::new(ladder.seat());
    for x in LADDERS {
      fixed.extend(ladder.seats(&seat, x, METER_FOOT));
    }
    fixed.extend(dots.seats(&Arc::new(dots.seat()), DOTS_X, METER_FOOT));

    // along the bottom: the gaps the buttons sit in, the slot of the fader on its side
    // under the seats of its row of lights, the xy pad's surface in its rim, and slots that
    // let the air out
    let buttons: Vec<_> = (buttons().into_iter())
      .map(|button| {
        fixed.put(button.shape.seat(2.5), Vec2::new(button.x, BUTTONS_Y), hole);
        let body = Arc::new(button.shape.body());
        (button, body)
      })
      .collect();
    let on_side = Mat4::from_rotation_z(FRAC_PI_2);
    fixed.put(
      parts::fader_slot(SLIDE_TRAVEL, 5.0).transformed(on_side),
      SLIDE_AT,
      hole,
    );
    for light in 0..LIGHT_COLORS.len() {
      fixed.put(parts::led_seat(LIGHT_SIZE), light_at(light), hole);
    }
    let xy = xy();
    fixed.put(xy.surface(), XY, Material::glossy(Vec3::splat(0.02)));
    fixed.put(xy.rim(), XY, chrome);
    for slot in 0..3 {
      let at = Vec2::new(862.0 + 12.0 * slot as f32, BUTTONS_Y);
      fixed.put(parts::slot(5.0, 60.0), at, hole);
    }

    let fader = Fader::console(24.0, 38.0, 14.0);
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      targets: Vec::new(),
      fixed,
      turned: (knobs.iter())
        .map(|shown| [shown.knob.body(), shown.knob.line()].map(Arc::new))
        .collect(),
      knobs,
      lamp: Arc::new(parts::led(LAMP_SIZE)),
      plunger: Arc::new(switch.plunger()),
      switch,
      cap: [fader.body(), fader.line()].map(Arc::new),
      lying: [fader.body(), fader.line()].map(|mesh| Arc::new(mesh.transformed(on_side))),
      light: Arc::new(parts::led(LIGHT_SIZE)),
      segments: [ladder.segment(), dots.segment()].map(Arc::new),
      ladder,
      dots,
      buttons,
      puck: Arc::new(xy.puck()),
      xy,
    }
  }

  /// Returns the scene and what each of its objects is.
  ///
  /// `values` is where each parameter stands, as a part of its whole turn, `feed` is how loud
  /// the sound is on each side, and `held` is the control that is held down.
  fn scene(
    &self,
    values: &[f32],
    feed: &[f32],
    held: Option<usize>,
  ) -> (Scene, Vec<Option<Target>>) {
    // the scene owns this frame's objects, so the ones that never move are copied into it
    let mut drawn = self.fixed.clone();
    let mut lamps = Vec::new();
    for (index, (shown, meshes)) in self.knobs.iter().zip(&self.turned).enumerate() {
      let materials = [shown.material, Material::matte(shown.mark)];
      drawn.knob(meshes, materials, shown.at, values[index], index);
    }

    // the lamp, as bright as its knob has it while it is on and all the way while the pad
    // is held, and the switch that is in while it is on
    let on = values[ON] > 0.5;
    let level = match (values[FLASH] > 0.5, on) {
      (true, _) => 1.0,
      (false, true) => values[BRIGHT],
      (false, false) => 0.0,
    };
    let color = hue(values[HUE]);
    drawn.place(
      &self.lamp,
      Mat4::from_translation(LAMP.extend(0.0)),
      parts::led_material(color, level, LAMP_SIZE),
      None,
    );
    let over = LAMP.extend(LAMP_SIZE);
    lamps.push(parts::led_lamp(over, color, level, LAMP_SIZE, 150.0));
    let rubber = Material::silicone(Vec3::splat(0.62));
    let plunger = self.switch.plunger;
    let depth = Push::Latching.depth(on, held == Some(SWITCH_CONTROL));
    drawn.place(
      &self.plunger,
      self.switch.transform(SWITCH.x, SWITCH.y, depth),
      rubber.lit(color, level, plunger.glow_radius()),
      Some(Target::Control(SWITCH_CONTROL)),
    );

    // a button with a color of its own is lit while its parameter stands where it lights,
    // and the pad is lit as the lamp is. each is in as far as its kind goes
    for (control, (button, body)) in (BUTTON_CONTROLS..).zip(&self.buttons) {
      let Pressed { shape, x, .. } = *button;
      let lit = (values[button.parameter] - button.lit_at).abs() < 0.25;
      let depth = match button.touch {
        Touch::Press(push) => push.depth(lit, held == Some(control)),
        _ => 0.0,
      };
      let (color, level) = match button.color {
        Some(color) => (color, f32::from(u8::from(lit))),
        None => (color, level),
      };
      drawn.place(
        body,
        shape.transform(x, BUTTONS_Y, depth),
        rubber.lit(color, level, shape.glow_radius()),
        Some(Target::Control(control)),
      );
      if level > 0.0 {
        lamps.push(shape.lamp(x, BUTTONS_Y, color, level));
      }
    }

    // the fader stands where the level is, and the meters beside it show the sound
    drawn.fader(&self.cap, FADER, TRAVEL, values[LEVEL], FADER_CONTROL);
    let [segment, dot] = &self.segments;
    for (x, level) in LADDERS.into_iter().zip([feed[LEFT], feed[RIGHT]]) {
      drawn.extend(self.ladder.lights(segment, x, METER_FOOT, level, None));
      lamps.extend(self.ladder.lamp(x, METER_FOOT, level));
    }
    drawn.extend((self.dots).lights(dot, DOTS_X, METER_FOOT, values[DOTS], None));
    lamps.extend(self.dots.lamp(DOTS_X, METER_FOOT, values[DOTS]));

    // the fader on its side, and over it a row of lights that it fills from the left
    let slid = values[SLIDE];
    let across = SLIDE_AT + Vec2::X * SLIDE_TRAVEL * (slid - 0.5);
    for (mesh, material) in self.lying.iter().zip(shared::cap_materials()) {
      let place = Mat4::from_translation(across.extend(0.0));
      drawn.place(mesh, place, material, Some(Target::Control(SLIDE_CONTROL)));
    }
    for (light, color) in LIGHT_COLORS.into_iter().enumerate() {
      let level = (slid * LIGHT_COLORS.len() as f32 - light as f32).clamp(0.0, 1.0);
      drawn.place(
        &self.light,
        Mat4::from_translation(light_at(light).extend(0.0)),
        parts::led_material(color, level, LIGHT_SIZE),
        None,
      );
    }

    // the puck of the xy pad, and the light from the side of the panel the puck is on
    let [across, up] = [LIGHT_ACROSS, LIGHT_UP].map(|light| values[light]);
    drawn.place(
      &self.puck,
      self.xy.transform(XY.x, XY.y, across, up),
      rubber,
      Some(Target::Control(XY_CONTROL)),
    );
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
        ambient: Vec3::splat(0.2),
        bounce: values[BOUNCE] * MOST_BOUNCE,
        ..Light::default()
      },
      look: Look::default(),
      objects: drawn.objects,
      lamps,
    };
    (scene, drawn.targets)
  }
}

impl Skin for Demo {
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat) {
    let (scene, targets) = self.scene(view.values, view.feed, view.held);
    (self.renderer).prepare(gpu.device, gpu.queue, &scene, view.pixels);
    self.targets = targets;

    // each wired knob's value, written flat over the glass of its window
    for (control, shown) in self.knobs.iter().take(WIRED).enumerate() {
      let middle = shown.at + Vec2::Y * READOUT_UNDER;
      shared::written(over, view, control, middle, LETTERS);
    }
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    shared::render(&self.renderer, encoder, target, place);
  }

  fn changed(&self) -> Option<&[[u32; 4]]> {
    self.renderer.changed()
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    let object = self.renderer.pick(at.into())?;
    self.targets[object]
  }
}

/// Returns the function that writes one block of the sound.
///
/// It writes how loud a made-up sound is on each side into `feed`. The sound swells and dies
/// away, a little later on the right, and is as loud at most as the level among `values` has it.
/// This is what a plugin's audio thread does at the end of each block.
fn sound(feed: Feed, values: Arc<[AtomicU32]>) -> impl FnMut() + Send {
  let mut blocks = 0.0_f32;
  move || {
    let level = shared::value(&values, LEVEL);
    let swell = |behind: f32| ((blocks - behind) * 0.02).sin().max(0.0);
    feed.set(LEFT, level * swell(0.0));
    feed.set(RIGHT, level * swell(20.0));
    blocks += 1.0;
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot", SIZE);
  let feed = Feed::new(2);
  settings.feed = feed.clone();
  let (dials, controls) = wiring();
  let (params, values) = shared::params(dials);
  if shared::window::file().is_some() {
    // a picture has the second pill in and lit, as a press leaves it
    shared::set(&values, BUTTONS + 1, 1.0);
  }
  let sound = shared::window::Sound {
    block: Box::new(sound(feed, values)),
    every: BLOCK,
    pictured_after: PICTURED_AFTER,
  };
  shared::window::run(settings, params, controls, Some(sound), Demo::new)
}
