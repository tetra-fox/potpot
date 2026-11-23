// the rendered picture stretched over its place in the window, as blocks, and cut to few colors

struct Present {
  // x the steps each channel is cut to or zero, y one when the target encodes srgb itself
  options: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> present: Present;

struct Spread {
  @builtin(position) clip: vec4<f32>,
  @location(0) at: vec2<f32>,
}

// one triangle that covers the viewport
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Spread {
  let at = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
  var spread: Spread;
  spread.clip = vec4<f32>(at * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
  spread.at = at;
  return spread;
}

// a 4 by 4 ordered dither, from zero to just under one
fn threshold(pixel: vec2<u32>) -> f32 {
  let bayer = array<f32, 16>(
    0.0, 8.0, 2.0, 10.0,
    12.0, 4.0, 14.0, 6.0,
    3.0, 11.0, 1.0, 9.0,
    15.0, 7.0, 13.0, 5.0,
  );
  return bayer[(pixel.y % 4u) * 4u + pixel.x % 4u] / 16.0;
}

@fragment
fn fragment(spread: Spread) -> @location(0) vec4<f32> {
  var color = textureSampleLevel(picture, picture_sampler, spread.at, 0.0).rgb;
  let steps = present.options.x;
  if steps > 0.0 {
    // the dither follows the picture's own pixels, so its pattern is as blocky as they are
    let pixel = vec2<u32>(spread.at * vec2<f32>(textureDimensions(picture)));
    color = floor(color * (steps - 1.0) + threshold(pixel)) / (steps - 1.0);
  }
  if present.options.y != 0.0 {
    let low = color / 12.92;
    let high = pow((color + 0.055) / 1.055, vec3<f32>(2.4));
    color = select(high, low, color <= vec3<f32>(0.04045));
  }
  return vec4<f32>(color, 1.0);
}
