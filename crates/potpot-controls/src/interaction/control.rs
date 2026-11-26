//! The controls on a panel and the way each answers.

use super::Drag;
use crate::Point;

/// The way a button that is pressed behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Push {
  /// A button that each push turns over.
  ///
  /// One push takes it in and on, the next out and off. It stays most of the way in while it
  /// is on.
  Latching,
  /// A button that is on while held.
  Momentary,
}

impl Push {
  /// Returns how far in such a button is.
  ///
  /// The depth runs from zero for out to one for all the way. The button is all the way in
  /// while it is `held`, and most of the way while a latching one is `on`.
  pub fn depth(self, on: bool, held: bool) -> f32 {
    match (held, self, on) {
      (true, ..) => 1.0,
      (false, Push::Latching, true) => 0.7,
      (false, Push::Latching, false) | (false, Push::Momentary, _) => 0.0,
    }
  }

  /// Returns whether such a button is on after a push is let go.
  ///
  /// `was_on` is whether it was on before. A momentary one is off once it is let go.
  pub fn released(self, was_on: bool) -> bool {
    self == Push::Latching && !was_on
  }
}

/// The effect of the left button on a control.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Touch {
  /// A control that is dragged.
  ///
  /// It is dragged the way its [`Feel`] says, and a double click puts it at its default.
  Turn,
  /// A control that is pressed like a button.
  ///
  /// Its parameter is at the end of its turn while it is on and at the start while it is off.
  Press(Push),
  /// A control that each click moves one step on.
  ///
  /// From the end of its turn a click moves it back to the start.
  Step,
  /// A control dragged two ways at once, like a pad's puck.
  ///
  /// A drag turns its own parameter the way its [`Feel`] says, and a drag to the right turns
  /// `across` on as well. The puck keeps to the pointer. A stepped parameter goes at the pace
  /// of its drag and lands on its steps, and a pointer gone past an end of a turn comes all the
  /// way back before the value moves. The fine key slows both, each from where it shows, and
  /// the puck is off the pointer from then on. A double click resets it.
  Pad {
    /// The parameter a drag across the panel turns.
    across: usize,
    /// The part of that parameter's whole turn that one point is.
    turn: f32,
  },
}

/// The way a control answers the pointer and the wheel.
///
/// A feel starts as [`Feel::default`] and is changed a field at a time.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Feel {
  /// The way it is dragged.
  pub drag: Drag,
  /// The least points a stepped control is dragged for each step.
  ///
  /// One with few steps is dragged further for each, at the pace of a control without any. A
  /// [`Touch::Pad`] goes at the pace of its drag whatever its steps.
  pub points_per_step: f32,
  /// The part of its pace it keeps while the fine key is held.
  pub fine: f32,
  /// The turn one line of the wheel gives a control that has no step.
  ///
  /// It is a part of the control's whole turn.
  pub wheel: f32,
}

impl Default for Feel {
  /// Returns a feel that is dragged up the panel.
  ///
  /// - Two hundred points up make the whole turn.
  /// - A step takes five points at least.
  /// - The fine key gives a tenth of that pace.
  /// - Twenty lines of the wheel make the whole turn.
  fn default() -> Self {
    Self {
      drag: Drag::Up { turn: 1.0 / 200.0 },
      points_per_step: 5.0,
      fine: 0.1,
      wheel: 0.05,
    }
  }
}

/// One of a panel's controls.
///
/// A control is something on the panel that is turned, with a readout of its own. It is known
/// by its place among the panel's controls, and names each parameter it turns by that one's
/// place among the panel's parameters. The two orders are separate: a parameter may have no
/// control, or more than one.
///
/// A control turns up to three parameters: its own, one more that a [`Touch::Pad`] is dragged
/// across for, and one that `wheel` gives the wheel. The three are different parameters. A
/// reset puts each one the control can turn at its default, which leaves out the wheel's on a
/// [`Touch::Press`]. The readout, a value typed into it, and the menu's copy and paste are of
/// its own parameter alone. Another of its parameters has those through a second control on
/// it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Control {
  /// The parameter it turns.
  pub parameter: usize,
  /// The way it answers the pointer and the wheel.
  pub feel: Feel,
  /// What its readout can show of a value typed into it.
  pub readout: Readout,
  /// The effect of the left button on it.
  pub touch: Touch,
  /// The parameter the wheel turns, if another than its own.
  ///
  /// It is neither `parameter` nor the one a [`Touch::Pad`] is dragged across for. The wheel
  /// turns it at the pace of `feel` and by the steps of its own [`Dial`](super::Dial). With `None` the wheel
  /// turns `parameter`.
  pub wheel: Option<usize>,
}

impl Control {
  /// Creates a dragged control on a parameter, with a readout.
  ///
  /// It feels as [`Feel::default`] does, and the wheel turns the same parameter.
  pub fn new(parameter: usize, readout: Readout) -> Self {
    Self {
      parameter,
      feel: Feel::default(),
      readout,
      touch: Touch::Turn,
      wheel: None,
    }
  }

  /// Creates a control dragged both ways, on two parameters.
  ///
  /// A drag to the right turns `across` and a drag up the panel turns `up`, which is the
  /// control's own parameter. `size` is the width and height in points of the room the middle
  /// of its puck has, and each parameter goes through its whole turn over its side of that. So
  /// the puck keeps to the pointer from wherever it was taken hold of, as [`Touch::Pad`] says.
  ///
  /// # Panics
  ///
  /// Panics if a side of `size` is not a finite number of points above zero.
  pub fn pad(across: usize, up: usize, size: Point, readout: Readout) -> Self {
    assert!(
      size.iter().all(|side| side.is_finite() && *side > 0.0),
      "the room of a pad has a width and a height"
    );
    let mut control = Self::new(up, readout);
    control.feel.drag = Drag::Up {
      turn: 1.0 / size[1],
    };
    control.touch = Touch::Pad {
      across,
      turn: 1.0 / size[0],
    };
    control
  }

  /// Returns the parameter dragged across for, and its drag.
  pub(super) fn across(&self) -> Option<(usize, Drag)> {
    match self.touch {
      Touch::Pad { across, turn } => {
        let way = [1.0, 0.0];
        Some((across, Drag::Along { way, turn }))
      }
      Touch::Turn | Touch::Press(_) | Touch::Step => None,
    }
  }

  /// Returns whether the wheel turns the control.
  ///
  /// A button is pressed, and the wheel does nothing to it.
  pub(super) fn takes_wheel(&self) -> bool {
    match self.touch {
      Touch::Press(_) => false,
      Touch::Turn | Touch::Step | Touch::Pad { .. } => true,
    }
  }

  /// Returns the parameter the wheel turns.
  pub(super) fn wheel_parameter(&self) -> usize {
    self.wheel.unwrap_or(self.parameter)
  }

  /// Returns the parameters it names, its own first.
  ///
  /// After it are the one across and the wheel's.
  fn named(&self) -> [Option<usize>; 3] {
    let across = self.across().map(|(across, _)| across);
    [Some(self.parameter), across, self.wheel]
  }

  /// Returns every parameter it can turn, its own first.
  pub(super) fn parameters(&self) -> impl Iterator<Item = usize> {
    let [own, across, wheel] = self.named();
    let wheel = wheel.filter(|_| self.takes_wheel());
    [own, across, wheel].into_iter().flatten()
  }

  /// Returns whether it fits a panel of so many parameters.
  ///
  /// It fits if every parameter it names is one of the `parameters` and none is named twice.
  pub fn fits(&self, parameters: usize) -> bool {
    let [own, across, wheel] = self.named();
    let twice = across == own || wheel == own || (wheel.is_some() && wheel == across);
    !twice && (self.named().into_iter().flatten()).all(|named| named < parameters)
  }
}

/// What a control's readout can show of a value typed into it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Readout {
  /// The number of letters it has room for.
  pub room: usize,
  /// The test of whether it has a letter for something typed.
  pub shows: fn(char) -> bool,
}

impl Readout {
  /// Creates a readout with room for `room` letters.
  ///
  /// It shows a space and whatever [`stroke`](crate::stroke) has a letter for.
  pub fn new(room: usize) -> Self {
    Self {
      room,
      shows: |letter| letter == ' ' || crate::stroke::lettered(letter),
    }
  }
}

/// What a control shows, as the panel's look is handed it.
///
/// All of it is of the control's own parameter.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Reading {
  /// The place its parameter stands at, as a part of its whole turn.
  pub value: f32,
  /// The text of its readout.
  ///
  /// It is the value as it is written, or what has been typed so far while one is being typed.
  pub text: String,
  /// The unit that goes after the text.
  ///
  /// It is empty while a value is typed.
  pub unit: String,
}

#[cfg(test)]
mod tests {
  use super::{Control, Push, Readout};
  use crate::interaction::bench::{FORMANT, MIX, PITCH, ROOM};

  #[test]
  fn a_control_fits_a_panel_that_has_each_parameter_it_names_and_names_none_twice() {
    let readout = Readout::new(ROOM);
    let puck = Control::pad(FORMANT, PITCH, [128.0, 64.0], readout);
    let wheeled = |control: Control, wheel| Control {
      wheel: Some(wheel),
      ..control
    };
    assert!(puck.fits(2) && wheeled(puck, MIX).fits(3));
    // a parameter past the panel's last, whichever way the control names it
    assert!(!puck.fits(1) && !wheeled(puck, MIX).fits(2));
    assert!(!Control::new(MIX, readout).fits(2));
    // one parameter both ways, or under the wheel as well as the pointer
    assert!(!Control::pad(PITCH, PITCH, [128.0, 64.0], readout).fits(4));
    assert!(!wheeled(puck, FORMANT).fits(4) && !wheeled(puck, PITCH).fits(4));
    assert!(!wheeled(Control::new(MIX, readout), MIX).fits(4));
  }

  #[test]
  fn a_latching_button_stays_in_and_a_momentary_one_comes_back() {
    // held, either is all the way in
    assert_eq!(Push::Latching.depth(false, true), 1.0);
    assert_eq!(Push::Momentary.depth(false, true), 1.0);
    // let go, a latching one that is on stays most of the way in and the rest are out
    let latched = Push::Latching.depth(true, false);
    assert!(latched > 0.5 && latched < 1.0);
    assert_eq!(Push::Latching.depth(false, false), 0.0);
    assert_eq!(Push::Momentary.depth(true, false), 0.0);
    // each push turns a latching one over
    assert!(Push::Latching.released(false));
    assert!(!Push::Latching.released(true));
    assert!(!Push::Momentary.released(false));
  }
}
