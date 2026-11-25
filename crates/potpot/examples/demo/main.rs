//! a window with every part on it, lamps and meters following the controls: `just demo`.
//!
//! the knobs are the only parts the pointer moves. the first six of them drive the rest:
//! how bright the lamp between the first two is and what color, whether it is on, how much
//! light bounces between neighbors, where the fader stands with the meters beside it, and
//! how many of a row of dots are lit

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Fader, Knob, Meter, Push, PushSwitch};
use potpot::{Lamp, Light, Look, Material, Mesh, Object, Place, Renderer, Scene};
use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Edit, Readout, Target};
use potpot_window::controls::stroke;
use potpot_window::{Gpu, Opening, Param, Settings, Skin, View, wgpu};

const SIZE: [f32; 2] = [900.0, 520.0];
/// the knobs that drive something, in the order of [`knobs`]
const BRIGHT: usize = 0;
const HUE: usize = 1;
const ON: usize = 2;
const BOUNCE: usize = 3;
const LEVEL: usize = 4;
const DOTS: usize = 5;
/// how many of the knobs drive something, which are the ones with a readout
const WIRED: usize = 6;
/// the most light that bounces, with its knob all the way up
const MOST_BOUNCE: f32 = 6.0;

/// the wired knobs' middles down the panel, and the names and readouts under them
const WIRED_Y: f32 = 100.0;
const NAME_UNDER: f32 = 52.0;
const READOUT_UNDER: f32 = 76.0;
/// half a readout's width and height
const READOUT: [f32; 2] = [32.0, 10.0];
const LETTERS: f32 = 8.0;
/// the lamp between the first two knobs
const LAMP: Vec2 = Vec2::new(150.0, WIRED_Y);
const LAMP_SIZE: f32 = 16.0;
const SWITCH: Vec2 = Vec2::new(398.0, WIRED_Y);
/// the fader's middle and how far it slides, and the foot of the meters beside it
const FADER: Vec2 = Vec2::new(690.0, WIRED_Y + 5.0);
const TRAVEL: f32 = 110.0;
const METER_FOOT: f32 = FADER.y + TRAVEL / 2.0;
/// the pad that is lit in the lamp's color
const PAD: Vec2 = Vec2::new(80.0, 440.0);

const INK: Color = [1.0, 0.66, 0.14, 1.0];
const SELECTED: Color = [0.45, 0.28, 0.05, 1.0];

/// one knob: what it is called, where it is, its shape and what it is made of, and the
/// parameter it turns
struct Shown {
  name: &'static str,
  at: Vec2,
  knob: Knob,
  material: Material,
  /// the color of the line painted on it
  mark: Vec3,
  dial: Dial,
}

/// the six wired knobs along the top, and under them one of every other style
fn knobs() -> Vec<Shown> {
  let black = Vec3::new(0.035, 0.035, 0.04);
  let white = Vec3::splat(0.9);
  let aluminum = Material::spun(Vec3::new(0.78, 0.79, 0.81));
  let percent = || Dial::linear([0.0, 100.0], 50.0, None, 0, "%");
  let wired = |name, x, knob, material, mark, dial| Shown {
    name,
    at: Vec2::new(x, WIRED_Y),
    knob,
    material,
    mark,
    dial,
  };
  let mut knobs = vec![
    wired(
      "BRIGHT",
      80.0,
      Knob::round(30.0, 26.0),
      Material::chrome(Vec3::splat(0.8)),
      black,
      Dial::linear([0.0, 100.0], 80.0, None, 0, "%"),
    ),
    wired(
      "HUE",
      220.0,
      Knob::fluted(30.0, 26.0),
      Material::glossy(black),
      white,
      Dial::linear([0.0, 360.0], 8.0, None, 0, ""),
    ),
    wired(
      "LAMP",
      330.0,
      Knob::skirted(30.0, 26.0),
      Material::glossy(black),
      white,
      Dial::linear([0.0, 1.0], 1.0, Some(1.0), 0, ""),
    ),
    wired(
      "BOUNCE",
      490.0,
      Knob::knurled(26.0, 26.0),
      aluminum,
      black,
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
  let rest = [
    ("DIMPLED", Knob::dimpled(26.0, 14.0)),
    ("CHICKEN", Knob::chicken_head(26.0, 22.0)),
    ("BAR", Knob::bar(26.0, 26.0)),
    ("LEVER", Knob::lever(26.0, 18.0)),
    ("RIDGE", Knob::ridge(26.0, 28.0)),
    ("HEX", Knob::hex(26.0, 24.0)),
    ("RIBBED", Knob::ribbed(22.0, 24.0)),
    ("CONICAL", Knob::conical(26.0, 22.0)),
    ("ROUND", Knob::round(26.0, 22.0)),
  ];
  for (index, (name, knob)) in rest.into_iter().enumerate() {
    let material = match index % 3 {
      0 => Material::glossy(black),
      1 => Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      _ => Material::glossy(Vec3::new(0.70, 0.36, 0.04)),
    };
    knobs.push(Shown {
      name,
      at: Vec2::new(70.0 + 95.0 * index as f32, 290.0),
      knob,
      material,
      mark: if index % 3 == 1 { black } else { white },
      dial: percent(),
    });
  }
  knobs
}

/// a color all the way around the wheel from red through green and blue back to red, as
/// bright as a color gets
fn hue(turn: f32) -> Vec3 {
  let around = turn * 6.0;
  let color = Vec3::new(
    (around - 3.0).abs() - 1.0,
    2.0 - (around - 2.0).abs(),
    2.0 - (around - 4.0).abs(),
  );
  color.clamp(Vec3::splat(0.04), Vec3::ONE)
}

/// objects on their way into a scene, and for each of them what a press on it is on
#[derive(Clone, Default)]
struct Drawn {
  objects: Vec<Object>,
  targets: Vec<Option<Target>>,
}

impl Drawn {
  fn place(
    &mut self,
    mesh: &Arc<Mesh>,
    transform: Mat4,
    material: Material,
    target: Option<Target>,
  ) {
    self.objects.push(Object {
      mesh: Arc::clone(mesh),
      transform,
      material,
    });
    self.targets.push(target);
  }

  fn put(&mut self, mesh: Mesh, at: Vec2, material: Material) {
    let transform = Mat4::from_translation(at.extend(0.0));
    self.place(&Arc::new(mesh), transform, material, None);
  }

  /// objects a part made itself, which no press is on
  fn extend(&mut self, objects: impl Iterator<Item = Object>) {
    self.objects.extend(objects);
    self.targets.resize(self.objects.len(), None);
  }
}

/// the panel as potpot draws it: its meshes, each made once, and the scene last laid out
struct Demo {
  renderer: Renderer,
  /// the scene last laid out, which is what the pointer is on. none until the first is
  scene: Option<Scene>,
  /// what each of the scene's objects is, in the same order
  targets: Vec<Option<Target>>,
  /// everything that never moves or changes
  fixed: Drawn,
  knobs: Vec<Shown>,
  /// each knob's body and the line painted on it
  turned: Vec<[Arc<Mesh>; 2]>,
  lamp: Arc<Mesh>,
  switch: PushSwitch,
  plunger: Arc<Mesh>,
  /// the fader's cap and its stripe
  cap: [Arc<Mesh>; 2],
  ladder: Meter,
  dots: Meter,
  /// one light of the ladder, and one of the dots
  segments: [Arc<Mesh>; 2],
  pad: Button,
  pad_body: Arc<Mesh>,
}

impl Demo {
  fn new(gpu: &Gpu) -> Self {
    let knobs = knobs();
    let mut fixed = Drawn::default();
    let ink = Material::matte(Vec3::splat(0.9));
    let hole = Material::matte(Vec3::splat(0.012));
    let chrome = Material::chrome(Vec3::splat(0.8));
    let black = Vec3::new(0.035, 0.035, 0.04);
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
    for corner in [(18.0, 18.0), (882.0, 18.0), (18.0, 502.0), (882.0, 502.0)] {
      let at = Vec2::from(corner);
      fixed.put(parts::screw_seat(7.0, 1.5), at, hole);
      fixed.put(
        parts::screw(7.0).transformed(Mat4::from_rotation_z(at.x * 0.3 + at.y)),
        at,
        chrome,
      );
    }

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
        // the window its value shows in: a frame around dark glass
        let at = shown.at + Vec2::Y * READOUT_UNDER;
        let (wide, tall) = (READOUT[0] * 2.0, READOUT[1] * 2.0);
        let frame = parts::frame(wide + 6.0, tall + 6.0, 3.0, 2.0, 3.0);
        let target = Some(Target::Readout(index));
        let place = Mat4::from_translation(at.extend(0.0));
        fixed.place(&Arc::new(frame), place, Material::glossy(black), target);
        let glass = Mesh::cuboid(
          Vec3::new(-wide / 2.0, -tall / 2.0, 0.0),
          Vec3::new(wide / 2.0, tall / 2.0, 0.4),
        );
        let pane = Material::pane(Vec3::splat(0.01), wide * 3.5);
        fixed.place(&Arc::new(glass), place, pane, target);
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
    for x in [736.0, 752.0] {
      fixed.extend(ladder.seats(&seat, x, METER_FOOT + 10.0));
    }
    fixed.extend(dots.seats(&Arc::new(dots.seat()), 872.0, METER_FOOT + 10.0));

    // along the bottom: buttons of each shape, some of them lit, a row of small lights,
    // and slots that let the air out
    let rubber = Material::silicone(Vec3::splat(0.62));
    let pad = Button::pad(60.0, 11.0);
    fixed.put(pad.seat(3.0), PAD, hole);
    let amber = Vec3::new(1.0, 0.55, 0.05);
    let buttons = [
      (Button::key(60.0, 26.0, 7.0), 170.0, 0.0),
      (Button::key(60.0, 26.0, 7.0), 245.0, 1.0),
      (Button::key(60.0, 26.0, 7.0), 320.0, 0.0),
      (Button::pill(24.0, 54.0, 8.0), 390.0, 0.0),
      (Button::pill(24.0, 54.0, 8.0), 430.0, 1.0),
      (Button::round(40.0, 10.0), 495.0, 0.0),
      (Button::round(40.0, 10.0), 555.0, 0.0),
    ];
    for (button, x, level) in buttons {
      let at = Vec2::new(x, PAD.y);
      fixed.put(button.seat(2.5), at, hole);
      fixed.place(
        &Arc::new(button.body()),
        parts::button_transform(x, PAD.y, button.height, 0.0),
        rubber.lit(amber, level, button.glow_radius()),
        None,
      );
    }
    let lights = [
      (Vec3::new(1.0, 0.12, 0.06), 1.0),
      (Vec3::new(1.0, 0.75, 0.1), 0.0),
      (Vec3::new(0.1, 0.9, 0.3), 1.0),
      (Vec3::new(0.1, 0.5, 1.0), 0.4),
    ];
    for (index, (color, level)) in lights.into_iter().enumerate() {
      let at = Vec2::new(630.0 + 32.0 * index as f32, PAD.y);
      fixed.put(parts::led_seat(11.0), at, hole);
      fixed.put(
        parts::led(11.0),
        at,
        parts::led_material(color, level, 11.0),
      );
    }
    for slot in 0..6 {
      let at = Vec2::new(790.0 + 12.0 * slot as f32, PAD.y);
      fixed.put(parts::slot(5.0, 60.0), at, hole);
    }

    let fader = Fader::console(24.0, 38.0, 14.0);
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      scene: None,
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
      segments: [ladder.segment(), dots.segment()].map(Arc::new),
      ladder,
      dots,
      pad_body: Arc::new(pad.body()),
      pad,
    }
  }

  /// the scene with each parameter where it stands, as a part of its whole turn, and what
  /// each of its objects is
  fn scene(&self, values: &[f32]) -> (Scene, Vec<Option<Target>>) {
    // the scene owns this frame's objects, so the ones that never move are copied into it
    let mut drawn = self.fixed.clone();
    let mut lamps = Vec::new();
    for (index, (shown, meshes)) in self.knobs.iter().zip(&self.turned).enumerate() {
      let transform =
        parts::knob_transform(shown.at.x, shown.at.y, values[index], parts::KNOB_SWEEP);
      let materials = [shown.material, Material::matte(shown.mark)];
      for (mesh, material) in meshes.iter().zip(materials) {
        drawn.place(mesh, transform, material, Some(Target::Control(index)));
      }
    }

    // the lamp, the switch that is in while it is on, and the pad lit in its color
    let on = values[ON] > 0.5;
    let level = if on { values[BRIGHT] } else { 0.0 };
    let color = hue(values[HUE]);
    drawn.place(
      &self.lamp,
      Mat4::from_translation(LAMP.extend(0.0)),
      parts::led_material(color, level, LAMP_SIZE),
      None,
    );
    lamps.push(Lamp {
      at: LAMP.extend(LAMP_SIZE),
      color: color * level * 2.5,
      reach: 150.0,
      radius: LAMP_SIZE / 2.0,
    });
    let plunger = self.switch.plunger;
    drawn.place(
      &self.plunger,
      (self.switch).transform(SWITCH.x, SWITCH.y, Push::Latching.depth(on, false)),
      Material::silicone(Vec3::splat(0.62)).lit(color, level, plunger.glow_radius()),
      None,
    );
    drawn.place(
      &self.pad_body,
      parts::button_transform(PAD.x, PAD.y, self.pad.height, 0.0),
      Material::silicone(Vec3::splat(0.62)).lit(color, level, self.pad.glow_radius()),
      None,
    );
    lamps.push(self.pad.lamp(PAD.x, PAD.y, color, level));

    // the fader stands where its knob has it, and the meters beside it follow, the second
    // a little behind the first
    let stands = values[LEVEL];
    let cap = parts::fader_transform(FADER.x, FADER.y, TRAVEL, stands);
    let materials = [
      Material::glossy(Vec3::splat(0.04)),
      Material::matte(Vec3::splat(0.9)),
    ];
    for (mesh, material) in self.cap.iter().zip(materials) {
      drawn.place(mesh, cap, material, None);
    }
    let foot = METER_FOOT + 10.0;
    let [segment, dot] = &self.segments;
    for (x, level) in [(736.0, stands), (752.0, stands * 0.85)] {
      drawn.extend(self.ladder.lights(segment, x, foot, level, None));
      lamps.extend(self.ladder.lamp(x, foot, level));
    }
    drawn.extend(self.dots.lights(dot, 872.0, foot, values[DOTS], None));
    lamps.extend(self.dots.lamp(872.0, foot, values[DOTS]));

    let scene = Scene {
      size: SIZE.into(),
      heights: [-8.0, 48.0],
      lean: Vec2::new(0.0, -0.25),
      // dimmed, so that what the lamps do shows
      light: Light {
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

/// lettering with its middle at a point
fn centered(over: &mut Flat, text: &str, at: [f32; 2], color: Color) {
  let corner = stroke::top_left(text.chars().count(), LETTERS);
  let from = [at[0] + corner[0], at[1] + corner[1]];
  over.legend(text, from, LETTERS, 1.2, color);
}

/// a value being typed: what is selected on a block, and the caret
fn typed(over: &mut Flat, edit: &Edit, middle: [f32; 2]) {
  let count = edit.text.chars().count();
  let gap = |place: usize| middle[0] + stroke::gap(count, place, LETTERS);
  let (top, foot) = (
    middle[1] - LETTERS / 2.0 - 2.0,
    middle[1] + LETTERS / 2.0 + 2.0,
  );
  let selected = edit.selected();
  if !selected.is_empty() {
    over.rect(
      [gap(selected.start), top],
      [gap(selected.end), foot],
      SELECTED,
    );
  }
  centered(over, &edit.text, middle, INK);
  over.line([gap(edit.caret), top], [gap(edit.caret), foot], 1.0, INK);
}

impl Skin for Demo {
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat) {
    let (scene, targets) = self.scene(view.values);
    (self.renderer).prepare(gpu.device, gpu.queue, &scene, view.pixels);
    (self.scene, self.targets) = (Some(scene), targets);

    // each wired knob's value, written flat over the glass of its window
    let wired = self.knobs.iter().zip(view.controls).take(WIRED);
    for (control, (shown, reading)) in wired.enumerate() {
      let middle = [shown.at.x, shown.at.y + READOUT_UNDER];
      match view.edit.filter(|edit| edit.control == control) {
        Some(edit) => typed(over, edit, middle),
        None => {
          let written = format!("{}{}", reading.text, reading.unit);
          centered(over, &written, middle, INK);
        }
      }
    }
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    let [x, y, width, height] = place.map(|side| side as f32);
    let on = Place {
      x,
      y,
      width,
      height,
    };
    self.renderer.render(encoder, target, on, place);
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    let object = self.scene.as_ref()?.pick(at.into())?;
    self.targets[object]
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let settings = Settings::new("potpot", SIZE);
  // a parameter for each knob, which no host has: each stands where the panel last set it
  let params = (knobs().into_iter())
    .map(|shown| Param::free(shown.dial))
    .collect();
  let controls = (0..knobs().len())
    .map(|parameter| Control::new(parameter, Readout::new(5)))
    .collect();
  let opened = potpot_window::open(settings, params, controls, Opening::alone(), Demo::new)?;
  opened.window.run_until_closed()?;
  Ok(())
}
