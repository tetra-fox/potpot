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
  // x how much it mirrors, y how far it is metal, z how far its highlight is spun
  @location(8) finish: vec4<f32>,
}

struct Lit {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) color: vec4<f32>,
  @location(3) highlight: vec4<f32>,
  @location(4) finish: vec4<f32>,
  // where the object stands and the way its own z axis points, for what is spun around it
  @location(5) center: vec3<f32>,
  @location(6) axis: vec3<f32>,
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
  lit.finish = placed.finish;
  lit.center = placed.model_3.xyz;
  lit.axis = placed.model_2.xyz;
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
  // spread wide enough that a shadow's edge is a few pixels of the picture
  var share = 0.0;
  for (var x = -2; x <= 2; x++) {
    for (var y = -2; y <= 2; y++) {
      let aside = vec2<f32>(f32(x), f32(y)) * globals.shadows.y * 2.5;
      share += textureSampleCompareLevel(shadow_map, shadow_sampler, at + aside, depth);
    }
  }
  return share / 25.0;
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

// the room around the panel as a mirror would show it, by the way a reflection leaves: a
// soft window around the light, a dimmer one across from it, a pale band where the walls
// meet the eye, and under that the lit panel itself. `normal` is the mirror's own facing,
// which says what side of the room it looks at when the reflection runs straight down
fn room(leaving: vec3<f32>, normal: vec3<f32>) -> f32 {
  let window = smoothstep(0.72, 0.9, dot(leaving, globals.toward_light.xyz));
  let across = vec3<f32>(-globals.toward_light.xy, globals.toward_light.z);
  let second = smoothstep(0.8, 0.95, dot(leaving, normalize(across))) * 0.25;
  let band = smoothstep(-0.25, 0.1, leaving.z) * (1.0 - smoothstep(0.1, 0.5, leaving.z));
  // the walls are brighter on the side the light is on
  let side = 0.5 + 0.5 * dot(normalize(normal.xy + vec2<f32>(0.0001)), normalize(globals.toward_light.xy));
  let ceiling = smoothstep(0.3, 1.0, leaving.z) * 0.12;
  let panel = (1.0 - smoothstep(-0.35, 0.0, leaving.z)) * mix(0.03, 0.2, side * side);
  return window + second + band * mix(0.12, 0.5, side) + ceiling + panel + 0.02;
}

// the highlight of a surface brushed in circles around an axis: bright where the brushing
// runs square to the way the light glances off, which makes wedges out from the middle
fn spun_shine(world: vec3<f32>, center: vec3<f32>, axis: vec3<f32>, halfway: vec3<f32>, tightness: f32) -> f32 {
  let turn = normalize(axis);
  let out = world - center;
  let radial = out - turn * dot(out, turn);
  if dot(radial, radial) < 0.000001 {
    return 0.0;
  }
  let along = normalize(cross(turn, radial));
  let slant = dot(along, halfway);
  return pow(sqrt(max(1.0 - slant * slant, 0.0)), tightness);
}

// the picture holds srgb-encoded color, so that cutting it to few steps later spreads them
// the way the eye does
@fragment
fn fragment(lit: Lit) -> @location(0) vec4<f32> {
  let normal = normalize(lit.normal);
  let toward_light = globals.toward_light.xyz;
  let toward_viewer = globals.toward_viewer.xyz;
  let facing = max(dot(normal, toward_light), 0.0);
  let share = lit_share(lit.world, facing);
  let seen = max(dot(normal, toward_viewer), 0.0);
  let mirrors = lit.finish.x;
  let metal = lit.finish.y;
  let spun = lit.finish.z;

  let surface = decode(lit.color.rgb);
  // more of the room's own light comes from the side the light is on than from under
  let around = mix(0.55, 1.0, 0.5 + 0.5 * dot(normal, toward_light));
  let lighting = globals.ambient.rgb * around + globals.light_color.rgb * facing * share;
  let glow = lit.color.a;
  var color = surface * mix(lighting, vec3<f32>(1.0), glow) * (1.0 - 0.7 * metal);

  // metal tints what it throws back, anything else throws it back white
  let tint = mix(vec3<f32>(1.0), surface, metal);
  let halfway = normalize(toward_light + toward_viewer);
  var shine = pow(max(dot(normal, halfway), 0.0), lit.highlight.y);
  if spun > 0.0 {
    let key = spun_shine(lit.world, lit.center, lit.axis, halfway, lit.highlight.y);
    // the room's second window makes a fainter pair of wedges across the first
    let across = normalize(vec3<f32>(-toward_light.xy, toward_light.z) + toward_viewer);
    let fill = spun_shine(lit.world, lit.center, lit.axis, across, lit.highlight.y) * 0.35;
    shine = mix(shine, (key + fill) * (0.35 + 0.65 * facing), spun);
    color += surface * (key + fill) * 0.12 * spun;
  }
  let lit_side = step(0.0001, facing) * mix(0.35, 1.0, share);
  color += globals.light_color.rgb * tint * shine * lit.highlight.x * lit_side;

  // a mirror shows more the more it is seen edge-on, and metal shows it from any side
  let edge_on = pow(1.0 - seen, 5.0);
  let base = mix(0.05, 0.75, metal);
  let mirrored = room(reflect(-toward_viewer, normal), normal) * (base + (1.0 - base) * edge_on);
  color += tint * mirrored * mirrors * mix(0.6, 1.0, share);

  return vec4<f32>(encode(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
