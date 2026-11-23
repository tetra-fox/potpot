// the panel's objects under one light, first into the shadow map and then into the picture

struct Globals {
  view: mat4x4<f32>,
  light_view: mat4x4<f32>,
  toward_light: vec4<f32>,
  toward_viewer: vec4<f32>,
  light_color: vec4<f32>,
  ambient: vec4<f32>,
  // x is one when shadow edges are softened, y the size of a shadow map texel, z how many
  // of the lamps are lit
  // and w how much the light all around is kept out of corners
  shadows: vec4<f32>,
  // panel points to clip space from straight above
  overhead: mat4x4<f32>,
  // the panel's width and height, and the lowest and highest anything on it reaches
  panel: vec4<f32>,
  lamps: array<Lamp, 32>,
}

// a small light on the panel
struct Lamp {
  // where it is, and in w how far its light gets
  at: vec4<f32>,
  color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var height_map: texture_depth_2d;

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
  // x the highlight's strength, y its tightness, z how far out its glow fades or zero
  @location(7) highlight: vec4<f32>,
  // x how much it mirrors, y how far it is metal, z how far its highlight is spun, w the
  // size of the pane it is or zero
  @location(8) finish: vec4<f32>,
  // x how grainy it is, y how deeply brushed
  @location(9) texture: vec4<f32>,
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
  @location(7) texture: vec4<f32>,
}

fn model(placed: Placed) -> mat4x4<f32> {
  return mat4x4<f32>(placed.model_0, placed.model_1, placed.model_2, placed.model_3);
}

@vertex
fn shadow_vertex(corner: Corner, placed: Placed) -> @builtin(position) vec4<f32> {
  return globals.light_view * model(placed) * vec4<f32>(corner.position, 1.0);
}

@vertex
fn height_vertex(corner: Corner, placed: Placed) -> @builtin(position) vec4<f32> {
  return globals.overhead * model(placed) * vec4<f32>(corner.position, 1.0);
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
  lit.texture = placed.texture;
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

// how high the highest thing over a point of the panel is
fn height_over(at: vec2<f32>) -> f32 {
  let size = vec2<f32>(textureDimensions(height_map));
  let texel = clamp(at / globals.panel.xy * size, vec2<f32>(0.0), size - 1.0);
  let depth = textureLoad(height_map, vec2<i32>(texel), 0);
  return globals.panel.w - depth * (globals.panel.w - globals.panel.z);
}

// how much of the light all around reaches a point, from all of it in the open down to
// little in a corner: less for everything near that stands higher than the point, and less
// the nearer and higher it stands
fn open_share(world: vec3<f32>) -> f32 {
  var closed = 0.0;
  var weight = 0.0;
  for (var ring = 0; ring < 3; ring++) {
    let reach = 3.0 + f32(ring * ring) * 5.5 + f32(ring) * 2.0;
    for (var step = 0; step < 10; step++) {
      // each ring turned against the last, so that their points do not line up
      let turn = (f32(step) + f32(ring) * 0.37) * 0.6283185;
      let over = height_over(world.xy + vec2<f32>(cos(turn), sin(turn)) * reach);
      let rise = over - world.z - 0.6;
      let near = 1.0 / (1.0 + reach * 0.12);
      closed += clamp(rise / reach, 0.0, 1.0) * near;
      weight += near;
    }
  }
  return 1.0 - clamp(closed / weight * 2.3, 0.0, 0.92);
}

// a number from zero to one out of a place, the same every time for the same place
fn scatter(at: vec2<f32>) -> f32 {
  var mixed = fract(vec3<f32>(at.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
  mixed += dot(mixed, mixed.yzx + 33.33);
  return fract((mixed.x + mixed.y) * mixed.z);
}

// scatter smoothed between whole places, so that it rolls instead of jumping
fn rolling(at: vec2<f32>) -> f32 {
  let whole = floor(at);
  let part = fract(at);
  let ease = part * part * (3.0 - 2.0 * part);
  return mix(
    mix(scatter(whole), scatter(whole + vec2<f32>(1.0, 0.0)), ease.x),
    mix(scatter(whole + vec2<f32>(0.0, 1.0)), scatter(whole + vec2<f32>(1.0, 1.0)), ease.x),
    ease.y,
  );
}

// the way a surface faces once its texture has bent it, and in w how much lighter or
// darker the texture makes that spot. the texture is made from where the spot is on the
// panel: grain in every direction at about the size of a pixel, and brushing in long
// streaks along x that only tilt the surface across themselves
fn textured(normal: vec3<f32>, world: vec3<f32>, texture: vec4<f32>) -> vec4<f32> {
  var bent = normal;
  var shade = 0.0;
  if texture.x > 0.0 {
    let at = world.xy * 1.1 + world.z * 0.7;
    let fine = vec2<f32>(rolling(at), rolling(at + 37.0)) - 0.5;
    let coarse = vec2<f32>(rolling(at * 0.37 + 11.0), rolling(at * 0.37 + 71.0)) - 0.5;
    bent += vec3<f32>(fine * 0.16 + coarse * 0.1, 0.0) * texture.x;
    shade += (fine.x + coarse.y) * 0.035 * texture.x;
  }
  if texture.y > 0.0 {
    let streak = rolling(vec2<f32>(world.x * 0.01, world.y * 3.1)) - 0.5;
    let hair = rolling(vec2<f32>(world.x * 0.04 + 9.0, world.y * 7.3)) - 0.5;
    bent.y += (streak * 0.14 + hair * 0.1) * texture.y;
    shade += (streak * 0.045 + hair * 0.035) * texture.y;
  }
  return vec4<f32>(normalize(bent), shade);
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
  let rough = textured(normalize(lit.normal), lit.world, lit.texture);
  let normal = rough.xyz;
  let toward_light = globals.toward_light.xyz;
  let toward_viewer = globals.toward_viewer.xyz;
  let facing = max(dot(normal, toward_light), 0.0);
  let share = lit_share(lit.world, facing);
  let seen = max(dot(normal, toward_viewer), 0.0);
  let mirrors = lit.finish.x;
  let metal = lit.finish.y;
  let spun = lit.finish.z;

  let surface = decode(lit.color.rgb) * (1.0 + rough.w);
  // more of the room's own light comes from the side the light is on than from under
  let around = mix(0.55, 1.0, 0.5 + 0.5 * dot(normal, toward_light));
  let open = mix(1.0, open_share(lit.world), globals.shadows.w);
  // the light is nearer one edge of the panel than the other
  let from_middle = (lit.world.xy - globals.panel.xy * 0.5) / max(globals.panel.x, globals.panel.y);
  let nearness = 1.0 + globals.light_color.w * 2.0 * dot(from_middle, normalize(toward_light.xy + vec2<f32>(0.0, 0.0001)));
  let direct = globals.light_color.rgb * nearness;
  let lighting = globals.ambient.rgb * around * open + direct * facing * share * mix(1.0, open, 0.35);
  var lighting_with_lamps = lighting;
  for (var index = 0; index < i32(globals.shadows.z); index++) {
    let lamp = globals.lamps[index];
    let toward = lamp.at.xyz - lit.world;
    let away = length(toward);
    let fade = clamp(1.0 - away / lamp.at.w, 0.0, 1.0);
    // a surface turned from the lamp still catches a little of it off everything else
    let catches = 0.25 + 0.75 * max(dot(normal, toward / max(away, 0.0001)), 0.0);
    lighting_with_lamps += lamp.color.rgb * fade * fade * catches;
  }
  let glow = lit.color.a;
  var color = surface * mix(lighting_with_lamps, vec3<f32>(1.0), glow) * (1.0 - 0.7 * metal);
  if lit.highlight.z > 0.0 {
    // lit from inside by one light under the middle: brightest and whitest over it, and
    // still the color, dimmer, out at the edges
    let turn = normalize(lit.axis);
    let out = lit.world - lit.center;
    let aside = out - turn * dot(out, turn);
    let near = exp(-dot(aside, aside) / (lit.highlight.z * lit.highlight.z));
    let inside = glow * (0.42 + 0.58 * near);
    color = surface * mix(lighting_with_lamps, vec3<f32>(1.1), inside);
    color += vec3<f32>(near * near * near * 0.3 * glow);
    // the light also runs along the inside of the walls and comes out where the top
    // turns over into them, which rims the lit thing in its color
    let turned = 1.0 - clamp(normalize(lit.normal).z, 0.0, 1.0);
    color += surface * sqrt(turned) * glow * 0.8;
  }

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
  color += direct * tint * shine * lit.highlight.x * lit_side;

  // a mirror shows more the more it is seen edge-on, and metal shows it from any side
  let edge_on = pow(1.0 - seen, 5.0);
  let base = mix(0.05, 0.75, metal);
  let leaving = reflect(-toward_viewer, normal);
  let mirrored = room(leaving, normal) * (base + (1.0 - base) * edge_on);
  // metal shows the room in its colors: a blue sky over a pale horizon over brown ground
  let sky = mix(vec3<f32>(0.62, 0.48, 0.36), vec3<f32>(0.66, 0.8, 1.0), smoothstep(-0.2, 0.35, leaving.z));
  let hue = mix(sky, vec3<f32>(1.0), smoothstep(0.75, 0.95, dot(leaving, toward_light)));
  color += tint * mix(vec3<f32>(1.0), hue * 1.15, metal) * mirrored * mirrors * mix(0.6, 1.0, share) * mix(0.5, 1.0, open);

  // a pane shows a sheen over the corner of it nearest the light, cut off along a slant,
  // the look of a flat piece of glass with a window somewhere behind the viewer
  if lit.finish.w > 0.0 {
    let from_middle = (lit.world.xy - lit.center.xy) / lit.finish.w;
    let toward = dot(from_middle, normalize(toward_light.xy + vec2<f32>(0.0, 0.0001)));
    let sheen = smoothstep(0.06, 0.1, toward) * (0.22 - 0.2 * clamp(toward, 0.0, 1.0));
    color += vec3<f32>(sheen) * max(normalize(lit.normal).z, 0.0);
  }

  return vec4<f32>(encode(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
