//! The bins of a spectrum gathered into the bands a display draws.
//!
//! The module holds what a display draws and transforms no sound. [`bands`] takes the levels of
//! a Fourier transform's bins and gives the level of each band of a display laid along an
//! [`Axis`]. It neither allocates nor locks, so an audio thread can call it.

use crate::axis::Axis;

/// Gathers the levels of a spectrum's bins into bands.
///
/// `bins` are the amplitudes of a real signal's Fourier transform, from no frequency up to half
/// the sample rate `rate` in hertz and evenly spaced. A transform of `n` samples has `n / 2 + 1`
/// of them. Each of `levels` is set to the amplitude of one band. The bands are laid evenly
/// along `across`, which is in hertz, and each covers its share of the axis's length.
///
/// A band that holds one bin or more has the amplitude of the loudest, so a peak stands as tall
/// however many bins a band holds. A band that holds none lies between two bins and has the
/// amplitude on the straight line between them at the band's middle. A band that lies under no
/// frequency, or past half the sample rate, has none.
///
/// # Panics
///
/// Panics if `bins` holds fewer than two bins.
pub fn bands(bins: &[f32], rate: f32, across: Axis, levels: &mut [f32]) {
  assert!(bins.len() >= 2, "a spectrum has two bins at least");
  let last = bins.len() - 1;
  // the bins run from nothing to half the rate in `last` equal steps
  let spacing = rate / 2.0 / last as f32;
  let count = levels.len() as f32;
  for (band, level) in levels.iter_mut().enumerate() {
    let ends = [band as f32, band as f32 + 1.0].map(|end| across.plain(end / count) / spacing);
    // an axis can run down, with its higher frequency first
    let (low, high) = (ends[0].min(ends[1]), ends[0].max(ends[1]));
    // the bins from the band's low end up to and not with its high end
    let held = (low.ceil() as usize).min(bins.len())..(high.ceil() as usize).min(bins.len());
    *level = if held.is_empty() {
      let middle = across.plain((band as f32 + 0.5) / count) / spacing;
      if (0.0..=last as f32).contains(&middle) {
        // the two bins around the middle, the last two for a middle on the last bin
        let below = (middle as usize).min(last - 1);
        let part = middle - below as f32;
        bins[below] + (bins[below + 1] - bins[below]) * part
      } else {
        0.0
      }
    } else {
      bins[held].iter().copied().fold(0.0, f32::max)
    };
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A transform of 64 samples at 6400 hertz, whose bins are 100 hertz apart.
  const RATE: f32 = 6400.0;

  #[test]
  fn a_peak_lands_in_the_band_that_holds_its_frequency_alone() {
    let mut bins = [0.0; 33];
    // 2000 hertz, among bands much wider than the bins
    bins[20] = 1.0;
    let across = Axis::linear([0.0, 3200.0]);
    let mut levels = [0.0; 4];
    bands(&bins, RATE, across, &mut levels);
    // the second band is 800 to 1600 hertz and the third 1600 to 2400
    assert_eq!(levels, [0.0, 0.0, 1.0, 0.0]);
    // the bin on the edge between two bands is in the higher one alone
    bins[20] = 0.0;
    bins[8] = 0.5;
    bands(&bins, RATE, across, &mut levels);
    assert_eq!(levels, [0.0, 0.5, 0.0, 0.0]);
  }

  #[test]
  fn a_band_narrower_than_the_bins_reads_the_line_between_two() {
    let mut bins = [0.0; 33];
    bins[2] = 1.0;
    bins[3] = 3.0;
    // eight bands from 200 to 300 hertz, each a hair over 5 hertz wide and holding no bin past
    // the first
    let across = Axis::log([200.0, 300.0]);
    let mut levels = [0.0; 8];
    bands(&bins, RATE, across, &mut levels);
    assert_eq!(levels[0], 1.0);
    for (band, level) in levels.iter().enumerate().skip(1) {
      let middle = across.plain((band as f32 + 0.5) / 8.0);
      let line = 1.0 + 2.0 * (middle - 200.0) / 100.0;
      assert!(
        (level - line).abs() < 1e-4,
        "band {band}: {level} off {line}"
      );
    }
  }

  #[test]
  fn bands_past_either_end_of_the_spectrum_are_silent() {
    let bins = [1.0; 33];
    let mut levels = [9.0; 4];
    // the second band reaches past 3200 hertz and the others start past it
    bands(&bins, RATE, Axis::linear([0.0, 8000.0]), &mut levels);
    assert_eq!(levels, [1.0, 1.0, 0.0, 0.0]);
    // a band that runs over half the rate holds the last bin
    bands(&bins, RATE, Axis::linear([0.0, 4000.0]), &mut levels);
    assert_eq!(levels, [1.0, 1.0, 1.0, 1.0]);
    // and bands under no frequency are as silent, whatever the bins over it hold
    bands(&bins, RATE, Axis::linear([-3200.0, 3200.0]), &mut levels);
    assert_eq!(levels, [0.0, 0.0, 1.0, 1.0]);
  }
}
