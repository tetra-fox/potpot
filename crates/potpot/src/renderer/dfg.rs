//! The DFG table of the split-sum highlight.
//!
//! The light a highlight throws back from a surround of even light is `f0 * scale + bias`. The
//! table holds the scale and the bias for each n.v and perceptual roughness, worked out by
//! importance sampling Filament's GGX, height-correlated Smith visibility and Schlick Fresnel.

use std::f32::consts::TAU;

/// The table's width and height in texels.
pub(super) const SIDE: u32 = 64;
/// The samples taken for each texel.
const SAMPLES: u32 = 512;

/// Returns the table as [`wgpu::TextureFormat::Rg16Float`] texels, row by row.
///
/// A texel's column is n.v and its row the perceptual roughness, each at the texel's middle.
pub(super) fn table() -> Vec<[half::f16; 2]> {
  let middle = |index: u32| (index as f32 + 0.5) / SIDE as f32;
  (0..SIDE)
    .flat_map(|row| (0..SIDE).map(move |column| (column, row)))
    .map(|(column, row)| scale_and_bias(middle(column), middle(row)).map(half::f16::from_f32))
    .collect()
}

/// Returns the scale and bias at an n.v and a roughness.
///
/// The roughness is perceptual.
fn scale_and_bias(seen: f32, roughness: f32) -> [f32; 2] {
  let alpha = roughness * roughness;
  let a2 = alpha * alpha;
  // the viewer in the surface's own frame, with the normal along z
  let viewer = [(1.0 - seen * seen).sqrt(), 0.0, seen];
  let [mut scale, mut bias] = [0.0; 2];
  for sample in 0..SAMPLES {
    // the Hammersley point of the sample, which spreads the samples evenly
    let [u, w] = [
      sample as f32 / SAMPLES as f32,
      sample.reverse_bits() as f32 / 2f32.powi(32),
    ];
    // a half vector drawn as often as GGX has facets facing that way
    let cosine = ((1.0 - u) / (1.0 + (a2 - 1.0) * u)).sqrt();
    let sine = (1.0 - cosine * cosine).sqrt();
    let turn = TAU * w;
    let half = [sine * turn.cos(), sine * turn.sin(), cosine];
    let voh = viewer[0] * half[0] + viewer[2] * half[2];
    let nol = 2.0 * voh * half[2] - viewer[2];
    if nol <= 0.0 {
      continue;
    }
    // the visibility over the share of the samples' density that is not GGX's
    let shown = visibility(seen, nol, a2) * 4.0 * voh * nol / cosine;
    let fresnel = (1.0 - voh).powi(5);
    scale += shown * (1.0 - fresnel);
    bias += shown * fresnel;
  }
  [scale, bias].map(|sum| sum / SAMPLES as f32)
}

/// Returns the height-correlated Smith visibility.
///
/// `a2` is the perceptual roughness to the fourth power.
fn visibility(seen: f32, nol: f32, a2: f32) -> f32 {
  let from_view = nol * (seen * seen * (1.0 - a2) + a2).sqrt();
  let from_light = seen * (nol * nol * (1.0 - a2) + a2).sqrt();
  0.5 / (from_view + from_light)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Returns the scale and bias at an n.v and a roughness, summed over the hemisphere.
  ///
  /// The highlight is summed over a grid of half vectors, Filament's GGX, visibility and
  /// Fresnel taken as they are. The grid's angles from the normal are spaced by a tangent
  /// scaled to the roughness, which puts as many of them inside a narrow highlight as a wide
  /// one.
  fn summed(seen: f32, roughness: f32) -> [f32; 2] {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    const STEPS: u32 = 400;
    let seen = f64::from(seen);
    let alpha = f64::from(roughness).powi(2);
    let a2 = alpha * alpha;
    let viewer = [(1.0 - seen * seen).sqrt(), 0.0, seen];
    let mut sums = [0.0; 2];
    for up in 0..STEPS {
      let spread = (f64::from(up) + 0.5) / f64::from(STEPS) * FRAC_PI_2;
      let tilt = (alpha * spread.tan()).atan();
      // how fast the angle turns with `spread`
      let along = alpha / spread.cos().powi(2) / (1.0 + (alpha * spread.tan()).powi(2));
      let (sine, cosine) = tilt.sin_cos();
      let facets = a2 / (PI * (cosine * cosine * (a2 - 1.0) + 1.0).powi(2));
      for around in 0..STEPS {
        let turn = TAU * (f64::from(around) + 0.5) / f64::from(STEPS);
        let half = [sine * turn.cos(), sine * turn.sin(), cosine];
        let voh = viewer[0] * half[0] + viewer[2] * half[2];
        let nol = 2.0 * voh * half[2] - viewer[2];
        if nol <= 0.0 || voh <= 0.0 {
          continue;
        }
        let from_view = nol * (seen * seen * (1.0 - a2) + a2).sqrt();
        let from_light = seen * (nol * nol * (1.0 - a2) + a2).sqrt();
        let visibility = 0.5 / (from_view + from_light);
        // a patch of half vectors is a quarter of the patch of light over v.h
        let area = sine * along * FRAC_PI_2 / f64::from(STEPS) * TAU / f64::from(STEPS) * 4.0 * voh;
        let highlight = facets * visibility * nol * area;
        let fresnel = (1.0 - voh).powi(5);
        sums[0] += highlight * (1.0 - fresnel);
        sums[1] += highlight * fresnel;
      }
    }
    sums.map(|sum| sum as f32)
  }

  // Karis's analytic `EnvBRDFApprox` fits Unreal's table, whose Schlick-Smith visibility
  // differs from Filament's by up to 0.17 in the scale, so the table is held to the integral
  // of its own model
  #[test]
  fn the_table_agrees_with_the_highlight_summed_whole() {
    let table = table();
    for row in (0..SIDE).step_by(7) {
      for column in (0..SIDE).step_by(7) {
        let [seen, roughness] = [column, row].map(|index| (index as f32 + 0.5) / SIDE as f32);
        let texel = table[(row * SIDE + column) as usize].map(f32::from);
        let sum = summed(seen, roughness);
        for (made, summed) in texel.iter().zip(sum) {
          assert!(
            (made - summed).abs() < 2e-2,
            "{texel:?} against {sum:?} at n.v {seen} and roughness {roughness}"
          );
        }
      }
    }
  }

  #[test]
  fn scale_and_bias_together_stay_within_zero_and_one() {
    // a smooth surface keeps all of its highlight, which rounds to a half float's step over
    for texel in table() {
      let sum = f32::from(texel[0]) + f32::from(texel[1]);
      assert!(
        (0.0..=1.0 + f32::from(half::f16::EPSILON)).contains(&sum),
        "{texel:?}"
      );
    }
  }

  #[test]
  fn a_smooth_surface_seen_square_on_keeps_nearly_all_its_highlight() {
    let [scale, bias] = scale_and_bias(1.0, 0.045);
    assert!(scale + bias > 0.95, "{scale} and {bias}");
  }
}
