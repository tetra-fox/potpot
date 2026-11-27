//! The bake of the texture sets a material can be drawn with.

use std::fmt::Write as _;
use std::path::Path;

use glam::{Vec2, Vec3};
use intel_tex_2::{bc4, bc5, bc7};

/// The sets in the order the renderer indexes them.
///
/// Each is its name in the generated source and where it comes from.
const SETS: [(&str, Source); 4] = [
  (
    "WALNUT",
    Source::Photo {
      id: "Wood027",
      even: false,
    },
  ),
  ("HAMMERTONE", Source::Hammered),
  (
    "BRUSHED_ALUMINIUM",
    Source::Photo {
      id: "Metal010",
      even: true,
    },
  ),
  (
    "POWDER_COAT",
    Source::Photo {
      id: "Metal027",
      even: false,
    },
  ),
];

/// Where a set's maps come from.
#[derive(Clone, Copy, Debug)]
enum Source {
  /// Photographs from ambientCG, by ID.
  ///
  /// `even` takes out the variation wider than [`EVEN_REACH`], such as stains, from its color
  /// and roughness, and keeps the fine detail.
  Photo { id: &'static str, even: bool },
  /// Hammered paint, made by the bake: shallow dishes of many sizes with raised rims.
  Hammered,
}

/// The width of the sources, in texels.
const SOURCE_SIDE: usize = 1024;
/// The width of the mean that evens out a set, in texels of the source.
///
/// A set's color and roughness are divided by their mean over this many texels around each.
const EVEN_REACH: usize = 24;
/// The steps across one tile of the grid hammered paint's dishes are scattered over.
const GRID: usize = 70;
/// The most of the grid's points a place leaves out, where its dishes run largest.
const LEFT_OUT: f32 = 0.75;
/// The steps across one tile of the field that says where dishes run larger.
const CLUSTERS: usize = 5;
/// The radius of the smallest dishes of hammered paint, in grid steps.
const RADIUS: f32 = 0.75;
/// The grid steps to each side a place looks for the points nearest it.
const SEARCH: i64 = 3;
/// How far below a ridge between two dishes its rim reaches, in grid steps.
const RIM: f32 = 0.03;
/// How deep a dish of hammered paint is, as a share of its radius.
const DISH_DEPTH: f32 = 0.1;
/// The folder the sources are in, each `<ID>_1K-JPG_<map>.jpg` as ambientCG names them.
const SOURCES: &str = "crates/potpot/assets/source";
/// The color maps of every set, as one array.
pub(crate) const ALBEDO: &str = "crates/potpot/assets/albedo.bin";
/// The normal maps of every set, as one array.
pub(crate) const NORMAL: &str = "crates/potpot/assets/normal.bin";
/// The roughness maps of every set, as one array.
pub(crate) const ROUGHNESS: &str = "crates/potpot/assets/roughness.bin";
/// The Rust source that names the maps and holds what the renderer knows of each set.
const GENERATED: &str = "crates/potpot/src/renderer/textures.rs";
/// The width and height of each map's first mip, in texels.
const SIDE: usize = 512;
/// The side of a compressed block, in texels.
const BLOCK: usize = 4;

/// Bakes the texture sets under the workspace at `root` into the renderer's assets.
pub fn bake(root: &Path) -> Result<(), String> {
  let mut albedo = Vec::new();
  let mut normal = Vec::new();
  let mut roughness = Vec::new();
  let mut mean_color = Vec::new();
  let mut rough_mean = Vec::new();
  for (_, source) in SETS {
    let (color, bent, rough) = match source {
      Source::Photo { id, even } => {
        let path = |map: &str| root.join(SOURCES).join(format!("{id}_1K-JPG_{map}.jpg"));
        let mut color = read(&path("Color"))?.map(decode);
        let mut rough = read(&path("Roughness"))?;
        if even {
          color = evened(&color);
          rough = evened(&rough);
        }
        (
          color,
          read(&path("NormalGL"))?.map(|channel| channel * 2.0 - 1.0),
          rough,
        )
      }
      Source::Hammered => hammered(),
    };
    let (color, bent, rough) = (halved(&color), halved(&bent), halved(&rough));
    mean_color.push(color.texels.iter().sum::<Vec3>() / color.texels.len() as f32);
    rough_mean
      .push(rough.texels.iter().map(|texel| texel.x).sum::<f32>() / rough.texels.len() as f32);
    albedo.push(color);
    normal.push(bent);
    roughness.push(rough);
  }
  let mips = SIDE.ilog2() as usize + 1;
  let mut encoded = [Vec::new(), Vec::new(), Vec::new()];
  for mip in 0..mips {
    for set in 0..SETS.len() {
      let (color, bent, rough) = (&albedo[set], &normal[set], &roughness[set]);
      let rgba: Vec<u8> = color
        .texels
        .iter()
        .flat_map(|light| {
          let [r, g, b] = light.to_array().map(|channel| unorm(encode(channel)));
          [r, g, b, 255]
        })
        .collect();
      let rg: Vec<u8> = bent
        .texels
        .iter()
        .flat_map(|toward| {
          let toward = toward.normalize_or(Vec3::Z);
          [unorm(toward.x * 0.5 + 0.5), unorm(toward.y * 0.5 + 0.5)]
        })
        .collect();
      let r: Vec<u8> = rough.texels.iter().map(|texel| unorm(texel.x)).collect();
      let side = color.side;
      let settings = bc7::opaque_basic_settings();
      encoded[0].extend(bc7::compress_blocks(
        &settings,
        &surface::<4>(&blocked(&rgba, side, 4)),
      ));
      encoded[1].extend(bc5::compress_blocks(&surface::<2>(&blocked(&rg, side, 2))));
      encoded[2].extend(bc4::compress_blocks(&surface::<1>(&blocked(&r, side, 1))));
    }
    if mip + 1 < mips {
      for maps in [&mut albedo, &mut normal, &mut roughness] {
        for map in maps.iter_mut() {
          *map = halved(map);
        }
      }
    }
  }
  for (path, bytes) in [ALBEDO, NORMAL, ROUGHNESS].into_iter().zip(encoded) {
    let path = root.join(path);
    std::fs::write(&path, bytes).map_err(|error| format!("{}: {error}", path.display()))?;
  }
  crate::write_source(
    &root.join(GENERATED),
    &generated_source(mips, &mean_color, &rough_mean),
  )
}

/// A square map of up to three channels, in rows from the top.
#[derive(Clone, Debug)]
struct Map {
  side: usize,
  texels: Vec<Vec3>,
}

impl Map {
  /// Returns the map with each channel changed by `change`.
  fn map(self, change: impl Fn(f32) -> f32) -> Self {
    let texels = self
      .texels
      .into_iter()
      .map(|texel| texel.to_array().map(&change).into());
    Self {
      side: self.side,
      texels: texels.collect(),
    }
  }
}

/// Returns the square picture a JPEG file holds, each channel from zero to one.
///
/// A grey picture has its one channel in x.
fn read(path: &Path) -> Result<Map, String> {
  let failed = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
  let file = std::fs::File::open(path).map_err(|error| failed(&error))?;
  let mut decoder = jpeg_decoder::Decoder::new(std::io::BufReader::new(file));
  let pixels = decoder.decode().map_err(|error| failed(&error))?;
  let info = decoder
    .info()
    .ok_or_else(|| failed(&"the picture has no size"))?;
  let (width, height) = (usize::from(info.width), usize::from(info.height));
  if width != height || !width.is_power_of_two() {
    return Err(failed(&format!(
      "the picture is {width} by {height}, not a power of two square"
    )));
  }
  let channels = match info.pixel_format {
    jpeg_decoder::PixelFormat::L8 => 1,
    jpeg_decoder::PixelFormat::RGB24 => 3,
    other => return Err(failed(&format!("the pixels are {other:?}"))),
  };
  let texels = pixels
    .chunks_exact(channels)
    .map(|pixel| {
      let value = |index: usize| f32::from(pixel[index.min(channels - 1)]) / 255.0;
      Vec3::new(value(0), value(1), value(2))
    })
    .collect();
  Ok(Map {
    side: width,
    texels,
  })
}

/// Returns a map with its variation wider than [`EVEN_REACH`] taken out.
///
/// Each texel is divided by the mean around it, wrapped as the map tiles, and multiplied by
/// the mean of the whole map.
fn evened(map: &Map) -> Map {
  let mut wide = map.clone();
  // three box blurs each way come close to a Gaussian
  for _ in 0..3 {
    wide = blurred(&blurred(&wide, 1, 0), 0, 1);
  }
  let mean = map.texels.iter().sum::<Vec3>() / map.texels.len() as f32;
  let texels = (map.texels.iter().zip(&wide.texels))
    .map(|(texel, around)| *texel / around.max(Vec3::splat(1e-4)) * mean)
    .collect();
  Map {
    side: map.side,
    texels,
  }
}

/// Returns a map averaged over [`EVEN_REACH`] texels along one axis, wrapped.
fn blurred(map: &Map, dx: usize, dy: usize) -> Map {
  let side = map.side;
  let reach = EVEN_REACH / 2;
  let count = (2 * reach + 1) as f32;
  let texels = (0..side * side)
    .map(|index| {
      let (x, y) = (index % side, index / side);
      let mut sum = Vec3::ZERO;
      for step in 0..=2 * reach {
        let along = step + side - reach;
        sum += map.texels[(y + along * dy) % side * side + (x + along * dx) % side];
      }
      sum / count
    })
    .collect();
  Map { side, texels }
}

/// Returns the color, the way the relief faces and the roughness of hammered paint.
///
/// Hammered paint dries into shallow dishes with a narrow raised rim where two dishes meet. The
/// dishes are cells around scattered points: most are small, a few are large, and they run
/// larger in some places than others. Some rims are faint, which merges two dishes. The key
/// light catches each dish on the side away from it and each rim on the side toward it, which
/// is what the finish reads by, so the color is nearly even. The rims are a little smoother
/// than the floors, so they glint.
fn hammered() -> (Map, Map, Map) {
  let side = SOURCE_SIDE;
  let field = Dishes::new();
  // the height of each texel, in grid steps, how strong the rim is there, and its cell
  let texels: Vec<(f32, f32, usize)> = (0..side * side)
    .map(|index| {
      let at = Vec2::new((index % side) as f32, (index / side) as f32) / side as f32;
      let near = field.near(at);
      let rim = (1.0 - smoothstep(0.0, RIM, near.border)) * near.rim;
      (near.height, rim, near.cell)
    })
    .collect();
  let height = |x: usize, y: usize| texels[(y % side) * side + x % side].0;
  let texels_per_step = side as f32 / GRID as f32;
  let mut bent = Vec::with_capacity(side * side);
  let mut rough = Vec::with_capacity(side * side);
  let mut color = Vec::with_capacity(side * side);
  for (index, &(_, rim, cell)) in texels.iter().enumerate() {
    let (x, y) = (index % side, index / side);
    // slopes in heights a grid step, and the picture's y runs up, its rows down
    let across = (height(x + 1, y) - height(x + side - 1, y)) * texels_per_step / 2.0;
    let up = (height(x, y + side - 1) - height(x, y + 1)) * texels_per_step / 2.0;
    bent.push(Vec3::new(-across, -up, 1.0).normalize());
    rough.push(Vec3::splat(0.45 - 0.15 * rim));
    let tone = 0.98 + 0.04 * scattered(cell as u32, 0, 5);
    color.push(Vec3::splat(tone * (1.0 - 0.12 * rim)));
  }
  let map = |texels| Map { side, texels };
  (map(color), map(bent), map(rough))
}

/// The round dishes hammered paint dries into, over a tiling grid of [`GRID`] steps a side.
///
/// Each step of the grid holds at most one dish's middle. A smooth field that tiles leaves out
/// more of them in some places, and grows the dishes there. Each dish's width is also drawn
/// from a skewed spread, so most are small and a few are large. Where dishes overlap the
/// surface is the deepest of them, so two meet in a ridge.
#[derive(Debug)]
struct Dishes {
  /// Each grid step's dish, if it has one.
  ///
  /// A dish is its middle in the grid, its radius in grid steps and how strong its rim is.
  points: Vec<Option<(Vec2, f32, f32)>>,
}

/// Where a place is among the dishes of hammered paint.
#[derive(Clone, Copy, Debug)]
struct Near {
  /// The height of the surface there, in grid steps, zero where no dish reaches.
  height: f32,
  /// How far the place is below the ridge where its dish meets the next, in grid steps.
  border: f32,
  /// How strong the rim there is, from the two cells either side of it.
  rim: f32,
  /// The grid step its cell's point is in.
  cell: usize,
}

impl Dishes {
  /// Creates the points of the dishes.
  fn new() -> Self {
    let points = (0..GRID * GRID)
      .map(|step| {
        let (x, y) = ((step % GRID) as u32, (step / GRID) as u32);
        let at = Vec2::new(x as f32, y as f32) / GRID as f32;
        let cluster = clustered(at);
        let kept = 1.0 - LEFT_OUT * cluster;
        (scattered(x, y, 21) < kept).then(|| {
          let place = Vec2::new(x as f32 + scattered(x, y, 1), y as f32 + scattered(x, y, 8));
          let radius = RADIUS * (1.0 + 0.6 * cluster) * (0.8 + 1.2 * scattered(x, y, 3).powi(3));
          let rim = 0.25 + 0.75 * scattered(x, y, 9);
          (place, radius, rim)
        })
      })
      .collect();
    Self { points }
  }

  /// Returns where a place of the unit square is among the dishes.
  ///
  /// A place is in the deepest dish there. The grid wraps, so the dishes are the same at
  /// opposite edges of the square.
  fn near(&self, at: Vec2) -> Near {
    let scaled = at * GRID as f32;
    let (cx, cy) = (scaled.x.floor() as i64, scaled.y.floor() as i64);
    let wrap = |n: i64| n.rem_euclid(GRID as i64);
    let mut best = [(0.0, 0.0, 0); 2];
    for dy in -SEARCH..=SEARCH {
      for dx in -SEARCH..=SEARCH {
        let (x, y) = (cx + dx, cy + dy);
        let step = (wrap(y) * GRID as i64 + wrap(x)) as usize;
        let Some((place, radius, rim)) = self.points[step] else {
          continue;
        };
        // the middle as it lies in this copy of the grid
        let place = place + Vec2::new((x - wrap(x)) as f32, (y - wrap(y)) as f32);
        // a bowl as deep as it is wide, by DISH_DEPTH, that meets the surface at its radius
        let out = (place.distance(scaled) / radius).min(1.0);
        let depth = (out * out - 1.0) * radius * DISH_DEPTH;
        if depth < best[0].0 {
          best = [(depth, rim, step), best[0]];
        } else if depth < best[1].0 {
          best[1] = (depth, rim, step);
        }
      }
    }
    let [(deepest, rim, cell), (next, other_rim, _)] = best;
    Near {
      height: deepest,
      border: next - deepest,
      rim: (rim + other_rim) / 2.0,
      cell,
    }
  }
}

/// Returns a smooth field from zero to one over the unit square, which tiles.
///
/// It is value noise on a lattice of [`CLUSTERS`] steps a side, eased between lattice points.
fn clustered(at: Vec2) -> f32 {
  let place = at * CLUSTERS as f32;
  let (x, y) = (place.x.floor() as i64, place.y.floor() as i64);
  let wrap = |n: i64| n.rem_euclid(CLUSTERS as i64) as u32;
  let value = |dx: i64, dy: i64| scattered(wrap(x + dx), wrap(y + dy), 31);
  let part = place - place.floor();
  let ease = part * part * (Vec2::splat(3.0) - 2.0 * part);
  let top = value(0, 0) + (value(1, 0) - value(0, 0)) * ease.x;
  let foot = value(0, 1) + (value(1, 1) - value(0, 1)) * ease.x;
  top + (foot - top) * ease.y
}

/// Returns zero below `low`, one above `high`, and a smooth step between.
fn smoothstep(low: f32, high: f32, at: f32) -> f32 {
  let t = ((at - low) / (high - low)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

/// Returns a number from zero to one, the same every time for a cell and seed.
fn scattered(x: u32, y: u32, seed: u32) -> f32 {
  // the pcg hash
  let state = (x.wrapping_mul(1973) ^ y.wrapping_mul(9277) ^ seed.wrapping_mul(26699))
    .wrapping_mul(747_796_405)
    .wrapping_add(2_891_336_453);
  let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
  ((word >> 22) ^ word) as f32 / 4_294_967_296.0
}

/// Returns the map half as wide, each texel the mean of the four under it.
///
/// A map one texel wide stays as it is.
fn halved(map: &Map) -> Map {
  if map.side == 1 {
    return map.clone();
  }
  let side = map.side / 2;
  let texel = |x: usize, y: usize| map.texels[y * map.side + x];
  let texels = (0..side * side)
    .map(|index| {
      let (x, y) = (index % side * 2, index / side * 2);
      (texel(x, y) + texel(x + 1, y) + texel(x, y + 1) + texel(x + 1, y + 1)) / 4.0
    })
    .collect();
  Map { side, texels }
}

/// Returns a map's bytes grown to whole blocks.
///
/// A map narrower than a block repeats across it, as the map tiles.
fn blocked(bytes: &[u8], side: usize, channels: usize) -> (Vec<u8>, usize) {
  let grown = side.max(BLOCK);
  let mut out = Vec::with_capacity(grown * grown * channels);
  for y in 0..grown {
    for x in 0..grown {
      let at = ((y % side) * side + x % side) * channels;
      out.extend_from_slice(&bytes[at..at + channels]);
    }
  }
  (out, grown)
}

/// Returns a surface of `CHANNELS` bytes to a texel over a map made whole blocks.
fn surface<const CHANNELS: usize>(
  (bytes, side): &(Vec<u8>, usize),
) -> intel_tex_2::Surface<'_, CHANNELS> {
  intel_tex_2::Surface {
    data: bytes,
    width: *side as u32,
    height: *side as u32,
    stride: (*side * CHANNELS) as u32,
  }
}

/// Returns a channel from zero to one as a byte.
fn unorm(channel: f32) -> u8 {
  (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Returns a channel of sRGB-encoded color as linear light.
fn decode(encoded: f32) -> f32 {
  if encoded <= 0.04045 {
    encoded / 12.92
  } else {
    ((encoded + 0.055) / 1.055).powf(2.4)
  }
}

/// Returns a channel of linear light as sRGB-encoded color.
fn encode(linear: f32) -> f32 {
  if linear <= 0.003_130_8 {
    linear * 12.92
  } else {
    1.055 * linear.powf(1.0 / 2.4) - 0.055
  }
}

/// Returns the Rust source that names the maps and holds the sets' means.
fn generated_source(mips: usize, color: &[Vec3], roughness: &[f32]) -> String {
  let mut sets = String::new();
  for (index, (name, source)) in SETS.iter().enumerate() {
    let made = match source {
      Source::Photo { id, .. } => format!("ambientCG's {id}"),
      Source::Hammered => "the bake's hammered paint".to_string(),
    };
    writeln!(sets, "/// The place of {made} among the sets.").expect("a string takes any write");
    writeln!(sets, "pub(super) const {name}: u32 = {index};").expect("a string takes any write");
  }
  let listed = |numbers: &[f32]| {
    let numbers: Vec<String> = numbers.iter().map(|number| format!("{number:?}")).collect();
    numbers.join(", ")
  };
  let count = SETS.len();
  format!(
    "//! The texture sets a material can be drawn with, as `cargo xtask assets` bakes them.\n\
     //!\n\
     //! The bake writes this file from the pictures in `{SOURCES}`.\n\
     \n\
     {sets}\
     /// The count of sets, the layers of each map.\n\
     pub(super) const SETS: u32 = {count};\n\
     /// The width and height of each map's first mip, in texels.\n\
     pub(super) const SIDE: u32 = {SIDE};\n\
     /// The mips of each map, down to one texel.\n\
     pub(super) const MIPS: u32 = {mips};\n\
     /// The mean of each set's color, in linear light.\n\
     pub(super) const MEAN_COLOR: [[f32; 3]; {count}] = [{}];\n\
     /// The mean of each set's roughness.\n\
     pub(super) const ROUGHNESS_MEAN: [f32; {count}] = [{}];\n\
     /// The color maps as [`wgpu::TextureFormat::Bc7RgbaUnormSrgb`].\n\
     ///\n\
     /// They are mip after mip, each mip's layers in turn, each layer in rows of blocks from the\n\
     /// top. A mip narrower than a block fills one block, repeated across it.\n\
     pub(super) const ALBEDO: &[u8] = include_bytes!(\"../../assets/albedo.bin\");\n\
     /// The normal maps as [`wgpu::TextureFormat::Bc5RgUnorm`], laid out as [`ALBEDO`] is.\n\
     ///\n\
     /// They hold x and y of the way the surface faces, with y toward the top of the picture.\n\
     pub(super) const NORMAL: &[u8] = include_bytes!(\"../../assets/normal.bin\");\n\
     /// The roughness maps as [`wgpu::TextureFormat::Bc4RUnorm`], laid out as [`ALBEDO`] is.\n\
     pub(super) const ROUGHNESS: &[u8] = include_bytes!(\"../../assets/roughness.bin\");\n",
    color
      .iter()
      .map(|mean| format!("[{}]", listed(&mean.to_array())))
      .collect::<Vec<_>>()
      .join(", "),
    listed(roughness),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_dishes_tile() {
    let dishes = Dishes::new();
    for along in [0.0, 0.13, 0.5, 0.91] {
      let pairs = [
        (Vec2::new(0.0, along), Vec2::new(1.0, along)),
        (Vec2::new(along, 0.0), Vec2::new(along, 1.0)),
      ];
      for (one, other) in pairs {
        let (one, other) = (dishes.near(one), dishes.near(other));
        assert!((one.border - other.border).abs() < 1e-4);
        assert_eq!(one.cell, other.cell);
      }
    }
  }

  #[test]
  fn evening_keeps_fine_detail_and_takes_out_wide_patches() {
    let side = 128;
    // stripes one texel apart, over a patch twice as bright on the left half
    let texels = (0..side * side)
      .map(|index| {
        let (x, y) = (index % side, index / side);
        let stripe = if y % 2 == 0 { 1.1 } else { 0.9 };
        Vec3::splat(stripe * if x < side / 2 { 2.0 } else { 1.0 })
      })
      .collect();
    let even = evened(&Map { side, texels });
    let at = |x: usize, y: usize| even.texels[y * side + x].x;
    // the middles of the two halves come out alike, and the stripes stay
    let (left, right) = (at(side / 4, 10), at(3 * side / 4, 10));
    assert!((left - right).abs() < 0.02 * left, "{left} against {right}");
    assert!(at(side / 4, 11) < 0.9 * left);
  }

  #[test]
  fn a_map_halves_to_the_means_of_its_quarters() {
    let map = Map {
      side: 2,
      texels: vec![Vec3::ZERO, Vec3::ONE, Vec3::splat(2.0), Vec3::splat(3.0)],
    };
    let half = halved(&map);
    assert_eq!((half.side, half.texels), (1, vec![Vec3::splat(1.5)]));
  }

  #[test]
  fn a_map_narrower_than_a_block_repeats_across_it() {
    let (bytes, side) = blocked(&[1, 2, 3, 4], 2, 1);
    assert_eq!(side, 4);
    assert_eq!(&bytes[..8], &[1, 2, 1, 2, 3, 4, 3, 4]);
    assert_eq!(&bytes[8..], &bytes[..8]);
  }
}
