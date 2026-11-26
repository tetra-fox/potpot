//! The part of a picture that a change of its scene shows in.
//!
//! A point of the picture takes its color from what is drawn there and from what the shaders
//! look up around it: the shadow map along the way to the light, the height map and the bounce
//! map near it, and the lamps within reach. [`Reach`] bounds how far each of those carries a
//! change, by the same numbers `scene.wgsl` looks them up with.
//!
//! The shaders light the place they are handed for a pixel. For a pixel that a triangle covers
//! in part without covering its middle, that place is on the triangle's plane along the line of
//! sight through the middle, at any height at all for a triangle seen nearly edge on. So a
//! change is taken to show wherever a line of sight passes through what the change reaches,
//! whether anything is drawn there or not.
//!
//! The bounce map is read at a place of its own, which `scene.wgsl` takes where the triangle
//! covers the pixel. That place is on the triangle, between the scene's heights, and is seen
//! inside the pixel.

use glam::{Mat4, Vec2, Vec3};

use super::uniforms::{Globals, Placed};
use super::{BOUNCE_SIZE, HEIGHT_SIZE, SHADOW_SIZE};
use crate::scene::LAMPS;

/// The places to each side that a soft shadow reads.
///
/// `scene.wgsl` is given this and the five constants after it.
pub(super) const SHADOW_SPREAD: i32 = 2;
/// The most a point's depth is moved toward the light.
///
/// The depth is then compared with the shadow map's. It runs from zero to one over all that the
/// shadow map holds.
pub(super) const SHADOW_BIAS: f32 = 0.012;
/// The reach in points of each ring read for what is overhead.
pub(super) const OPEN_REACHES: [f32; 3] = {
  let mut reaches = [0.0; 3];
  let mut ring = 0;
  while ring < reaches.len() {
    let out = ring as f32;
    reaches[ring] = 3.0 + out * out * 5.5 + out * 2.0;
    ring += 1;
  }
  reaches
};
/// The height in points that counts as over a point.
///
/// Something near a point is over it from this far above it.
pub(super) const OPEN_CLEAR: f32 = 0.6;
/// The reach of each ring read for bounced light.
///
/// The reaches are in the longer sides of a bounce map texel.
pub(super) const BOUNCE_REACHES: [f32; 3] = {
  let mut reaches = [0.0; 3];
  let mut ring = 0;
  while ring < reaches.len() {
    reaches[ring] = 0.6 + 0.8 * ring as f32;
    ring += 1;
  }
  reaches
};

/// The side of a glow map cell in the picture's pixels.
///
/// `present.wgsl` is given this.
pub(super) const HAZE_CELL: u32 = 4;

/// The Gaussian the glow map is blurred by, along each axis.
///
/// The blur is in the map's cells. A cell far enough off to be past the radius is not read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Spread {
  /// The cells read to each side.
  pub(super) taps: [u32; 2],
  /// The weight of a cell `i` cells off is `exp(-falloff * i * i)`.
  pub(super) falloff: [f32; 2],
}

impl Spread {
  /// Creates the blur that fades to nothing `radius` picture pixels off.
  ///
  /// The radius is three standard deviations, where the weight is about a hundredth. `cells` is
  /// the glow map's width and height, which no blur reads further than.
  pub(super) fn new(radius: Vec2, cells: [u32; 2]) -> Self {
    // a radius that is no number reads no cell to either side
    let radius = (radius / HAZE_CELL as f32).max(Vec2::ZERO);
    let taps = [0, 1].map(|axis| (radius[axis].ceil() as u32).min(cells[axis]));
    // a radius of nothing gives a falloff past every number, which leaves only the middle cell
    let falloff = radius.to_array().map(|radius| 4.5 / (radius * radius));
    Self { taps, falloff }
  }

  /// Returns the reach of the haze in the picture's pixels along each axis.
  ///
  /// A pixel that changes changes its cell, each cell the blur reads it from, and each pixel
  /// given a mix of those: out to the last tap, a cell more for the cells a pixel is mixed from,
  /// and half a cell to their far edges.
  pub(super) fn reach(&self) -> Vec2 {
    Vec2::from(self.taps.map(|taps| (taps as f32 + 1.5) * HAZE_CELL as f32))
  }
}

/// The reach in points of the last of [`OPEN_REACHES`].
const OPEN_REACH: f32 = OPEN_REACHES[OPEN_REACHES.len() - 1];
/// The reach in texels of the last of [`BOUNCE_REACHES`].
const BOUNCE_REACH: f32 = BOUNCE_REACHES[BOUNCE_REACHES.len() - 1];
/// The pixels a bound is widened by on every side.
///
/// A smoothed edge spreads into the pixels beside it, and corners are rounded to whole pixels.
const MARGIN: f32 = 2.0;
/// The height in pixels of a band of the picture.
///
/// A bound keeps where it lies across each band.
pub(super) const BAND: f32 = 16.0;
/// The most stretches of a band that a bound is kept as.
///
/// Changes far apart in the same rows stay apart, and the two nearest of more become one.
const SPANS: usize = 4;
/// The stretches of a band that a bound lies in, from the left.
///
/// Each is its start and end in pixels, and they are apart from one another. The ones after the
/// last are [`NO_SPAN`].
pub(super) type Band = [[f32; 2]; SPANS];
/// A stretch that holds nothing, which starts past its end.
const NO_SPAN: [f32; 2] = [f32::MAX, f32::MIN];
/// The most rectangles of a target that a bound is told as.
///
/// Each is a copy of its own for whoever reads the target back.
const MOST_PARTS: usize = 16;
/// The shares of a rectangle that may lie outside a bound.
///
/// A rectangle is put around stretches under one another while no more of it than that is
/// outside them. The first share is tried first, and the next while the rectangles are more
/// than [`MOST_PARTS`].
const WASTES: [f32; 3] = [0.1, 0.25, 0.5];
/// The share of the picture that counts as all of it.
///
/// A bound of that many of its pixels is taken for the whole picture. Wiping a part costs a
/// little, which a part this large does not save again.
const MOSTLY: f32 = 0.8;
/// The fewest shapes that a moved object adds to a bound.
///
/// They are its surroundings and its shadow, where it was and where it is.
pub(super) const MOVED_SHAPES: usize = 4;
/// The most points one shape of a bound is put around.
///
/// They are the corners of a box where it is and where it is carried to.
const MOST_POINTS: usize = 8 * 2;
/// The most shapes a bound is made of.
///
/// A change that takes more is taken to show everywhere. Each shape takes a third of a
/// microsecond or so to gather, which is lost when the bound comes out as most of the picture,
/// and so many are a few hundredths of the time the cheapest whole picture takes to draw.
pub(super) const MOST_SHAPES: usize = 64;
/// A box along the axes, as its least and its greatest corner.
pub(super) type Bounds = [Vec3; 2];

/// The part of a picture or of a map that is drawn again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Redrawn {
  Nothing,
  /// A rectangle of it, as x, y, width and height in its pixels.
  ///
  /// Of a picture, it is what the triangles last gathered cover inside the rectangle.
  Within([u32; 4]),
  Whole,
}

/// The bound of everything a set of changes shows in.
///
/// It is gathered as triangles over a picture. It holds for two scenes that differ in the places
/// and materials of some objects and in some lamps, and in nothing else.
#[derive(Debug)]
pub(super) struct Reach<'a> {
  /// The matrix from panel points to the picture's pixels.
  to_pixels: Mat4,
  /// The matrix from panel points to the shadow map's clip.
  light_view: Mat4,
  picture: Vec2,
  panel: Vec2,
  /// The lowest and highest anything on the panel reaches.
  heights: [f32; 2],
  toward_light: Vec3,
  /// The distance in points a shadow's edge is looked up over.
  shadow_margin: f32,
  /// The depth in points of all the shadow map holds.
  shadow_depth: f32,
  /// The size in points of a texel of the height map.
  height_texel: f32,
  /// The size in points of a bounce map texel, if light bounces.
  bounce: Option<f32>,
  /// The lamps that are the same in both scenes.
  ///
  /// The first `lit` of them shine.
  lamps: [[[f32; 4]; 2]; LAMPS],
  lit: usize,
  /// The corners of the triangles that cover the bound.
  ///
  /// They are in the picture's clip space.
  corners: &'a mut Vec<[f32; 2]>,
  /// Where the bound lies across each band.
  ///
  /// The bands are [`BAND`] pixels high, from the top of the picture.
  bands: &'a mut Vec<Band>,
  /// The rectangle of the panel whose bounced light differs.
  ///
  /// Light comes off it otherwise in one scene than in the other. It is the least and greatest
  /// corner of it, and `None` while there is none.
  relit: Option<[Vec2; 2]>,
  /// The count of shapes the bound is made of so far.
  shapes: usize,
  /// Whether the bound is as good as the whole picture.
  ///
  /// It is if some change has no bound, the bound is most of the picture, or it takes more
  /// than [`MOST_SHAPES`].
  everywhere: bool,
}

impl<'a> Reach<'a> {
  /// Creates the bound of two scenes that are lit alike.
  ///
  /// Both are scenes that [`bounded`] holds for. `picture` is the width and height of the
  /// picture in pixels. The triangles are gathered in `corners` and the stretches in `bands`,
  /// in place of what the two held.
  pub(super) fn new(
    before: &Globals,
    now: &Globals,
    picture: [u32; 2],
    corners: &'a mut Vec<[f32; 2]>,
    bands: &'a mut Vec<Band>,
  ) -> Self {
    let [toward_x, toward_y, toward_z, softness] = now.toward_light;
    let picture = Vec2::new(picture[0] as f32, picture[1] as f32);
    let [width, height, low, high] = now.panel;
    let panel = Vec2::new(width, height);
    let longer = panel.max_element();
    // each of the shadow map's three axes is as long as a row of the light's matrix is short
    let light_view = Mat4::from_cols_array_2d(&now.light_view);
    let along = |row: usize| light_view.row(row).truncate().length();
    let shadow_texel = 2.0 / along(0).min(along(1)) / SHADOW_SIZE as f32;
    // the places looked at to each side, one more for the four texels each is mixed from and one
    // for the texel a point lies in, along both axes at once
    let places = SHADOW_SPREAD as f32 * softness.abs() + 2.0;
    corners.clear();
    bands.clear();
    bands.resize((picture.y / BAND).ceil() as usize, [NO_SPAN; SPANS]);
    let mut reach = Self {
      to_pixels: to_pixels(&now.view, picture),
      light_view,
      picture,
      panel,
      heights: [low, high],
      toward_light: Vec3::new(toward_x, toward_y, toward_z),
      shadow_margin: places * shadow_texel * std::f32::consts::SQRT_2,
      shadow_depth: 1.0 / along(2),
      height_texel: longer / HEIGHT_SIZE as f32,
      bounce: now.bounces().then(|| longer / BOUNCE_SIZE as f32),
      lamps: [[[0.0; 4]; 2]; LAMPS],
      lit: 0,
      corners,
      bands,
      relit: None,
      shapes: 0,
      everywhere: false,
    };
    for (_, now) in same_lamps(before, now, true) {
      reach.lamps[reach.lit] = *now;
      reach.lit += 1;
    }
    reach
  }

  /// Returns the count of shapes the bound is made of.
  ///
  /// `moved` is where each object that differs in shape or place was and is, and `recolored`
  /// the count of boxes around what differs in what it shows alone. A bound of more than
  /// [`MOST_SHAPES`] is not worth building.
  pub(super) fn takes(
    &self,
    before: &Globals,
    now: &Globals,
    moved: impl Iterator<Item = Bounds>,
    recolored: usize,
  ) -> usize {
    // what something lights, and what that lights in turn if light bounces
    let lit = 1 + usize::from(self.bounce.is_some());
    let lamps = same_lamps(before, now, false).count() * 2;
    // an object's surroundings, its shadow and what each lamp lights across it
    let behind = |bounds| {
      let over = self.over(bounds);
      (0..self.lit)
        .filter_map(|lamp| self.behind(over, lamp))
        .count()
    };
    let moved: usize = moved.map(|bounds| 1 + lit + behind(bounds)).sum();
    lamps * lit + moved + recolored * lit
  }

  /// Adds everything that the lamps that differ light.
  pub(super) fn relight(&mut self, before: &Globals, now: &Globals) {
    for (before, now) in same_lamps(before, now, false) {
      self.lamp(*before);
      self.lamp(*now);
    }
  }

  /// Returns the rectangle of height map texels a box lies in.
  ///
  /// It is a rectangle of the panel, in points.
  fn over(&self, [min, max]: Bounds) -> [Vec2; 2] {
    let texel = Vec2::splat(self.height_texel);
    self.past_edges([min.truncate() - texel, max.truncate() + texel])
  }

  /// Returns the box that a lamp lights across a rectangle.
  ///
  /// The rectangle is of the panel and the box of its space. `lamp` is its place among the lamps
  /// that are the same in both scenes. The box is as far up and down as the lamp reaches, and a
  /// lamp that lights nothing across the rectangle has none.
  fn behind(&self, over: [Vec2; 2], lamp: usize) -> Option<Bounds> {
    let [[x, y, height, reach], _] = self.lamps[lamp];
    let [from, to] = behind(over, Vec2::new(x, y), reach)?;
    Some([from.extend(height - reach), to.extend(height + reach)])
  }

  /// Adds the pixels that points of the panel are seen at.
  ///
  /// The pixels between them are added too.
  fn cover(&mut self, points: impl Iterator<Item = Vec3>) {
    if self.everywhere {
      return;
    }
    let mut seen = [Vec2::ZERO; MOST_POINTS];
    let mut count = 0;
    for point in points {
      seen[count] = self.to_pixels.transform_point3(point).truncate();
      count += 1;
    }
    let mut tight = [Vec2::ZERO; MOST_POINTS + 1];
    let tight = hull(&mut seen[..count], &mut tight);
    let mut around = [Vec2::ZERO; MOST_POINTS * 2];
    let around = widened(tight, MARGIN, &mut around);
    let Some((&first, rest)) = around.split_first() else {
      return;
    };
    if !around.iter().all(|corner| corner.is_finite()) {
      self.everywhere = true;
      return;
    }
    let (top, foot) = (around.iter()).fold((f32::MAX, f32::MIN), |(top, foot), corner| {
      (top.min(corner.y), foot.max(corner.y))
    });
    let sides = || (around.iter()).zip(around.iter().cycle().skip(1));
    let highest = (top / BAND).floor().max(0.0) as usize;
    for (band, spans) in self.bands.iter_mut().enumerate().skip(highest) {
      let (upper, lower) = (band as f32 * BAND, (band + 1) as f32 * BAND);
      if foot < upper {
        break;
      }
      // the shape has no dent, so it is one stretch of the band: between where its sides come
      // into the band and where they leave it. a level side ends where two others do
      let mut span = NO_SPAN;
      for (from, to) in sides() {
        let (high, low) = if from.y <= to.y {
          (from, to)
        } else {
          (to, from)
        };
        if high.y == low.y || low.y < upper || high.y > lower {
          continue;
        }
        for down in [upper.max(high.y), lower.min(low.y)] {
          let across = high.x + (low.x - high.x) * (down - high.y) / (low.y - high.y);
          span = [span[0].min(across), span[1].max(across)];
        }
      }
      join(spans, span);
    }
    // the picture's pixels that the bands hold between them
    let held: f32 = (self.bands.iter().flatten())
      .map(|&[starts, ends]| (ends.min(self.picture.x) - starts.max(0.0)).max(0.0) * BAND)
      .sum();
    self.shapes += 1;
    if held >= self.picture.x * self.picture.y * MOSTLY || self.shapes > MOST_SHAPES {
      self.everywhere = true;
      return;
    }
    let to_clip = |corner: Vec2| {
      let across = corner / self.picture * 2.0 - 1.0;
      [across.x, -across.y]
    };
    for pair in rest.windows(2) {
      (self.corners).extend([first, pair[0], pair[1]].map(to_clip));
    }
  }

  /// Returns a rectangle taken past each panel edge it reaches.
  ///
  /// The maps hold nothing past the panel's edges, and a point out there is given what is at
  /// the edge.
  fn past_edges(&self, [min, max]: [Vec2; 2]) -> [Vec2; 2] {
    let far = self.far();
    [
      Vec2::select(min.cmple(Vec2::ZERO), Vec2::splat(-far), min),
      Vec2::select(max.cmpge(self.panel), self.panel + far, max),
    ]
  }

  /// Returns a distance in points that is off the picture.
  ///
  /// It is past every edge of the picture from anywhere on it.
  fn far(&self) -> f32 {
    self.panel.max_element() * 4.0 + 1e4
  }

  /// Adds all that takes light off a rectangle of the panel.
  ///
  /// Nothing does unless light bounces. Such a point is over or under a place near the
  /// rectangle, between the scene's heights, where the place the bounce map is read at is.
  fn add_bounced(&mut self, [min, max]: [Vec2; 2]) {
    let Some(texel) = self.bounce else {
      return;
    };
    // the texels the rectangle lies in, of those the map has
    let (min, max) = ((min - texel).max(Vec2::ZERO), (max + texel).min(self.panel));
    if min.cmpgt(max).any() {
      return;
    }
    let relit = self.relit.unwrap_or([min, max]);
    self.relit = Some([relit[0].min(min), relit[1].max(max)]);
    // from them out to the last ring, and half a texel more for the texels a place is mixed from
    let [min, max] = self.past_edges([min, max]);
    let carried = (BOUNCE_REACH + 0.5) * texel;
    let [low, high] = self.heights;
    self.cover(corners([
      (min - carried).extend(low),
      (max + carried).extend(high),
    ]));
  }

  /// Adds a box whose objects cast the shadows they did.
  ///
  /// What differs in it is drawn again, and so is what its light is thrown on.
  pub(super) fn recolored(&mut self, bounds: Bounds) {
    self.add_drawn(bounds);
    self.add_bounced([bounds[0].truncate(), bounds[1].truncate()]);
  }

  /// Adds the part of a box that is drawn.
  ///
  /// That is what lies between the scene's heights.
  fn add_drawn(&mut self, [min, max]: Bounds) {
    let [low, high] = self.heights;
    if min.z <= high && max.z >= low {
      self.cover(corners([
        min.with_z(min.z.max(low)),
        max.with_z(max.z.min(high)),
      ]));
    }
  }

  /// Adds an object whose shape or place differs.
  ///
  /// `bounds` is where it is in one of them, and the object is added once for each.
  pub(super) fn moved(&mut self, bounds: Bounds) {
    let [min, max] = bounds;
    let [low, high] = self.heights;
    let over = self.over(bounds);
    // what is behind it from each lamp. this comes first, since a lamp over it lights across
    // it every way, which is often most of the picture
    for lamp in 0..self.lit {
      if let Some(behind) = self.behind(over, lamp) {
        self.cover(corners(behind));
      }
    }
    // the object and what is near it. what stands near a point tells of it only from the last
    // ring's reach under the lowest there is to just under the highest
    self.cover(corners([
      (over[0] - OPEN_REACH).extend(low - OPEN_REACH - OPEN_CLEAR),
      (over[1] + OPEN_REACH).extend(high),
    ]));
    if self.everywhere {
      return;
    }

    // its shadow: the box, and around it the places a shadow edge is looked up over, carried
    // away from the light for as far as the shadow map holds anything
    let lit = [min - self.shadow_margin, max + self.shadow_margin];
    let on_map = |corner: Vec3| self.light_view.transform_point3(corner).truncate();
    // a place past the map's edge is given what is at the edge, wherever it is
    if !corners(lit).all(|corner| on_map(corner).abs().max_element() < 1.0) {
      self.everywhere = true;
      return;
    }
    let run = self.shadow_depth * (1.0 + SHADOW_BIAS) / self.toward_light.length();
    let carried = -self.toward_light * run;
    let shadow = corners(lit).flat_map(|corner| [corner, corner + carried]);
    self.cover(shadow.clone());
    // the light off it and off what its shadow falls on
    let fallen = shadow.fold([Vec2::MAX, Vec2::MIN], |[least, most], corner| {
      [least.min(corner.truncate()), most.max(corner.truncate())]
    });
    self.add_bounced(fallen);
  }

  /// Adds everything a lamp lights.
  fn lamp(&mut self, [[x, y, height, reach], _]: [[f32; 4]; 2]) {
    if reach > 0.0 {
      let at = Vec3::new(x, y, height);
      self.cover(corners([at - reach, at + reach]));
      self.add_bounced([at.truncate() - reach, at.truncate() + reach]);
    }
  }

  /// Returns the part of the bounce map the changes show in.
  ///
  /// The part is in the map's texels.
  pub(super) fn relit(&self) -> Redrawn {
    if self.everywhere {
      return Redrawn::Whole;
    }
    let Some([min, max]) = self.relit else {
      return Redrawn::Nothing;
    };
    let to_texels = BOUNCE_SIZE as f32 / self.panel;
    texels([min * to_texels, max * to_texels], BOUNCE_SIZE)
  }

  /// Returns the part of the picture the changes show in.
  pub(super) fn redrawn(&self) -> Redrawn {
    if self.everywhere {
      return Redrawn::Whole;
    }
    // the bands the bound is in, cut to the picture
    let spans = (self.bands.iter().enumerate())
      .flat_map(|(band, spans)| spans.iter().map(move |span| (band, *span)));
    let within = spans.filter_map(|(band, [starts, ends])| {
      let (starts, ends) = (starts.floor().max(0.0), ends.ceil().min(self.picture.x));
      (starts < ends).then_some((band, starts, ends))
    });
    let bound = within.fold(None, |bound: Option<[f32; 4]>, (band, starts, ends)| {
      let [left, top, right, _] = bound.unwrap_or([f32::MAX, band as f32 * BAND, f32::MIN, 0.0]);
      let foot = ((band + 1) as f32 * BAND).min(self.picture.y);
      Some([left.min(starts), top, right.max(ends), foot])
    });
    match bound {
      Some([left, top, right, foot]) => {
        Redrawn::Within([left, top, right - left, foot - top].map(|side| side as u32))
      }
      None => Redrawn::Nothing,
    }
  }
}

/// Returns the matrix from panel points to a view's pixels.
///
/// `view` is the matrix from panel points to clip space, and `size` the width and height in
/// pixels of the picture or the map it is drawn on.
pub(super) fn to_pixels(view: &[[f32; 4]; 4], size: Vec2) -> Mat4 {
  Mat4::from_scale(Vec3::new(size.x / 2.0, -size.y / 2.0, 1.0))
    * Mat4::from_translation(Vec3::new(1.0, -1.0, 0.0))
    * Mat4::from_cols_array_2d(view)
}

/// Returns the texels of a map that a rectangle of it is in.
///
/// The texels next to them are taken in too. The rectangle is its least and greatest corner in
/// texels, and the map is `size` texels wide and high. A rectangle with a corner that is no
/// number is taken for the whole map.
pub(super) fn texels([min, max]: [Vec2; 2], size: u32) -> Redrawn {
  if !(min.is_finite() && max.is_finite()) {
    return Redrawn::Whole;
  }
  let min = (min.floor() - 1.0).max(Vec2::ZERO);
  let reach = (max.ceil() + 1.0).min(Vec2::splat(size as f32)) - min;
  if reach.x > 0.0 && reach.y > 0.0 {
    Redrawn::Within([min.x, min.y, reach.x, reach.y].map(|side| side as u32))
  } else {
    Redrawn::Nothing
  }
}

/// Returns the eight corners of a box.
pub(super) fn corners([min, max]: Bounds) -> impl Iterator<Item = Vec3> + Clone {
  (0..8).map(move |corner| {
    let pick = |bit: usize| if corner >> bit & 1 == 0 { min } else { max };
    Vec3::new(pick(0).x, pick(1).y, pick(2).z)
  })
}

/// Adds a box to boxes that lie apart seen from above.
///
/// The box and every one it lies over become the one box around them.
pub(super) fn add_box(boxes: &mut Vec<Bounds>, mut added: Bounds) {
  let over = |[min, max]: &Bounds, [least, most]: &Bounds| {
    min.truncate().cmple(most.truncate()).all() && least.truncate().cmple(max.truncate()).all()
  };
  while let Some(under) = boxes.iter().position(|held| over(held, &added)) {
    let [min, max] = boxes.swap_remove(under);
    added = [added[0].min(min), added[1].max(max)];
  }
  boxes.push(added);
}

/// Adds a stretch to those of a band.
///
/// It and every stretch it touches become one. If that leaves more than a band keeps, the two
/// with the least between them become one too. A stretch that holds nothing adds nothing.
fn join(spans: &mut Band, mut added: [f32; 2]) {
  if added[0] > added[1] {
    return;
  }
  let mut apart = [NO_SPAN; SPANS + 1];
  let mut count = 0;
  for span in spans.iter().filter(|span| span[0] <= span[1]) {
    if span[0] <= added[1] && added[0] <= span[1] {
      added = [added[0].min(span[0]), added[1].max(span[1])];
    } else {
      apart[count] = *span;
      count += 1;
    }
  }
  apart[count] = added;
  count += 1;
  apart[..count].sort_unstable_by(|one, other| one[0].total_cmp(&other[0]));
  if count > SPANS {
    let gap = |before: &usize| apart[before + 1][0] - apart[*before][1];
    let nearest = (0..SPANS)
      .min_by(|one, other| gap(one).total_cmp(&gap(other)))
      .expect("there is more than one stretch");
    apart[nearest][1] = apart[nearest + 1][1];
    apart.copy_within(nearest + 2.., nearest + 1);
    apart[SPANS] = NO_SPAN;
  }
  spans.copy_from_slice(&apart[..SPANS]);
}

/// Gathers the rectangles of a target that a bound shows in.
///
/// `bands` is where the bound lies across each band of a picture of `picture` pixels, which is
/// stretched over `target` pixels. Each rectangle is x, y, width and height in the target's
/// pixels, and they are added to `parts`, [`MOST_PARTS`] of them at most. What the present pass
/// gathers around a pixel is taken in, which is `reach` of the picture's pixels along each axis.
pub(super) fn parts(
  bands: &[Band],
  picture: [u32; 2],
  target: [u32; 2],
  reach: Vec2,
  parts: &mut Vec<[u32; 4]>,
) {
  let kept = parts.len();
  for waste in WASTES {
    parts.truncate(kept);
    gather_parts(bands, picture, target, reach, waste, parts);
    if parts.len() - kept <= MOST_PARTS {
      return;
    }
  }
  // too many still, and the one rectangle around them all is told in their place
  let around = parts[kept..]
    .iter()
    .fold([u32::MAX, u32::MAX, 0, 0], |around, part| {
      [
        around[0].min(part[0]),
        around[1].min(part[1]),
        around[2].max(part[0] + part[2]),
        around[3].max(part[1] + part[3]),
      ]
    });
  parts.truncate(kept);
  parts.push([
    around[0],
    around[1],
    around[2] - around[0],
    around[3] - around[1],
  ]);
}

/// Adds the rectangles of a target that a bound shows in.
///
/// Everything is as [`parts`] takes it. A rectangle is put around the stretches of bands under
/// one another, while no more than `waste` of it lies outside them.
fn gather_parts(
  bands: &[Band],
  picture: [u32; 2],
  target: [u32; 2],
  reach: Vec2,
  waste: f32,
  parts: &mut Vec<[u32; 4]>,
) {
  let [picture, target] =
    [picture, target].map(|[width, height]| Vec2::new(width as f32, height as f32));
  let scale = target / picture;
  // one more for the pixel of the picture that a pixel of the target is given
  let reach = reach + 1.0;
  // the bands to each side of one that what shows in it may come from
  let aside = (reach.y / BAND).ceil() as usize;
  let mut spans = Vec::with_capacity((2 * aside + 1) * SPANS);
  // the rectangles that reach down to the band before, each with the count of its pixels that
  // lie in the stretches it is around, and those that reach down to this one
  let (mut open, mut next) = (Vec::<([u32; 4], u64)>::new(), Vec::new());
  for band in 0..bands.len() {
    let top = (band as f32 * BAND * scale.y).floor();
    let foot = (((band + 1) as f32 * BAND).min(picture.y) * scale.y)
      .ceil()
      .min(target.y);
    // what shows in this band of the target is in this band of the picture or in one `aside`
    // or fewer from it
    let near = &bands[band.saturating_sub(aside)..(band + aside + 1).min(bands.len())];
    spans.clear();
    for &[starts, ends] in near.iter().flatten().filter(|span| span[0] <= span[1]) {
      let left = ((starts - reach.x) * scale.x).floor().max(0.0);
      let right = ((ends + reach.x) * scale.x).ceil().min(target.x);
      if left < right && top < foot {
        spans.push([left, right]);
      }
    }
    let count = spans.len();
    spans.sort_unstable_by(|one, other| one[0].total_cmp(&other[0]));
    let mut span = 0;
    while span < count {
      // the stretches that touch are one
      let [left, mut right] = spans[span];
      span += 1;
      while span < count && spans[span][0] <= right {
        right = right.max(spans[span][1]);
        span += 1;
      }
      let [left, right, top, foot] = [left, right, top, foot].map(|side| side as u32);
      let lies = u64::from(right - left) * u64::from(foot - top);
      // the rectangle around this and one from the band before that it is under
      let both = |&([last_left, last_top, last_width, _], held): &([u32; 4], u64)| {
        let (least, most) = (last_left.min(left), (last_left + last_width).max(right));
        (
          [least, last_top, most - least, foot - last_top],
          held + lies,
        )
      };
      let over = open.iter().position(|last| {
        let ([last_left, _, last_width, _], _) = *last;
        let ([_, _, width, height], held) = both(last);
        let around = u64::from(width) * u64::from(height);
        last_left < right
          && left < last_left + last_width
          && around.saturating_sub(held) as f32 <= around as f32 * waste
      });
      next.push(match over {
        Some(last) => both(&open.swap_remove(last)),
        None => ([left, top, right - left, foot - top], lies),
      });
    }
    parts.extend(open.drain(..).map(|(part, _)| part));
    std::mem::swap(&mut open, &mut next);
  }
  parts.extend(open.into_iter().map(|(part, _)| part));
}

/// Returns the lamps two scenes share, or those they do not.
///
/// The same ones are those that are so in both scenes and shine, if `same` is set. Each is a
/// lamp as the first scene has it and as the second does.
fn same_lamps<'a>(
  before: &'a Globals,
  now: &'a Globals,
  same: bool,
) -> impl Iterator<Item = (&'a [[f32; 4]; 2], &'a [[f32; 4]; 2])> {
  (before.lamps.iter().zip(&now.lamps)).filter(move |(before, now)| {
    if same {
      before == now && now[0][3] > 0.0
    } else {
      before != now
    }
  })
}

/// Returns whether a change of a scene can be bounded.
///
/// The bound takes these for granted, and a scene without one of them is drawn whole:
///
/// - Every number of the view, the light and the lamps is finite.
/// - The light is over the panel, the panel has a width and a height, and the highest
///   anything reaches is over the lowest.
/// - No lamp has a reach or a radius under zero.
/// - Every object's transform is finite and keeps flat things flat: its last row is 0, 0, 0, 1.
/// - Every mesh's box is finite.
///
/// `placed` is the scene's objects and `boxes` the box around each of its meshes.
pub(super) fn bounded(
  globals: &Globals,
  placed: &[Placed],
  mut boxes: impl Iterator<Item = Option<Bounds>>,
) -> bool {
  let numbers: &[f32] = bytemuck::cast_slice(std::slice::from_ref(globals));
  let [width, height, low, high] = globals.panel;
  let lamps =
    (globals.lamps.iter()).all(|[[.., reach], [.., radius]]| *reach >= 0.0 && *radius >= 0.0);
  let flat = |placed: &Placed| {
    let model = placed.model;
    model.as_flattened().iter().all(|number| number.is_finite())
      && [model[0][3], model[1][3], model[2][3], model[3][3]] == [0.0, 0.0, 0.0, 1.0]
  };
  numbers.iter().all(|number| number.is_finite())
    && globals.toward_light[2] > 0.0
    && width > 0.0
    && height > 0.0
    && low < high
    && lamps
    && placed.iter().all(flat)
    && boxes.all(|bounds| bounds.is_none_or(|[min, max]| min.is_finite() && max.is_finite()))
}

/// Returns a shape with every side moved outward.
///
/// `tight` is the shape's corners as [`hull`] gives them, and each side is moved `by`. The new
/// corners are written to the start of `around`, which has room for twice as many. A shape of
/// fewer than three corners becomes the rectangle around them.
fn widened<'a>(tight: &[Vec2], by: f32, around: &'a mut [Vec2]) -> &'a [Vec2] {
  if tight.len() < 3 {
    let Some(&first) = tight.first() else {
      return &[];
    };
    let (min, max) = (tight.iter()).fold((first, first), |(min, max), corner| {
      (min.min(*corner), max.max(*corner))
    });
    let (min, max) = (min - by, max + by);
    around[..4].copy_from_slice(&[min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)]);
    return &around[..4];
  }
  let mut count = 0;
  for (index, &corner) in tight.iter().enumerate() {
    let before = tight[(index + tight.len() - 1) % tight.len()];
    let after = tight[(index + 1) % tight.len()];
    // the way each of the two sides runs, and the way out of the shape across it
    let (into, onward) = ((corner - before).normalize(), (after - corner).normalize());
    let (out_before, out_after) = (-into.perp(), -onward.perp());
    let turn = 1.0 + out_before.dot(out_after);
    if turn >= 0.5 {
      // where the two moved sides meet
      around[count] = corner + (out_before + out_after) * by / turn;
      count += 1;
    } else {
      // they meet far past a sharp corner, so it is cut square across instead
      around[count] = corner + (out_before + into) * by;
      around[count + 1] = corner + (out_after - onward) * by;
      count += 2;
    }
  }
  &around[..count]
}

/// Returns the corners of the hull of some points.
///
/// The hull is the smallest shape without a dent that holds every point, and its corners run
/// around it one way. They are written to the start of `around`, which has room for one more
/// than there are points, and `points` are left in another order.
fn hull<'a>(points: &mut [Vec2], around: &'a mut [Vec2]) -> &'a [Vec2] {
  points.sort_unstable_by(|one, other| (one.x.total_cmp(&other.x)).then(one.y.total_cmp(&other.y)));
  let mut count = 0;
  // a point is kept while the way around turns left at it. `from` corners are already settled
  let mut turn = |count: &mut usize, from: usize, point: Vec2| {
    while *count > from
      && let [.., before, last] = around[..*count]
      && (last - before).perp_dot(point - last) <= 0.0
    {
      *count -= 1;
    }
    around[*count] = point;
    *count += 1;
  };
  // the lower side from left to right, and then the upper side back to the first corner. a
  // point given twice is one corner
  for (index, &point) in points.iter().enumerate() {
    if index == 0 || point != points[index - 1] {
      turn(&mut count, 1, point);
    }
  }
  let lower = count;
  if lower < 3 && points.first() == points.last() {
    return &around[..1.min(count)];
  }
  for &point in points.iter().rev().skip(1) {
    turn(&mut count, lower, point);
  }
  &around[..count - 1]
}

/// Returns the rectangle a lamp lights across another.
///
/// It holds every point within `reach` of the lamp whose straight line to the lamp passes over
/// the rectangle given, all seen from above. It is `None` if there is no such point.
fn behind([min, max]: [Vec2; 2], lamp: Vec2, reach: f32) -> Option<[Vec2; 2]> {
  let nearest = lamp.clamp(min, max);
  let away = nearest.distance(lamp);
  if away >= reach {
    return None;
  }
  let lit = [lamp - reach, lamp + reach];
  // the lamp is over the rectangle, and lights across it every way
  if away <= f32::EPSILON {
    return Some(lit);
  }
  // each such point is some point of the rectangle pushed out from the lamp, by no more than
  // takes the nearest one to the end of the lamp's reach
  let most = reach / away;
  let pushed = [lamp + (min - lamp) * most, lamp + (max - lamp) * most];
  Some([
    min.min(pushed[0]).max(lit[0]),
    max.max(pushed[1]).min(lit[1]),
  ])
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::scene::{Lamp, Light, Look, Scene};

  const PICTURE: [u32; 2] = [800, 400];

  fn scene(bounce: f32) -> Scene {
    Scene {
      size: Vec2::new(400.0, 200.0),
      heights: [-10.0, 40.0],
      lean: Vec2::new(0.1, -0.3),
      light: Light {
        bounce,
        ..Light::default()
      },
      look: Look::default(),
      objects: Vec::new(),
      lamps: Vec::new(),
    }
  }

  /// Returns numbers from zero to one, the same ones every time.
  fn numbers() -> impl FnMut() -> f32 {
    let mut state = 0x2545_f491_u32;
    move || {
      state ^= state << 13;
      state ^= state >> 17;
      state ^= state << 5;
      (state >> 8) as f32 / (1 << 24) as f32
    }
  }

  /// Returns the pixel a point of the scene is seen at.
  fn pixel(scene: &Scene, point: Vec3) -> Vec2 {
    let seen = scene.view().project_point3(point);
    Vec2::new(seen.x + 1.0, 1.0 - seen.y) / 2.0 * Vec2::new(PICTURE[0] as f32, PICTURE[1] as f32)
  }

  /// Returns whether the triangles gathered cover a pixel.
  ///
  /// A pixel they cover has to be in its band too.
  fn shows(corners: &[[f32; 2]], bands: &[Band], pixel: Vec2) -> bool {
    let at = pixel / Vec2::new(PICTURE[0] as f32, PICTURE[1] as f32) * 2.0 - 1.0;
    let at = Vec2::new(at.x, -at.y);
    let covered = corners.as_chunks::<3>().0.iter().any(|triangle| {
      let [a, b, c] = triangle.map(Vec2::from);
      let sides = [(a, b), (b, c), (c, a)].map(|(from, to)| (to - from).perp_dot(at - from));
      sides.iter().all(|side| *side >= 0.0) || sides.iter().all(|side| *side <= 0.0)
    });
    let band = bands.get((pixel.y / BAND) as usize);
    let banded = band.is_some_and(|spans| {
      (spans.iter()).any(|&[starts, ends]| starts <= pixel.x && pixel.x <= ends)
    });
    let on_picture = pixel.cmpge(Vec2::ZERO).all() && pixel.y < PICTURE[1] as f32;
    assert!(
      !covered || !on_picture || banded,
      "{pixel} is covered outside its band"
    );
    covered
  }

  #[test]
  fn a_hull_goes_around_every_point_without_a_dent() {
    let mut number = numbers();
    for _ in 0..200 {
      let count = 3 + (number() * 30.0) as usize;
      let points: Vec<Vec2> = (0..count)
        .map(|_| Vec2::new(number(), number()).round() * 3.0 + Vec2::new(number(), number()))
        .collect();
      let mut room = vec![Vec2::ZERO; points.len() + 1];
      let around = hull(&mut points.clone(), &mut room).to_vec();
      assert!(around.len() >= 3);
      let sides =
        || (0..around.len()).map(|side| (around[side], around[(side + 1) % around.len()]));
      for point in points {
        assert!(
          sides().all(|(from, to)| (to - from).perp_dot(point - from) >= -1e-4),
          "{point} is outside {around:?}"
        );
      }
      for ((from, to), (_, next)) in sides().zip(sides().cycle().skip(1)) {
        assert!(
          (to - from).perp_dot(next - to) > 0.0,
          "{around:?} has a dent"
        );
      }
    }
    // two points and one have no inside
    let mut room = [Vec2::ZERO; 4];
    assert_eq!(
      hull(&mut [Vec2::ONE, Vec2::ZERO, Vec2::ONE], &mut room),
      [Vec2::ZERO, Vec2::ONE]
    );
    assert_eq!(hull(&mut [Vec2::ONE, Vec2::ONE], &mut room), [Vec2::ONE]);
    assert_eq!(hull(&mut [], &mut room), []);
  }

  #[test]
  fn a_shape_is_widened_by_as_much_on_every_side() {
    let mut number = numbers();
    for round in 0..300 {
      // some of them slivers, whose corners are sharp
      let squash = if round % 3 == 0 { 0.01 } else { 1.0 };
      let count = 1 + (number() * 12.0) as usize;
      let mut points: Vec<Vec2> = (0..count)
        .map(|_| Vec2::new(number() * 80.0, number() * 80.0 * squash))
        .collect();
      let mut room = vec![Vec2::ZERO; count + 1];
      let tight = hull(&mut points, &mut room).to_vec();
      let mut room = vec![Vec2::ZERO; tight.len().max(2) * 2];
      let around = widened(&tight, 2.0, &mut room);
      assert!(around.len() >= 3);
      let sides =
        || (0..around.len()).map(|side| (around[side], around[(side + 1) % around.len()]));
      // every side runs the same way around, whichever way that is
      let turns: Vec<f32> = (sides().zip(sides().cycle().skip(1)))
        .map(|((from, to), (_, next))| (to - from).perp_dot(next - to))
        .collect();
      assert!(
        turns.iter().all(|turn| *turn >= -1e-3) || turns.iter().all(|turn| *turn <= 1e-3),
        "{around:?} has a dent"
      );
      let way = turns.iter().sum::<f32>().signum();
      for corner in &tight {
        for step in 0..16 {
          let near = corner + Vec2::from_angle(step as f32 / 16.0 * std::f32::consts::TAU) * 2.0;
          assert!(
            sides().all(|(from, to)| (to - from).perp_dot(near - from) * way >= -1e-2),
            "{near} is outside {around:?}"
          );
        }
      }
    }
    assert!(widened(&[], 2.0, &mut [Vec2::ZERO; 4]).is_empty());
  }

  /// A name for what no bound takes, and what breaks a scene so.
  type Broken = (&'static str, fn(&mut Scene));

  #[test]
  fn a_scene_is_bounded_only_with_the_numbers_a_bound_takes_for_granted() {
    use crate::material::Material;
    let object = |transform: Mat4| crate::scene::Object {
      mesh: std::sync::Arc::new(crate::mesh::Mesh::default()),
      transform,
      material: Material::matte(Vec3::ONE),
    };
    let boxed = Some([Vec3::ZERO, Vec3::ONE]);
    let holds = |scene: &Scene, bounds: Option<Bounds>| {
      let placed: Vec<Placed> = scene.objects.iter().map(Placed::of).collect();
      bounded(&Globals::of(scene), &placed, std::iter::once(bounds))
    };
    let lamp = Lamp {
      at: Vec3::new(100.0, 100.0, 12.0),
      color: Vec3::ONE,
      reach: 60.0,
      radius: 4.0,
    };
    let mut plain = scene(3.0);
    plain.lamps = vec![lamp];
    plain.objects = vec![object(
      Mat4::from_rotation_x(0.3) * Mat4::from_scale(Vec3::splat(-2.0)),
    )];
    assert!(holds(&plain, boxed));
    assert!(holds(&plain, None));

    let broken: [Broken; 12] = [
      ("a lamp that reaches less than nowhere", |scene| {
        scene.lamps[0].reach = -50.0
      }),
      ("a lamp smaller than nothing", |scene| {
        scene.lamps[0].radius = -1.0
      }),
      ("a lamp nowhere", |scene| scene.lamps[0].at.x = f32::NAN),
      ("a lamp of no color", |scene| {
        scene.lamps[0].color.y = f32::INFINITY
      }),
      ("a light level with the panel", |scene| {
        scene.light.toward = Vec3::X
      }),
      ("a light under the panel", |scene| {
        scene.light.toward.z = -1.0
      }),
      ("a shadow edge of no width", |scene| {
        scene.light.softness = f32::NAN
      }),
      ("a panel of no width", |scene| scene.size.x = 0.0),
      ("a panel of less than no height", |scene| {
        scene.size.y = -200.0
      }),
      ("heights the wrong way up", |scene| {
        scene.heights = [40.0, -10.0]
      }),
      ("a view that leans without end", |scene| {
        scene.lean.x = f32::INFINITY
      }),
      ("an object that is not flat", |scene| {
        scene.objects[0].transform.x_axis.w = 0.001
      }),
    ];
    for (name, breaks) in broken {
      let mut scene = plain.clone();
      breaks(&mut scene);
      assert!(!holds(&scene, boxed), "{name}");
    }
    let mut nowhere = plain.clone();
    nowhere.objects[0].transform.w_axis.y = f32::NAN;
    assert!(!holds(&nowhere, boxed));
    let mut scaled = plain.clone();
    scaled.objects[0].transform.w_axis.w = 2.0;
    assert!(!holds(&scaled, boxed));
    // a mesh with a vertex nowhere
    assert!(!holds(
      &plain,
      Some([Vec3::ZERO, Vec3::new(1.0, f32::NAN, 1.0)])
    ));
    assert!(!holds(
      &plain,
      Some([Vec3::splat(f32::NEG_INFINITY), Vec3::ONE])
    ));
  }

  #[test]
  fn each_band_is_as_wide_as_the_triangles_that_cross_it() {
    let globals = Globals::of(&scene(0.0));
    let mut number = numbers();
    for _ in 0..200 {
      let (mut corners, mut bands) = (Vec::new(), Vec::new());
      let mut reach = Reach::new(&globals, &globals, PICTURE, &mut corners, &mut bands);
      let mut point = || Vec3::new(number() * 500.0 - 50.0, number() * 300.0 - 50.0, 0.0);
      reach.cover([point(), point(), point(), point()].into_iter());
      let size = Vec2::new(PICTURE[0] as f32, PICTURE[1] as f32);
      for triangle in corners.as_chunks::<3>().0 {
        let [a, b, c] = triangle.map(|[x, y]| (Vec2::new(x, -y) + 1.0) / 2.0 * size);
        // the corners, and along the sides between them
        for (from, to) in [(a, b), (b, c), (c, a)] {
          for step in 0..=20 {
            let on_side = from.lerp(to, step as f32 / 20.0);
            if (0.0..size.y).contains(&on_side.y) {
              let spans = bands[(on_side.y / BAND) as usize];
              assert!(
                (spans.iter())
                  .any(|[starts, ends]| starts - 1e-2 <= on_side.x && on_side.x <= ends + 1e-2),
                "{on_side} is outside {spans:?}"
              );
            }
          }
        }
      }
    }
  }

  /// The reach of a haze two cells to each side.
  const HAZE: Vec2 = Vec2::splat(3.5 * HAZE_CELL as f32);

  /// Asserts that parts hold all a bound shows in.
  ///
  /// That is all the present pass gathers around the bound, `reach` pixels of the picture
  /// along each axis. Returns the count of the target's pixels in the parts, with one in two
  /// parts counted twice.
  fn assert_held(
    bands: &[Band],
    picture: [u32; 2],
    target: [u32; 2],
    reach: Vec2,
    found: &[[u32; 4]],
  ) -> u32 {
    let scale = Vec2::new(target[0] as f32, target[1] as f32)
      / Vec2::new(picture[0] as f32, picture[1] as f32);
    for across in 0..target[0] {
      for down in 0..target[1] {
        // the pixel of the picture this one of the target is given, as the present pass has it
        let shown = (Vec2::new(across as f32, down as f32) + 0.5) / scale;
        let near = |band: usize, [starts, ends]: [f32; 2]| {
          let (upper, lower) = (band as f32 * BAND, (band + 1) as f32 * BAND);
          starts <= ends
            && (starts - reach.x..=ends + reach.x).contains(&shown.x)
            && (upper - reach.y..=lower + reach.y).contains(&shown.y)
        };
        let spans = (bands.iter().enumerate())
          .flat_map(|(band, spans)| spans.iter().map(move |span| (band, *span)));
        if spans.into_iter().any(|(band, span)| near(band, span)) {
          let held = found.iter().any(|&[left, top, width, height]| {
            (left..left + width).contains(&across) && (top..top + height).contains(&down)
          });
          assert!(
            held,
            "{across} {down} of {target:?} is in none of {found:?}"
          );
        }
      }
    }
    // no part reaches off the target, and they are few
    let within = |&[left, top, width, height]: &[u32; 4]| {
      width > 0 && height > 0 && left + width <= target[0] && top + height <= target[1]
    };
    assert!(found.iter().all(within), "{found:?}");
    assert!(found.len() <= MOST_PARTS, "{found:?}");
    found.iter().map(|part| part[2] * part[3]).sum()
  }

  #[test]
  fn the_haze_reaches_past_the_blur_radius_by_the_cells_a_pixel_is_mixed_from() {
    let cell = HAZE_CELL as f32;
    let reach = |radius: Vec2| Spread::new(radius, [64, 32]).reach();
    // a radius of whole cells reads that many, and one part way into a cell reads it too
    assert_eq!(reach(Vec2::splat(2.0 * cell)), Vec2::splat(3.5 * cell));
    assert_eq!(
      reach(Vec2::new(2.2 * cell, 5.0 * cell)),
      Vec2::new(4.5, 6.5) * cell
    );
    // past the radius by at least a cell and a half and by less than two and a half
    for radius in [0.5, 3.0, 17.0, 40.0, 101.0] {
      let past = reach(Vec2::splat(radius)) - radius;
      assert!(past.cmpge(Vec2::splat(1.5 * cell)).all(), "{radius} {past}");
      assert!(past.cmplt(Vec2::splat(2.5 * cell)).all(), "{radius} {past}");
    }
    // no radius reads only the cell itself, and none reads past the map
    for none in [0.0, -3.0, f32::NAN] {
      assert_eq!(reach(Vec2::splat(none)), Vec2::splat(1.5 * cell));
    }
    assert_eq!(
      Spread::new(Vec2::splat(f32::INFINITY), [64, 32]).taps,
      [64, 32]
    );
  }

  #[test]
  fn the_parts_of_a_target_hold_all_that_the_present_pass_gathers_around_a_bound() {
    let mut number = numbers();
    // the soften alone, a haze two cells to each side, and hazes that reach past a band, as
    // far along both axes and not
    let wide = |radius: Vec2| Spread::new(radius, [200, 100]).reach();
    let reaches = [
      Vec2::ONE,
      HAZE,
      wide(Vec2::splat(BAND * 2.5)),
      wide(Vec2::new(6.0, BAND * 4.0)),
      wide(Vec2::new(BAND * 3.0, 2.0)),
    ];
    for reach in reaches {
      for (picture, target) in [
        ([800, 400], [800, 400]),
        ([400, 200], [800, 400]),
        ([531, 301], [797, 452]),
      ] {
        // a bound in a few bands, some next to each other and some apart, and some of it in
        // the same band
        let mut bands = vec![[NO_SPAN; SPANS]; (picture[1] as f32 / BAND).ceil() as usize];
        for _ in 0..5 {
          let band = (number() * bands.len() as f32) as usize;
          let starts = number() * picture[0] as f32;
          join(&mut bands[band], [starts, starts + number() * 90.0]);
        }
        let mut found = Vec::new();
        parts(&bands, picture, target, reach, &mut found);
        let held = assert_held(&bands, picture, target, reach, &found);
        // a short reach keeps them far from all of the target
        if reach.max_element() < BAND {
          assert!(held < target[0] * target[1] / 2, "{found:?}");
        }
      }
    }
    // a bound of nothing has no parts
    let mut found = Vec::new();
    parts(
      &[[NO_SPAN; SPANS]; 25],
      [800, 400],
      [800, 400],
      HAZE,
      &mut found,
    );
    assert!(found.is_empty());
  }

  #[test]
  fn two_changes_far_apart_in_the_same_rows_are_parts_of_their_own() {
    let globals = Globals::of(&scene(0.0));
    let (mut corners, mut bands) = (Vec::new(), Vec::new());
    let mut reach = Reach::new(&globals, &globals, PICTURE, &mut corners, &mut bands);
    // a round thing on the left and a tall one on the right, in the same rows
    let knob = [Vec3::new(40.0, 80.0, 0.0), Vec3::new(70.0, 110.0, 20.0)];
    let meter = [Vec3::new(360.0, 40.0, 0.0), Vec3::new(368.0, 160.0, 2.0)];
    reach.recolored(knob);
    reach.recolored(meter);
    let Redrawn::Within([left, _, width, _]) = reach.redrawn() else {
      panic!("the bound is a part of the picture");
    };
    // the rectangle around both is most of the picture's width
    assert!(left < 100 && width > 600);
    let mut found = Vec::new();
    parts(&bands, PICTURE, PICTURE, HAZE, &mut found);
    let held = assert_held(&bands, PICTURE, PICTURE, HAZE, &found);
    // what lies between the two is in no part
    let between = |&[left, _, width, _]: &[u32; 4]| left < 400 && left + width > 400;
    assert!(!found.iter().any(between), "{found:?}");
    assert!(found.len() <= 6, "{found:?}");
    assert!(held < PICTURE[0] * PICTURE[1] / 6, "{found:?}");
  }

  #[test]
  fn a_band_keeps_stretches_apart_and_joins_those_that_touch() {
    let mut spans = [NO_SPAN; SPANS];
    join(&mut spans, [50.0, 60.0]);
    join(&mut spans, [10.0, 20.0]);
    join(&mut spans, NO_SPAN);
    assert_eq!(spans[..2], [[10.0, 20.0], [50.0, 60.0]]);
    assert_eq!(spans[2..], [NO_SPAN; 2]);
    // one that touches both makes them one
    join(&mut spans, [18.0, 52.0]);
    assert_eq!(spans, [[10.0, 60.0], NO_SPAN, NO_SPAN, NO_SPAN]);
    // one more than a band keeps: the two with the least between them become one
    for starts in [100.0, 200.0, 300.0, 215.0] {
      join(&mut spans, [starts, starts + 10.0]);
    }
    assert_eq!(
      spans,
      [[10.0, 60.0], [100.0, 110.0], [200.0, 225.0], [300.0, 310.0]]
    );
    join(&mut spans, [400.0, 410.0]);
    assert_eq!(
      spans,
      [
        [10.0, 110.0],
        [200.0, 225.0],
        [300.0, 310.0],
        [400.0, 410.0]
      ]
    );
  }

  #[test]
  fn a_bound_that_slants_across_the_target_is_told_as_few_parts() {
    // a stretch that moves across by more than its width from each band to the next
    let mut bands = vec![[NO_SPAN; SPANS]; 60];
    for (band, spans) in bands.iter_mut().enumerate() {
      let starts = band as f32 * 30.0;
      join(spans, [starts, starts + 4.0]);
    }
    let (picture, mut found) = ([1900, 960], Vec::new());
    parts(&bands, picture, picture, HAZE, &mut found);
    assert_held(&bands, picture, picture, HAZE, &found);
    assert!(found.len() > 1, "{found:?}");
    // two that cross, each a long way across from one band to the next: no rectangles few
    // enough hold them closely, and they are told as the one around them all
    let mut bands = vec![[NO_SPAN; SPANS]; 40];
    for (band, spans) in bands.iter_mut().enumerate() {
      let starts = band as f32 * 100.0;
      join(spans, [starts, starts + 4.0]);
      join(spans, [3900.0 - starts, 3904.0 - starts]);
    }
    let (picture, mut found) = ([4000, 640], Vec::new());
    parts(&bands, picture, picture, HAZE, &mut found);
    assert_held(&bands, picture, picture, HAZE, &found);
    assert_eq!(found.len(), 1, "{found:?}");
  }

  #[test]
  fn everything_a_lamp_lights_across_a_rectangle_is_behind_it() {
    let mut number = numbers();
    for _ in 0..500 {
      let mut point = || Vec2::new(number(), number()) * 200.0;
      let (one, other, lamp) = (point(), point(), point());
      let over = [one.min(other), one.max(other)];
      let reach = 20.0 + number() * 150.0;
      let found = behind(over, lamp, reach);
      for _ in 0..50 {
        let lit = lamp + Vec2::from_angle(number() * std::f32::consts::TAU) * number() * reach;
        let crosses = (0..=200).any(|step| {
          let on_line = lit.lerp(lamp, step as f32 / 200.0);
          on_line.cmpge(over[0]).all() && on_line.cmple(over[1]).all()
        });
        if crosses {
          let [min, max] = found.expect("the lamp lights across the rectangle");
          assert!(
            lit.cmpge(min - 1e-3).all() && lit.cmple(max + 1e-3).all(),
            "{lit} is lit across {over:?} by a lamp at {lamp} reaching {reach}"
          );
        }
      }
    }
    // a lamp too far off lights nothing across it
    let over = [Vec2::ZERO, Vec2::ONE];
    assert_eq!(behind(over, Vec2::new(50.0, 0.0), 40.0), None);
  }

  #[test]
  fn a_moved_box_shows_in_its_shadow_and_around_itself_and_nowhere_far_off() {
    let scene = scene(0.0);
    let globals = Globals::of(&scene);
    let (mut corners, mut bands) = (Vec::new(), Vec::new());
    let mut reach = Reach::new(&globals, &globals, PICTURE, &mut corners, &mut bands);
    let (min, max) = (Vec3::new(180.0, 90.0, 0.0), Vec3::new(200.0, 110.0, 20.0));
    reach.moved([min, max]);
    assert_ne!(reach.redrawn(), Redrawn::Whole);
    let toward = scene.light.toward.normalize();
    let mut number = numbers();
    for _ in 0..2000 {
      let within = min + (max - min) * Vec3::new(number(), number(), number());
      // the box itself, as far as it is drawn
      assert!(shows(&corners, &bands, pixel(&scene, within)));
      // behind it from the light, on the panel and far under it
      let behind = within - toward * number() * 300.0;
      assert!(shows(&corners, &bands, pixel(&scene, behind)), "{behind}");
      // as far around it as the last ring, at the heights that what stands near tells of
      let turn = number() * std::f32::consts::TAU;
      let aside = Vec2::from_angle(turn) * number() * OPEN_REACH;
      let near = (within.truncate() + aside).extend(-39.0 + number() * 79.0);
      assert!(shows(&corners, &bands, pixel(&scene, near)), "{near}");
    }
    // toward the light from it, and well off to the side of its shadow
    for clear in [Vec3::new(120.0, 20.0, 0.0), Vec3::new(330.0, 60.0, 0.0)] {
      assert!(!shows(&corners, &bands, pixel(&scene, clear)), "{clear}");
    }
  }

  #[test]
  fn light_that_bounces_carries_a_change_to_every_height_a_place_has_over_and_under_it() {
    let scene = scene(3.0);
    let globals = Globals::of(&scene);
    let (mut corners, mut bands) = (Vec::new(), Vec::new());
    let mut reach = Reach::new(&globals, &globals, PICTURE, &mut corners, &mut bands);
    let (min, max) = (Vec3::new(180.0, 90.0, 0.0), Vec3::new(200.0, 110.0, 20.0));
    reach.recolored([min, max]);
    assert_ne!(reach.redrawn(), Redrawn::Whole);
    let texel = 400.0 / BOUNCE_SIZE as f32;
    let mut number = numbers();
    for _ in 0..2000 {
      let turn = number() * std::f32::consts::TAU;
      let aside = Vec2::from_angle(turn) * number() * BOUNCE_REACH * texel;
      let within = min.truncate() + (max - min).truncate() * Vec2::new(number(), number());
      let near = (within + aside).extend(-10.0 + number() * 50.0);
      assert!(shows(&corners, &bands, pixel(&scene, near)), "{near}");
    }
    // as far off as the last ring on each side, at the lowest and the highest a place is. under
    // this view the highest is seen 12 points up the picture from the lowest
    let reach = BOUNCE_REACH * texel;
    let middle = (min + max).truncate() / 2.0;
    for aside in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
      let edge = middle + aside * (10.0 + reach);
      for height in scene.heights {
        let near = edge.extend(height);
        assert!(shows(&corners, &bands, pixel(&scene, near)), "{near}");
      }
    }
    assert!(!shows(
      &corners,
      &bands,
      pixel(&scene, Vec3::new(60.0, 100.0, 0.0))
    ));
  }

  #[test]
  fn a_lamp_that_differs_shows_as_far_as_it_reaches_and_shadows_show_behind_what_moved() {
    let mut scene = scene(0.0);
    let lamp = Lamp {
      at: Vec3::new(100.0, 100.0, 12.0),
      color: Vec3::ONE,
      reach: 60.0,
      radius: 4.0,
    };
    scene.lamps = vec![lamp];
    let lit = Globals::of(&scene);
    scene.lamps[0].color = Vec3::X;
    let recolored = Globals::of(&scene);
    let mut number = numbers();
    let mut around = |reach: f32| {
      let turn = number() * std::f32::consts::TAU;
      let out = Vec2::from_angle(turn) * number() * reach;
      lamp.at + out.extend((number() - 0.5) * reach)
    };

    let (mut corners, mut bands) = (Vec::new(), Vec::new());
    let mut reach = Reach::new(&lit, &recolored, PICTURE, &mut corners, &mut bands);
    assert_eq!(reach.takes(&lit, &recolored, std::iter::empty(), 0), 2);
    reach.relight(&lit, &recolored);
    assert_ne!(reach.redrawn(), Redrawn::Whole);
    for _ in 0..2000 {
      let near = around(lamp.reach);
      assert!(shows(&corners, &bands, pixel(&scene, near)), "{near}");
    }
    assert!(!shows(
      &corners,
      &bands,
      pixel(&scene, Vec3::new(300.0, 100.0, 0.0))
    ));

    // the same lamp in both, and a box that moved beside it
    let mut reach = Reach::new(&lit, &lit, PICTURE, &mut corners, &mut bands);
    let over = [Vec2::new(120.0, 95.0), Vec2::new(130.0, 105.0)];
    let post = [over[0].extend(0.0), over[1].extend(15.0)];
    // its surroundings, its shadow and what the lamp lights across it
    assert_eq!(reach.takes(&lit, &lit, std::iter::once(post), 0), 3);
    reach.moved(post);
    for _ in 0..4000 {
      // a point the lamp lights whose line to the lamp passes over the box
      let lit = around(lamp.reach);
      let crosses = (0..=100).any(|step| {
        let on_line = lit.truncate().lerp(lamp.at.truncate(), step as f32 / 100.0);
        on_line.cmpge(over[0]).all() && on_line.cmple(over[1]).all()
      });
      if crosses {
        assert!(shows(&corners, &bands, pixel(&scene, lit)), "{lit}");
      }
    }
    // on the far side of the lamp from the box
    assert!(!shows(
      &corners,
      &bands,
      pixel(&scene, Vec3::new(60.0, 100.0, 30.0))
    ));
  }

  #[test]
  fn a_change_with_no_bound_shows_everywhere() {
    let (mut corners, mut bands) = (Vec::new(), Vec::new());
    // a box past the edge of what the light sees is given what the shadow map has at its edge
    let globals = Globals::of(&scene(0.0));
    let mut reach = Reach::new(&globals, &globals, PICTURE, &mut corners, &mut bands);
    assert_eq!(reach.redrawn(), Redrawn::Nothing);
    reach.moved([Vec3::new(380.0, 90.0, 0.0), Vec3::new(460.0, 110.0, 20.0)]);
    assert_eq!(reach.redrawn(), Redrawn::Whole);
    // and so is a change over most of the picture
    let mut reach = Reach::new(&globals, &globals, PICTURE, &mut corners, &mut bands);
    reach.recolored([Vec3::new(20.0, 10.0, 0.0), Vec3::new(380.0, 190.0, 2.0)]);
    assert_eq!(reach.redrawn(), Redrawn::Whole);
  }
}
