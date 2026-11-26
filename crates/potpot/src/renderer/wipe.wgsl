// triangles that put what they cover of the picture back to what a picture starts from: black,
// as far off as anything gets

@vertex
fn vertex(@location(0) at: vec2<f32>) -> @builtin(position) vec4<f32> {
  return vec4<f32>(at, 1.0, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
  return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
