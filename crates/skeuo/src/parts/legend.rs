//! lettering in a plain drawn face of capitals and figures, painted flat on a panel

use std::f32::consts::PI;

use controls::stroke;
pub use controls::stroke::{gap as legend_gap, lettered, width as legend_width};
use glam::{Vec2, Vec3};

use super::PAINT;
use crate::mesh::{Mesh, Vertex};

/// the sides the half round at each end of a stroke is cut into
const END: usize = 4;

/// lettering painted flat on the panel in a plain drawn face of capitals and figures,
/// centered on the origin: letters `height` tall with strokes `weight` wide. anything it
/// has no letter for is a space
pub fn legend(text: &str, height: f32, weight: f32) -> Mesh {
  let mut legend = Mesh::default();
  for line in stroke::lines(text, height) {
    let [from, to] = line.map(Vec2::from);
    // a stroke with a half round past each end, so that strokes meet at any angle
    // without a notch and one of no length is a dot
    let along = (to - from).try_normalize().unwrap_or(Vec2::X) * weight / 2.0;
    let across = along.perp();
    legend.quad(
      [from - across, from + across, to + across, to - across].map(|at| at.extend(PAINT)),
      Vec3::Z,
    );
    for (end, out) in [(from, -along), (to, along)] {
      let middle = legend.vertices.len() as u32;
      let rim = (0..=END).map(|step| {
        let turn = (step as f32 / END as f32 - 0.5) * PI;
        end + Vec2::from_angle(turn).rotate(out)
      });
      for at in std::iter::once(end).chain(rim) {
        legend.vertices.push(Vertex {
          position: at.extend(PAINT).into(),
          normal: Vec3::Z.into(),
        });
      }
      for side in 1..=END as u32 {
        (legend.indices).extend([middle, middle + side, middle + side + 1]);
      }
    }
  }
  legend
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_legend_is_centered_and_as_tall_as_it_is_told() {
    let legend = legend("CUTOFF 12", 12.0, 1.5);
    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    for vertex in &legend.vertices {
      let position = Vec3::from(vertex.position);
      (min, max) = (min.min(position), max.max(position));
    }
    // nine letters of four and eight gaps of two between them, at two to a unit, and
    // then half a stroke more, or a little over that at the end of a slanted one
    let half = (9.0 * 6.0 - 2.0) * 2.0 / 2.0 + 0.75;
    assert!(
      (max.x - half).abs() < 0.35 && (min.x + half).abs() < 0.35,
      "{min} to {max}"
    );
    assert!(
      (max.y - 6.75).abs() < 0.35 && (min.y + 6.75).abs() < 0.35,
      "{min} to {max}"
    );
    // what it has no letter for is a space, and takes a letter's room
    assert!(
      super::legend(" \u{e9}\u{4e16}\u{1f39b}", 12.0, 1.5)
        .vertices
        .is_empty()
    );
    let spaced = super::legend("\u{e9}I", 12.0, 1.5);
    let leftmost = (spaced.vertices.iter())
      .map(|vertex| vertex.position[0])
      .fold(f32::MAX, f32::min);
    assert!(leftmost > 0.0, "{leftmost}");
  }

  #[test]
  fn the_letters_of_a_legend_are_six_units_apart() {
    // three letters at two to a unit: each 8 wide with 4 between, 32 in all
    let legend = legend("III", 12.0, 1.0);
    // a stroke's corners are its length's four, and the middle and rim of each end
    let letter = 3 * (4 + 2 * (END + 2));
    let middles: Vec<f32> = (legend.vertices.chunks(letter))
      .map(|letter| {
        letter.iter().map(|vertex| vertex.position[0]).sum::<f32>() / letter.len() as f32
      })
      .collect();
    for (middle, wanted) in middles.iter().zip([-12.0, 0.0, 12.0]) {
      assert!((middle - wanted).abs() < 1e-4, "{middles:?}");
    }
    assert_eq!(middles.len(), 3);
  }
}
