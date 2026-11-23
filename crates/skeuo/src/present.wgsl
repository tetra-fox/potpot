// the rendered picture stretched over its place in the window, as blocks, and cut to few colors

struct Present {
  // x the steps each channel is cut to or zero, y one when the target encodes srgb itself,
  // z how much the bright parts spill over, w how much the corners darken
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
  if present.options.z > 0.0 {
    // what is as bright as a color gets, gathered from a spiral of places around the
    // pixel, so that the haze has no shape of its own
    let pixel = 1.0 / vec2<f32>(textureDimensions(picture));
    var haze = vec3<f32>(0.0);
    const PLACES: i32 = 40;
    for (var step = 0; step < PLACES; step++) {
      let out = sqrt((f32(step) + 0.5) / f32(PLACES));
      let turn = f32(step) * 2.3999632;
      let aside = vec2<f32>(cos(turn), sin(turn)) * out * 13.0 * pixel;
      let near = textureSampleLevel(picture, picture_sampler, spread.at + aside, 0.0).rgb;
      // by how bright it is at its brightest, so that a strong color spills as itself,
      // and less the further it has to come
      let bright = max(max(near.r, near.g), near.b);
      haze += near * smoothstep(0.965, 1.0, bright) * (1.0 - 0.6 * out);
    }
    color += haze / f32(PLACES) * 1.6 * present.options.z;
  }
  // darker toward the corners, by how far out along the picture's own shape
  let out = (spread.at - 0.5) * 2.0;
  color *= 1.0 - present.options.w * dot(out, out) * 0.5;
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
