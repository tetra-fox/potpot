// flat shapes in one color each, drawn over whatever the target already shows

struct Flat {
  // the width and height the shapes' points run over, and those of their place in pixels
  size: vec2<f32>,
  pixels: vec2<f32>,
  // x one when the target encodes srgb itself
  options: vec4<f32>,
}

@group(0) @binding(0) var<uniform> flat: Flat;

struct Corner {
  @builtin(position) clip: vec4<f32>,
  @location(0) color: vec4<f32>,
  // where in the shape this is from its middle, along it and across, and how far a line's
  // points are from its middle and its edge from its middle line, all in pixels
  @location(1) within: vec2<f32>,
  @location(2) @interpolate(flat) half: vec2<f32>,
  @location(3) @interpolate(flat) snap: f32,
}

@vertex
fn vertex(
  @location(0) middle: vec2<f32>,
  @location(1) along: vec2<f32>,
  @location(2) half: vec2<f32>,
  @location(3) side: vec2<f32>,
  @location(4) color: vec4<f32>,
  @location(5) snap: f32,
) -> Corner {
  let scale = flat.pixels / flat.size;
  var corner: Corner;
  var at: vec2<f32>;
  if snap > 0.5 {
    // a rectangle that lies along the pixels: each corner goes on the nearest pixel edge,
    // which keeps its sides sharp at any scale. halves all go the same way, so that a line
    // one pixel wide stays one pixel wide
    at = floor((middle + side * half) * scale + 0.5);
  } else {
    // a line any way round, with round ends half its width past its two points: drawn a
    // pixel wider all around than it is, for the pixels its edge only partly covers
    let reach = vec2<f32>(half.x + half.y, half.y) * scale.x + 1.0;
    corner.within = side * reach;
    at = middle * scale + along * corner.within.x + vec2<f32>(-along.y, along.x) * corner.within.y;
  }
  corner.clip = vec4<f32>(at / flat.pixels * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
  corner.color = color;
  corner.half = half * scale.x;
  corner.snap = snap;
  return corner;
}

fn decode(encoded: vec3<f32>) -> vec3<f32> {
  let low = encoded / 12.92;
  let high = pow((encoded + 0.055) / 1.055, vec3<f32>(2.4));
  return select(high, low, encoded <= vec3<f32>(0.04045));
}

@fragment
fn fragment(corner: Corner) -> @location(0) vec4<f32> {
  var color = corner.color;
  // colors are given as they are to look, which a target that encodes them itself wants undone
  if flat.options.x > 0.5 {
    color = vec4<f32>(decode(color.rgb), color.a);
  }
  if corner.snap < 0.5 {
    // how much of the pixel the line covers, from how far the pixel's middle is from the
    // stretch between the line's two points
    let past = max(abs(corner.within.x) - corner.half.x, 0.0);
    let away = length(vec2<f32>(past, corner.within.y));
    color.a *= clamp(corner.half.y - away + 0.5, 0.0, 1.0);
  }
  return color;
}
