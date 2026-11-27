//! Screens set in a bezel.

use glam::{Vec2, Vec3};

use super::outline::{Outline, band};
use crate::mesh::Mesh;
use crate::scene::Lamp;

/// The most lamps the light of one screen is a row of.
const ROW: u32 = 4;

/// A screen, a face set in a bezel.
///
/// The face lies on the panel around the origin and the bezel stands around it. The face shows
/// a picture as its [`Object::picture`](crate::Object::picture), and
/// [`Material::display_glass`](crate::Material::display_glass) is a material for it.
#[derive(Clone, Copy, Debug)]
pub struct Screen {
  /// The face's width along x, in points.
  pub width: f32,
  /// The face's length along y, in points.
  pub length: f32,
  /// The radius of the face's corners, in points.
  pub corner: f32,
  /// The width of the bezel around the face, in points.
  pub border: f32,
  /// The height of the bezel, in points.
  pub height: f32,
}

impl Screen {
  /// Creates a screen with a low bezel and small corners.
  ///
  /// `width` and `length` are those of its face.
  pub fn new(width: f32, length: f32) -> Self {
    let shorter = width.min(length);
    Self {
      width,
      length,
      corner: shorter * 0.03,
      border: shorter * 0.05,
      height: shorter * 0.03,
    }
  }

  fn outline(&self) -> Outline {
    Outline::new(self.width, self.length, self.corner)
  }

  /// Returns the face, as a low strip.
  pub fn face(&self) -> Mesh {
    self.outline().strip()
  }

  /// Returns the bezel, a raised band around the face.
  pub fn bezel(&self) -> Mesh {
    let face = self.outline();
    let cut = self.height.min(self.border) * 0.3;
    band(face.grown(self.border), face, self.height, [cut; 2])
  }

  /// Returns the light a lit screen throws on what is around it.
  ///
  /// The screen's middle is at (`x`, `y`), and it is lit in `color` at `level` from zero to
  /// one. Its light is a short row of [`Lamp`]s along the face's longer side, one for each part
  /// of the face about as long as it is wide. A face that mirrors and does not light itself
  /// shows each of them as a disc, which one of
  /// [`Material::display_glass`](crate::Material::display_glass) does not.
  pub fn lamps(&self, x: f32, y: f32, color: Vec3, level: f32) -> impl Iterator<Item = Lamp> {
    let (longer, shorter) = (self.width.max(self.length), self.width.min(self.length));
    let along = if self.width >= self.length {
      Vec2::X
    } else {
      Vec2::Y
    };
    let count = ((longer / shorter).round() as u32).clamp(1, ROW);
    let (middle, height) = (Vec2::new(x, y), self.height);
    (0..count).map(move |lamp| {
      let from_middle = ((lamp as f32 + 0.5) / count as f32 - 0.5) * longer;
      Lamp {
        at: (middle + along * from_middle).extend(height * 2.0),
        color: color * level.clamp(0.0, 1.0),
        reach: shorter * 1.5,
        radius: shorter / 2.0,
      }
    })
  }
}
