//! A panel of four controls for the tests to drive.

use std::time::{Duration, Instant};

use super::Action::{Focus, Redraw, Set};
use super::{
  Action, Button, Control, Dial, Event, Interaction, Item, Key, Moment, Readout, Target,
};
use crate::Point;

/// The number of letters the readouts of the bench have room for.
pub(super) const ROOM: usize = 9;

/// A panel with every control at its default.
///
/// The shifts are in the middle of their turn and the mix at the end of its own, and events
/// arrive a hundredth of a second apart.
pub(super) struct Bench {
  pub(super) interaction: Interaction,
  /// The values the controls stand at.
  ///
  /// They are the defaults unless a test says otherwise.
  pub(super) values: [f32; 4],
  /// The number of times its own size the window is.
  pub(super) scale: f32,
  now: Instant,
}

// the bench's controls: two shifts of 24 semitones either way and a mix in hundredths,
// all three turning smoothly, and a shift like the others that moves in whole semitones
pub(super) const PITCH: usize = 0;
pub(super) const FORMANT: usize = 1;
pub(super) const MIX: usize = 2;
pub(super) const STEPPED: usize = 3;
pub(super) const SIZE: Point = [500.0, 250.0];
pub(super) const SCALES: [f32; 2] = [0.75, 3.0];

pub(super) fn shift(step: Option<f32>) -> Dial {
  Dial {
    default: 0.5,
    step,
    plain: Box::new(|value| value * 48.0 - 24.0),
    turn: Box::new(|plain| ((plain + 24.0) / 48.0).clamp(0.0, 1.0)),
    write: Box::new(|value| format!("{:.2}", value * 48.0 - 24.0)),
    unit: " st".to_owned(),
    read: Box::new(|text| {
      let plain: f32 = text.trim().trim_end_matches(" st").parse().ok()?;
      Some(((plain + 24.0) / 48.0).clamp(0.0, 1.0))
    }),
  }
}

pub(super) fn mix() -> Dial {
  Dial {
    default: 1.0,
    step: None,
    plain: Box::new(|value| value),
    turn: Box::new(|plain| plain.clamp(0.0, 1.0)),
    write: Box::new(|value| format!("{:.0}", value * 100.0)),
    unit: "%".to_owned(),
    read: Box::new(|text| {
      let hundredths: f32 = text.trim().trim_end_matches([' ', '%']).parse().ok()?;
      Some((hundredths / 100.0).clamp(0.0, 1.0))
    }),
  }
}

impl Bench {
  pub(super) fn new() -> Self {
    let dials = vec![shift(None), shift(None), mix(), shift(Some(1.0))];
    // a control for each parameter, in the same order, with readouts all alike
    let readout = Readout {
      room: ROOM,
      shows: |letter| letter.is_ascii_digit() || " +-.,%st".contains(letter),
    };
    let controls = (0..dials.len())
      .map(|parameter| Control::new(parameter, readout))
      .collect();
    Self {
      interaction: Interaction::new(dials, controls, SIZE, SCALES),
      values: [0.5, 0.5, 1.0, 0.5],
      scale: 1.0,
      now: Instant::now(),
    }
  }

  pub(super) fn after(&mut self, wait: Duration, event: Event) -> Vec<Action> {
    self.now += wait;
    let moment = Moment {
      now: self.now,
      values: &self.values,
      scale: self.scale,
    };
    self.interaction.handle(event, moment)
  }

  pub(super) fn send(&mut self, event: Event) -> Vec<Action> {
    self.after(Duration::from_millis(10), event)
  }

  /// Returns a place on the panel and where that is in the window.
  ///
  /// The window is at the bench's scale.
  pub(super) fn place(&self, at: Point) -> (Point, Point) {
    (at, at.map(|across| across * self.scale))
  }

  pub(super) fn press(&mut self, over: Option<Target>, y: f32) -> Vec<Action> {
    let (at, window) = self.place([100.0, y]);
    self.send(Event::Press {
      button: Button::Left,
      at,
      window,
      over,
      reset: false,
    })
  }

  /// Presses the left button with the reset key held.
  pub(super) fn reset(&mut self, over: Option<Target>) -> Vec<Action> {
    let (at, window) = self.place([100.0, 100.0]);
    self.send(Event::Press {
      button: Button::Left,
      at,
      window,
      over,
      reset: true,
    })
  }

  pub(super) fn right(&mut self, over: Option<Target>) -> Vec<Action> {
    let (at, window) = self.place([100.0, 100.0]);
    self.send(Event::Press {
      button: Button::Right,
      at,
      window,
      over,
      reset: false,
    })
  }

  pub(super) fn drag(&mut self, y: f32, fine: bool) -> Vec<Action> {
    let (at, window) = self.place([100.0, y]);
    self.send(Event::Move {
      at,
      window,
      over: None,
      fine,
    })
  }

  pub(super) fn release(&mut self) -> Vec<Action> {
    self.send(Event::Release(Button::Left))
  }

  pub(super) fn wheel(&mut self, over: Target, lines: f32, fine: bool) -> Vec<Action> {
    self.send(Event::Wheel {
      over: Some(over),
      lines,
      fine,
    })
  }

  pub(super) fn key(&mut self, key: Key) -> Vec<Action> {
    self.send(Event::Key(key))
  }

  pub(super) fn kind(&mut self, text: &str) {
    for letter in text.chars() {
      self.key(Key::Letter(letter));
    }
  }

  /// Double clicks a readout, which begins typing its value.
  pub(super) fn edit(&mut self, control: usize) {
    let readout = Some(Target::Readout(control));
    assert_eq!(self.press(readout, 100.0), []);
    self.release();
    assert_eq!(self.press(readout, 100.0), [Focus, Redraw]);
    self.release();
  }

  pub(super) fn typed(&self) -> (&str, usize, bool) {
    let edit = self.interaction.edit().expect("a value is being typed");
    (&edit.text, edit.caret, !edit.selected().is_empty())
  }
}

/// Returns the value a list of actions sets.
///
/// Setting it has to be the only thing they do.
pub(super) fn set(actions: &[Action], control: usize) -> f32 {
  match actions {
    [Set(set, value)] if *set == control => *value,
    other => panic!("the actions set one value for {control:?}: {other:?}"),
  }
}

pub(super) fn close(a: f32, b: f32) -> bool {
  (a - b).abs() < 1e-5
}

/// Returns the value of the bench's shifts for `plain` semitones.
pub(super) fn semitones(plain: f32) -> f32 {
  (plain + 24.0) / 48.0
}

/// Opens the menu on a target and presses one of its rows.
pub(super) fn choose(bench: &mut Bench, on: Target, item: Item) -> Vec<Action> {
  bench.after(Duration::from_secs(1), Event::Tick);
  assert_eq!(bench.right(Some(on)), [Redraw]);
  bench.press(Some(Target::Item(item)), 100.0)
}

/// Creates the bench with its pitch control made a pad.
///
/// A drag up turns the pitch and a drag to the right turns `across`. The puck has 128 points
/// of room across and 64 up, so each move is a turn that an `f32` holds exactly.
pub(super) fn pad(across: usize) -> Bench {
  let mut bench = Bench::new();
  let readout = bench.interaction.controls[PITCH].readout;
  bench.interaction.controls[PITCH] = Control::pad(across, PITCH, [128.0, 64.0], readout);
  bench
}

pub(super) fn slide(bench: &mut Bench, at: Point) -> Vec<Action> {
  let (at, window) = bench.place(at);
  bench.send(Event::Move {
    at,
    window,
    over: None,
    fine: false,
  })
}
