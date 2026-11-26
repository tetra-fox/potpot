//! Knobs, an XY pad and a wave, drawn with flat shapes alone.
//!
//! It opens in a window of its own with no host, and is the least there is to write to have
//! working controls. `just example flat` runs it, and `just render flat out.png` draws it to a
//! file in place of the window.
//!
//! Everything the pointer and the keys do comes with the window:
//!
//! - dragging and the wheel
//! - the fine key
//! - a double click to reset
//! - typing a value into a readout
//! - the menu on the right button
//! - the grip in the corner
//!
//! The example holds the table of controls and how they are drawn.

mod shared;

use std::f32::consts::{PI, TAU};

use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Feel, Readout, Target};
use potpot_window::{Gpu, Param, Settings, Skin, View, wgpu};

/// The panel's width and height in its own points.
const SIZE: [f32; 2] = [600.0, 390.0];
// how far down the knobs' middles, their names and their readouts are
const KNOB_Y: f32 = 120.0;
const NAME_Y: f32 = 178.0;
const READOUT_Y: f32 = 214.0;
const RADIUS: f32 = 30.0;
/// Half a readout's width and height.
const READOUT: [f32; 2] = [38.0, 12.0];
const LETTERS: f32 = 9.0;
/// The width of the strokes of all the lettering.
const WEIGHT: f32 = 1.2;
// the middle of the pad's well, half its side, and how far out from its middle the puck reaches
const PAD: [f32; 2] = [500.0, 100.0];
const PAD_HALF: f32 = 60.0;
const PUCK: f32 = 9.0;
/// The width and height of the room the puck's middle has.
const ROOM: f32 = (PAD_HALF - PUCK) * 2.0;
// the two parameters the pad turns, and its two controls: the pad itself, whose readout is
// of the one up, and a readout alone on the one across
const PAN: usize = 4;
const DEPTH: usize = 5;
const PAD_CONTROL: usize = 4;
const PAN_CONTROL: usize = 5;
/// The room each of the pad's readouts has across the panel.
const READOUT_SLOT: f32 = 80.0;
/// The pad's two readouts.
///
/// Each is its control, its name and how far across the panel its middle is.
const UNDER_PAD: [(usize, &str, f32); 2] = [
  (PAN_CONTROL, "PAN", PAD[0] - READOUT_SLOT / 2.0),
  (PAD_CONTROL, "DEPTH", PAD[0] + READOUT_SLOT / 2.0),
];

/// The top left and bottom right of the wave's well.
const WAVE: [[f32; 2]; 2] = [[22.0, 250.0], [578.0, 370.0]];

const GROUND: Color = [0.11, 0.12, 0.14, 1.0];
const WELL: Color = [0.05, 0.055, 0.065, 1.0];
const LINE: Color = [0.42, 0.45, 0.5, 1.0];
const INK: Color = [0.9, 0.91, 0.93, 1.0];
const MARK: Color = [0.3, 0.75, 0.95, 1.0];

/// One knob: where it is and what it turns.
struct Knob {
  name: &'static str,
  x: f32,
  /// The plain values at the two ends of its turn.
  range: [f32; 2],
  default: f32,
  step: Option<f32>,
  decimals: usize,
  unit: &'static str,
  /// The number of letters that can be typed into its readout.
  room: usize,
  feel: Feel,
}

/// Returns the panel's knobs, left to right.
///
/// - The first moves in whole semitones.
/// - The second is smooth, and the wheel moves it slowly.
/// - The third moves in steps of five.
/// - The fine key slows the fourth down a great deal.
fn knobs() -> [Knob; 4] {
  // how each answers the pointer and the wheel: as usual, with a wheel that moves it a
  // hundredth of its turn for each line, and with a fine key that slows it fifty times
  let feel = Feel::default();
  let (mut slow_wheel, mut very_fine) = (feel, feel);
  slow_wheel.wheel = 0.01;
  very_fine.fine = 0.02;
  [
    Knob {
      name: "TUNE",
      x: 60.0,
      range: [-12.0, 12.0],
      default: 0.0,
      step: Some(1.0),
      decimals: 0,
      unit: " st",
      room: 3,
      feel,
    },
    Knob {
      name: "DRIVE",
      x: 160.0,
      range: [0.0, 10.0],
      default: 2.5,
      step: None,
      decimals: 2,
      unit: "",
      room: 5,
      feel: slow_wheel,
    },
    Knob {
      name: "TONE",
      x: 260.0,
      range: [0.0, 100.0],
      default: 50.0,
      step: Some(5.0),
      decimals: 0,
      unit: "%",
      room: 3,
      feel,
    },
    Knob {
      name: "LEVEL",
      x: 360.0,
      range: [-60.0, 6.0],
      default: 0.0,
      step: None,
      decimals: 1,
      unit: " dB",
      room: 5,
      feel: very_fine,
    },
  ]
}

impl Knob {
  fn dial(&self) -> Dial {
    Dial::linear(
      self.range,
      self.default,
      self.step,
      self.decimals,
      self.unit,
    )
  }
}

/// Adds a control's readout, its middle `x` across the panel.
fn readout(over: &mut Flat, view: &View, control: usize, x: f32) {
  let middle = [x, READOUT_Y];
  let corner = |sign: f32| [middle[0] + sign * READOUT[0], middle[1] + sign * READOUT[1]];
  let edit = view.edit.filter(|edit| edit.control == control);
  over.rect(
    corner(-1.0),
    corner(1.0),
    if edit.is_some() { MARK } else { LINE },
  );
  let inside = |sign: f32| corner(sign).map(|along| along - sign * view.pixel);
  over.rect(inside(-1.0), inside(1.0), WELL);
  match edit {
    // what is selected is on a bright block
    Some(edit) => over.typed(edit, middle, LETTERS, WEIGHT, INK, MARK),
    None => {
      let reading = &view.controls[control];
      let written = format!("{}{}", reading.text, reading.unit);
      over.centered(&written, middle, LETTERS, WEIGHT, INK);
    }
  }
}

/// Adds the wave the four knobs shape.
///
/// It lies over the region between it and its middle line. `values` is where each knob stands, as a
/// part of its whole turn: the tune sets how many cycles show, the drive flattens their crests, the
/// tone adds a ripple and the level is their height.
fn wave(over: &mut Flat, values: [f32; 4]) {
  let [tune, drive, tone, level] = values;
  let [min, max] = WAVE;
  over.rect(min, max, WELL);
  let (middle, reach) = ((min[1] + max[1]) / 2.0, (max[1] - min[1]) / 2.0 - 8.0);
  // an octave down at the start of the tune's turn and an octave up at its end
  let cycles = 4.0 * 2.0_f32.powf(tune * 2.0 - 1.0);
  let gain = 0.5 + drive * 6.0;
  let points: Vec<[f32; 2]> = (0..=270u16)
    .map(|step| {
      let across = f32::from(step) / 270.0;
      let turn = across * cycles * TAU;
      let driven = (gain * (turn.sin() + tone * 0.4 * (turn * 5.0).sin())).tanh() / gain.tanh();
      [
        min[0] + 8.0 + across * (max[0] - min[0] - 16.0),
        middle - driven * level * reach,
      ]
    })
    .collect();
  let [red, green, blue, _] = MARK;
  over.area(&points, middle, [red, green, blue, 0.25]);
  over.curve(&points, 2.0, MARK);
}

/// The look of the panel.
///
/// Flat shapes need nothing of the graphics device, so it keeps only the knobs and where the
/// pad's puck was last drawn.
struct Look {
  knobs: [Knob; 4],
  puck: [f32; 2],
}

impl Skin for Look {
  fn prepare(&mut self, _gpu: &Gpu, view: &View, over: &mut Flat) {
    over.rect([0.0, 0.0], SIZE, GROUND);
    over.centered("FLAT", [SIZE[0] / 2.0, 36.0], 14.0, WEIGHT, LINE);
    for (control, (knob, reading)) in self.knobs.iter().zip(view.controls).enumerate() {
      let value = reading.value;
      // a ring of short lines, and a pointer that sweeps three quarters of the way round
      let on_ring = |turn: f32, reach: f32| {
        let angle = (turn * 1.5 - 0.75) * PI;
        [knob.x + angle.sin() * reach, KNOB_Y - angle.cos() * reach]
      };
      for part in 0..48u8 {
        let (from, to) = (f32::from(part) / 48.0, f32::from(part + 1) / 48.0);
        let color = if to <= value { MARK } else { LINE };
        over.line(on_ring(from, RADIUS), on_ring(to, RADIUS), 2.0, color);
      }
      over.line(on_ring(value, 8.0), on_ring(value, RADIUS - 6.0), 3.0, INK);
      over.centered(knob.name, [knob.x, NAME_Y], 8.0, WEIGHT, LINE);
      readout(over, view, control, knob.x);
    }

    // the pad: a well, a line each way through the puck, and the puck as a dot
    let corner = |sign: f32| PAD.map(|middle| middle + sign * PAD_HALF);
    over.rect(corner(-1.0), corner(1.0), WELL);
    let [pan, depth] = [PAN_CONTROL, PAD_CONTROL].map(|control| view.controls[control].value);
    self.puck = [PAD[0] + (pan - 0.5) * ROOM, PAD[1] + (0.5 - depth) * ROOM];
    let [x, y] = self.puck;
    over.rect([corner(-1.0)[0], y], [corner(1.0)[0], y + view.pixel], LINE);
    over.rect([x, corner(-1.0)[1]], [x + view.pixel, corner(1.0)[1]], LINE);
    over.line(self.puck, self.puck, PUCK * 2.0, MARK);
    for (control, name, x) in UNDER_PAD {
      over.centered(name, [x, NAME_Y], 8.0, WEIGHT, LINE);
      readout(over, view, control, x);
    }
    wave(over, std::array::from_fn(|knob| view.controls[knob].value));
  }

  fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {
    // the window draws the flat shapes itself, and there is nothing else
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    let within =
      |middle: [f32; 2], reach: f32| (at[0] - middle[0]).hypot(at[1] - middle[1]) <= reach;
    if within(self.puck, PUCK) {
      return Some(Target::Control(PAD_CONTROL));
    }
    let on_readout =
      |x: f32| (at[0] - x).abs() <= READOUT[0] && (at[1] - READOUT_Y).abs() <= READOUT[1];
    let knobs = (self.knobs.iter().enumerate()).map(|(control, knob)| (control, knob.x));
    let knob = (knobs.clone()).find(|&(_, x)| within([x, KNOB_Y], RADIUS));
    let under_pad = UNDER_PAD.into_iter().map(|(control, _, x)| (control, x));
    let readout = knobs.chain(under_pad).find(|&(_, x)| on_readout(x));
    knob
      .map(|(control, _)| Target::Control(control))
      .or(readout.map(|(control, _)| Target::Readout(control)))
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let settings = Settings::new("Flat", SIZE);
  // a parameter for each knob and two for the pad, which no host has: each stands where the
  // panel last set it
  let mut params: Vec<Param> = knobs()
    .iter()
    .map(|knob| Param::free(knob.dial()))
    .collect();
  params.push(Param::free(Dial::linear([-50.0, 50.0], 0.0, None, 0, "")));
  params.push(Param::free(Dial::linear([0.0, 100.0], 50.0, None, 0, "%")));
  let mut controls: Vec<Control> = (knobs().iter().enumerate())
    .map(|(parameter, knob)| {
      let mut control = Control::new(parameter, Readout::new(knob.room));
      control.feel = knob.feel;
      control
    })
    .collect();
  // dragged over the room the puck's middle has
  controls.push(Control::pad(PAN, DEPTH, [ROOM; 2], Readout::new(3)));
  controls.push(Control::new(PAN, Readout::new(3)));
  shared::run(settings, params, controls, None, |_| Look {
    knobs: knobs(),
    puck: PAD,
  })
}
