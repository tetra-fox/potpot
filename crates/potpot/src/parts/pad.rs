//! XY pads, each a puck on a surface set in a rim.

use glam::{Mat4, Vec2, Vec3};

use super::button::Button;
use super::outline::{Outline, band};
use crate::mesh::Mesh;

/// An XY pad, a puck that slides on a surface set in a rim.
///
/// The surface lies on the panel around the origin and the rim stands over it on every side.
#[derive(Clone, Copy, Debug)]
pub struct Pad {
  /// The surface's width along x, in points.
  pub width: f32,
  /// The surface's length along y, in points.
  pub length: f32,
  /// The radius of the surface's corners, in points.
  pub corner: f32,
  /// The width of the rim around the surface, in points.
  pub rim: f32,
  /// The height of the rim, in points.
  pub rim_height: f32,
  /// The puck, as tall as it stands on the surface.
  pub puck: Button,
}

impl Pad {
  /// Creates a pad with a round puck, hollow on top.
  ///
  /// The puck is a quarter of the surface's shorter side across.
  pub fn new(width: f32, length: f32) -> Self {
    let shorter = width.min(length);
    Self {
      width,
      length,
      corner: shorter * 0.06,
      rim: shorter * 0.05,
      rim_height: shorter * 0.04,
      puck: Button {
        roll: 0.3,
        crown: -0.12,
        ..Button::round(shorter * 0.25, shorter * 0.07)
      },
    }
  }

  fn outline(&self) -> Outline {
    Outline::new(self.width, self.length, self.corner)
  }

  /// Returns the surface the puck slides on, as a low strip.
  pub fn surface(&self) -> Mesh {
    self.outline().strip()
  }

  /// Returns the rim, a raised band around the surface.
  pub fn rim(&self) -> Mesh {
    let surface = self.outline();
    let cut = self.rim_height.min(self.rim) * 0.3;
    band(surface.grown(self.rim), surface, self.rim_height, cut)
  }

  /// Returns the mesh of the puck.
  pub fn puck(&self) -> Mesh {
    self.puck.body()
  }

  /// Returns the size of the room the puck's middle has.
  ///
  /// The puck stays whole on the surface, so the room is the surface less the puck, across and
  /// down. [`Control::pad`](potpot_controls::interaction::Control::pad) takes it as the size
  /// the control is dragged over, and panics over a room with no width or no height, which a
  /// puck as large as the surface leaves.
  pub fn travel(&self) -> Vec2 {
    Vec2::new(self.width - self.puck.width, self.length - self.puck.length)
  }

  /// Returns the transform of the puck of a pad at (`x`, `y`).
  ///
  /// The puck is `across` of the way from the left of its room to the right, and `up` of the
  /// way from the foot of it, down the panel, to the top.
  pub fn transform(&self, x: f32, y: f32, across: f32, up: f32) -> Mat4 {
    let from_middle = self.travel() * Vec2::new(across - 0.5, 0.5 - up);
    Mat4::from_translation(Vec3::new(x + from_middle.x, y + from_middle.y, 0.0))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_puck_goes_right_and_up_the_panel_and_stays_on_the_surface() {
    let pad = Pad::new(120.0, 80.0);
    let at = |across, up| {
      pad
        .transform(300.0, 200.0, across, up)
        .transform_point3(Vec3::ZERO)
        .truncate()
    };
    assert_eq!(at(0.5, 0.5), Vec2::new(300.0, 200.0));
    // at each end of its room its edge is on the surface's edge
    let reach = Vec2::new(pad.puck.width, pad.puck.length) / 2.0;
    assert_eq!(
      at(0.0, 0.0) - reach * Vec2::new(1.0, -1.0),
      Vec2::new(240.0, 240.0)
    );
    assert_eq!(
      at(1.0, 1.0) + reach * Vec2::new(1.0, -1.0),
      Vec2::new(360.0, 160.0)
    );
  }
}
