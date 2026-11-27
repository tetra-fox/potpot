//! Flat shapes to draw straight onto a panel.
//!
//! Rectangles, lines, curves, filled regions and lettering in plain colors go over whatever
//! picture the panel has. The module holds only the list of them, which anything may draw.

use crate::interaction::Edit;
use crate::{Point, stroke};

/// A color as it is to look, with its cover.
///
/// The cover is how much of what is under the color it covers.
pub type Color = [f32; 4];

/// One shape of a [`Flat`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
  /// A rectangle that lies along the pixels.
  ///
  /// Its sides go on the nearest pixel edges.
  Rect {
    /// Its top left corner.
    min: Point,
    /// Its bottom right corner.
    max: Point,
    /// The color it is filled with.
    color: Color,
  },
  /// A line with smoothed edges and round ends.
  Line {
    /// The point it runs from.
    from: Point,
    /// The point it runs to.
    to: Point,
    /// Its width.
    ///
    /// Its round ends reach half of it past its points.
    weight: f32,
    /// The color it is drawn in.
    color: Color,
    /// The way a curve runs through each of its two ends.
    ///
    /// The first is at `from` and the second at `to`, each one long. A line of a curve has it
    /// at an end where the next line of the curve takes over. The line is drawn up to the
    /// straight cut through that end which this crosses squarely, and the other line from
    /// there on. A point on the cut belongs to the line that runs to it. At a round end it is
    /// zero.
    joins: [Point; 2],
  },
  /// A stretch of a filled region with upright sides.
  ///
  /// It lies between a sloping edge and a level line, which the edge may cross. Both are
  /// smoothed, and its two sides are sharp.
  Span {
    /// How far across its left side and its right side are.
    sides: [f32; 2],
    /// How far down its sloping edge is at each side.
    edge: [f32; 2],
    /// How far down its level line is.
    level: f32,
    /// The color it is filled with.
    color: Color,
  },
}

impl Shape {
  /// Returns the color the shape is drawn in.
  pub fn color(&self) -> Color {
    match *self {
      Self::Rect { color, .. } | Self::Line { color, .. } | Self::Span { color, .. } => color,
    }
  }
}

/// The dashes and gaps a dashed curve is drawn in.
///
/// Both are measured along the curve as it is seen: a dash with its round ends is `length`
/// long. A dash shorter than the curve's width is a dot as wide as the curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dashes {
  /// The length of each dash, in points.
  pub length: f32,
  /// The length of the gap between two dashes, in points.
  pub gap: f32,
}

/// A list of shapes to draw.
///
/// The shapes are drawn in the order they were added, the later over the earlier. Their
/// places are in the panel's points, from its top left.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flat {
  shapes: Vec<Shape>,
}

/// One line of a curve, with the way it runs and its length.
#[derive(Clone, Copy)]
struct Run {
  from: Point,
  to: Point,
  way: Point,
  length: f32,
}

/// Returns the lines of a curve through `points`.
fn runs(points: &[Point]) -> Vec<Run> {
  (points.windows(2))
    .filter_map(|pair| {
      let (from, to) = (pair[0], pair[1]);
      let reach = [to[0] - from[0], to[1] - from[1]];
      let length = reach[0].hypot(reach[1]);
      // two points on one place have no way between them, and add nothing to the curve
      (length > 0.0).then(|| Run {
        from,
        to,
        way: [reach[0] / length, reach[1] / length],
        length,
      })
    })
    .collect()
}

/// Returns the way a curve runs through a joint of two lines.
///
/// It is one long, or zero where the two lines keep their round ends and lie over each other.
fn joint(before: Run, after: Run, weight: f32) -> Point {
  let (one, other) = (before.way, after.way);
  let facing = one[0] * other[0] + one[1] * other[1];
  let turned = (one[0] * other[1] - one[1] * other[0]).abs();
  // the tangent of half the turn, which has no end where the curve doubles straight back
  let half_turn = turned / (1.0 + facing);
  // the cut gives every point past it to the other line, which is the nearer one only where
  // it has not ended. lines this long for the turn have not, anywhere the curve's width reaches
  if before.length.min(after.length) >= 2.0 * weight * half_turn {
    let sum = [one[0] + other[0], one[1] + other[1]];
    let each = 1.0 / sum[0].hypot(sum[1]);
    [sum[0] * each, sum[1] * each]
  } else {
    [0.0, 0.0]
  }
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
    self.shapes.push(Shape::Rect { min, max, color });
  }

  /// Adds a line `weight` wide from one point to another.
  ///
  /// Its round ends reach half its width past the points. Its edges are smoothed, whichever
  /// way it runs.
  pub fn line(&mut self, from: Point, to: Point, weight: f32, color: Color) {
    self.shapes.push(Shape::Line {
      from,
      to,
      weight,
      color,
      joins: [[0.0; 2]; 2],
    });
  }

  /// Adds a curve `weight` wide through `points`.
  ///
  /// Its turns and its ends are round. The curve is drawn once all along, so a color that covers in
  /// part stays even through its turns. The exception is a turn too tight for the lines that meet
  /// in it, where the two lie over each other as separate [`Flat::line`]s do. A right-angle turn is
  /// that tight between lines shorter than twice the curve's width, and a gentler turn between
  /// shorter lines still. One point alone is a dot.
  pub fn curve(&mut self, points: &[Point], weight: f32, color: Color) {
    let runs = runs(points);
    if let (None, Some(&dot)) = (runs.first(), points.first()) {
      self.line(dot, dot, weight, color);
    }
    let mut before = [0.0; 2];
    for (index, run) in runs.iter().enumerate() {
      let after = (runs.get(index + 1)).map_or([0.0; 2], |next| joint(*run, *next, weight));
      self.shapes.push(Shape::Line {
        from: run.from,
        to: run.to,
        weight,
        color,
        joins: [before, after],
      });
      before = after;
    }
  }

  /// Adds a dashed curve `weight` wide through `points`.
  ///
  /// The dashes follow the curve's length through its turns, so they are as long on a bend as
  /// on a straight. The first dash starts where the curve's round end starts, half its width
  /// before the first point. A dash that runs past the last point is cut short there and keeps
  /// its round end. Each dash is drawn as a [`Flat::curve`] is.
  ///
  /// # Panics
  ///
  /// Panics if the length of a dash is below zero or the gap is not above zero.
  pub fn dashed(&mut self, points: &[Point], weight: f32, dashes: Dashes, color: Color) {
    let Dashes { length, gap } = dashes;
    assert!(
      length >= 0.0 && gap > 0.0,
      "a dash is at least zero long and a gap longer than zero"
    );
    let runs = runs(points);
    // how far along the curve each of its lines ends
    let ends: Vec<f32> = (runs.iter())
      .scan(0.0, |along, run| {
        *along += run.length;
        Some(*along)
      })
      .collect();
    let Some(&whole) = ends.last() else {
      // a curve of no length is a dot or nothing, which has no room for a gap
      self.curve(points, weight, color);
      return;
    };
    // the point a distance along the curve is at, and the line it is on
    let place = |along: f32| {
      let line = (ends.partition_point(|&end| end < along)).min(runs.len() - 1);
      let run = runs[line];
      let into = along - (ends[line] - run.length);
      let at = [
        run.from[0] + run.way[0] * into,
        run.from[1] + run.way[1] * into,
      ];
      (at, line)
    };
    // a dash's round ends reach half the weight past the stretch between its points, and a
    // dash shorter than the weight is a dot in the middle of its length
    let between = (length - weight).max(0.0);
    let lead = (length - weight - between) / 2.0;
    let period = length + gap;
    let last = ((whole - lead) / period) as u32;
    let mut dash = Vec::new();
    for count in 0..=last {
      let start = count as f32 * period + lead;
      let [start, end] = [start, start + between].map(|along| along.clamp(0.0, whole));
      let ((first, first_line), (end, end_line)) = (place(start), place(end));
      dash.clear();
      dash.push(first);
      dash.extend(runs[first_line..end_line].iter().map(|run| run.to));
      dash.push(end);
      self.curve(&dash, weight, color);
    }
  }

  /// Adds the region between a curve and a level line.
  ///
  /// The curve runs through `points`. `level` is how far down the panel the line is, and the
  /// curve may run to either side of it. The region's edge along the curve is smoothed and its
  /// two ends are sharp. The points run across the panel one way: where they turn back the
  /// region is drawn twice.
  ///
  /// A pixel is drawn from the one stretch between two points that its middle is over, so
  /// points closer together than a pixel give a stepped edge. A curve of more points than the
  /// pixels it spans is thinned before it is given.
  pub fn area(&mut self, points: &[Point], level: f32, color: Color) {
    for pair in points.windows(2) {
      // a stretch has its left side first, whichever way the points run
      let (left, right) = if pair[0][0] <= pair[1][0] {
        (pair[0], pair[1])
      } else {
        (pair[1], pair[0])
      };
      self.shapes.push(Shape::Span {
        sides: [left[0], right[0]],
        edge: [left[1], right[1]],
        level,
        color,
      });
    }
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

#[cfg(test)]
mod tests {
  use super::*;

  /// Returns the joins of each line of a curve.
  fn joins(points: &[Point], weight: f32) -> Vec<[Point; 2]> {
    let mut flat = Flat::default();
    flat.curve(points, weight, [1.0; 4]);
    (flat.shapes().iter())
      .map(|shape| match shape {
        Shape::Line { joins, .. } => *joins,
        other => panic!("{other:?} is a line"),
      })
      .collect()
  }

  #[test]
  fn lines_long_enough_for_their_turn_are_cut_apart_along_its_middle() {
    let corner = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
    // a right angle between lines of 10 is cut apart at a width of 5 and no wider
    let cut = joins(&corner, 5.0);
    assert_eq!(cut[0][0], [0.0, 0.0]);
    assert!((cut[0][1][0] - cut[0][1][1]).abs() < 1e-6 && cut[0][1][0] > 0.7);
    // both lines hold the very same numbers for the cut, which a drawer parts them by
    assert_eq!(cut[0][1], cut[1][0]);
    assert_eq!(cut[1][1], [0.0, 0.0]);
    assert_eq!(joins(&corner, 5.1), [[[0.0; 2]; 2]; 2]);
    // a gentle turn is cut between much shorter lines
    let gentle = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.1]];
    assert_ne!(joins(&gentle, 5.0)[0][1], [0.0, 0.0]);
  }

  /// Returns the ends and joins of each line a dashed curve is drawn as.
  fn dashes(points: &[Point], weight: f32, length: f32, gap: f32) -> Vec<[Point; 4]> {
    let mut flat = Flat::default();
    flat.dashed(points, weight, Dashes { length, gap }, [1.0; 4]);
    (flat.shapes().iter())
      .map(|shape| match *shape {
        Shape::Line {
          from, to, joins, ..
        } => [from, to, joins[0], joins[1]],
        other => panic!("{other:?} is a line"),
      })
      .collect()
  }

  #[test]
  fn dashes_run_on_through_a_turn_and_stop_where_the_curve_does() {
    // dashes of 5 with their round ends are 4 between their points, and start every 8
    let turn = [[0.0, 0.0], [10.0, 0.0], [10.0, 13.0]];
    let lines = dashes(&turn, 1.0, 5.0, 3.0);
    let ends: Vec<[Point; 2]> = lines.iter().map(|line| [line[0], line[1]]).collect();
    assert_eq!(
      ends,
      [
        [[0.0, 0.0], [4.0, 0.0]],
        // the second dash goes round the turn as one curve, cut apart where its lines meet
        [[8.0, 0.0], [10.0, 0.0]],
        [[10.0, 0.0], [10.0, 2.0]],
        // and the curve ends 3 on, in the gap after it
        [[10.0, 6.0], [10.0, 10.0]],
      ]
    );
    assert_ne!(lines[1][3], [0.0, 0.0]);
    assert_eq!(lines[1][3], lines[2][2]);
    // a dash that the curve's end cuts short ends on its last point
    let short = dashes(&[[0.0, 0.0], [10.0, 0.0]], 1.0, 5.0, 3.0);
    assert_eq!(short[1][..2], [[8.0, 0.0], [10.0, 0.0]]);
  }

  #[test]
  fn dashes_on_a_bend_are_as_long_as_on_a_straight() {
    // a quarter of a circle 40 across, in many short lines that no dash starts or ends on
    let bend: Vec<Point> = (0..=97)
      .map(|step| {
        let angle = step as f32 / 97.0 * std::f32::consts::FRAC_PI_2;
        [40.0 * angle.cos(), 40.0 * angle.sin()]
      })
      .collect();
    let mut flat = Flat::default();
    flat.dashed(
      &bend,
      1.0,
      Dashes {
        length: 6.0,
        gap: 4.0,
      },
      [1.0; 4],
    );
    // each dash is a curve whose first line has a round start
    let mut lengths = Vec::new();
    for shape in flat.shapes() {
      let Shape::Line {
        from, to, joins, ..
      } = *shape
      else {
        panic!("{shape:?} is a line");
      };
      if joins[0] == [0.0, 0.0] {
        lengths.push(0.0);
      }
      *lengths.last_mut().expect("a dash has begun") += (to[0] - from[0]).hypot(to[1] - from[1]);
    }
    // the bend is 62.8 long, so six dashes and a seventh cut short
    assert_eq!(lengths.len(), 7, "{lengths:?}");
    for length in &lengths[..6] {
      assert!((length - 5.0).abs() < 1e-3, "{lengths:?}");
    }
    assert!(lengths[6] < 5.0);
  }

  #[test]
  fn a_dash_shorter_than_the_width_is_a_dot_in_the_middle_of_its_length() {
    // dashes of 1 at a width of 2 every 5, each a dot half a point before where it starts,
    // and the first moved onto the curve's first point
    let lines = dashes(&[[0.0, 0.0], [20.0, 0.0]], 2.0, 1.0, 4.0);
    let dots: Vec<Point> = (lines.iter())
      .map(|line| {
        assert_eq!(line[0], line[1], "a dot has no length");
        line[0]
      })
      .collect();
    assert_eq!(
      dots,
      [[0.0, 0.0], [4.5, 0.0], [9.5, 0.0], [14.5, 0.0], [19.5, 0.0]]
    );
  }

  #[test]
  fn a_curve_leaves_out_what_has_no_way_to_run() {
    // a curve that doubles straight back has no cut that parts its two lines
    let back = [[0.0, 0.0], [50.0, 0.0], [0.0, 0.0]];
    assert_eq!(joins(&back, 1.0), [[[0.0; 2]; 2]; 2]);
    // a point given twice adds no line, and the lines to each side of it still meet in it
    let twice = [[0.0, 0.0], [10.0, 0.0], [10.0, 0.0], [20.0, 0.0]];
    assert_eq!(
      joins(&twice, 1.0),
      joins(&[twice[0], twice[1], twice[3]], 1.0)
    );
    // one place alone is a dot
    let mut dot = Flat::default();
    dot.curve(&[[3.0, 4.0], [3.0, 4.0]], 2.0, [1.0; 4]);
    let mut line = Flat::default();
    line.line([3.0, 4.0], [3.0, 4.0], 2.0, [1.0; 4]);
    assert_eq!(dot, line);
  }
}
