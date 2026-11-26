//! The gain of an equalizer's bands over frequency.
//!
//! The module holds what a display draws and filters no sound. A [`Band`] gives its gain at a
//! frequency, and [`trace`] the points of the curve several bands make together.

use std::f64::consts::{PI, TAU};

use crate::Point;
use crate::axis::Axis;

/// The shape of a band's response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// A boost or a cut around a center frequency.
  Bell,
  /// A boost or a cut of everything under a frequency.
  LowShelf,
  /// A boost or a cut of everything over a frequency.
  HighShelf,
  /// A filter that takes away what is under a frequency.
  ///
  /// It is a high-pass filter and falls 12 decibels an octave.
  LowCut,
  /// A filter that takes away what is over a frequency.
  ///
  /// It is a low-pass filter and falls 12 decibels an octave.
  HighCut,
}

/// One band of an equalizer.
///
/// Its response is that of the second-order digital filter the Audio EQ Cookbook gives for its
/// [`Kind`], at the sample rate it is asked at. An equalizer that runs those filters is drawn
/// as it sounds, up to half the sample rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
  /// The shape of its response.
  pub kind: Kind,
  /// The band's frequency in hertz.
  ///
  /// It is a bell's center, a shelf's middle or a cut's corner.
  pub frequency: f32,
  /// The band's gain in decibels.
  ///
  /// A bell has it at its center and a shelf far past its middle. A cut has none.
  pub gain: f32,
  /// The Q, above zero.
  ///
  /// More of it makes a bell narrower and a shelf or a cut steeper at its frequency. At the
  /// square root of a half a cut is three decibels down at its corner and has no peak before
  /// it.
  pub q: f32,
}

/// The coefficients of a second-order filter.
#[derive(Clone, Copy, Debug)]
struct Biquad {
  zeros: [f64; 3],
  poles: [f64; 3],
}

impl Band {
  /// Returns the band's gain in decibels at a frequency.
  ///
  /// `frequency` and the sample rate `rate` are in hertz.
  pub fn gain_at(&self, frequency: f32, rate: f32) -> f32 {
    self.biquad(rate).gain_at(frequency, rate) as f32
  }

  /// Returns the band's filter at a sample rate in hertz.
  fn biquad(&self, rate: f32) -> Biquad {
    let (sin, cos) = (TAU * f64::from(self.frequency) / f64::from(rate)).sin_cos();
    let alpha = sin / (2.0 * f64::from(self.q));
    // the gain as a ratio of amplitudes, halved in decibels: a bell and a shelf have it once
    // in their zeros and once more, turned over, in their poles
    let amp = 10.0_f64.powf(f64::from(self.gain) / 40.0);
    let slope = 2.0 * amp.sqrt() * alpha;
    let (more, less) = (amp + 1.0, amp - 1.0);
    let cut = [1.0 + alpha, -2.0 * cos, 1.0 - alpha];
    match self.kind {
      Kind::Bell => Biquad {
        zeros: [1.0 + alpha * amp, -2.0 * cos, 1.0 - alpha * amp],
        poles: [1.0 + alpha / amp, -2.0 * cos, 1.0 - alpha / amp],
      },
      Kind::LowShelf => Biquad {
        zeros: [
          amp * (more - less * cos + slope),
          2.0 * amp * (less - more * cos),
          amp * (more - less * cos - slope),
        ],
        poles: [
          more + less * cos + slope,
          -2.0 * (less + more * cos),
          more + less * cos - slope,
        ],
      },
      Kind::HighShelf => Biquad {
        zeros: [
          amp * (more + less * cos + slope),
          -2.0 * amp * (less + more * cos),
          amp * (more + less * cos - slope),
        ],
        poles: [
          more - less * cos + slope,
          2.0 * (less - more * cos),
          more - less * cos - slope,
        ],
      },
      Kind::LowCut => Biquad {
        zeros: [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
        poles: cut,
      },
      Kind::HighCut => Biquad {
        zeros: [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
        poles: cut,
      },
    }
  }
}

impl Biquad {
  /// Returns the filter's gain in decibels at a frequency.
  ///
  /// `frequency` and the sample rate `rate` are in hertz.
  fn gain_at(&self, frequency: f32, rate: f32) -> f64 {
    let half = (PI * f64::from(frequency) / f64::from(rate)).sin();
    let turned = half * half;
    // the squared size of a + b/z + c/z^2, in the sine of half the angle. in the cosines of
    // the angle and its double it is a small difference of large numbers at low frequencies
    let power = |[a, b, c]: [f64; 3]| {
      let sum = a + b + c;
      sum * sum - 4.0 * (a * b + 4.0 * a * c + b * c) * turned + 16.0 * a * c * turned * turned
    };
    // a cut lets nothing through at one end, where rounding can leave a hair under nothing
    10.0 * (power(self.zeros).max(0.0) / power(self.poles)).log10()
  }
}

/// Returns the curve several bands make together.
///
/// The curve has `count` points evenly spread along `across`, which is in hertz. Each is how
/// far along `across` it is and how far along `up` the gains of all the `bands` at its
/// frequency add up to, in decibels, both as parts of the [`Axis`]'s length. A gain past an end
/// of `up` lies on that end. `rate` is the sample rate in hertz.
///
/// Two controls made with [`Dial::along`](crate::interaction::Dial::along) on the same axes
/// have their values at the place of a band's frequency and gain in the same picture. That
/// place is on the curve only for a bell with no other band near it: a shelf has half its gain
/// at its frequency, and the other bands move every point.
pub fn trace(bands: &[Band], rate: f32, across: Axis, up: Axis, count: usize) -> Vec<Point> {
  let filters: Vec<Biquad> = bands.iter().map(|band| band.biquad(rate)).collect();
  // one point alone is at the start of the axis
  let last = count.saturating_sub(1).max(1) as f32;
  (0..count)
    .map(|index| {
      let along = index as f32 / last;
      let frequency = across.plain(along);
      let gain: f64 = (filters.iter())
        .map(|filter| filter.gain_at(frequency, rate))
        .sum();
      [along, up.turn(gain as f32)]
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use std::f32::consts::FRAC_1_SQRT_2;

  use super::*;

  const RATE: f32 = 48000.0;

  fn band(kind: Kind, frequency: f32, gain: f32, q: f32) -> Band {
    Band {
      kind,
      frequency,
      gain,
      q,
    }
  }

  #[test]
  fn a_bell_has_its_gain_at_its_center_and_none_far_from_it() {
    for gain in [6.0, -9.0] {
      let bell = band(Kind::Bell, 1000.0, gain, 1.0);
      assert!((bell.gain_at(1000.0, RATE) - gain).abs() < 1e-4);
      for far in [20.0, 20000.0] {
        assert!(bell.gain_at(far, RATE).abs() < 0.01, "{gain} at {far}");
      }
      // an octave under the center and an octave over it, which are as far from it on a
      // log axis, and part of the way to the gain
      let (under, over) = (bell.gain_at(500.0, RATE), bell.gain_at(2000.0, RATE));
      assert!((under - over).abs() < 0.02, "{under} and {over}");
      assert!(under / gain > 0.2 && under / gain < 0.5, "{under}");
    }
    // a narrower bell is back at nothing sooner
    let narrow = band(Kind::Bell, 1000.0, 6.0, 8.0).gain_at(2000.0, RATE);
    assert!(narrow < 0.1, "{narrow}");
  }

  #[test]
  fn a_cut_is_three_decibels_down_at_its_corner_and_falls_twelve_an_octave() {
    let half_power = 10.0 * 0.5_f32.log10();
    let low_cut = band(Kind::LowCut, 1000.0, 0.0, FRAC_1_SQRT_2);
    let high_cut = band(Kind::HighCut, 1000.0, 0.0, FRAC_1_SQRT_2);
    for cut in [low_cut, high_cut] {
      assert!((cut.gain_at(1000.0, RATE) - half_power).abs() < 1e-3);
    }
    // three octaves into what each takes away, and three into what it leaves
    let (under, over) = (125.0, 8000.0);
    assert!((low_cut.gain_at(under, RATE) - -36.0).abs() < 0.5);
    assert!(low_cut.gain_at(over, RATE).abs() < 0.01);
    assert!(high_cut.gain_at(under, RATE).abs() < 0.01);
    // nearer half the sample rate a digital filter falls faster than twelve an octave
    assert!(high_cut.gain_at(over, RATE) < -36.0);
    // nothing at all gets through a low cut at no frequency
    assert_eq!(low_cut.gain_at(0.0, RATE), f32::NEG_INFINITY);
  }

  #[test]
  fn a_shelf_reaches_its_gain_on_its_side_and_half_of_it_at_its_middle() {
    let low = band(Kind::LowShelf, 200.0, 6.0, FRAC_1_SQRT_2);
    let high = band(Kind::HighShelf, 4000.0, -9.0, FRAC_1_SQRT_2);
    assert!((low.gain_at(20.0, RATE) - 6.0).abs() < 0.01);
    assert!((low.gain_at(200.0, RATE) - 3.0).abs() < 1e-3);
    assert!(low.gain_at(20000.0, RATE).abs() < 0.01);
    assert!(high.gain_at(20.0, RATE).abs() < 0.01);
    assert!((high.gain_at(4000.0, RATE) - -4.5).abs() < 1e-3);
    assert!((high.gain_at(20000.0, RATE) - -9.0).abs() < 0.01);
  }

  #[test]
  fn a_trace_adds_its_bands_up_along_both_axes() {
    let (hertz, decibels) = (Axis::log([20.0, 20000.0]), Axis::linear([-12.0, 12.0]));
    // a bell and the same bell turned over undo each other
    let undone = [
      band(Kind::Bell, 1000.0, 9.0, 2.0),
      band(Kind::Bell, 1000.0, -9.0, 2.0),
    ];
    let flat = trace(&undone, RATE, hertz, decibels, 33);
    for (index, point) in flat.iter().enumerate() {
      assert!((point[0] - index as f32 / 32.0).abs() < 1e-6);
      assert!((point[1] - 0.5).abs() < 1e-5, "{point:?}");
    }
    // two bells of 9 decibels on one center are 18 there, which is off the top of the axis
    let stacked = [undone[0], undone[0]];
    let peak = hertz.turn(1000.0);
    let curve = trace(&stacked, RATE, hertz, decibels, 33);
    let nearest = (curve.iter())
      .min_by(|one, other| (one[0] - peak).abs().total_cmp(&(other[0] - peak).abs()))
      .expect("the curve has points");
    assert_eq!(nearest[1], 1.0);
    assert!(trace(&stacked, RATE, hertz, decibels, 0).is_empty());
  }
}
