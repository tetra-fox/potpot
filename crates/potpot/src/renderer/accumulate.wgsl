// the picture's running mean over the samples drawn of it, which the present pass shows. each
// pixel keeps its own count, so that a part of the picture drawn afresh starts over alone
// SETTLED is put ahead of this source when it is loaded

// the sample just drawn and resolved
@group(0) @binding(0) var drawn: texture_2d<f32>;
// the mean and the count of samples in it as they were before this sample
@group(0) @binding(1) var held: texture_2d<f32>;
@group(0) @binding(2) var held_count: texture_2d<f32>;

struct Accumulated {
  @location(0) mean: vec4<f32>,
  @location(1) count: vec4<f32>,
}

// one triangle that covers the viewport
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
  let at = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
  return vec4<f32>(at * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

// the triangles over what was drawn afresh, as the wipe takes them
@vertex
fn wiped_vertex(@location(0) at: vec2<f32>) -> @builtin(position) vec4<f32> {
  return vec4<f32>(at, 0.0, 1.0);
}

// the sample added to the mean, until the mean holds SETTLED
@fragment
fn add(@builtin(position) pixel: vec4<f32>) -> Accumulated {
  let at = vec2<i32>(pixel.xy);
  let mean = textureLoad(held, at, 0);
  let count = textureLoad(held_count, at, 0).r;
  var out: Accumulated;
  if count >= f32(SETTLED) {
    out.mean = mean;
    out.count = vec4<f32>(count);
    return out;
  }
  out.mean = mean + (textureLoad(drawn, at, 0) - mean) / (count + 1.0);
  out.count = vec4<f32>(count + 1.0);
  return out;
}

// the mean as it was
@fragment
fn keep(@builtin(position) pixel: vec4<f32>) -> Accumulated {
  let at = vec2<i32>(pixel.xy);
  var out: Accumulated;
  out.mean = textureLoad(held, at, 0);
  out.count = vec4<f32>(textureLoad(held_count, at, 0).r);
  return out;
}

// the sample as the first of a new mean
@fragment
fn first(@builtin(position) pixel: vec4<f32>) -> Accumulated {
  var out: Accumulated;
  out.mean = textureLoad(drawn, vec2<i32>(pixel.xy), 0);
  out.count = vec4<f32>(1.0);
  return out;
}
