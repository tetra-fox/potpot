//! The description of a parameter, and its steps.

use super::{Drag, Feel};
use crate::axis;

/// The distance within which a count of steps or lines is whole.
///
/// Sums of their parts come out a hair off.
pub(super) const SLACK: f32 = 1e-4;

/// The description of one parameter, for the controls that turn it.
///
/// Its value is a part of its whole turn, from zero to one, and its plain value is that in the
/// units it is read and written in.
///
/// A dial is [`Send`]: on some systems a window runs on another thread than the one that
/// opened it, and the dial is handed to that thread.
#[non_exhaustive]
pub struct Dial {
  /// The value a reset gives it.
  pub default: f32,
  /// The coarse step it moves by, in plain units.
  ///
  /// Dragged or wheeled without the fine key it lands on whole steps only. `None` turns
  /// smoothly.
  pub step: Option<f32>,
  /// The conversion of a value to plain units.
  pub plain: Box<dyn Fn(f32) -> f32 + Send>,
  /// The conversion of a plain value to a part of the turn.
  ///
  /// A plain value past either end comes out as that end.
  pub turn: Box<dyn Fn(f32) -> f32 + Send>,
  /// The writing of a value, without its unit.
  pub write: Box<dyn Fn(f32) -> String + Send>,
  /// The unit it is in, as it is written straight after a value.
  ///
  /// It starts with a space if one belongs there.
  pub unit: String,
  /// The reading of the value that text stands for, if it is one.
  pub read: Read,
}

type Read = Box<dyn Fn(&str) -> Option<f32> + Send>;

impl std::fmt::Debug for Dial {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Dial")
      .field("default", &self.default)
      .field("step", &self.step)
      .field("unit", &self.unit)
      .finish_non_exhaustive()
  }
}

impl Dial {
  /// Creates a parameter that turns smoothly and has no unit.
  ///
  /// `plain` and `turn` are its ways between a value and its plain value, and `write` and
  /// `read` those between a value and its text. `step` and `unit` are set after.
  pub fn new(
    default: f32,
    plain: impl Fn(f32) -> f32 + Send + 'static,
    turn: impl Fn(f32) -> f32 + Send + 'static,
    write: impl Fn(f32) -> String + Send + 'static,
    read: impl Fn(&str) -> Option<f32> + Send + 'static,
  ) -> Self {
    Self {
      default,
      step: None,
      plain: Box::new(plain),
      turn: Box::new(turn),
      write: Box::new(write),
      unit: String::new(),
      read: Box::new(read),
    }
  }

  /// Creates a parameter over a straight run of plain values.
  ///
  /// The run is from the first of `range` to the second, and `default` is the plain value a
  /// reset gives. A value is written with `decimals` decimals and `unit` after it, and read
  /// with or without the unit.
  pub fn linear(
    range: [f32; 2],
    default: f32,
    step: Option<f32>,
    decimals: usize,
    unit: &'static str,
  ) -> Self {
    Self::along(axis::Axis::linear(range), default, step, decimals, unit)
  }

  /// Creates a parameter whose plain values lie along an axis.
  ///
  /// A value is the place of its plain value on the [`Axis`](axis::Axis), so something drawn along
  /// the same axis lines up with the control. `default` is the plain value a reset gives. A value
  /// is written with `decimals` decimals and `unit` after it, and read with or without the unit.
  pub fn along(
    axis: axis::Axis,
    default: f32,
    step: Option<f32>,
    decimals: usize,
    unit: &'static str,
  ) -> Self {
    let plain = move |value: f32| axis.plain(value);
    let turn = move |plain: f32| axis.turn(plain);
    Self {
      default: turn(default),
      step,
      plain: Box::new(plain),
      turn: Box::new(turn),
      write: Box::new(move |value| format!("{:.decimals$}", plain(value))),
      unit: unit.to_owned(),
      read: Box::new(move |text| {
        let figures = text.trim().trim_end_matches(unit.trim()).trim();
        figures.parse().ok().map(turn)
      }),
    }
  }

  /// Returns the nearest value that is a whole number of steps.
  ///
  /// A control without steps keeps the value itself.
  pub(super) fn snapped(&self, value: f32) -> f32 {
    match self.step {
      Some(step) => (self.turn)(((self.plain)(value) / step).round() * step),
      None => value,
    }
  }

  /// Returns how far one step is as a part of the turn.
  fn step_turn(&self, step: f32) -> f32 {
    ((self.turn)((self.plain)(0.0) + step) - (self.turn)((self.plain)(0.0))).abs()
  }

  /// Returns how far a point of dragging turns it.
  ///
  /// `drag` is the way it is dragged, and a step of it takes `points_per_step` points at least.
  pub(super) fn turn_per_point(&self, drag: Drag, points_per_step: f32) -> f32 {
    match self.step {
      Some(step) => (drag.turn()).min(self.step_turn(step) / points_per_step),
      None => drag.turn(),
    }
  }

  /// Returns the value some lines of the wheel on from another.
  ///
  /// A dial without a step turns `feel.wheel` of its whole turn for each line, and `feel.fine`
  /// of that with the fine key. A stepped one goes `feel.fine` of a step for each line with the
  /// fine key, and without it to the next whole steps that way, one for each of its lines,
  /// which are whole.
  pub(super) fn wheeled(&self, value: f32, lines: f32, fine: bool, feel: Feel) -> f32 {
    let turned = match self.step {
      None => value + lines * feel.wheel * if fine { feel.fine } else { 1.0 },
      Some(step) if fine => (self.turn)((self.plain)(value) + lines * step * feel.fine),
      Some(step) => {
        let steps = (self.plain)(value) / step;
        // from between two steps the first line goes to the one that way, and no further
        let from = if lines > 0.0 {
          (steps + SLACK).floor()
        } else {
          (steps - SLACK).ceil()
        };
        (self.turn)((from + lines) * step)
      }
    };
    turned.clamp(0.0, 1.0)
  }

  /// Returns the value `choice` steps from the turn's start.
  ///
  /// A dial without a step takes a step to be one of its plain units.
  pub(super) fn chosen(&self, choice: usize) -> f32 {
    let step = self.step.unwrap_or(1.0);
    (self.turn)((self.plain)(0.0) + choice as f32 * step)
  }

  /// Returns the value one step on from another.
  ///
  /// From the end of the turn that is its start.
  pub(super) fn stepped_on(&self, value: f32, feel: Feel) -> f32 {
    if value > 1.0 - 1e-4 {
      0.0
    } else {
      self.wheeled(value, 1.0, false, feel)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Dial;

  #[test]
  fn a_linear_dial_is_written_and_read_in_its_plain_units() {
    let dial = Dial::linear([-12.0, 12.0], 0.0, Some(1.0), 1, " st");
    assert_eq!(dial.default, 0.5);
    assert_eq!((dial.plain)(0.75), 6.0);
    assert_eq!((dial.turn)(-6.0), 0.25);
    assert_eq!(
      ((dial.write)(0.75), dial.unit.as_str()),
      ("6.0".to_owned(), " st")
    );
    assert_eq!((dial.write)(0.25), "-6.0");
    // read with the unit or without, and with room around either
    for text in ["6", "6.0 st", " 6 st ", "6st"] {
      assert_eq!((dial.read)(text), Some(0.75), "{text}");
    }
    // past an end is that end, and what is not a number is nothing
    assert_eq!((dial.read)("40"), Some(1.0));
    assert_eq!((dial.read)("-40 st"), Some(0.0));
    assert_eq!((dial.read)("six"), None);
    assert_eq!((dial.read)(""), None);
  }
}
