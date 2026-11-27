// the samples of each pixel of the picture averaged into one. a sample counts for less the
// brighter it is, so that a bright highlight on an edge does not step along it

@group(0) @binding(0) var samples: texture_multisampled_2d<f32>;

// one triangle that covers the viewport
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
  let at = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
  return vec4<f32>(at * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
  let at = vec2<i32>(pixel.xy);
  let count = i32(textureNumSamples(samples));
  var mapped = vec3<f32>(0.0);
  var own = 0.0;
  for (var index = 0; index < count; index++) {
    let sample = textureLoad(samples, at, index);
    mapped += sample.rgb / (1.0 + max(sample.r, max(sample.g, sample.b)));
    own += sample.a;
  }
  mapped /= f32(count);
  // the weighting undone, which a mean below one always allows
  let light = mapped / (1.0 - max(mapped.r, max(mapped.g, mapped.b)));
  return vec4<f32>(light, own / f32(count));
}
