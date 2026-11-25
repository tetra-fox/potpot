//! Flat shapes to draw straight onto a panel.
//!
//! Rectangles, lines and lettering in plain colors go over whatever picture the panel has.
//! The module holds only the list of them, which anything may draw.

use crate::interaction::Edit;
use crate::{Point, stroke};

/// A color as it is to look, with its cover.
///
/// The cover is how much of what is under the color it covers.
pub type Color = [f32; 4];

/// A rectangle about a middle, lying any way round.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Shape {
  /// The place of its middle.
  pub middle: Point,
  /// The way its length runs, one long.
  pub along: Point,
  /// The reach from its middle along its length and across it.
  ///
  /// For a line the first is the reach to its two points, which its round ends are past.
  pub half: Point,
  /// The color it is filled with.
  pub color: Color,
  /// Whether its sides go on the nearest pixel edges.
  ///
  /// A shape with it set is a rectangle. One without is a line with round ends and smoothed
  /// edges.
  pub snap: bool,
}

/// A list of shapes to draw.
///
/// The shapes are drawn in the order they were added, the later over the earlier. Their
/// places are in the panel's points, from its top left.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flat {
  shapes: Vec<Shape>,
}

impl Flat {
  /// Returns the shapes, first drawn first.
  pub fn shapes(&self) -> &[Shape] {
    &self.shapes
  }

  /// Adds a filled rectangle between two corners.
  ///
  /// Its sides are drawn on pixel edges, so they are sharp whatever the scale.
  pub fn rect(&mut self, min: Point, max: Point, color: Color) {
    self.shapes.push(Shape {
      middle: [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0],
      along: [1.0, 0.0],
      half: [(max[0] - min[0]) / 2.0, (max[1] - min[1]) / 2.0],
      color,
      snap: true,
    });
  }

  /// Adds a line `weight` wide from one point to another.
  ///
  /// Its round ends reach half its width past the points. Its edges are smoothed, whichever
  /// way it runs.
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

  /// Adds lettering with the top left of its first letter at `from`.
  ///
  /// The lettering is in the face of [`stroke`], with letters `height` tall and strokes
  /// `weight` wide.
  pub fn legend(&mut self, text: &str, from: Point, height: f32, weight: f32, color: Color) {
    let corner = stroke::top_left(text.chars().count(), height);
    let middle = [from[0] - corner[0], from[1] - corner[1]];
    self.centered(text, middle, height, weight, color);
  }

  /// Adds lettering with the middle of it all at `middle`.
  ///
  /// The lettering is the same as that of [`Flat::legend`].
  pub fn centered(&mut self, text: &str, middle: Point, height: f32, weight: f32, color: Color) {
    for [start, end] in stroke::lines(text, height) {
      self.line(
        [middle[0] + start[0], middle[1] + start[1]],
        [middle[0] + end[0], middle[1] + end[1]],
        weight,
        color,
      );
    }
  }

  /// Adds a value being typed, with its middle at `middle`.
  ///
  /// The text of the [`Edit`] is lettering in `ink`. What is selected lies on a block of
  /// `selected`, two points taller than the letters at either end, and the caret is a line one
  /// point wide.
  pub fn typed(
    &mut self,
    edit: &Edit,
    middle: Point,
    height: f32,
    weight: f32,
    ink: Color,
    selected: Color,
  ) {
    let count = edit.text.chars().count();
    let gap = |place: usize| middle[0] + stroke::gap(count, place, height);
    let (top, foot) = (
      middle[1] - height / 2.0 - 2.0,
      middle[1] + height / 2.0 + 2.0,
    );
    let marked = edit.selected();
    if !marked.is_empty() {
      self.rect([gap(marked.start), top], [gap(marked.end), foot], selected);
    }
    self.centered(&edit.text, middle, height, weight, ink);
    self.line([gap(edit.caret), top], [gap(edit.caret), foot], 1.0, ink);
  }
}
