//! Values the editor has set and not yet seen in the parameters.
//!
//! A value set from the editor reaches its parameter later: the wrappers take it at the end of
//! an audio block, and the VST3 one waits for the host to hand it back first. Until then the
//! panel shows what was set. Whether the value has got there is told by looking at the
//! parameter, each time the panel is about to be drawn or checked. What the wrapper reports
//! may come late or never, so nothing is told by it.
//!
//! What is kept for a parameter is empty whenever the parameter is at the value set last, and
//! is emptied by the parameter going anywhere the editor did not send it. So the panel shows
//! a value of the editor's own only while the parameter is still on its way to it.

use std::collections::VecDeque;

/// The most values of one parameter that are remembered.
///
/// A host that takes none of them leaves them here, and past this many the oldest are
/// forgotten.
const ROOM: usize = 256;
/// The distance within which a parameter has a value that was set.
const SAME: f32 = 1e-6;

/// The values the editor has set and not yet seen arrive.
///
/// A parameter is known by its place among the panel's.
#[derive(Debug)]
pub struct Sent {
  /// The values set and not yet seen in each parameter, oldest first.
  waiting: Vec<VecDeque<f32>>,
  /// The value each parameter had when it was last looked at.
  seen: Vec<f32>,
}

impl Sent {
  /// Creates the record for parameters that stand at `values`.
  ///
  /// `values` is in the parameters' order on the panel.
  pub fn new(values: &[f32]) -> Self {
    Self {
      waiting: vec![VecDeque::new(); values.len()],
      seen: values.to_vec(),
    }
  }

  /// Records that the editor has set a parameter.
  ///
  /// The parameter has to have been looked at just before, with [`Sent::look`].
  ///
  /// # Panics
  ///
  /// Panics if `parameter` is past the last of the parameters given to [`Sent::new`].
  pub fn set(&mut self, parameter: usize, value: f32) {
    let waiting = &mut self.waiting[parameter];
    if waiting.len() == ROOM {
      waiting.pop_front();
    }
    waiting.push_back(value);
  }

  /// Looks at where a parameter is.
  ///
  /// Values get there in the order they were set, so one of them in the parameter means every
  /// one before it has been and gone.
  ///
  /// # Panics
  ///
  /// Panics if `parameter` is past the last of the parameters given to [`Sent::new`].
  pub fn look(&mut self, parameter: usize, value: f32) {
    let (waiting, seen) = (&mut self.waiting[parameter], &mut self.seen[parameter]);
    let same = |sent: &f32| (sent - value).abs() <= SAME;
    // at the value set last, whether it has just got there or was there all along
    if waiting.back().is_some_and(same) {
      waiting.clear();
    } else if value != *seen {
      // it has moved: to one of the values still on their way, or to one of the host's own
      match waiting.iter().position(same) {
        Some(place) => drop(waiting.drain(..=place)),
        None => waiting.clear(),
      }
    }
    *seen = value;
  }

  /// Returns the value to show for a parameter that is at `value`.
  ///
  /// # Panics
  ///
  /// Panics if `parameter` is past the last of the parameters given to [`Sent::new`].
  pub fn shown(&self, parameter: usize, value: f32) -> f32 {
    (self.waiting[parameter].back().copied()).unwrap_or(value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // two of a panel's three parameters
  const PITCH: usize = 0;
  const MIX: usize = 2;

  /// Creates a record with the pitch at a half and nothing set.
  fn sent() -> Sent {
    Sent::new(&[0.5, 0.5, 1.0])
  }

  /// Looks at the pitch and returns what the panel shows of it.
  fn shown(sent: &mut Sent, parameter: f32) -> f32 {
    sent.look(PITCH, parameter);
    sent.shown(PITCH, parameter)
  }

  #[test]
  fn with_nothing_set_the_parameter_shows_wherever_it_goes() {
    let mut sent = sent();
    for parameter in [0.5, 0.7, 0.7, 0.2] {
      assert_eq!(shown(&mut sent, parameter), parameter);
    }
  }

  #[test]
  fn what_was_set_shows_until_the_parameter_gets_there() {
    let mut sent = sent();
    for value in [0.1, 0.2, 0.3] {
      sent.set(PITCH, value);
    }
    // not yet moved, and then through the values in the order they were set
    for parameter in [0.5, 0.5, 0.1, 0.2] {
      assert_eq!(shown(&mut sent, parameter), 0.3, "at {parameter}");
    }
    assert_eq!(shown(&mut sent, 0.3), 0.3);
    // and from there on it is the parameter that shows
    assert_eq!(shown(&mut sent, 0.9), 0.9);
    // none of which touches another parameter
    assert_eq!(sent.shown(MIX, 1.0), 1.0);
  }

  #[test]
  fn values_that_were_skipped_on_the_way_are_settled_with_the_one_that_came() {
    // the parameter is looked at now and then, and may have taken several values between
    let mut sent = sent();
    for value in [0.1, 0.2, 0.3, 0.4] {
      sent.set(PITCH, value);
    }
    assert_eq!(shown(&mut sent, 0.3), 0.4);
    assert_eq!(sent.waiting[PITCH], [0.4]);
    assert_eq!(shown(&mut sent, 0.4), 0.4);
    assert!(sent.waiting[PITCH].is_empty());
  }

  #[test]
  fn the_hosts_own_value_takes_over_at_once() {
    let mut sent = sent();
    sent.set(PITCH, 0.4);
    assert_eq!(shown(&mut sent, 0.5), 0.4);
    assert_eq!(shown(&mut sent, 0.7), 0.7);
    assert!(sent.waiting[PITCH].is_empty());
    assert_eq!(shown(&mut sent, 0.4), 0.4);
    assert_eq!(shown(&mut sent, 0.45), 0.45);
  }

  #[test]
  fn a_value_set_where_the_parameter_already_is_leaves_nothing_waiting() {
    // the wrapper reports no change for it, and the parameter never moves to show it came
    let mut sent = sent();
    sent.set(PITCH, 0.5);
    assert_eq!(shown(&mut sent, 0.5), 0.5);
    assert!(sent.waiting[PITCH].is_empty());
    // dragged away and back before the parameter was looked at again
    sent.set(PITCH, 0.6);
    sent.set(PITCH, 0.5);
    assert_eq!(shown(&mut sent, 0.5), 0.5);
    assert!(sent.waiting[PITCH].is_empty());
    // so the host putting it where the drag had passed shows
    assert_eq!(shown(&mut sent, 0.6), 0.6);
  }

  #[test]
  fn the_host_moving_through_a_value_still_waiting_does_not_hold_the_panel_there() {
    // set 0.2 then 0.3, with only 0.2 arrived: the host now takes the parameter through 0.3
    // and on. the 0.3 counts as the editor's own arriving, and what follows comes from the host
    let mut sent = sent();
    sent.set(PITCH, 0.2);
    sent.set(PITCH, 0.3);
    assert_eq!(shown(&mut sent, 0.2), 0.3);
    assert_eq!(shown(&mut sent, 0.3), 0.3);
    for parameter in [0.35, 0.4, 0.2, 0.3] {
      assert_eq!(shown(&mut sent, parameter), parameter);
    }
  }

  #[test]
  fn a_host_that_takes_nothing_does_not_fill_the_room() {
    let mut sent = sent();
    for step in 0..ROOM * 3 {
      sent.set(MIX, step as f32);
    }
    assert_eq!(sent.waiting[MIX].len(), ROOM);
    assert_eq!(sent.shown(MIX, 1.0), (ROOM * 3 - 1) as f32);
  }
}
