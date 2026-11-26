//! Runs of plain values laid along a line, evenly or by ratio.

/// The way plain values are spaced along an [`Axis`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Spacing {
  /// Equal differences take equal lengths.
  Even,
  /// Equal ratios take equal lengths.
  Ratio,
}

/// A run of plain values laid along a line.
///
/// A place on it is a part of its whole length, from zero at its first end to one at its second,
/// which is what a [`Dial`](crate::interaction::Dial) calls a value. A frequency axis from 20
/// to 20000 hertz and a gain axis in decibels are each one, and so is the scale of any
/// parameter. [`Dial::along`](crate::interaction::Dial::along) makes a parameter of an axis, so
/// that a control and a drawing made with the same axis put a plain value at the same place.
///
/// The two ends of an axis are different finite numbers. An axis whose ends are the same has
/// no place for anything, and its places and plain values come out as NaN.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
  ends: [f32; 2],
  spacing: Spacing,
}

impl Axis {
  /// Creates an axis that spaces plain values evenly.
  ///
  /// Equal differences take equal lengths. `range` is the plain values at its two ends.
  pub const fn linear(range: [f32; 2]) -> Self {
    Self {
      ends: range,
      spacing: Spacing::Even,
    }
  }

  /// Creates an axis that spaces plain values by ratio.
  ///
  /// Equal ratios take equal lengths, the way frequencies are laid out, an octave as long at
  /// the low end as at the high. `range` is the plain values at its two ends.
  ///
  /// # Panics
  ///
  /// Panics if an end of `range` is not a finite number above zero.
  pub const fn log(range: [f32; 2]) -> Self {
    let [from, to] = range;
    assert!(
      from.is_finite() && from > 0.0 && to.is_finite() && to > 0.0,
      "both ends of a log axis are above zero"
    );
    Self {
      ends: range,
      spacing: Spacing::Ratio,
    }
  }

  /// Returns the plain value at a place along the axis.
  ///
  /// `turn` is the part of the axis's length the place is from its first end. Zero and one
  /// give the two ends themselves.
  pub fn plain(&self, turn: f32) -> f32 {
    let [from, to] = self.ends;
    match self.spacing {
      Spacing::Even => from + turn * (to - from),
      Spacing::Ratio => {
        // worked out twice as finely as it is given back, which lands the second end on itself
        let (from, to) = (f64::from(from), f64::from(to));
        (from * (to / from).powf(f64::from(turn))) as f32
      }
    }
  }

  /// Returns the place of a plain value along the axis.
  ///
  /// The place is a part of the axis's length. A plain value past either end comes out as that
  /// end, and on an axis spaced by ratio so does zero and anything under it, past the lower
  /// end. NaN comes out as NaN.
  pub fn turn(&self, plain: f32) -> f32 {
    let [from, to] = self.ends;
    let part = match self.spacing {
      Spacing::Even => (plain - from) / (to - from),
      Spacing::Ratio => {
        // a value under zero has no logarithm, and is as far past the lower end as zero is
        let above = if plain < 0.0 { 0.0 } else { plain };
        (above / from).ln() / (to / from).ln()
      }
    };
    part.clamp(0.0, 1.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_log_axis_spaces_by_ratio_both_ways_and_stops_at_its_ends() {
    let hertz = Axis::log([20.0, 20000.0]);
    // the middle of the axis is as many times its low end as its high end is times the middle
    let middle = hertz.plain(0.5);
    assert!(
      (middle - (20.0_f32 * 20000.0).sqrt()).abs() < 0.01,
      "{middle}"
    );
    for turn in [0.1, 0.5, 0.9] {
      let back = hertz.turn(hertz.plain(turn));
      assert!((back - turn).abs() < 1e-6, "{turn} came back as {back}");
    }
    // zero and anything under it have no logarithm, and are past the low end like 5 is
    for under in [5.0, 0.0, -100.0] {
      assert_eq!(hertz.turn(under), 0.0);
    }
    assert_eq!(hertz.turn(96000.0), 1.0);
    // an axis that runs down has its low end second
    assert_eq!(Axis::log([20000.0, 20.0]).turn(0.0), 1.0);
    assert!(hertz.turn(f32::NAN).is_nan());
  }

  #[test]
  fn the_ends_of_a_log_axis_are_its_ends_exactly() {
    // ends whose ratio is no whole number, either way round
    for ends in [[20.0, 20000.0], [0.3, 8.0], [18000.0, 30.0], [1e-3, 7e5]] {
      let axis = Axis::log(ends);
      assert_eq!([axis.plain(0.0), axis.plain(1.0)], ends);
      assert_eq!([axis.turn(ends[0]), axis.turn(ends[1])], [0.0, 1.0]);
    }
  }
}
