//! The bake of the texture sets a material can be drawn with.

use std::fmt::Write as _;
use std::path::Path;

use glam::Vec3;
use intel_tex_2::{bc4, bc5, bc7};

/// The sets in the order the renderer indexes them, each as its name in the generated source
/// and its ambientCG ID.
const SETS: [(&str, &str); 4] = [
  ("WALNUT", "Wood027"),
  ("HAMMERTONE", "Metal044B"),
  ("BRUSHED_ALUMINIUM", "Metal010"),
  ("POWDER_COAT", "Metal027"),
];
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
  for (_, id) in SETS {
    let source = |map: &str| root.join(SOURCES).join(format!("{id}_1K-JPG_{map}.jpg"));
    let color = halved(&read(&source("Color"))?.map(decode));
    let bent = halved(&read(&source("NormalGL"))?.map(|channel| channel * 2.0 - 1.0));
    let rough = halved(&read(&source("Roughness"))?);
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
  for (index, (name, id)) in SETS.iter().enumerate() {
    writeln!(sets, "/// The place of ambientCG's {id} among the sets.")
      .expect("a string takes any write");
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
     /// The color maps as `Bc7RgbaUnormSrgb`.\n\
     ///\n\
     /// They are mip after mip, each mip's layers in turn, each layer in rows of blocks from the\n\
     /// top. A mip narrower than a block fills one block, repeated across it.\n\
     pub(super) const ALBEDO: &[u8] = include_bytes!(\"../../assets/albedo.bin\");\n\
     /// The normal maps as `Bc5RgUnorm`, laid out as [`ALBEDO`] is.\n\
     ///\n\
     /// They hold x and y of the way the surface faces, with y toward the top of the picture.\n\
     pub(super) const NORMAL: &[u8] = include_bytes!(\"../../assets/normal.bin\");\n\
     /// The roughness maps as `Bc4RUnorm`, laid out as [`ALBEDO`] is.\n\
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
