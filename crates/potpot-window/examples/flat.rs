//! a panel of four knobs drawn with nothing but flat shapes, in a window of its own with no
//! host: the least there is to write to have working controls. `cargo run -p potpot-window
//! --example flat`.
//!
//! everything the pointer and the keys do comes with the window: dragging and the wheel,
//! the fine key, a double click to reset, typing a value into a readout, the menu on the
//! right button and the grip in the corner. what is written here is the table of controls
//! and how they are drawn.

use std::f32::consts::PI;

use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Edit, Feel, Readout, Target};
use potpot_window::controls::stroke;
use potpot_window::{Gpu, Opening, Param, Settings, Skin, View, wgpu};

/// the panel's width and height in its own points
const SIZE: [f32; 2] = [420.0, 300.0];
/// how far down the knobs' middles, their names and their readouts are
const KNOB_Y: f32 = 120.0;
const NAME_Y: f32 = 178.0;
const READOUT_Y: f32 = 214.0;
const RADIUS: f32 = 30.0;
/// half a readout's width and height
const READOUT: [f32; 2] = [38.0, 12.0];
const LETTERS: f32 = 9.0;

const GROUND: Color = [0.11, 0.12, 0.14, 1.0];
const WELL: Color = [0.05, 0.055, 0.065, 1.0];
const LINE: Color = [0.42, 0.45, 0.5, 1.0];
const INK: Color = [0.9, 0.91, 0.93, 1.0];
const MARK: Color = [0.3, 0.75, 0.95, 1.0];

/// one knob: where it is, and what it turns
struct Knob {
  name: &'static str,
  x: f32,
  /// the plain values at the two ends of its turn
  range: [f32; 2],
  default: f32,
  step: Option<f32>,
  decimals: usize,
  unit: &'static str,
  /// how many letters can be typed into its readout
  room: usize,
  feel: Feel,
}

/// the panel's knobs, left to right: one in whole semitones, one smooth, one in steps of
/// five that the wheel moves slowly, and one that the fine key slows down a great deal
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
      feel,
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
      feel: slow_wheel,
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

/// the look: nothing of the graphics device's is needed for flat shapes, so all it keeps is
/// where the knobs are
struct Look {
  knobs: [Knob; 4],
}

/// lettering with its middle at a point
fn centered(over: &mut Flat, text: &str, at: [f32; 2], height: f32, color: Color) {
  let corner = stroke::top_left(text.chars().count(), height);
  over.legend(
    text,
    [at[0] + corner[0], at[1] + corner[1]],
    height,
    1.2,
    color,
  );
}

impl Skin for Look {
  fn prepare(&mut self, _gpu: &Gpu, view: &View, over: &mut Flat) {
    over.rect([0.0, 0.0], SIZE, GROUND);
    centered(over, "FLAT", [SIZE[0] / 2.0, 36.0], 14.0, LINE);
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
      centered(over, knob.name, [knob.x, NAME_Y], 8.0, LINE);

      let middle = [knob.x, READOUT_Y];
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
        Some(edit) => typed(over, edit, middle),
        None => {
          let written = format!("{}{}", reading.text, reading.unit);
          centered(over, &written, middle, LETTERS, INK);
        }
      }
    }
  }

  fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {
    // the window draws the flat shapes itself, and there is nothing else
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    self.knobs.iter().enumerate().find_map(|(control, knob)| {
      let from = [at[0] - knob.x, at[1] - KNOB_Y];
      if from[0].hypot(from[1]) <= RADIUS {
        return Some(Target::Control(control));
      }
      let within = (at[0] - knob.x).abs() <= READOUT[0] && (at[1] - READOUT_Y).abs() <= READOUT[1];
      within.then_some(Target::Readout(control))
    })
  }
}

/// a value being typed: what is selected on a bright block, and the caret
fn typed(over: &mut Flat, edit: &Edit, middle: [f32; 2]) {
  let count = edit.text.chars().count();
  let gap = |place: usize| middle[0] + stroke::gap(count, place, LETTERS);
  let (top, foot) = (
    middle[1] - LETTERS / 2.0 - 2.0,
    middle[1] + LETTERS / 2.0 + 2.0,
  );
  let selected = edit.selected();
  if !selected.is_empty() {
    over.rect([gap(selected.start), top], [gap(selected.end), foot], MARK);
  }
  centered(over, &edit.text, middle, LETTERS, INK);
  over.line([gap(edit.caret), top], [gap(edit.caret), foot], 1.0, INK);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let settings = Settings::new("Flat", SIZE);
  // a parameter for each knob, which no host has: each stands where the panel last set it
  let params = knobs()
    .iter()
    .map(|knob| Param::free(knob.dial()))
    .collect();
  let controls = (knobs().iter().enumerate())
    .map(|(parameter, knob)| {
      let mut control = Control::new(parameter, Readout::new(knob.room));
      control.feel = knob.feel;
      control
    })
    .collect();
  let opened = potpot_window::open(settings, params, controls, Opening::alone(), |_| Look {
    knobs: knobs(),
  })?;
  opened.window.run_until_closed()?;
  Ok(())
}
