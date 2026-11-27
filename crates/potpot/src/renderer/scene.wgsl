// the panel's objects under one light: into the shadow map, into the maps of how high they
// are and of the light they give off, and then into the picture
// `encode` and `decode` are in srgb.wgsl, which is put ahead of this source when it is loaded

struct Globals {
  view: mat4x4<f32>,
  light_view: mat4x4<f32>,
  // and in w how wide shadow edges are: the shadow map texels between the places
  // `lit_share` looks at
  toward_light: vec4<f32>,
  // w is not read
  toward_viewer: vec4<f32>,
  // and in w how much of the light that comes off surfaces lands on what is next to them
  light_color: vec4<f32>,
  // x is one when shadow edges are softened, y the size of a shadow map texel, z how many
  // of the lamps are lit, and w how much the light all around is kept out of corners
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
  // the light at its radius, and in w that radius
  color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var height_map: texture_depth_2d;
// the light every surface gives off as seen from straight above, and in a how high it is
@group(1) @binding(3) var bounce_map: texture_2d<f32>;
// linear and clamped to the edge, for the bounce map and the dfg table
@group(1) @binding(4) var bounce_sampler: sampler;
// the scale and bias on f0 of the split-sum highlight, across by n.v and down by roughness
@group(1) @binding(5) var dfg_table: texture_2d<f32>;

const TAU: f32 = 6.2831853;
const PI: f32 = 3.14159265;
// the least perceptual roughness, under which a highlight is too small to sample
const LEAST_ROUGHNESS: f32 = 0.045;
// the reflectance seen square on of plastic, paint, glass and a clear coat
const DIELECTRIC: f32 = 0.04;
// the most a half float holds, with room to spare
const BRIGHTEST: f32 = 60000.0;
// the light that reaches every surface, shadowed or not
const AMBIENT: f32 = 0.25;
// how bright the room is drawn
const ROOM: f32 = 2.0;
// SHADOW_SPREAD, SHADOW_BIAS, OPEN_REACHES, OPEN_CLEAR and BOUNCE_REACHES are put ahead of this
// source when it is loaded
// how many places are in each ring around a point that is looked at for what stands over it.
// OPEN_REACHES is how far off each ring is
const OPEN_STEPS: i32 = 10;
// a turn over OPEN_STEPS. worked out from TAU it rounds one bit higher than this, which
// changes pixels
const OPEN_TURN: f32 = 0.6283185;
// how many places between a point and a lamp are looked at for what blocks the lamp
const LAMP_STEPS: i32 = 12;
// how many places are in each ring around a point that is looked at for the light that comes
// off it. BOUNCE_REACHES is how many texels off each ring is
const BOUNCE_STEPS: i32 = 8;

struct Corner {
  @location(0) position: vec3<f32>,
  @location(1) normal: vec3<f32>,
}

struct Placed {
  @location(2) model_0: vec4<f32>,
  @location(3) model_1: vec4<f32>,
  @location(4) model_2: vec4<f32>,
  @location(5) model_3: vec4<f32>,
  // rgb and the emission
  @location(6) color: vec4<f32>,
  // x how far it is metal, y its roughness, z its clear coat and w the coat's roughness
  @location(7) finish: vec4<f32>,
  // x how grainy it is, y how worn, z how far out its emission fades or zero. w is not read
  @location(8) texture: vec4<f32>,
  // x how strongly it is brushed along x, y how strongly spun around its own z axis
  @location(9) anisotropy: vec2<f32>,
}

struct Lit {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) color: vec4<f32>,
  @location(3) finish: vec4<f32>,
  @location(4) texture: vec4<f32>,
  // where the object stands and the way its own z axis points, for what is spun around it
  @location(5) center: vec3<f32>,
  @location(6) axis: vec3<f32>,
  @location(7) anisotropy: vec2<f32>,
  // `world` taken where the triangle covers the pixel, which is a place on the triangle for a
  // pixel covered in part too. the bounce map is read at it
  @location(8) @interpolate(perspective, centroid) on_triangle: vec3<f32>,
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

// one triangle over the whole of a map, as far off as anything gets, which puts what it is
// drawn over back to what a map starts from
@vertex
fn far_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
  let at = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
  return vec4<f32>(at * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 1.0, 1.0);
}

fn seen_through(view: mat4x4<f32>, corner: Corner, placed: Placed) -> Lit {
  let world = model(placed) * vec4<f32>(corner.position, 1.0);
  var lit: Lit;
  lit.clip = view * world;
  lit.world = world.xyz;
  lit.on_triangle = world.xyz;
  lit.normal = (model(placed) * vec4<f32>(corner.normal, 0.0)).xyz;
  lit.color = placed.color;
  lit.finish = placed.finish;
  lit.texture = placed.texture;
  lit.center = placed.model_3.xyz;
  lit.axis = placed.model_2.xyz;
  lit.anisotropy = placed.anisotropy;
  return lit;
}

@vertex
fn vertex(corner: Corner, placed: Placed) -> Lit {
  return seen_through(globals.view, corner, placed);
}

@vertex
fn bounce_vertex(corner: Corner, placed: Placed) -> Lit {
  return seen_through(globals.overhead, corner, placed);
}

// how much of the light reaches a point, from none to all of it
fn lit_share(world: vec3<f32>, facing: f32) -> f32 {
  let seen = globals.light_view * vec4<f32>(world, 1.0);
  let at = vec2<f32>(seen.x * 0.5 + 0.5, 0.5 - seen.y * 0.5);
  // a surface nearly edge-on to the light covers many depths in one texel, so it is
  // compared from further off
  let depth = seen.z - mix(SHADOW_BIAS, 0.003, facing);
  if globals.shadows.x == 0.0 {
    return textureSampleCompareLevel(shadow_map, shadow_sampler, at, depth);
  }
  // spread wide enough that a shadow's edge is a few pixels of the picture
  var share = 0.0;
  let side = SHADOW_SPREAD * 2 + 1;
  for (var x = -SHADOW_SPREAD; x <= SHADOW_SPREAD; x++) {
    for (var y = -SHADOW_SPREAD; y <= SHADOW_SPREAD; y++) {
      let aside = vec2<f32>(f32(x), f32(y)) * globals.shadows.y * globals.toward_light.w;
      share += textureSampleCompareLevel(shadow_map, shadow_sampler, at + aside, depth);
    }
  }
  return share / f32(side * side);
}

// how high the highest thing over a point of the panel is
fn height_over(at: vec2<f32>) -> f32 {
  let size = vec2<f32>(textureDimensions(height_map));
  let texel = clamp(at / globals.panel.xy * size, vec2<f32>(0.0), size - 1.0);
  let depth = textureLoad(height_map, vec2<i32>(texel), 0);
  return globals.panel.w - depth * (globals.panel.w - globals.panel.z);
}

// how much of the light all around reaches a point: less for everything near that stands
// higher than the point, and less the nearer and higher it stands
fn open_share(world: vec3<f32>) -> f32 {
  var closed = 0.0;
  var weight = 0.0;
  for (var ring = 0; ring < OPEN_RINGS; ring++) {
    let reach = OPEN_REACHES[ring];
    for (var step = 0; step < OPEN_STEPS; step++) {
      // each ring turned against the last, so that their points do not line up
      let turn = (f32(step) + f32(ring) * 0.37) * OPEN_TURN;
      let over = height_over(world.xy + vec2<f32>(cos(turn), sin(turn)) * reach);
      let rise = over - world.z - OPEN_CLEAR;
      let near = 1.0 / (1.0 + reach * 0.12);
      closed += clamp(rise / reach, 0.0, 1.0) * near;
      weight += near;
    }
  }
  return 1.0 - clamp(closed / weight * 2.3, 0.0, 0.92);
}

// how much of a lamp's light gets to a point past what stands between the two, going by
// the height map along the straight line to the lamp
fn lamp_share(world: vec3<f32>, lamp: Lamp) -> f32 {
  let toward = lamp.at.xyz - world;
  let away = length(toward.xy);
  // a texel of the height map can hold the point's own surface
  let texel = max(globals.panel.x, globals.panel.y) / f32(textureDimensions(height_map).x);
  let start = texel * 1.5;
  // the lamp's own body is in the height map, so the line is followed only up to its edge
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
    // what stands in the way has a softer edge the further it is from the point and the
    // wider the lamp
    let soft = 1.0 + lamp.color.w * along / (away - along);
    blocked = max(blocked, clamp((over - line) / soft, 0.0, 1.0));
  }
  return 1.0 - blocked;
}

// the light of a lamp that lands on a point squarely, before what stands between them: its
// color a quarter of its reach away, down with the square of the distance to its radius, and
// windowed to nothing at its reach
fn lamp_light(world: vec3<f32>, lamp: Lamp) -> vec3<f32> {
  let toward = lamp.at.xyz - world;
  let far = dot(toward, toward);
  let reach = lamp.at.w;
  let ends = 1.0 - pow(far / max(reach * reach, 1e-6), 2.0);
  let window = clamp(ends, 0.0, 1.0);
  let quarter = reach * reach / 16.0;
  let near = max(lamp.color.w * lamp.color.w, 1e-6);
  return lamp.color.rgb * quarter / max(far, near) * window * window;
}

// the light a surface gives off to what is next to it: its color under the light, without
// the shine, the corners or the lamps' shadows, which the little that lands does not show
@fragment
fn bounce_fragment(lit: Lit) -> @location(0) vec4<f32> {
  let normal = normalize(lit.normal);
  let facing = max(dot(normal, globals.toward_light.xyz), 0.0);
  var lighting = AMBIENT + globals.light_color.rgb * facing * lit_share(lit.world, facing);
  for (var index = 0; index < i32(globals.shadows.z); index++) {
    let lamp = globals.lamps[index];
    let toward = normalize(lamp.at.xyz - lit.world);
    lighting += lamp_light(lit.world, lamp) * max(dot(normal, toward), 0.0);
  }
  let base = decode(lit.color.rgb);
  let emission = lit.color.a;
  let given = base * (1.0 - lit.finish.x) * lighting * (1.0 - emission) + base * emission;
  return vec4<f32>(given, lit.world.z);
}

// what the bounce map starts from: no light, from as low as anything gets
@fragment
fn no_bounce_fragment() -> @location(0) vec4<f32> {
  return vec4<f32>(0.0, 0.0, 0.0, globals.panel.z);
}

// the light that lands on a point off the surfaces around it, from rings of places in the
// bounce map, each lighting the point by how squarely the point faces it
fn bounced(world: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
  let texel = globals.panel.xy / vec2<f32>(textureDimensions(bounce_map));
  let reach = max(texel.x, texel.y);
  var caught = vec3<f32>(0.0);
  var weight = 0.0;
  for (var ring = 0; ring < BOUNCE_RINGS; ring++) {
    let far = reach * BOUNCE_REACHES[ring];
    // a further ring is as much less of what the point sees
    let share = reach / far;
    for (var step = 0; step < BOUNCE_STEPS; step++) {
      // each ring turned against the last, so that their places do not line up
      let turn = (f32(step) + f32(ring) * 0.37) * (TAU / f32(BOUNCE_STEPS));
      let aside = vec2<f32>(cos(turn), sin(turn)) * far;
      let tap = textureSampleLevel(bounce_map, bounce_sampler, (world.xy + aside) / globals.panel.xy, 0.0);
      let toward = normalize(vec3<f32>(aside, tap.a - world.z));
      // a wall catches the floor in front of it, the floor catches what stands on it, and
      // nothing catches itself
      caught += tap.rgb * max(dot(normal, toward), 0.0) * share;
      weight += share;
    }
  }
  return caught / weight;
}

// the way to the light along the panel's face, which is along y when the light is straight
// overhead
fn across_to_light() -> vec2<f32> {
  return normalize(globals.toward_light.xy + vec2<f32>(0.0, 0.0001));
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
// darker the texture makes that spot. the texture is made from where the spot is on the panel
fn textured(normal: vec3<f32>, world: vec3<f32>, grain: f32, brushed: f32, wear: f32) -> vec4<f32> {
  var bent = normal;
  var shade = 0.0;
  if grain > 0.0 {
    // grain in every direction at about the size of a pixel
    let at = world.xy * 1.1 + world.z * 0.7;
    let fine = vec2<f32>(rolling(at), rolling(at + 37.0)) - 0.5;
    let coarse = vec2<f32>(rolling(at * 0.37 + 11.0), rolling(at * 0.37 + 71.0)) - 0.5;
    bent += vec3<f32>(fine * 0.16 + coarse * 0.1, 0.0) * grain;
    shade += (fine.x + coarse.y) * 0.035 * grain;
  }
  if brushed > 0.0 {
    // brushing in long streaks along x that only tilt the surface across themselves
    let streak = rolling(vec2<f32>(world.x * 0.01, world.y * 3.1)) - 0.5;
    let hair = rolling(vec2<f32>(world.x * 0.04 + 9.0, world.y * 7.3)) - 0.5;
    bent.y += (streak * 0.14 + hair * 0.1) * brushed;
    shade += (streak * 0.045 + hair * 0.035) * brushed;
  }
  if wear > 0.0 {
    // slow blotches of grime, and here and there a patch of fine scratches running one of
    // three ways, each a hairline that catches the light
    let blotch = rolling(world.xy * 0.03 + 5.0) * 0.65 + rolling(world.xy * 0.11 + 31.0) * 0.35;
    shade -= smoothstep(0.4, 0.85, blotch) * 0.13 * wear;
    for (var way = 0; way < 3; way++) {
      let turn = 0.5 + f32(way) * 1.9;
      let across = vec2<f32>(cos(turn), sin(turn));
      let along = vec2<f32>(-across.y, across.x);
      let at = vec2<f32>(dot(world.xy, along) * 0.07, dot(world.xy, across) * 2.3) + f32(way) * 17.0;
      let worn = smoothstep(0.6, 0.8, rolling(world.xy * 0.023 + f32(way) * 43.0));
      let hairline = smoothstep(0.88, 0.97, rolling(at)) * worn * wear;
      bent += vec3<f32>(across * hairline * 0.12, 0.0);
      shade += hairline * 0.03;
    }
  }
  return vec4<f32>(normalize(bent), shade);
}

// the room around the panel as a mirror would show it, by the way a reflection leaves.
// `crisp` is how sharp its edges are
fn room(leaving: vec3<f32>, normal: vec3<f32>, crisp: f32) -> f32 {
  // the mirror's own facing says what side of the room it looks at when the reflection
  // runs straight up or down
  let aside = normalize(leaving.xy + normal.xy * 0.02 + vec2<f32>(0.00001));
  let side = dot(aside, across_to_light());
  let high = leaving.z;
  // from a blur to a clean cut
  let edge = mix(0.3, 0.025, crisp);
  // a wide lamp on the light's side, from well up the wall nearly to the ceiling
  let lamp = smoothstep(0.24 - edge, 0.24 + edge, high) * (1.0 - smoothstep(0.82 - edge, 0.82 + edge, high))
    * smoothstep(0.4 - edge * 2.0, 0.4 + edge * 2.0, side);
  // a small dim one across from it
  let second = smoothstep(0.3 - edge, 0.3 + edge, high) * (1.0 - smoothstep(0.6 - edge, 0.6 + edge, high))
    * smoothstep(0.8 - edge, 0.8 + edge, -side) * 0.3;
  // a ceiling paler than the walls
  let ceiling = smoothstep(0.5, 1.0, high) * 0.42;
  // the walls and the line along their foot are brighter on the side the light is on
  let lit_side = 0.5 + 0.5 * side;
  let over = smoothstep(-edge * 0.4, edge * 0.4, high);
  // a bright line where the walls meet the floor
  let line = over * (1.0 - smoothstep(0.07, 0.07 + edge * 2.0, high)) * mix(0.3, 0.95, lit_side);
  let walls = over * mix(0.04, 0.12, lit_side);
  // a floor that is dark at its far edge and lighter toward the panel
  let floor = (1.0 - over) * mix(0.015, mix(0.05, 0.24, lit_side * lit_side), smoothstep(-0.05, -0.6, high));
  return lamp + second + max(ceiling, walls) + line + floor + 0.015;
}

// a point of a surface as the shading model takes it
struct Surface {
  normal: vec3<f32>,
  // the way the clear coat faces, which the grain under it does not bend
  coat_normal: vec3<f32>,
  toward_viewer: vec3<f32>,
  // n.v, kept off zero
  seen: f32,
  diffuse: vec3<f32>,
  f0: vec3<f32>,
  // the roughness squared along the grooves and across them, the same for no grooves
  along: f32,
  across: f32,
  // the way the grooves run, along the surface
  grooves: vec3<f32>,
  // what makes up for the light a rough highlight loses to bouncing between its facets
  energy: vec3<f32>,
  coat: f32,
  // the coat's roughness squared
  coat_alpha: f32,
}

fn fresnel(f0: vec3<f32>, cosine: f32) -> vec3<f32> {
  return f0 + (1.0 - f0) * pow(1.0 - cosine, 5.0);
}

// the GGX distribution of facets, times pi
fn ggx(noh: f32, alpha: f32) -> f32 {
  let a2 = alpha * alpha;
  let f = (noh * a2 - noh) * noh + 1.0;
  return a2 / (f * f);
}

// the height-correlated Smith visibility
fn smith(nov: f32, nol: f32, alpha: f32) -> f32 {
  let a2 = alpha * alpha;
  let from_view = nol * sqrt(nov * nov * (1.0 - a2) + a2);
  let from_light = nov * sqrt(nol * nol * (1.0 - a2) + a2);
  return 0.5 / (from_view + from_light);
}

// the highlight of a light from `l`, before n.l and the light's color. it is times pi, as
// the diffuse light is
fn highlight(surface: Surface, l: vec3<f32>) -> vec3<f32> {
  let n = surface.normal;
  let v = surface.toward_viewer;
  let h = normalize(l + v);
  let nol = max(dot(n, l), 1e-4);
  let noh = clamp(dot(n, h), 0.0, 1.0);
  let loh = clamp(dot(l, h), 0.0, 1.0);
  var shape: f32;
  if surface.along == surface.across {
    shape = ggx(noh, surface.along) * smith(surface.seen, nol, surface.along);
  } else {
    // Filament's anisotropic GGX and its visibility, with the grooves as the tangent
    let t = surface.grooves;
    let b = cross(n, t);
    let at = surface.along;
    let ab = surface.across;
    let a2 = at * ab;
    let tilt = vec3<f32>(ab * dot(t, h), at * dot(b, h), a2 * noh);
    let w2 = a2 / dot(tilt, tilt);
    let from_view = nol * length(vec3<f32>(at * dot(t, v), ab * dot(b, v), surface.seen));
    let from_light = surface.seen * length(vec3<f32>(at * dot(t, l), ab * dot(b, l), nol));
    shape = a2 * w2 * w2 * 0.5 / (from_view + from_light);
  }
  return fresnel(surface.f0, loh) * shape * surface.energy;
}

// the highlights of a light from `l`, with n.l, times `normalized`, and in w how much of the
// light the clear coat keeps off what is under it
fn highlights_of(surface: Surface, l: vec3<f32>, normalized: f32) -> vec4<f32> {
  let nol = max(dot(surface.normal, l), 0.0);
  let under = highlight(surface, l) * nol * normalized;
  if surface.coat == 0.0 {
    return vec4<f32>(under, 0.0);
  }
  let h = normalize(l + surface.toward_viewer);
  let noh = clamp(dot(surface.coat_normal, h), 0.0, 1.0);
  let loh = max(dot(l, h), 1e-4);
  let kept = fresnel(vec3<f32>(DIELECTRIC), loh).x * surface.coat;
  // Kelemen's visibility, which suits a smooth coat
  let coat = ggx(noh, surface.coat_alpha) * 0.25 / (loh * loh) * kept;
  let coat_nol = max(dot(surface.coat_normal, l), 0.0);
  return vec4<f32>(under * (1.0 - kept) + coat * coat_nol * normalized, kept);
}

// Karis's representative point for a lamp's highlight: the point of the lamp's ball nearest
// the reflection, in xyz, and in w what keeps the highlight it widens to the lamp's light
fn nearest_on_lamp(surface: Surface, toward: vec3<f32>, radius: f32) -> vec4<f32> {
  let leaving = reflect(-surface.toward_viewer, surface.normal);
  let to_ray = dot(toward, leaving) * leaving - toward;
  let nearest = toward + to_ray * clamp(radius / max(length(to_ray), 1e-4), 0.0, 1.0);
  let alpha = sqrt(surface.along * surface.across);
  let widened = clamp(alpha + radius / (3.0 * max(length(toward), 1e-4)), 0.0, 1.0);
  let kept = alpha / widened;
  return vec4<f32>(normalize(nearest), kept * kept);
}

// the picture holds linear light in rgb, and in a the share of it that the surface gives off
// itself
@fragment
fn fragment(lit: Lit) -> @location(0) vec4<f32> {
  let flat_normal = normalize(lit.normal);
  let rough = textured(flat_normal, lit.world, lit.texture.x, lit.anisotropy.x, lit.texture.y);
  let base = decode(lit.color.rgb) * (1.0 + rough.w);
  let emission = lit.color.a;
  let metallic = lit.finish.x;
  let roughness = clamp(lit.finish.y, LEAST_ROUGHNESS, 1.0);
  let toward_light = globals.toward_light.xyz;
  let toward_viewer = globals.toward_viewer.xyz;

  var surface: Surface;
  surface.normal = rough.xyz;
  surface.coat_normal = flat_normal;
  surface.toward_viewer = toward_viewer;
  surface.seen = max(dot(rough.xyz, toward_viewer), 1e-4);
  surface.diffuse = base * (1.0 - metallic);
  surface.f0 = mix(vec3<f32>(DIELECTRIC), base, metallic);
  surface.coat = lit.finish.z;
  let coat_roughness = clamp(lit.finish.w, LEAST_ROUGHNESS, 1.0);
  surface.coat_alpha = coat_roughness * coat_roughness;
  let alpha = roughness * roughness;
  surface.along = alpha;
  surface.across = alpha;
  // the grooves of brushing run along x, and those of spinning around the object's z axis
  var grooves = vec3<f32>(0.0);
  var drawn_out = 0.0;
  if lit.anisotropy.x > 0.0 {
    grooves = vec3<f32>(1.0, 0.0, 0.0);
    drawn_out = lit.anisotropy.x;
  } else if lit.anisotropy.y > 0.0 {
    grooves = cross(normalize(lit.axis), lit.world - lit.center);
    drawn_out = lit.anisotropy.y;
  }
  let on_surface = grooves - surface.normal * dot(surface.normal, grooves);
  // the middle of a spun face has no way around it, and a brushed sheet's edge none along it
  if drawn_out > 0.0 && dot(on_surface, on_surface) > 1e-8 {
    surface.grooves = normalize(on_surface);
    surface.along = max(alpha * (1.0 - drawn_out), LEAST_ROUGHNESS * LEAST_ROUGHNESS);
    surface.across = min(alpha * (1.0 + drawn_out), 1.0);
  } else {
    drawn_out = 0.0;
  }
  let dfg = textureSampleLevel(dfg_table, bounce_sampler, vec2<f32>(surface.seen, roughness), 0.0).xy;
  let held = surface.f0 * dfg.x + dfg.y;
  surface.energy = 1.0 + surface.f0 * (1.0 / (dfg.x + dfg.y) - 1.0);

  let facing = max(dot(surface.normal, toward_light), 0.0);
  let share = lit_share(lit.world, facing);
  let open = mix(1.0, open_share(lit.world), globals.shadows.w);

  // the key light, whose diffuse light the corners keep a little of out too
  let key = highlights_of(surface, toward_light, 1.0);
  let key_diffuse = surface.diffuse * facing * mix(1.0, open, 0.35) * (1.0 - key.w);
  var diffuse_light = globals.light_color.rgb * key_diffuse * share;
  var highlights = globals.light_color.rgb * key.rgb * share;

  // the lamps, none of whose highlights show on what lights itself
  for (var index = 0; index < i32(globals.shadows.z); index++) {
    let lamp = globals.lamps[index];
    let light = lamp_light(lit.world, lamp);
    if all(light == vec3<f32>(0.0)) {
      continue;
    }
    let toward = lamp.at.xyz - lit.world;
    let reached = lamp_share(lit.world, lamp) * light;
    let nearest = nearest_on_lamp(surface, toward, lamp.color.w);
    let given = highlights_of(surface, nearest.xyz, nearest.w);
    let facing_lamp = max(dot(surface.normal, normalize(toward)), 0.0);
    diffuse_light += surface.diffuse * facing_lamp * (1.0 - given.w) * reached;
    highlights += given.rgb * reached * (1.0 - emission);
  }

  // the light all around, which more of comes from the side the light is on than from under,
  // and the room in what mirrors
  let around = mix(0.55, 1.0, 0.5 + 0.5 * dot(surface.normal, toward_light));
  var all_around = vec3<f32>(AMBIENT * around * open);
  if globals.light_color.w > 0.0 {
    all_around += bounced(lit.on_triangle, flat_normal) * globals.light_color.w;
  }
  var around_diffuse = surface.diffuse * all_around * (1.0 - held);
  // grooves bend what the surface mirrors toward the way across them
  var mirror_normal = surface.normal;
  if drawn_out > 0.0 {
    let across = cross(surface.grooves, toward_viewer);
    let bent = cross(across, surface.grooves);
    mirror_normal = normalize(mix(surface.normal, bent, drawn_out));
  }
  let crisp = clamp(1.0 - (roughness - 0.1) / 0.5, 0.0, 1.0);
  let leaving = reflect(-toward_viewer, mirror_normal);
  let room_light = ROOM * mix(0.6, 1.0, share) * mix(0.5, 1.0, open);
  var mirrored = held * surface.energy * room(leaving, mirror_normal, crisp) * room_light;
  if surface.coat > 0.0 {
    let coat_seen = max(dot(flat_normal, toward_viewer), 1e-4);
    let kept = fresnel(vec3<f32>(DIELECTRIC), coat_seen).x * surface.coat;
    let coat_crisp = clamp(1.0 - (coat_roughness - 0.1) / 0.5, 0.0, 1.0);
    let coat_leaving = reflect(-toward_viewer, flat_normal);
    around_diffuse *= 1.0 - kept;
    mirrored = mirrored * (1.0 - kept) + kept * room(coat_leaving, flat_normal, coat_crisp) * room_light;
  }
  diffuse_light += around_diffuse;
  highlights += mirrored;

  // the light the surface gives off itself, which takes the place of the diffuse light it
  // would have under the scene's
  var emitted = base * emission;
  var lit_over = emission;
  if lit.texture.z > 0.0 {
    // lit from inside by one light under the middle: brightest and whitest over it, and
    // still the color, dimmer, out at the edges
    let turn = normalize(lit.axis);
    let out = lit.world - lit.center;
    let aside = out - turn * dot(out, turn);
    let near = exp(-dot(aside, aside) / (lit.texture.z * lit.texture.z));
    lit_over = emission * (0.42 + 0.58 * near);
    emitted = base * 1.1 * lit_over + vec3<f32>(near * near * near * 0.3 * emission);
    // the light also runs along the inside of the walls and comes out where the top
    // turns over into them, which rims the lit thing in its color
    let turned = 1.0 - clamp(flat_normal.z, 0.0, 1.0);
    emitted += base * sqrt(turned) * emission * 0.8;
  }
  let color = min(diffuse_light * (1.0 - lit_over) + highlights + emitted, vec3<f32>(BRIGHTEST));
  // the share of the light that is the surface's own, which is what spills a haze later
  let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
  let own = dot(emitted, weights) / max(dot(color, weights), 1e-6);
  return vec4<f32>(color, clamp(own, 0.0, 1.0));
}
