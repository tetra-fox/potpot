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

/// Returns the table as `Rg16Float` texels, row by row.
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
  use std::f32::consts::PI;

  use glam::Vec3;

  use super::*;

  /// Returns the scale and bias at an n.v and a roughness, summed over the hemisphere.
  ///
  /// The highlight is summed over a grid of the ways light can come from. The sum is Filament's GGX, visibility and Fresnel taken as they are, without the
  /// importance sampling the table is made by. It needs a highlight wider than a step of the
  /// grid.
  fn summed(seen: f32, roughness: f32) -> [f32; 2] {
    const STEPS: u32 = 640;
    let alpha = roughness * roughness;
    let a2 = alpha * alpha;
    let viewer = Vec3::new((1.0 - seen * seen).sqrt(), 0.0, seen);
    let mut sums = [0.0; 2];
    // even steps of the angle from the normal and of the turn around it, each a patch of the
    // hemisphere as large as the sine of that angle
    let step = PI / 2.0 / STEPS as f32;
    for up in 0..STEPS {
      let (sine, nol) = ((up as f32 + 0.5) * step).sin_cos();
      for around in 0..STEPS {
        let turn = TAU * (around as f32 + 0.5) / STEPS as f32;
        let light = Vec3::new(sine * turn.cos(), sine * turn.sin(), nol);
        let half = (light + viewer).normalize();
        let facets = (half.z * half.z * (a2 - 1.0) + 1.0).powi(2);
        let highlight = a2 / (PI * facets) * visibility(seen, nol, a2) * nol;
        let fresnel = (1.0 - light.dot(half)).powi(5);
        let area = sine * step * TAU / STEPS as f32;
        sums[0] += highlight * (1.0 - fresnel) * area;
        sums[1] += highlight * fresnel * area;
      }
    }
    sums
  }

  #[test]
  fn the_table_agrees_with_the_highlight_summed_whole() {
    let table = table();
    // from a roughness of about 0.3 and an n.v of about 0.1, short of which the grid is too
    // coarse for the highlight
    for row in (19..SIDE).step_by(7) {
      for column in (6..SIDE).step_by(7) {
        let [seen, roughness] = [column, row].map(|index| (index as f32 + 0.5) / SIDE as f32);
        let texel = table[(row * SIDE + column) as usize].map(f32::from);
        let sum = summed(seen, roughness);
        for (made, summed) in texel.iter().zip(sum) {
          assert!(
            (made - summed).abs() < 5e-2,
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
