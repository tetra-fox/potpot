//! The ways a control is dragged, and a drag under way.

use super::{Action, Control, Dial, Touch};
use crate::Point;

/// The way the pointer is dragged to turn a control.
///
/// Each way also says how far the drag turns the control.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Drag {
  /// A drag up and down the panel.
  ///
  /// Up turns the control on and down turns it back, by `turn` of its whole turn for each
  /// point.
  Up {
    /// The part of the whole turn that one point is.
    turn: f32,
  },
  /// A drag along a line across the panel, the way a fader slides.
  ///
  /// Toward `way` turns the control on and away from it turns it back, by `turn` of its whole
  /// turn for each point.
  Along {
    /// The way that turns it on, one point long, across and down.
    way: Point,
    /// The part of the whole turn that one point is.
    turn: f32,
  },
}

impl Drag {
  /// Returns how far the pointer has gone the way that turns it on.
  ///
  /// The distance is in points.
  fn travel(self, from: Point, to: Point) -> f32 {
    match self {
      Self::Up { .. } => from[1] - to[1],
      Self::Along { way, .. } => (to[0] - from[0]) * way[0] + (to[1] - from[1]) * way[1],
    }
  }

  /// Returns how far one point of that turns the control.
  pub(super) fn turn(self) -> f32 {
    match self {
      Self::Up { turn } | Self::Along { turn, .. } => turn,
    }
  }
}

/// One of the parameters a drag turns.
#[derive(Clone, Copy, Debug)]
struct Axis {
  parameter: usize,
  /// The way the pointer is dragged to turn it.
  drag: Drag,
  /// The value the pointer has taken it to.
  ///
  /// Between two steps the control shows another value, and so it does while a pad's pointer
  /// is past an end of the turn.
  reached: f32,
  /// The value last shown.
  shown: f32,
  /// Whether the host has heard of a gesture on it.
  begun: bool,
}

/// A drag of a control, from the press to the release.
#[derive(Clone, Copy, Debug)]
pub(super) struct Dragging {
  pub(super) control: usize,
  /// The control's own parameter.
  own: Axis,
  /// The parameter a pad is dragged across for.
  across: Option<Axis>,
  /// The place the pointer last was.
  from: Point,
  /// Whether the fine key was held at the last move.
  fine: bool,
}

impl Dragging {
  /// Creates the drag of a control pressed at `at`.
  ///
  /// `place` is the control's place among the panel's controls, and each parameter starts from
  /// where `values` has it.
  pub(super) fn begin(place: usize, control: &Control, at: Point, values: &[f32]) -> Self {
    let axis = |(parameter, drag): (usize, Drag)| Axis {
      parameter,
      drag,
      reached: values[parameter],
      shown: values[parameter],
      begun: false,
    };
    Self {
      control: place,
      own: axis((control.parameter, control.feel.drag)),
      across: control.across().map(axis),
      from: at,
      fine: false,
    }
  }

  /// Moves the drag on to where the pointer has gone.
  pub(super) fn moved(
    &mut self,
    control: &Control,
    dials: &[Dial],
    at: Point,
    now_fine: bool,
    values: &[f32],
    actions: &mut Vec<Action>,
  ) {
    let Control { feel, touch, .. } = *control;
    // a pad's puck keeps to the pointer: its steps do not slow it, and what the pointer
    // goes past an end of the turn is kept for it to come back
    let follows = matches!(touch, Touch::Pad { .. });
    for axis in std::iter::once(&mut self.own).chain(&mut self.across) {
      let dial = &dials[axis.parameter];
      // until the host hears of it, it goes on from wherever its parameter is taken, less
      // what the pointer has gone past an end of the turn
      let stands = values[axis.parameter];
      if !axis.begun && stands != axis.shown {
        axis.reached = stands + (axis.reached.clamp(0.0, 1.0) - axis.shown);
        axis.shown = stands;
      }
      // with the fine key the knob goes on from where it shows, so pressing the key moves
      // nothing. every move adds its own part of a turn, so the key changes only the pace
      if now_fine && !self.fine {
        axis.reached = axis.shown;
      }
      let pace = if follows {
        axis.drag.turn()
      } else {
        dial.turn_per_point(axis.drag, feel.points_per_step)
      };
      let slowed = if now_fine { feel.fine } else { 1.0 };
      let travel = axis.drag.travel(self.from, at);
      // a move another way than its own begins nothing on it, though its parameter may
      // stand off the step it would show
      if !axis.begun && travel == 0.0 {
        continue;
      }
      axis.reached += travel * pace * slowed;
      let within = axis.reached.clamp(0.0, 1.0);
      if !follows {
        axis.reached = within;
      }
      let now = if now_fine {
        within
      } else {
        dial.snapped(within)
      };
      // at the end of the turn, between two steps, or with the pointer moved only another
      // way than this one's
      if now == axis.shown {
        continue;
      }
      axis.shown = now;
      if !axis.begun {
        axis.begun = true;
        actions.push(Action::Begin(axis.parameter));
      }
      actions.push(Action::Set(axis.parameter, now));
    }
    self.from = at;
    self.fine = now_fine;
  }

  /// Returns the end of each gesture the host has heard begin.
  pub(super) fn ended(&self) -> impl Iterator<Item = Action> {
    let begun = std::iter::once(self.own)
      .chain(self.across)
      .filter(|axis| axis.begun);
    begun.map(|axis| Action::End(axis.parameter))
  }
}

#[cfg(test)]
mod tests;
