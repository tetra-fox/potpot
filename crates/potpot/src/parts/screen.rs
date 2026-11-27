//! Screens, and flat shapes shown lit on them.

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use potpot_controls::flat::{Flat, Shape};

use super::legend::{END, add_stroke};
use super::outline::{Outline, STRIP, band};
use crate::material::Material;
use crate::mesh::Mesh;
use crate::scene::{Lamp, Object};

/// The rise from one color of a picture to the next.
const LAYER: f32 = 0.01;
/// The most lamps the light of one screen is a row of.
const ROW: u32 = 4;
/// The most sides the round end of a wide line has.
const ROUND: usize = 24;

/// A screen, a face set in a bezel.
///
/// The face lies on the panel around the origin and the bezel stands around it. [`picture`]
/// makes what the face shows, and [`Material::screen`] is a material for the face.
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
  /// shows each of them as a disc, which one of [`Material::screen`] does not.
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

/// Returns flat shapes as lit objects on the face of a screen.
///
/// The shapes of `flat` are in the panel's points, as they are when they are drawn flat, and
/// (`x`, `y`) is the middle of the [`Screen`] they show on. `face` is the material of its face.
/// Each [`Object`] lights itself in the color of its shapes and is finished as the face is, so
/// the highlights of the face's glass lie over the picture too. Each is
/// [shadowless](Material::shadowless), so a picture made again costs the drawing of where it is
/// seen and, while light bounces, of what its light is thrown on.
///
/// Each run of shapes of one color is one object, a hundredth of a point over the one before
/// it, so every change of color along `flat` costs an object and that much height. `under` is
/// the count of objects already on the face, which this picture lies over: none for the first
/// picture, and for the next the count of all the objects before it. A picture that changes
/// often is best kept apart from one that does not, since only its own objects are made again.
///
/// The scene has no clear colors. A color that covers in part is mixed with the color of the
/// face by its cover, whatever shapes lie under it, and is that much less lit. The sides of a
/// rectangle lie where they are given, and the edges of every shape are as smooth as the
/// scene's [`Look`](crate::Look) has them. A shape that reaches past the face is drawn there
/// too.
pub fn picture(flat: &Flat, x: f32, y: f32, face: Material, under: usize) -> Vec<Object> {
  let middle = Vec2::new(x, y);
  // placed at the middle of the screen, as the face is
  let transform = Mat4::from_translation(middle.extend(0.0));
  let runs = flat
    .shapes()
    .chunk_by(|one, other| one.color() == other.color());
  (runs.zip(under + 1..))
    .map(|(shapes, layer)| {
      let height = STRIP + LAYER * layer as f32;
      let mut mesh = Mesh::default();
      let quad = |mesh: &mut Mesh, corners: [Vec2; 4]| {
        mesh.quad(corners.map(|at| (at - middle).extend(height)), Vec3::Z);
      };
      for shape in shapes {
        match *shape {
          Shape::Rect { min, max, .. } => {
            let (min, max) = (Vec2::from(min), Vec2::from(max));
            let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
            quad(&mut mesh, corners);
          }
          // lines of one color lie in one plane, where the cuts between them change nothing
          Shape::Line {
            from, to, weight, ..
          } => {
            let ends = [from, to].map(|end| Vec2::from(end) - middle);
            // two sides for each point of width, so that a wide dot still looks round
            let sides = ((weight * 2.0) as usize).clamp(END, ROUND);
            add_stroke(&mut mesh, ends, weight, height, sides);
          }
          Shape::Span {
            sides, edge, level, ..
          } => {
            let [from, to] = [0, 1].map(|side| Vec2::new(sides[side], edge[side]));
            let on_level = |at: Vec2| Vec2::new(at.x, level);
            let (over, under) = (from.y - level, to.y - level);
            if over * under < 0.0 {
              // the edge crosses the level line, and the stretch is a triangle to each side
              let crossing = from.lerp(to, over / (over - under));
              quad(&mut mesh, [from, crossing, crossing, on_level(from)]);
              quad(&mut mesh, [crossing, to, on_level(to), crossing]);
            } else {
              quad(&mut mesh, [from, to, on_level(to), on_level(from)]);
            }
          }
        }
      }
      let [red, green, blue, cover] = shapes[0].color();
      Object {
        mesh: Arc::new(mesh),
        transform,
        material: Material {
          color: face.color.lerp(Vec3::new(red, green, blue), cover),
          emission: face.emission + (1.0 - face.emission) * cover,
          emission_radius: 0.0,
          shadowless: true,
          ..face
        },
      }
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_picture_stacks_its_colors_in_order_over_the_face() {
    let (cyan, amber) = ([0.2, 0.9, 1.0, 1.0], [1.0, 0.6, 0.1, 0.5]);
    let mut flat = Flat::default();
    flat.area(&[[100.0, 60.0], [140.0, 40.0]], 80.0, amber);
    flat.curve(&[[100.0, 60.0], [140.0, 40.0], [180.0, 70.0]], 2.0, cyan);
    flat.rect([90.0, 30.0], [92.0, 90.0], cyan);
    flat.line([90.0, 50.0], [190.0, 50.0], 1.0, amber);
    let face = Material::screen(Vec3::splat(0.02));
    let mut objects = picture(&flat, 140.0, 60.0, face, 0);
    // the region, then the curve's two lines and the rectangle as one, then the line
    assert_eq!(objects.len(), 3);
    // and a second picture over the first, whose edge crosses its level line
    let mut crossed = Flat::default();
    crossed.area(&[[100.0, 60.0], [140.0, 100.0]], 80.0, cyan);
    objects.extend(picture(&crossed, 140.0, 60.0, face, objects.len()));
    let crossing = [-20.0, 20.0];
    let triangles = &objects[3].mesh.vertices;
    assert_eq!(triangles.len(), 8);
    assert!(
      triangles
        .iter()
        .any(|vertex| vertex.position[..2] == crossing)
    );
    let heights: Vec<f32> = (objects.iter())
      .map(|object| {
        let first = object.mesh.vertices[0].position[2];
        assert!(
          (object.mesh.vertices.iter()).all(|vertex| vertex.position[2] == first),
          "an object lies at one height"
        );
        first
      })
      .collect();
    assert!(heights[0] > STRIP && heights.windows(2).all(|pair| pair[1] > pair[0]));
    // a shape is as far from the screen's middle as it was drawn from it
    let region = &objects[0].mesh.vertices;
    assert_eq!(region[0].position[..2], [-40.0, 0.0]);
    let middle = objects[0].transform.w_axis.truncate();
    assert_eq!(middle, Vec3::new(140.0, 60.0, 0.0));
  }
}
