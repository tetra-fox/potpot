// flat shapes in one color each, drawn over whatever the target already shows
// RECT and SPAN, the kinds of shape that are not a line, are put ahead of this source when it
// is loaded, and so is PAST, how many pixels past its edge a line or a span is drawn

struct Flat {
  // xy the pixels to a point, and zw the pixel the panel's top left lands on, both in the
  // shapes' place
  mapping: vec4<f32>,
  // the width and height of that place in pixels
  pixels: vec2<f32>,
  // the left and top of that place, in the target's pixels
  origin: vec2<f32>,
  // x one when the target encodes srgb itself, y one when edges are left rough
  options: vec4<f32>,
}

@group(0) @binding(0) var<uniform> flat: Flat;

struct Corner {
  @builtin(position) clip: vec4<f32>,
  @location(0) color: vec4<f32>,
  // in a line, where this is from its middle, along it and across. in a span, how far under its
  // sloping edge this is, squarely across it, and how far under its level line. all in pixels
  @location(1) within: vec2<f32>,
  // how far a line's points are from its middle and its edge from its middle line, in pixels
  @location(2) @interpolate(flat) half: vec2<f32>,
  @location(3) @interpolate(flat) kind: f32,
  // a line's two points in pixels, and the way a curve runs through each or zero at a round end
  @location(4) @interpolate(flat) ends: vec4<f32>,
  @location(5) @interpolate(flat) joins: vec4<f32>,
  // in a span, how far under the highest and the lowest point of its sloping edge this is
  @location(6) past: vec2<f32>,
}

@vertex
fn vertex(
  @location(0) middle: vec2<f32>,
  @location(1) along: vec2<f32>,
  @location(2) half: vec2<f32>,
  @location(3) side: vec2<f32>,
  @location(4) color: vec4<f32>,
  @location(5) ends: vec4<f32>,
  @location(6) joins: vec4<f32>,
  @location(7) kind: f32,
) -> Corner {
  let scale = flat.mapping.xy;
  let shift = flat.mapping.zw;
  var corner: Corner;
  var at: vec2<f32>;
  if kind == RECT {
    // a rectangle along the pixels: each corner goes on the nearest pixel edge, so its sides are
    // sharp at any scale. halves all go the same way, so a line one pixel wide stays that wide
    at = floor((middle + side * half) * scale + shift + 0.5);
  } else if kind == SPAN {
    // `middle` is its left and right, `along` how far down its sloping edge is at the two and
    // `half.x` its level line. the stretches beside it have the very same sides
    let sides = middle * scale.x + shift.x;
    let edge = along * scale.y + shift.y;
    let level = half.x * scale.y + shift.y;
    let top = min(min(edge.x, edge.y), level);
    let foot = max(max(edge.x, edge.y), level);
    // drawn PAST its edges, for the pixels they only partly cover
    let right = side.x > 0.0;
    at = vec2<f32>(select(sides.x, sides.y, right), select(top - PAST, foot + PAST, side.y > 0.0));
    // a stretch of no width covers nothing, and has no slant to divide by
    let wide = max(sides.y - sides.x, 0.000001);
    let slant = length(vec2<f32>(wide, edge.y - edge.x)) / wide;
    corner.within = vec2<f32>((at.y - select(edge.x, edge.y, right)) / slant, at.y - level);
    corner.past = at.y - vec2<f32>(min(edge.x, edge.y), max(edge.x, edge.y));
  } else {
    // a line any way round, with round ends half its width past its two points. it is drawn
    // wider all around by PAST, for the pixels its edge only partly covers
    let reach = vec2<f32>(half.x + half.y, half.y) * scale.x + PAST;
    corner.within = side * reach;
    at = middle * scale + shift + along * corner.within.x + vec2<f32>(-along.y, along.x) * corner.within.y;
  }
  corner.clip = vec4<f32>(at / flat.pixels * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
  corner.color = color;
  corner.half = half * scale.x;
  corner.kind = kind;
  corner.ends = ends * scale.xyxy + shift.xyxy;
  corner.joins = joins;
  return corner;
}

fn decode(encoded: vec3<f32>) -> vec3<f32> {
  let low = encoded / 12.92;
  let high = pow((encoded + 0.055) / 1.055, vec3<f32>(2.4));
  return select(high, low, encoded <= vec3<f32>(0.04045));
}

// how much of a pixel is under the sloping edge of a span. `across` is how far under the
// edge's line the pixel's middle is, and `past` how far under the edge's highest and lowest point
fn under(across: f32, past: vec2<f32>) -> f32 {
  // the edge's line runs on beyond its ends, where the stretch is all under it or all over it
  return clamp(across + 0.5, clamp(past.y + 0.5, 0.0, 1.0), clamp(past.x + 0.5, 0.0, 1.0));
}

// how far past the cut through a joint of a curve a pixel is, the way the curve runs. the two
// lines of the joint are given the same three numbers, so they agree on every pixel
fn past_cut(pixel: vec2<f32>, joint: vec2<f32>, way: vec2<f32>) -> f32 {
  return dot(pixel - joint, way);
}

@fragment
fn fragment(corner: Corner) -> @location(0) vec4<f32> {
  var color = corner.color;
  // colors are given as they are to look, which a target that encodes them itself wants undone
  if flat.options.x > 0.5 {
    color = vec4<f32>(decode(color.rgb), color.a);
  }
  // how much of the pixel the shape covers
  var covered = 1.0;
  if corner.kind == SPAN {
    // the stretch is what is under one of its edge and its level line and over the other
    let under_level = clamp(corner.within.y + 0.5, 0.0, 1.0);
    covered = abs(under(corner.within.x, corner.past) - under_level);
  } else if corner.kind != RECT {
    // from how far the pixel's middle is from the stretch between the line's two points
    let past = max(abs(corner.within.x) - corner.half.x, 0.0);
    let away = length(vec2<f32>(past, corner.within.y));
    covered = clamp(corner.half.y - away + 0.5, 0.0, 1.0);
    // a line of a curve stops at the cut through an end where the next line takes over. a
    // pixel on the cut is drawn by the line that runs to it
    let pixel = corner.clip.xy - flat.origin;
    let before = past_cut(pixel, corner.ends.xy, corner.joins.xy);
    let after = past_cut(pixel, corner.ends.zw, corner.joins.zw);
    if after > 0.0 || (before <= 0.0 && dot(corner.joins.xy, corner.joins.xy) > 0.0) {
      covered = 0.0;
    }
  }
  if flat.options.y > 0.5 {
    covered = step(0.5, covered);
  }
  color.a *= covered;
  return color;
}
