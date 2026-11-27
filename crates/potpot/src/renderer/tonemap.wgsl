// the Khronos PBR Neutral tone map, from linear light to linear light from zero to one. it
// leaves base colors under unit white light as they are, less the reflectance of 0.04 every
// dielectric adds, and rolls everything brighter off toward white

fn tonemap(light: vec3<f32>) -> vec3<f32> {
  let start = 0.8 - 0.04;
  let desaturation = 0.15;
  let least = min(light.r, min(light.g, light.b));
  let offset = select(0.04, least - 6.25 * least * least, least < 0.08);
  var color = light - offset;
  let peak = max(color.r, max(color.g, color.b));
  if peak < start {
    return color;
  }
  let d = 1.0 - start;
  let new_peak = 1.0 - d * d / (peak + d - start);
  color *= new_peak / peak;
  let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
  return mix(color, vec3<f32>(new_peak), g);
}
