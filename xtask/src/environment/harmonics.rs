//! Spherical harmonics of the first three bands, for the light all around a surface.

use glam::Vec3;

/// Returns the nine real spherical harmonics at a way out, in the order `scene.wgsl` reads them.
pub fn basis(Vec3 { x, y, z }: Vec3) -> [f32; 9] {
  [
    0.282_095,
    0.488_603 * y,
    0.488_603 * z,
    0.488_603 * x,
    1.092_548 * x * y,
    1.092_548 * y * z,
    0.315_392 * (3.0 * z * z - 1.0),
    1.092_548 * x * z,
    0.546_274 * (x * x - y * y),
  ]
}

/// Returns the projection of light onto the harmonics.
///
/// Each sample is a way out, the light from it, and the solid angle it stands for.
pub fn projected(samples: impl Iterator<Item = (Vec3, Vec3, f32)>) -> [Vec3; 9] {
  let mut coefficients = [Vec3::ZERO; 9];
  for (out, light, angle) in samples {
    for (coefficient, value) in coefficients.iter_mut().zip(basis(out)) {
      *coefficient += light * value * angle;
    }
  }
  coefficients
}

/// Returns the coefficients turned from light to what a white Lambert surface gives back.
///
/// That is the irradiance over pi, the cosine lobe of each band in Ramamoorthi and Hanrahan's
/// form.
pub fn diffused(coefficients: [Vec3; 9]) -> [Vec3; 9] {
  let bands = [
    1.0,
    2.0 / 3.0,
    2.0 / 3.0,
    2.0 / 3.0,
    0.25,
    0.25,
    0.25,
    0.25,
    0.25,
  ];
  std::array::from_fn(|index| coefficients[index] * bands[index])
}

/// Returns the sum of the harmonics at a way out, weighed by the coefficients.
#[cfg(test)]
fn evaluated(coefficients: &[Vec3; 9], out: Vec3) -> Vec3 {
  coefficients
    .iter()
    .zip(basis(out))
    .map(|(coefficient, value)| *coefficient * value)
    .sum()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::environment::latitude_longitude;

  #[test]
  fn a_constant_studio_projects_to_itself() {
    let light = Vec3::new(0.4, 1.0, 2.5);
    let (width, height) = (512, 256);
    let samples = latitude_longitude(width, height).map(|(_, out, angle)| (out, light, angle));
    let coefficients = projected(samples);
    for out in [Vec3::X, Vec3::new(0.3, -0.8, 0.52).normalize(), Vec3::NEG_Z] {
      for back in [
        evaluated(&coefficients, out),
        evaluated(&diffused(coefficients), out),
      ] {
        assert!(
          ((back - light) / light).abs().max_element() < 1e-4,
          "{out} gave {back}"
        );
      }
    }
  }
}
