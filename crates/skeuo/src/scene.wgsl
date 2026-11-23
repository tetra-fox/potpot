// the panel's objects under one light, first into the shadow map and then into the picture

struct Globals {
  view: mat4x4<f32>,
  light_view: mat4x4<f32>,
  toward_light: vec4<f32>,
  toward_viewer: vec4<f32>,
  light_color: vec4<f32>,
  ambient: vec4<f32>,
  // x is one when shadow edges are softened, y the size of a shadow map texel
  shadows: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

struct Corner {
  @location(0) position: vec3<f32>,
  @location(1) normal: vec3<f32>,
}

struct Placed {
  @location(2) model_0: vec4<f32>,
  @location(3) model_1: vec4<f32>,
  @location(4) model_2: vec4<f32>,
  @location(5) model_3: vec4<f32>,
  // rgb and the glow
  @location(6) color: vec4<f32>,
  // x the highlight's strength, y its tightness
  @location(7) highlight: vec4<f32>,
}

struct Lit {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) color: vec4<f32>,
  @location(3) highlight: vec4<f32>,
}

fn model(placed: Placed) -> mat4x4<f32> {
  return mat4x4<f32>(placed.model_0, placed.model_1, placed.model_2, placed.model_3);
}

@vertex
fn shadow_vertex(corner: Corner, placed: Placed) -> @builtin(position) vec4<f32> {
  return globals.light_view * model(placed) * vec4<f32>(corner.position, 1.0);
}

@vertex
fn vertex(corner: Corner, placed: Placed) -> Lit {
  let world = model(placed) * vec4<f32>(corner.position, 1.0);
  var lit: Lit;
  lit.clip = globals.view * world;
  lit.world = world.xyz;
  lit.normal = (model(placed) * vec4<f32>(corner.normal, 0.0)).xyz;
  lit.color = placed.color;
  lit.highlight = placed.highlight;
  return lit;
}

// how much of the light reaches a point, from none to all of it
fn lit_share(world: vec3<f32>, facing: f32) -> f32 {
  let seen = globals.light_view * vec4<f32>(world, 1.0);
  let at = vec2<f32>(seen.x * 0.5 + 0.5, 0.5 - seen.y * 0.5);
  // a surface nearly edge-on to the light covers many depths in one texel, so it is
  // compared from further off
  let depth = seen.z - mix(0.012, 0.003, facing);
  if globals.shadows.x == 0.0 {
    return textureSampleCompareLevel(shadow_map, shadow_sampler, at, depth);
  }
  var share = 0.0;
  for (var x = -1; x <= 1; x++) {
    for (var y = -1; y <= 1; y++) {
      let aside = vec2<f32>(f32(x), f32(y)) * globals.shadows.y * 1.5;
      share += textureSampleCompareLevel(shadow_map, shadow_sampler, at + aside, depth);
    }
  }
  return share / 9.0;
}

fn encode(linear: vec3<f32>) -> vec3<f32> {
  let low = linear * 12.92;
  let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
  return select(high, low, linear <= vec3<f32>(0.0031308));
}

fn decode(encoded: vec3<f32>) -> vec3<f32> {
  let low = encoded / 12.92;
  let high = pow((encoded + 0.055) / 1.055, vec3<f32>(2.4));
  return select(high, low, encoded <= vec3<f32>(0.04045));
}

// the picture holds srgb-encoded color, so that cutting it to few steps later spreads them
// the way the eye does
@fragment
fn fragment(lit: Lit) -> @location(0) vec4<f32> {
  let normal = normalize(lit.normal);
  let toward_light = globals.toward_light.xyz;
  let facing = max(dot(normal, toward_light), 0.0);
  let share = lit_share(lit.world, facing);

  let halfway = normalize(toward_light + globals.toward_viewer.xyz);
  let shine = pow(max(dot(normal, halfway), 0.0), lit.highlight.y) * lit.highlight.x;

  let surface = decode(lit.color.rgb);
  let lighting = globals.ambient.rgb + globals.light_color.rgb * facing * share;
  let glow = lit.color.a;
  var color = surface * mix(lighting, vec3<f32>(1.0), glow);
  color += globals.light_color.rgb * shine * share * step(0.0001, facing);
  return vec4<f32>(encode(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
