// the panel's objects under one light: into the shadow map, into the maps of how high they
// are and of the light they give off, and then into the picture
// `encode` and `decode` are in srgb.wgsl, which is put ahead of this source when it is loaded

struct Globals {
  view: mat4x4<f32>,
  light_view: mat4x4<f32>,
  // and in w how wide shadow edges are: the shadow map texels between the places
  // `lit_share` looks at
  toward_light: vec4<f32>,
  // and in w how bright the studio is
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
  // the light a quarter of its reach away, and in w its radius
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
// the studio without its softbox as a mirror of each roughness shows it, a mip for each
// 1 / STUDIO_LAST of roughness
@group(1) @binding(6) var studio_map: texture_cube<f32>;
@group(1) @binding(7) var studio_sampler: sampler;
// the softbox alone, as studio_map holds the rest
@group(1) @binding(8) var softbox_map: texture_cube<f32>;
// the texture sets' color, the x and y of the way their relief faces, and their roughness,
// each an array of the sets
@group(1) @binding(9) var albedo_map: texture_2d_array<f32>;
@group(1) @binding(10) var normal_map: texture_2d_array<f32>;
@group(1) @binding(11) var roughness_map: texture_2d_array<f32>;
// tiling, for the three above
@group(1) @binding(12) var texture_sampler: sampler;

const TAU: f32 = 6.2831853;
const PI: f32 = 3.14159265;
// the least perceptual roughness, under which a highlight is too small to sample
const LEAST_ROUGHNESS: f32 = 0.045;
// the reflectance seen square on of plastic, paint, glass and a clear coat
const DIELECTRIC: f32 = 0.04;
// the light a surface that lights itself fully gives off, as a multiple of its color. it is
// past where the tone map starts to roll off, so that a lit thing outshines a lit white card
const EMITTED: f32 = 1.5;
// how much brighter again the middle of something lit from inside is
const CORE: f32 = 0.8;
// how far past its color the middle of something lit from inside is driven, which fills its
// weaker channels toward white
const HEAT: f32 = 20.0;
// the share of a display's cell its lit square takes across
const CELL_FILL: f32 = 0.78;
// the share of a display's light that shows in the gaps between its cells
const GAP_LIGHT: f32 = 0.3;
// the most a half float holds, with room to spare
const BRIGHTEST: f32 = 60000.0;
// SHADOW_SPREAD, SHADOW_BIAS, OPEN_REACHES, OPEN_CLEAR, BOUNCE_REACHES, HARMONICS,
// STUDIO_LAST, MEAN_COLOR and ROUGHNESS_MEAN are put ahead of this source when it is loaded
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
  // x the texture set's place among the sets plus one, or zero for none, y the points one tile
  // of it covers, z one when it wraps around the object's own z axis, w the pitch of a
  // display's cells or zero
  @location(10) textures: vec4<f32>,
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
  @location(9) textures: vec4<f32>,
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
  lit.textures = placed.textures;
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
// color a quarter of its reach away, down with the square of the distance and levelling off
// within its radius, and windowed to nothing at its reach
fn lamp_light(world: vec3<f32>, lamp: Lamp) -> vec3<f32> {
  let toward = lamp.at.xyz - world;
  let far = dot(toward, toward);
  let reach = lamp.at.w;
  let ends = 1.0 - pow(far / max(reach * reach, 1e-6), 2.0);
  let window = clamp(ends, 0.0, 1.0);
  let near = max(lamp.color.w * lamp.color.w, 1e-6);
  let quarter = reach * reach / 16.0 + near;
  return lamp.color.rgb * quarter / (far + near) * window * window;
}

// how squarely a surface facing `normal` takes the light of a lamp's ball of `radius` out
// along `toward`: n.l while all of the ball is over the surface's plane, and more as the plane
// cuts it, from the sphere illuminance in Lagarde and de Rousiers' Frostbite notes
fn lamp_facing(normal: vec3<f32>, toward: vec3<f32>, radius: f32) -> f32 {
  let far = dot(toward, toward);
  let cos_theta = clamp(dot(normal, toward) * inverseSqrt(max(far, 1e-8)), -1.0, 1.0);
  // the sine of half the angle the ball covers, squared, short of one inside the ball
  let sin_sigma2 = min(radius * radius / max(far, 1e-8), 0.9999);
  if cos_theta * cos_theta > sin_sigma2 {
    return max(cos_theta, 0.0);
  }
  let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
  let x = sqrt(1.0 / sin_sigma2 - 1.0);
  let y = -x * cos_theta / max(sin_theta, 1e-4);
  let cut = sin_theta * sqrt(max(1.0 - y * y, 0.0));
  let lit = (cos_theta * acos(clamp(y, -1.0, 1.0)) - x * cut) * sin_sigma2 + atan(cut / x);
  return max(lit, 0.0) / (PI * sin_sigma2);
}

// the light a surface gives off to what is next to it: its color under the light, without
// the shine, the corners or the lamps' shadows, which the little that lands does not show
@fragment
fn bounce_fragment(lit: Lit) -> @location(0) vec4<f32> {
  let normal = normalize(lit.normal);
  let facing = max(dot(normal, globals.toward_light.xyz), 0.0);
  var lighting = all_around(normal) * globals.toward_viewer.w;
  lighting += globals.light_color.rgb * facing * lit_share(lit.world, facing);
  for (var index = 0; index < i32(globals.shadows.z); index++) {
    let lamp = globals.lamps[index];
    let toward = lamp.at.xyz - lit.world;
    lighting += lamp_light(lit.world, lamp) * lamp_facing(normal, toward, lamp.color.w);
  }
  let base = decode(lit.color.rgb);
  let emission = lit.color.a;
  let given = base * (1.0 - lit.finish.x) * lighting * (1.0 - emission) + base * emission * EMITTED;
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

// grain in every direction at about the size of a pixel, on a plane at `at`: how far it
// tilts the surface along the plane's two axes, and in z how much lighter or darker it is
fn grain_on(at: vec2<f32>) -> vec3<f32> {
  let place = at * 1.1;
  let fine = vec2<f32>(rolling(place), rolling(place + 37.0)) - 0.5;
  let coarse = vec2<f32>(rolling(place * 0.37 + 11.0), rolling(place * 0.37 + 71.0)) - 0.5;
  return vec3<f32>(fine * 0.16 + coarse * 0.1, (fine.x + coarse.y) * 0.035);
}

// the way a surface faces once its texture has bent it, and in w how much lighter or
// darker the texture makes that spot. the texture is made from where the spot is on the panel
fn textured(normal: vec3<f32>, world: vec3<f32>, grain: f32, wear: f32) -> vec4<f32> {
  var bent = normal;
  var shade = 0.0;
  if grain > 0.0 {
    // grain laid on the three planes of the axes and mixed by how squarely the surface faces
    // each, so that it is as fine on a wall as on a top
    var facing = normal * normal;
    facing *= facing;
    facing /= facing.x + facing.y + facing.z;
    let across = grain_on(world.yz) * facing.x;
    let along = grain_on(world.xz) * facing.y;
    let up = grain_on(world.xy) * facing.z;
    bent += vec3<f32>(along.x + up.x, across.x + up.y, across.y + along.y) * grain;
    shade += (across.z + along.z + up.z) * grain;
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

// the studio's light all around a surface facing `normal`, as a white Lambert surface gives
// it back, from the spherical harmonics in HARMONICS. the key light's own part is left out
fn all_around(normal: vec3<f32>) -> vec3<f32> {
  let n = normal;
  let basis = array<f32, 9>(
    0.282095,
    0.488603 * n.y,
    0.488603 * n.z,
    0.488603 * n.x,
    1.092548 * n.x * n.y,
    1.092548 * n.y * n.z,
    0.315392 * (3.0 * n.z * n.z - 1.0),
    1.092548 * n.x * n.z,
    0.546274 * (n.x * n.x - n.y * n.y),
  );
  var light = vec3<f32>(0.0);
  for (var index = 0; index < 9; index++) {
    light += HARMONICS[index] * basis[index];
  }
  return max(light, vec3<f32>(0.0));
}

// the studio as a mirror of a perceptual roughness shows it along `leaving`, with its softbox
// times `lit`, the share of the key light that reaches the mirror
fn mirrored_studio(leaving: vec3<f32>, roughness: f32, lit: f32) -> vec3<f32> {
  let lod = roughness * STUDIO_LAST;
  let rest = textureSampleLevel(studio_map, studio_sampler, leaving, lod).rgb;
  return rest + textureSampleLevel(softbox_map, studio_sampler, leaving, lod).rgb * lit;
}

// a texture layer as a surface shows it at a spot
struct Photo {
  // the way the surface faces once the set's relief has bent it
  normal: vec3<f32>,
  // the set's color, in linear light
  color: vec3<f32>,
  roughness: f32,
}

// a texture layer at `at`, in tiles, on a surface facing `normal`. `across` and `up` are the ways
// the set's x and y run on the surface, and `ddx` and `ddy` how far `at` moves across and down
// a pixel, for the mip
fn sampled_set(
  layer: i32,
  at: vec2<f32>,
  ddx: vec2<f32>,
  ddy: vec2<f32>,
  normal: vec3<f32>,
  across: vec3<f32>,
  up: vec3<f32>,
) -> Photo {
  let toward = textureSampleGrad(normal_map, texture_sampler, at, layer, ddx, ddy).rg * 2.0 - 1.0;
  let t = normalize(across - normal * dot(normal, across));
  let b = normalize(up - normal * dot(normal, up) - t * dot(t, up));
  let rise = sqrt(max(1.0 - dot(toward, toward), 0.0));
  var photo: Photo;
  photo.normal = t * toward.x + b * toward.y + normal * rise;
  photo.color = textureSampleGrad(albedo_map, texture_sampler, at, layer, ddx, ddy).rgb;
  photo.roughness = textureSampleGrad(roughness_map, texture_sampler, at, layer, ddx, ddy).r;
  return photo;
}

// a texture layer laid flat on the plane through the spot spanned by `across` and `down`, a
// tile every `tile` points. `moved` is how far the spot moves across and down a pixel
fn laid(
  layer: i32,
  tile: f32,
  world: vec3<f32>,
  moved: mat2x3<f32>,
  normal: vec3<f32>,
  across: vec3<f32>,
  down: vec3<f32>,
) -> Photo {
  let at = vec2<f32>(dot(world, across), dot(world, down)) / tile;
  let ddx = vec2<f32>(dot(moved[0], across), dot(moved[0], down)) / tile;
  let ddy = vec2<f32>(dot(moved[1], across), dot(moved[1], down)) / tile;
  return sampled_set(layer, at, ddx, ddy, normal, across, -down);
}

// a texture layer as an object shows it: from above on a face turned up, wrapped around the
// object's own z axis on the sides of what is spun, and from the three axes on the rest, mixed
// by how squarely the surface faces each
fn photographed(lit: Lit, normal: vec3<f32>, moved: mat2x3<f32>) -> Photo {
  let layer = i32(lit.textures.x) - 1;
  let tile = lit.textures.y;
  let world = lit.world;
  if normal.z > 0.7 {
    return laid(layer, tile, world, moved, normal, vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0));
  }
  if lit.textures.z > 0.0 {
    // as many whole tiles around as come nearest to one every `tile` points at the spot's
    // distance from the axis, so that the way around closes on itself, and a tile every `tile`
    // points up it. the mip follows the way around, which has no seam
    let axis = normalize(lit.axis);
    let out = world - lit.center;
    let high = dot(out, axis);
    let aside = out - axis * high;
    let far = max(length(aside), 1e-4);
    // the turn counts from the panel's y, or its x for an axis that lies along y
    var start = vec3<f32>(0.0, 1.0, 0.0);
    if abs(axis.y) > 0.9 {
      start = vec3<f32>(1.0, 0.0, 0.0);
    }
    let first = normalize(start - axis * dot(start, axis));
    let turn = atan2(dot(aside, cross(axis, first)), dot(aside, first));
    let around = cross(axis, aside / far);
    let tiles = max(round(TAU * far / tile), 1.0);
    // tiles around for each point moved along the way around
    let per_point = tiles / (TAU * far);
    let at = vec2<f32>(turn / TAU * tiles, -high / tile);
    let ddx = vec2<f32>(dot(moved[0], around) * per_point, -dot(moved[0], axis) / tile);
    let ddy = vec2<f32>(dot(moved[1], around) * per_point, -dot(moved[1], axis) / tile);
    return sampled_set(layer, at, ddx, ddy, normal, around, axis);
  }
  var facing = normal * normal;
  facing *= facing;
  facing /= facing.x + facing.y + facing.z;
  var mixed: Photo;
  mixed.normal = vec3<f32>(0.0);
  mixed.color = vec3<f32>(0.0);
  mixed.roughness = 0.0;
  // the walls take the layer upright, its y running down the panel's z
  let planes = array<mat2x3<f32>, 3>(
    mat2x3<f32>(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, -1.0)),
    mat2x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, -1.0)),
    mat2x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0)),
  );
  for (var axis = 0; axis < 3; axis++) {
    let weight = facing[axis];
    if weight < 0.01 {
      continue;
    }
    let photo = laid(layer, tile, world, moved, normal, planes[axis][0], planes[axis][1]);
    mixed.normal += photo.normal * weight;
    mixed.color += photo.color * weight;
    mixed.roughness += photo.roughness * weight;
  }
  mixed.normal = normalize(mixed.normal);
  return mixed;
}

// how much of a display's light shows at a place: all of it in each square cell `pitch`
// apart, and little in the gaps between. where the cells are finer than a pixel they fade to
// their mean, which keeps them from shimmering
fn cells(at: vec2<f32>, pitch: f32) -> f32 {
  let across = at / pitch;
  let blur = max(fwidth(across), vec2<f32>(1e-4));
  // a cell's lit square is CELL_FILL of its width, and the gap shows GAP_LIGHT of the light
  let edge = CELL_FILL * 0.5;
  let off = abs(fract(across) - 0.5);
  let inside = smoothstep(vec2<f32>(edge) + blur * 0.5, vec2<f32>(edge) - blur * 0.5, off);
  let sharp = inside.x * inside.y;
  let mean = CELL_FILL * CELL_FILL;
  let fine = smoothstep(0.35, 0.7, max(blur.x, blur.y));
  return mix(GAP_LIGHT, 1.0, mix(sharp, mean, fine));
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
// light the clear coat keeps off what is under it. the coat's highlight comes from `coat_l`,
// times `coat_normalized`
fn highlights_of(
  surface: Surface,
  l: vec3<f32>,
  normalized: f32,
  coat_l: vec3<f32>,
  coat_normalized: f32,
) -> vec4<f32> {
  let nol = max(dot(surface.normal, l), 0.0);
  let under = highlight(surface, l) * nol * normalized;
  if surface.coat == 0.0 {
    return vec4<f32>(under, 0.0);
  }
  let h = normalize(coat_l + surface.toward_viewer);
  let noh = clamp(dot(surface.coat_normal, h), 0.0, 1.0);
  let loh = max(dot(coat_l, h), 1e-4);
  let kept = fresnel(vec3<f32>(DIELECTRIC), loh).x * surface.coat;
  // Kelemen's visibility, which suits a smooth coat
  let coat = ggx(noh, surface.coat_alpha) * 0.25 / (loh * loh) * kept;
  let coat_nol = max(dot(surface.coat_normal, coat_l), 0.0);
  return vec4<f32>(under * (1.0 - kept) + coat * coat_nol * coat_normalized, kept);
}

// Karis's representative point for a lamp's highlight off a surface facing `normal` with a
// roughness squared of `alpha`: the point of the lamp's ball nearest the reflection, in xyz,
// and in w what keeps the highlight it widens to the lamp's light
fn nearest_on_lamp(
  toward_viewer: vec3<f32>,
  normal: vec3<f32>,
  alpha: f32,
  toward: vec3<f32>,
  whole: f32,
) -> vec4<f32> {
  // the surface sees no part of the ball under its own plane, so the ball is taken no larger
  // than the lamp's middle is high over that plane. a ball the surface is inside would
  // otherwise mirror evenly over all of it
  let radius = min(whole, max(dot(toward, normal), 0.0));
  let leaving = reflect(-toward_viewer, normal);
  let to_ray = dot(toward, leaving) * leaving - toward;
  let nearest = toward + to_ray * clamp(radius / max(length(to_ray), 1e-4), 0.0, 1.0);
  let widened = clamp(alpha + radius / (3.0 * max(length(toward), 1e-4)), 0.0, 1.0);
  let kept = alpha / widened;
  return vec4<f32>(normalize(nearest), kept * kept);
}

// a perceptual roughness widened by how fast `normal` turns across a pixel, Tokuyoshi and
// Kaplanyan's filter with Filament's constants. a highlight on a thin curved edge then spreads
// over the pixel instead of lighting single samples
fn filtered(roughness: f32, normal: vec3<f32>) -> f32 {
  let across = dpdx(normal);
  let down = dpdy(normal);
  let variance = 0.15 * (dot(across, across) + dot(down, down));
  let alpha = roughness * roughness;
  return sqrt(sqrt(clamp(alpha * alpha + min(2.0 * variance, 0.2), 0.0, 1.0)));
}

// the picture holds linear light in rgb, and in a the share of it that the surface gives off
// itself
@fragment
fn fragment(lit: Lit) -> @location(0) vec4<f32> {
  let flat_normal = normalize(lit.normal);
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
  // the grooves lie along the face. where they run nearly square to it, as on a brushed
  // sheet's side, they fade out, so that the way they run never comes from the grain alone.
  // the middle of a spun face has none
  let on_face = grooves - flat_normal * dot(flat_normal, grooves);
  let lying = length(on_face) / max(length(grooves), 1e-6);
  drawn_out *= smoothstep(0.2, 0.6, lying);
  // the texture layer, whose color and roughness are scaled to a mean of one, so that the
  // material's own say how light and how rough the surface is
  var relief = flat_normal;
  var tinted = decode(lit.color.rgb);
  var given_roughness = lit.finish.y;
  let moved = mat2x3<f32>(dpdx(lit.world), dpdy(lit.world));
  if lit.textures.x > 0.0 {
    let layer = i32(lit.textures.x) - 1;
    let photo = photographed(lit, flat_normal, moved);
    relief = photo.normal;
    tinted *= photo.color / MEAN_COLOR[layer];
    given_roughness *= photo.roughness / ROUGHNESS_MEAN[layer];
  }
  let rough = textured(relief, lit.world, lit.texture.x, lit.texture.y);
  let base = tinted * (1.0 + rough.w);
  let emission = lit.color.a;
  let metallic = lit.finish.x;
  // the relief and grain bend the normal from pixel to pixel too, which widens the highlight
  // the same way
  let roughness = filtered(clamp(given_roughness, LEAST_ROUGHNESS, 1.0), rough.xyz);
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
  let coat_roughness = filtered(clamp(lit.finish.w, LEAST_ROUGHNESS, 1.0), flat_normal);
  surface.coat_alpha = coat_roughness * coat_roughness;
  let alpha = roughness * roughness;
  surface.along = alpha;
  surface.across = alpha;
  let on_surface = on_face - surface.normal * dot(surface.normal, on_face);
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

  // how much of the light the clear coat keeps off what is under it
  var coat_kept = 0.0;
  if surface.coat > 0.0 {
    let coat_seen = max(dot(flat_normal, toward_viewer), 1e-4);
    coat_kept = fresnel(vec3<f32>(DIELECTRIC), coat_seen).x * surface.coat;
  }

  // the key light, which lights only diffusely, and whose light the corners keep a little of
  // out too. its highlight is the softbox the studio shows
  let key_diffuse = surface.diffuse * facing * mix(1.0, open, 0.35) * (1.0 - coat_kept);
  var diffuse_light = globals.light_color.rgb * key_diffuse * share;
  var highlights = vec3<f32>(0.0);

  // the lamps, whose light skips what lights itself, since a lamp is most often the light of
  // such a thing and would light it twice
  for (var index = 0; index < i32(globals.shadows.z); index++) {
    let lamp = globals.lamps[index];
    let light = lamp_light(lit.world, lamp);
    if all(light == vec3<f32>(0.0)) {
      continue;
    }
    let toward = lamp.at.xyz - lit.world;
    let reached = lamp_share(lit.world, lamp) * light * (1.0 - emission);
    let alpha = sqrt(surface.along * surface.across);
    let nearest = nearest_on_lamp(toward_viewer, surface.normal, alpha, toward, lamp.color.w);
    let coat = nearest_on_lamp(toward_viewer, flat_normal, surface.coat_alpha, toward, lamp.color.w);
    let given = highlights_of(surface, nearest.xyz, nearest.w, coat.xyz, coat.w);
    let facing_lamp = lamp_facing(surface.normal, toward, lamp.color.w);
    diffuse_light += surface.diffuse * facing_lamp * (1.0 - given.w) * reached;
    highlights += given.rgb * reached;
  }

  // the studio: its light all around, kept out of corners, and what it shows in a mirror, whose
  // softbox the key light's shadow hides as it hides the key light
  let studio = globals.toward_viewer.w;
  var all_around_light = all_around(surface.normal) * studio * open;
  if globals.light_color.w > 0.0 {
    all_around_light += bounced(lit.on_triangle, flat_normal) * globals.light_color.w;
  }
  diffuse_light += surface.diffuse * all_around_light * (1.0 - held) * (1.0 - coat_kept);
  // grooves bend what the surface mirrors toward the way across them, as Filament bends the
  // normal, less on a smooth surface
  var mirror_normal = surface.normal;
  if drawn_out > 0.0 {
    let across = cross(surface.grooves, toward_viewer);
    let bent = cross(across, surface.grooves);
    let bend = drawn_out * clamp(5.0 * roughness, 0.0, 1.0);
    mirror_normal = normalize(mix(surface.normal, bent, bend));
  }
  let leaving = reflect(-toward_viewer, mirror_normal);
  var mirrored = held * surface.energy * mirrored_studio(leaving, roughness, share);
  if surface.coat > 0.0 {
    let coat_leaving = reflect(-toward_viewer, flat_normal);
    mirrored = mirrored * (1.0 - coat_kept) + coat_kept * mirrored_studio(coat_leaving, coat_roughness, share);
  }
  highlights += mirrored * studio;

  // the light the surface gives off itself, which takes the place of the diffuse light it
  // would have under the scene's
  var emitted = base * emission * EMITTED;
  var lit_over = emission;
  if lit.texture.z > 0.0 {
    // lit from inside by one light under the middle: brightest and whitest over it, and
    // still the color, dimmer, out at the edges
    let turn = normalize(lit.axis);
    let out = lit.world - lit.center;
    let aside = out - turn * dot(out, turn);
    let near = exp(-dot(aside, aside) / (lit.texture.z * lit.texture.z));
    lit_over = emission * (0.42 + 0.58 * near);
    // a light too bright for its color fills its weaker channels as a camera's do, which
    // turns an amber core yellow and a blue one pale cyan on the way to white. heat is kept
    // off zero, where the fill is the color itself
    let heat = HEAT * pow(near, 4.0) * emission + 1e-3;
    let hot = (1.0 - exp(-heat * base)) / (1.0 - exp(-heat));
    emitted = hot * EMITTED * (lit_over + pow(near, 4.0) * CORE * emission);
    // the light also runs along the inside of the walls and comes out where the top
    // turns over into them, which rims the lit thing in its color
    let turned = 1.0 - clamp(flat_normal.z, 0.0, 1.0);
    emitted += base * sqrt(turned) * emission * 0.8 * EMITTED;
  }
  // taken everywhere, since a derivative needs every pixel around to take it
  let shown = cells(lit.world.xy, max(lit.textures.w, 1.0));
  emitted *= select(1.0, shown, lit.textures.w > 0.0);
  let color = min(diffuse_light * (1.0 - lit_over) + highlights + emitted, vec3<f32>(BRIGHTEST));
  // the share of the light that is the surface's own, which is what spills a haze later
  let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
  let own = dot(emitted, weights) / max(dot(color, weights), 1e-6);
  return vec4<f32>(color, clamp(own, 0.0, 1.0));
}
