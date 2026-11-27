//! The `Rgb9e5Ufloat` texel format: three 9-bit mantissas with one 5-bit exponent.

use glam::Vec3;

const MANTISSA_BITS: i32 = 9;
const BIAS: i32 = 15;
const MAX_EXPONENT: i32 = 31;
/// The brightest a channel holds.
const MAX: f32 = (511.0 / 512.0) * (1 << (MAX_EXPONENT - BIAS)) as f32;

/// Returns light as one texel, rounded to the nearest the format holds.
///
/// Negative channels become zero and those past the format's range its brightest.
pub fn encoded(light: Vec3) -> u32 {
  let light = light.clamp(Vec3::ZERO, Vec3::splat(MAX));
  let brightest = light.max_element();
  if brightest == 0.0 {
    return 0;
  }
  let mut exponent = (brightest.log2().floor() as i32).max(-BIAS - 1) + 1 + BIAS;
  let mut step = 2f32.powi(exponent - BIAS - MANTISSA_BITS);
  // a channel that rounds up past the mantissa takes the next exponent
  if (brightest / step + 0.5).floor() as u32 == 1 << MANTISSA_BITS {
    step *= 2.0;
    exponent += 1;
  }
  let [r, g, b] = (light / step + 0.5)
    .floor()
    .to_array()
    .map(|channel| channel as u32);
  r | g << 9 | b << 18 | (exponent as u32) << 27
}

/// Returns the light a texel holds.
#[cfg(test)]
pub fn decoded(texel: u32) -> Vec3 {
  let exponent = (texel >> 27) as i32;
  let step = 2f32.powi(exponent - BIAS - MANTISSA_BITS);
  let mantissa = |shift: u32| ((texel >> shift) & 0x1ff) as f32;
  Vec3::new(mantissa(0), mantissa(9), mantissa(18)) * step
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn light_comes_back_to_within_a_step_of_its_brightest_channel() {
    for light in [
      Vec3::new(1.0, 0.5, 0.25),
      Vec3::new(0.003, 0.0, 0.0001),
      Vec3::new(37.0, 12.5, 0.9),
      Vec3::new(0.999_99, 0.999_99, 0.999_99),
      Vec3::new(60_000.0, 1.0, 0.0),
    ] {
      let back = decoded(encoded(light));
      let step = light.max_element() / 256.0;
      assert!(
        (back - light).abs().max_element() <= step,
        "{light} came back {back}"
      );
    }
    assert_eq!(decoded(encoded(Vec3::ZERO)), Vec3::ZERO);
    assert_eq!(decoded(encoded(Vec3::splat(1e9))), Vec3::splat(MAX));
  }
}
