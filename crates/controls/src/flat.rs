//! flat shapes to draw straight onto a panel, over whatever picture it has: rectangles, lines
//! and lettering in plain colors. this is only the list of them, which anything may draw

use crate::{Point, stroke};

/// a color as it is to look, with how much of what is under it it covers
pub type Color = [f32; 4];

/// a rectangle about a middle, lying any way round
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
  /// where its middle is
  pub middle: Point,
  /// the way its length runs, one long
  pub along: Point,
  /// how far it reaches from its middle along its length and across it. for a line the
  /// first is to its two points, which its round ends are past
  pub half: Point,
  /// what it is filled with
  pub color: Color,
  /// whether it is a rectangle whose sides go on the nearest pixel edges, and not a line
  /// with round ends and smoothed edges
  pub snap: bool,
}

/// shapes to draw, in the order they were added, the later over the earlier. their places
/// are in the panel's points, from its top left
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flat {
  shapes: Vec<Shape>,
}

impl Flat {
  /// the shapes, first drawn first
  pub fn shapes(&self) -> &[Shape] {
    &self.shapes
  }

  /// a filled rectangle between two corners. its sides are drawn on pixel edges, so they
  /// are sharp whatever the scale
  pub fn rect(&mut self, min: Point, max: Point, color: Color) {
    self.shapes.push(Shape {
      middle: [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0],
      along: [1.0, 0.0],
      half: [(max[0] - min[0]) / 2.0, (max[1] - min[1]) / 2.0],
      color,
      snap: true,
    });
  }

  /// a line `weight` wide from one point to another, with round ends half its width past
  /// them. its edges are smoothed, whichever way it runs
  pub fn line(&mut self, from: Point, to: Point, weight: f32, color: Color) {
    let reach = [to[0] - from[0], to[1] - from[1]];
    let length = (reach[0] * reach[0] + reach[1] * reach[1]).sqrt();
    let each = 1.0 / length;
    // a line of no length is a dot, and lies across
    let along = if each.is_finite() && each > 0.0 {
      [reach[0] * each, reach[1] * each]
    } else {
      [1.0, 0.0]
    };
    self.shapes.push(Shape {
      middle: [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0],
      along,
      half: [length / 2.0, weight / 2.0],
      color,
      snap: false,
    });
  }

  /// lettering in the face of [`crate::stroke`], with the top left of its first letter at
  /// `from`: letters `height` tall with strokes `weight` wide
  pub fn legend(&mut self, text: &str, from: Point, height: f32, weight: f32, color: Color) {
    let corner = stroke::top_left(text.chars().count(), height);
    let offset = [from[0] - corner[0], from[1] - corner[1]];
    for [start, end] in stroke::lines(text, height) {
      self.line(
        [offset[0] + start[0], offset[1] + start[1]],
        [offset[0] + end[0], offset[1] + end[1]],
        weight,
        color,
      );
    }
  }
}
