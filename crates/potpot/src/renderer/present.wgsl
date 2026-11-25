// the rendered picture stretched over its place in the window, as blocks, and cut to few colors
// `decode` is in srgb.wgsl, which is put ahead of this source when it is loaded

struct Present {
  // x the steps each channel is cut to if two or more, y one when the target encodes srgb
  // itself, z how much what lights itself spills over, w how much the corners darken
  options: vec4<f32>,
  // x how far the picture is blurred
  more: vec4<f32>,
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
fn threshold(cell: vec2<u32>) -> f32 {
  let bayer = array<f32, 16>(
    0.0, 8.0, 2.0, 10.0,
    12.0, 4.0, 14.0, 6.0,
    3.0, 11.0, 1.0, 9.0,
    15.0, 7.0, 13.0, 5.0,
  );
  return bayer[(cell.y % 4u) * 4u + cell.x % 4u] / 16.0;
}

@fragment
fn fragment(spread: Spread) -> @location(0) vec4<f32> {
  let size = vec2<f32>(textureDimensions(picture));
  // how far apart the picture's pixels are in `spread.at`
  let pixel = 1.0 / size;
  var color = textureSampleLevel(picture, picture_sampler, spread.at, 0.0).rgb;
  if present.more.x > 0.0 {
    // toward the mean of the pixel and the eight around it, the nearer ones counting more
    var around = color * 4.0;
    for (var x = -1; x <= 1; x++) {
      for (var y = -1; y <= 1; y++) {
        if x != 0 || y != 0 {
          let weight = select(1.0, 2.0, x == 0 || y == 0);
          let aside = vec2<f32>(f32(x), f32(y)) * pixel;
          around += textureSampleLevel(picture, picture_sampler, spread.at + aside, 0.0).rgb * weight;
        }
      }
    }
    color = mix(color, around / 16.0, present.more.x);
  }
  if present.options.z > 0.0 {
    // what lights itself, gathered from a spiral of places around the pixel, so that the
    // haze has no shape of its own
    var haze = vec3<f32>(0.0);
    const PLACES: i32 = 40;
    for (var step = 0; step < PLACES; step++) {
      let out = sqrt((f32(step) + 0.5) / f32(PLACES));
      let turn = f32(step) * 2.3999632;
      let aside = vec2<f32>(cos(turn), sin(turn)) * out * 13.0 * pixel;
      let near = textureSampleLevel(picture, picture_sampler, spread.at + aside, 0.0);
      // less the further it has to come
      haze += near.rgb * near.a * (1.0 - 0.6 * out);
    }
    color += haze / f32(PLACES) * 1.6 * present.options.z;
  }
  // darker toward the corners, by how far out along the picture's own shape
  let out = (spread.at - 0.5) * 2.0;
  color *= 1.0 - present.options.w * dot(out, out) * 0.5;
  let steps = present.options.x;
  if steps > 1.0 {
    // the dither follows the picture's own pixels, so its pattern is as blocky as they are
    let cell = vec2<u32>(spread.at * size);
    color = floor(color * (steps - 1.0) + threshold(cell)) / (steps - 1.0);
  }
  if present.options.y != 0.0 {
    color = decode(color);
  }
  return vec4<f32>(color, 1.0);
}
