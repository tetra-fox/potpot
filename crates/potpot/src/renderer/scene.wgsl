// the panel's objects under one light, first into the shadow map and then into the picture

struct Globals {
  view: mat4x4<f32>,
  light_view: mat4x4<f32>,
  toward_light: vec4<f32>,
  // and in w how far off the light and the viewer are for what shines, or zero for
  // infinitely far
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
  // LAMPS is put ahead of this source when it is loaded
  lamps: array<Lamp, LAMPS>,
}

// a small light on the panel
struct Lamp {
  // where it is, and in w how far its light gets
  at: vec4<f32>,
  // and in w how far from its middle the thing that shines reaches
  color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var height_map: texture_depth_2d;

// how many places between a point and a lamp are looked at for what blocks the lamp
const LAMP_STEPS: i32 = 12;

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
  // x how grainy it is, y how deeply brushed, z how worn
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
      let aside = vec2<f32>(f32(x), f32(y)) * globals.shadows.y * globals.toward_light.w;
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

// how much of a lamp's light gets to a point past what stands between the two, going by
// the height map along the straight line to the lamp. the lamp's own body is in that map,
// so the line is followed only up to the lamp's edge. what stands in the way has a softer
// edge the further it is from the point and the wider the lamp
fn lamp_share(world: vec3<f32>, lamp: Lamp) -> f32 {
  let toward = lamp.at.xyz - world;
  let away = length(toward.xy);
  // a texel of the height map can hold the point's own surface
  let texel = max(globals.panel.x, globals.panel.y) / f32(textureDimensions(height_map).x);
  let start = texel * 1.5;
  let end = away - lamp.color.w - texel;
  if end <= start {
    return 1.0;
  }
  // each point starts its steps somewhere else, which turns bands into grain
  let shift = scatter(world.xy * 7.0 + world.z);
  var blocked = 0.0;
  for (var step = 0; step < LAMP_STEPS; step++) {
    let along = mix(start, end, (f32(step) + shift) / f32(LAMP_STEPS));
    let line = world.z + toward.z * along / away;
    let over = height_over(world.xy + toward.xy / away * along);
    let soft = 1.0 + lamp.color.w * along / (away - along);
    blocked = max(blocked, clamp((over - line) / soft, 0.0, 1.0));
  }
  return 1.0 - blocked;
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
// panel: grain in every direction at about the size of a pixel, brushing in long streaks
// along x that only tilt the surface across themselves, and the marks of wear
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
  if texture.z > 0.0 {
    // slow blotches of grime, and here and there a patch of fine scratches running one of
    // three ways, each a hairline that catches the light
    let blotch = rolling(world.xy * 0.03 + 5.0) * 0.65 + rolling(world.xy * 0.11 + 31.0) * 0.35;
    shade -= smoothstep(0.4, 0.85, blotch) * 0.13 * texture.z;
    for (var way = 0; way < 3; way++) {
      let turn = 0.5 + f32(way) * 1.9;
      let across = vec2<f32>(cos(turn), sin(turn));
      let along = vec2<f32>(-across.y, across.x);
      let at = vec2<f32>(dot(world.xy, along) * 0.07, dot(world.xy, across) * 2.3) + f32(way) * 17.0;
      let worn = smoothstep(0.6, 0.8, rolling(world.xy * 0.023 + f32(way) * 43.0));
      let hairline = smoothstep(0.88, 0.97, rolling(at)) * worn * texture.z;
      bent += vec3<f32>(across * hairline * 0.12, 0.0);
      shade += hairline * 0.03;
    }
  }
  return vec4<f32>(normalize(bent), shade);
}

// the room around the panel as a mirror would show it, by the way a reflection leaves: a
// wide lamp on the light's side that reaches from well up the wall nearly to the ceiling,
// a small dim one across from it, a ceiling paler than the walls, a bright line where the
// walls meet the floor, and under that line a floor that is dark at its far edge and
// lighter toward the panel. `normal` is the mirror's own facing, which says what side of
// the room it looks at when the reflection runs straight up or down. `crisp` is how sharp
// the edges of all this are, from a blur to a clean cut, and `tinted` how far the room
// keeps its colors: a blue ceiling, a warm line and a brown floor
fn room(leaving: vec3<f32>, normal: vec3<f32>, crisp: f32, tinted: f32) -> vec3<f32> {
  let aside = normalize(leaving.xy + normal.xy * 0.02 + vec2<f32>(0.00001));
  let side = dot(aside, normalize(globals.toward_light.xy + vec2<f32>(0.0, 0.0001)));
  let high = leaving.z;
  let edge = mix(0.3, 0.025, crisp);
  let lamp = smoothstep(0.24 - edge, 0.24 + edge, high) * (1.0 - smoothstep(0.82 - edge, 0.82 + edge, high))
    * smoothstep(0.4 - edge * 2.0, 0.4 + edge * 2.0, side);
  let second = smoothstep(0.3 - edge, 0.3 + edge, high) * (1.0 - smoothstep(0.6 - edge, 0.6 + edge, high))
    * smoothstep(0.8 - edge, 0.8 + edge, -side) * 0.3;
  let ceiling = smoothstep(0.5, 1.0, high) * 0.42;
  // the walls and the line along their foot are brighter on the side the light is on
  let lit_side = 0.5 + 0.5 * side;
  let over = smoothstep(-edge * 0.4, edge * 0.4, high);
  let line = over * (1.0 - smoothstep(0.07, 0.07 + edge * 2.0, high)) * mix(0.3, 0.95, lit_side);
  let walls = over * mix(0.04, 0.12, lit_side);
  let floor = (1.0 - over) * mix(0.015, mix(0.05, 0.24, lit_side * lit_side), smoothstep(-0.05, -0.6, high));
  let shown = lamp + second + max(ceiling, walls) + line + floor + 0.015;
  let above = mix(vec3<f32>(1.0, 0.96, 0.88), vec3<f32>(0.6, 0.76, 1.0), smoothstep(0.05, 0.6, high));
  let hue = mix(vec3<f32>(0.72, 0.56, 0.42), mix(above, vec3<f32>(1.0), lamp), over);
  return shown * mix(vec3<f32>(1.0), hue * 1.12, tinted);
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
  // what lights a surface comes from one way everywhere, but what shines off it comes from
  // a light and a viewer at a place, so that the shine is somewhere and not all over
  var toward_viewer = globals.toward_viewer.xyz;
  var shining = toward_light;
  if globals.toward_viewer.w > 0.0 {
    let middle = vec3<f32>(globals.panel.xy * 0.5, 0.0);
    shining = normalize(middle + toward_light * globals.toward_viewer.w - lit.world);
    toward_viewer = normalize(middle + toward_viewer * globals.toward_viewer.w * 2.0 - lit.world);
  }
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
  // how sharply it mirrors goes with how tight its highlight is
  let crisp = clamp((log2(max(lit.highlight.y, 1.0)) - 4.0) / 3.6, 0.0, 1.0);
  let leaving = reflect(-toward_viewer, normal);
  var lighting_with_lamps = lighting;
  // the highlights the lamps make, and what a mirror shows of the lamps themselves
  var glints = vec3<f32>(0.0);
  var lamps_mirrored = vec3<f32>(0.0);
  for (var index = 0; index < i32(globals.shadows.z); index++) {
    let lamp = globals.lamps[index];
    let toward = lamp.at.xyz - lit.world;
    let away = length(toward);
    let fade = clamp(1.0 - away / lamp.at.w, 0.0, 1.0);
    if fade == 0.0 {
      continue;
    }
    let reaching = toward / max(away, 0.0001);
    let strength = lamp.color.rgb * fade * fade;
    let reached = lamp_share(lit.world, lamp);
    // a surface turned from the lamp or shadowed from it still catches a little of it off
    // everything else
    let catches = 0.25 + 0.75 * max(dot(normal, reaching), 0.0) * reached;
    lighting_with_lamps += strength * catches;
    // how wide the lamp looks from here
    let wide = atan2(lamp.color.w, away);
    if dot(normal, reaching) > 0.0 {
      // a lamp that looks wide makes a highlight wider than the surface alone would
      let spread = sqrt(2.0 / max(lit.highlight.y, 1.0)) + wide * 0.5;
      let tightness = 2.0 / (spread * spread);
      let between = normalize(reaching + toward_viewer);
      var glint = pow(max(dot(normal, between), 0.0), tightness);
      if spun > 0.0 {
        glint = mix(glint, spun_shine(lit.world, lit.center, lit.axis, between, tightness), spun);
      }
      glints += strength * glint * reached;
    }
    // the lamp in a mirror is a disc of that width, with an edge as soft as the mirror is
    // dull, and dimmer the more the edge spreads it
    let blur = mix(0.35, 0.02, crisp);
    let off = acos(clamp(dot(leaving, reaching), -1.0, 1.0));
    let disc = (1.0 - smoothstep(wide - blur, wide + blur, off)) * wide * wide / (wide * wide + blur * blur);
    lamps_mirrored += lamp.color.rgb * fade * disc * reached;
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
  let halfway = normalize(shining + toward_viewer);
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
  // what lights itself shows no lamp's shine over its own light
  color += glints * tint * lit.highlight.x * (1.0 - glow);

  // a mirror shows more the more it is seen edge-on, and metal shows it from any side
  let edge_on = pow(1.0 - seen, 5.0);
  let base = mix(0.05, 0.75, metal);
  let mirroring = (base + (1.0 - base) * edge_on) * mirrors;
  color += tint * room(leaving, normal, crisp, metal) * mirroring * mix(0.6, 1.0, share) * mix(0.5, 1.0, open);
  color += tint * lamps_mirrored * mirroring * (1.0 - glow);

  // a pane shows a sheen over the corner of it nearest the light, cut off along a slant,
  // the look of a flat piece of glass with a window somewhere behind the viewer
  if lit.finish.w > 0.0 {
    let from_middle = (lit.world.xy - lit.center.xy) / lit.finish.w;
    let toward = dot(from_middle, normalize(toward_light.xy + vec2<f32>(0.0, 0.0001)));
    let sheen = smoothstep(0.06, 0.1, toward) * (0.22 - 0.2 * clamp(toward, 0.0, 1.0));
    color += vec3<f32>(sheen) * max(normalize(lit.normal).z, 0.0);
  }

  // the picture's fourth channel says how far a spot lights itself, which is what spills
  // a haze later: paint under a bright light is as bright, and spills none
  return vec4<f32>(encode(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), clamp(glow, 0.0, 1.0));
}
