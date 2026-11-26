//! Handles to pull a panel out of a rack by.

use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Vec2, Vec3};

use super::panel::chamfered_slab;
use crate::mesh::Mesh;

/// The count of straight stretches in each bend of a handle.
const BEND: usize = 8;

/// A round bar bent into a U, standing off the panel on a block at each end.
///
/// The handle runs along y, centered on the origin, like the one on a rack ear.
#[derive(Clone, Copy, Debug)]
pub struct Handle {
  /// The distance between the middles of its two legs, in points.
  pub span: f32,
  /// The height of the middle of the bar across the top, in points.
  pub reach: f32,
  /// The thickness of the bar, in points.
  pub thickness: f32,
  /// The radius of the two bends, at the middle of the bar, in points.
  ///
  /// It is kept to what the span and the height over the blocks have room for.
  pub bend: f32,
  /// The size of each block along x, y and z, in points.
  pub mount: Vec3,
}

impl Handle {
  /// Creates a handle on two blocky mounts.
  ///
  /// The legs are `span` apart, the bar's middle `reach` over the panel and `thickness` thick.
  pub fn new(span: f32, reach: f32, thickness: f32) -> Self {
    Self {
      span,
      reach,
      thickness,
      bend: reach * 0.45,
      mount: Vec3::new(thickness * 2.2, thickness * 2.8, thickness * 1.3),
    }
  }

  /// Returns the path along the middle of the bar, in the plane of y and z.
  ///
  /// Each leg starts halfway into its block, so that the bar meets it without a gap.
  fn path(&self) -> Vec<Vec2> {
    let foot = self.mount.z / 2.0;
    let bend = (self.bend)
      .min(self.span / 2.0)
      .min(self.reach - self.mount.z)
      .max(0.0);
    let half = self.span / 2.0;
    let mut path = vec![Vec2::new(-half, foot)];
    // the middles of the two bends, and the way each turns from upright to across
    let corners = [
      (Vec2::new(-half + bend, self.reach - bend), -1.0),
      (Vec2::new(half - bend, self.reach - bend), 1.0),
    ];
    for (middle, side) in corners {
      let quarter = (0..=BEND).map(|step| {
        let angle = step as f32 / BEND as f32 * FRAC_PI_2;
        // from the leg over to the top on the left, and from the top down to the leg on
        // the right
        let angle = if side < 0.0 { angle } else { FRAC_PI_2 - angle };
        middle + Vec2::new(side * angle.cos(), angle.sin()) * bend
      });
      path.extend(quarter);
    }
    path.push(Vec2::new(half, foot));
    path
  }

  /// Returns the bar.
  pub fn bar(&self) -> Mesh {
    let path: Vec<Vec3> = (self.path().into_iter())
      .map(|point| Vec3::new(0.0, point.x, point.y))
      .collect();
    Mesh::tube(&path, self.thickness / 2.0, 20)
  }

  /// Returns the two blocks the bar stands on.
  pub fn mounts(&self) -> Mesh {
    let block = chamfered_slab(
      self.mount.x,
      self.mount.y,
      self.mount.z,
      self.mount.z * 0.25,
      self.mount.x * 0.12,
    );
    let at = |side: f32| Mat4::from_translation(Vec3::Y * side * self.span / 2.0);
    (block.clone().transformed(at(-1.0))).merged(&block.transformed(at(1.0)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_handle_stands_in_its_blocks_and_reaches_its_height() {
    let handle = Handle::new(120.0, 24.0, 6.0);
    let (bar, mounts) = (handle.bar(), handle.mounts());
    let top = (bar.vertices.iter())
      .map(|vertex| vertex.position[2])
      .fold(0.0, f32::max);
    assert!((top - 27.0).abs() < 1e-3, "{top}");
    let reach = (bar.vertices.iter())
      .map(|vertex| vertex.position[1].abs())
      .fold(0.0, f32::max);
    assert!((reach - 63.0).abs() < 1e-3, "{reach}");
    // under the top of its blocks, the bar is inside one of them
    let block = handle.mount / 2.0;
    for vertex in &bar.vertices {
      let position = Vec3::from(vertex.position);
      if position.z < handle.mount.z {
        let leg = position.y.signum() * 60.0;
        assert!(
          position.x.abs() < block.x && (position.y - leg).abs() < block.y,
          "{position}"
        );
        assert!(position.z > 0.0, "{position}");
      }
    }
    // and each block stands on the panel under its leg
    for vertex in &mounts.vertices {
      let position = Vec3::from(vertex.position);
      let leg = position.y.signum() * 60.0;
      assert!((position.y - leg).abs() <= block.y + 1e-4, "{position}");
      assert!((0.0..=handle.mount.z + 1e-4).contains(&position.z));
    }
  }

  #[test]
  fn a_handle_bends_no_tighter_than_it_has_room_for() {
    // a bend wider than the height over the blocks leaves the legs a straight start
    let handle = Handle {
      bend: 40.0,
      ..Handle::new(60.0, 20.0, 4.0)
    };
    let path = handle.path();
    let first_bend = path[1];
    assert!((first_bend.y - handle.mount.z).abs() < 1e-4, "{first_bend}");
    assert!(path.iter().all(|point| point.y <= 20.0 + 1e-4));
    assert!(path.windows(2).all(|pair| pair[1].x >= pair[0].x - 1e-4));
  }
}
