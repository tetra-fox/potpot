//! The wheel's turning of a control.

use std::time::{Duration, Instant};

use super::dial::SLACK;
use super::{Action, Interaction, Moment, State, Target};

/// The rest after which a turn of the wheel is a new gesture.
const WHEEL_REST: Duration = Duration::from_millis(400);

/// The wheel turning a control.
#[derive(Debug)]
pub(super) struct Wheel {
  control: usize,
  /// The value it last set.
  ///
  /// It is `None` until the wheel sets one, which is when the host hears of it.
  set: Option<f32>,
  /// The part of a line it has turned a stepped control, short of one.
  gathered: f32,
  /// The time it last moved.
  last: Instant,
}

impl Interaction {
  pub(super) fn wheeled(
    &mut self,
    over: Option<Target>,
    lines: f32,
    fine: bool,
    moment: Moment,
    actions: &mut Vec<Action>,
  ) {
    let (State::Idle, Some(Target::Control(control) | Target::Readout(control))) =
      (&self.state, over)
    else {
      return;
    };
    // a wheel turned sideways comes as no lines this way
    if lines == 0.0 {
      return;
    }
    let turned = self.controls[control];
    if !turned.takes_wheel() {
      return;
    }
    let (feel, parameter) = (turned.feel, turned.wheel_parameter());
    if (self.wheel.as_ref()).is_none_or(|wheel| wheel.control != control) {
      self.rest_wheel(actions);
    }
    let wheel = self.wheel.get_or_insert(Wheel {
      control,
      set: None,
      gathered: 0.0,
      last: moment.now,
    });
    wheel.last = moment.now;
    let dial = &self.dials[parameter];
    // a control that moves in steps goes one for each whole line, and what is left of a
    // line waits for the rest of it
    let lines = if dial.step.is_some() && !fine {
      wheel.gathered += lines;
      let whole = (wheel.gathered + SLACK.copysign(wheel.gathered)).trunc();
      wheel.gathered -= whole;
      if whole == 0.0 {
        return;
      }
      whole
    } else {
      lines
    };
    let from = match wheel.set {
      Some(value) => value,
      None => {
        actions.push(Action::Begin(parameter));
        moment.values[parameter]
      }
    };
    let value = dial.wheeled(from, lines, fine, feel);
    wheel.set = Some(value);
    actions.push(Action::Set(parameter, value));
  }

  /// Rests the wheel from the control it was turning.
  ///
  /// The gesture ends if the host has heard of one.
  pub(super) fn rest_wheel(&mut self, actions: &mut Vec<Action>) {
    if let Some(Wheel {
      control,
      set: Some(_),
      ..
    }) = self.wheel.take()
    {
      actions.push(Action::End(self.controls[control].wheel_parameter()));
    }
  }

  /// Rests the wheel once it has been still for [`WHEEL_REST`].
  pub(super) fn rest_wheel_if_still(&mut self, now: Instant, actions: &mut Vec<Action>) {
    if let Some(Wheel { last, .. }) = self.wheel
      && now.duration_since(last) >= WHEEL_REST
    {
      self.rest_wheel(actions);
    }
  }
}

#[cfg(test)]
mod tests;
