// the rendered picture of linear light stretched over its place in the window, as blocks, tone
// mapped and cut to few colors
// srgb.wgsl, with `encode` and `decode`, tonemap.wgsl, with `tonemap`, and HAZE_CELL are put
// ahead of this source when loaded

struct Present {
  // x the steps each channel is cut to if two or more, y one when the target encodes srgb
  // itself, z how much what lights itself spills over, w how much the corners darken
  options: vec4<f32>,
  // x how far the picture is blurred, y what the light is multiplied by before it is tone
  // mapped
  more: vec4<f32>,
  // the Gaussian the glow map is blurred by: xy the falloff across and down, a cell `i` off
  // weighing exp(-falloff i^2), and zw the cells read to each side across and down
  spread: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> present: Present;
// what lights itself in each cell of HAZE_CELL of the picture's pixels a side, with one cell
// more all around, blurred
@group(0) @binding(3) var haze: texture_2d<f32>;
@group(0) @binding(4) var haze_sampler: sampler;
// the glow map as blurred so far, for a pass that blurs it along one axis
@group(0) @binding(5) var unspread: texture_2d<f32>;

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

// a cell of the glow map: the mean over its pixels of each one's color by how much it lights
// itself. what a cell holds past the picture's edge is dark
@fragment
fn glow_fragment(@builtin(position) cell: vec4<f32>) -> @location(0) vec4<f32> {
  let last = vec2<i32>(textureDimensions(picture)) - 1;
  let corner = (vec2<i32>(cell.xy) - 1) * HAZE_CELL;
  var lit = vec3<f32>(0.0);
  for (var x = 0; x < HAZE_CELL; x++) {
    for (var y = 0; y < HAZE_CELL; y++) {
      let at = corner + vec2<i32>(x, y);
      if all(at <= last) {
        let pixel = textureLoad(picture, at, 0);
        lit += pixel.rgb * pixel.a;
      }
    }
  }
  return vec4<f32>(lit / f32(HAZE_CELL * HAZE_CELL), 1.0);
}

// a cell of `unspread` blurred along `step` by the Gaussian, reading `taps` cells to each side.
// what lies past the map is dark
fn blurred(cell: vec2<i32>, step: vec2<i32>, falloff: f32, taps: i32) -> vec4<f32> {
  let last = vec2<i32>(textureDimensions(unspread)) - 1;
  var lit = textureLoad(unspread, cell, 0).rgb;
  var counted = 1.0;
  for (var i = 1; i <= taps; i++) {
    let weight = exp(-falloff * f32(i * i));
    for (var side = -1; side <= 1; side += 2) {
      let at = cell + step * i * side;
      if all(at >= vec2<i32>(0)) && all(at <= last) {
        lit += textureLoad(unspread, at, 0).rgb * weight;
      }
    }
    counted += weight * 2.0;
  }
  return vec4<f32>(lit / counted, 1.0);
}

@fragment
fn spread_across(@builtin(position) cell: vec4<f32>) -> @location(0) vec4<f32> {
  return blurred(vec2<i32>(cell.xy), vec2<i32>(1, 0), present.spread.x, i32(present.spread.z));
}

@fragment
fn spread_down(@builtin(position) cell: vec4<f32>) -> @location(0) vec4<f32> {
  return blurred(vec2<i32>(cell.xy), vec2<i32>(0, 1), present.spread.y, i32(present.spread.w));
}

// a number from zero to one out of a pixel, the same every time for the same pixel
fn noise(pixel: vec2<u32>, seed: u32) -> f32 {
  // the pcg hash
  let state = (pixel.x * 1973u + pixel.y * 9277u + seed * 26699u) * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return f32((word >> 22u) ^ word) / 4294967296.0;
}

// the picture at a place as it is shown: its light and its haze, exposed, tone mapped and
// sRGB-encoded
fn shown(at: vec2<f32>) -> vec3<f32> {
  var light = textureSampleLevel(picture, picture_sampler, at, 0.0).rgb;
  if present.options.z > 0.0 {
    // a mix of the four blurred cells around the pixel
    let size = vec2<f32>(textureDimensions(picture));
    let cells = vec2<f32>(textureDimensions(haze));
    let middle = (at * size / f32(HAZE_CELL) + 1.0) / cells;
    light += textureSampleLevel(haze, haze_sampler, middle, 0.0).rgb * present.options.z;
  }
  return encode(tonemap(light * present.more.y));
}

@fragment
fn fragment(spread: Spread) -> @location(0) vec4<f32> {
  let size = vec2<f32>(textureDimensions(picture));
  // how far apart the picture's pixels are in `spread.at`
  let pixel = 1.0 / size;
  var color = shown(spread.at);
  if present.more.x > 0.0 {
    // toward the mean of the pixel and the eight around it, the nearer ones counting more
    var around = color * 4.0;
    for (var x = -1; x <= 1; x++) {
      for (var y = -1; y <= 1; y++) {
        if x != 0 || y != 0 {
          let weight = select(1.0, 2.0, x == 0 || y == 0);
          let aside = vec2<f32>(f32(x), f32(y)) * pixel;
          around += shown(spread.at + aside) * weight;
        }
      }
    }
    color = mix(color, around / 16.0, present.more.x);
  }
  // darker toward the corners, by how far out along the picture's own shape
  let out = (spread.at - 0.5) * 2.0;
  color *= 1.0 - present.options.w * dot(out, out) * 0.5;
  let steps = present.options.x;
  if steps > 1.0 {
    // the dither follows the picture's own pixels, so its pattern is as blocky as they are
    let cell = vec2<u32>(spread.at * size);
    color = floor(color * (steps - 1.0) + threshold(cell)) / (steps - 1.0);
  } else {
    // a triangular dither one step of eight bits wide, which keeps dark gradients from
    // banding when they are stored
    let target_pixel = vec2<u32>(spread.clip.xy);
    let spread_by = noise(target_pixel, 0u) + noise(target_pixel, 1u) - 1.0;
    color = clamp(color + spread_by / 255.0, vec3<f32>(0.0), vec3<f32>(1.0));
  }
  if present.options.y != 0.0 {
    color = decode(color);
  }
  return vec4<f32>(color, 1.0);
}
