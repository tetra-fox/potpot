//! The bake of the studio a panel is lit by, from a picture of all around it.

mod cube;
mod harmonics;
mod shared_exponent;

use std::f32::consts::PI;
use std::fmt::Write as _;
use std::path::Path;

use potpot::glam::{Mat3, Quat, Vec3};

use self::cube::{Cube, FACES, Level};
use crate::hdr::{self, Picture};

/// The studio, as a latitude-longitude picture.
const SOURCE: &str = "crates/potpot/assets/source/wooden_studio_16_1k.hdr";
/// The mips of the specular cube map.
const SPECULAR: &str = "crates/potpot/assets/specular.bin";
/// The Rust source that holds the diffuse light and names the specular map.
const GENERATED: &str = "crates/potpot/src/renderer/environment.rs";
/// The width of the specular cube map's first mip, in texels.
const SIDE: usize = 128;
/// The mips of the specular map, for roughness from zero to one in fifths.
const MIPS: usize = 6;
/// The width of the cube the studio is put on before it is prefiltered, in texels.
const SOURCE_SIDE: usize = 256;
/// The ways each texel of a rough mip is the mean of.
const SAMPLES: u32 = 1024;
/// The share of the brightest light at which the key light's patch of the picture ends.
const PATCH_EDGE: f32 = 0.08;
/// The side of the cells the picture is averaged over to find the key light's patch.
const CELL: usize = 4;

/// Bakes the studio under the workspace at `root` into the renderer's assets.
pub fn bake(root: &Path) -> Result<(), String> {
  let source = root.join(SOURCE);
  let bytes = std::fs::read(&source).map_err(|error| format!("{}: {error}", source.display()))?;
  let picture = hdr::read(&bytes).map_err(|error| format!("{}: {error}", source.display()))?;
  let patch = key_patch(&picture);
  let studio = Studio::new(&picture, &patch);

  let cube = Cube::new(Level::from_fn(SOURCE_SIDE, |out| studio.light(out)));
  let mut texels = Vec::new();
  for mip in 0..MIPS {
    let level = cube.prefiltered(SIDE >> mip, mip as f32 / (MIPS - 1) as f32, SAMPLES);
    let encoded = level
      .texels
      .iter()
      .map(|&light| shared_exponent::encoded(light));
    texels.extend(encoded.flat_map(u32::to_le_bytes));
  }
  let specular = root.join(SPECULAR);
  std::fs::write(&specular, texels).map_err(|error| format!("{}: {error}", specular.display()))?;

  // the key light's patch is left out of the diffuse light, which the key light gives itself
  let samples = latitude_longitude(picture.width, picture.height)
    .filter(|&(index, _, _)| !patch.inside[index])
    .map(|(index, out, angle)| {
      (
        studio.turn * out,
        picture.pixels[index] * studio.scale,
        angle,
      )
    });
  let diffuse = harmonics::diffused(harmonics::projected(samples));
  let generated = root.join(GENERATED);
  std::fs::write(&generated, generated_source(&diffuse))
    .map_err(|error| format!("{}: {error}", generated.display()))?;
  let formatted = std::process::Command::new("rustfmt")
    .args(["--edition", "2024"])
    .arg(&generated)
    .status()
    .map_err(|error| format!("rustfmt could not run: {error}"))?;
  if !formatted.success() {
    return Err(format!("rustfmt failed on {}", generated.display()));
  }
  Ok(())
}

/// The light of the key light's patch of the picture.
#[derive(Debug)]
struct Patch {
  /// Whether each pixel of the picture is in the patch.
  inside: Vec<bool>,
  /// The way to the patch's middle, weighed by its light, in the picture's own space.
  middle: Vec3,
  /// The light the patch throws on a surface facing its middle.
  irradiance: Vec3,
}

/// The studio turned and scaled to light a panel.
#[derive(Debug)]
struct Studio<'a> {
  picture: &'a Picture,
  /// The turn from the picture's space to the panel's.
  turn: Mat3,
  /// What the picture's light is multiplied by, in each channel.
  scale: Vec3,
}

impl<'a> Studio<'a> {
  /// Creates the studio turned so that the key light's patch lies where the panel's key light
  /// comes from, and scaled so that the patch throws as much light as the key light and in its
  /// color, as a camera balanced to the key light sees it.
  ///
  /// The panel lies on the studio's floor face up, as for a shot from above: the studio's up
  /// is the panel's +z, so that flat faces mirror its ceiling and the sides of things its floor.
  fn new(picture: &'a Picture, patch: &Patch) -> Self {
    let key = potpot::Light::default();
    let toward = key.toward.normalize();
    let lying = Mat3::from_cols(Vec3::X, Vec3::Z, Vec3::NEG_Y);
    let middle = lying * patch.middle;
    // turned around the studio's up until the patch is on the key light's side, then tipped
    // the least way onto it
    let around = (toward.y.atan2(toward.x)) - (middle.y.atan2(middle.x));
    let spun = Mat3::from_rotation_z(around) * lying;
    let tip = Quat::from_rotation_arc(spun * patch.middle, toward);
    Self {
      picture,
      turn: Mat3::from_quat(tip) * spun,
      // the renderer's light colors are irradiance over pi, as a white surface gives it back
      scale: key.color * PI / patch.irradiance,
    }
  }

  /// Returns the light from a way out of the panel's space.
  fn light(&self, out: Vec3) -> Vec3 {
    sampled(self.picture, self.turn.transpose() * out) * self.scale
  }
}

/// Returns the patch of the picture around its brightest light.
///
/// The picture is averaged over cells to find it, so that a lone hot pixel does not pass for a
/// light. The patch is every cell joined to the brightest that is at least [`PATCH_EDGE`] as
/// bright.
fn key_patch(picture: &Picture) -> Patch {
  let (across, down) = (picture.width / CELL, picture.height / CELL);
  let luminance = |light: Vec3| light.dot(Vec3::new(0.2126, 0.7152, 0.0722));
  let cells: Vec<f32> = (0..across * down)
    .map(|cell| {
      let (cx, cy) = (cell % across, cell / across);
      let pixels = (0..CELL * CELL).map(|at| {
        let (x, y) = (cx * CELL + at % CELL, cy * CELL + at / CELL);
        luminance(picture.pixels[y * picture.width + x])
      });
      pixels.sum::<f32>() / (CELL * CELL) as f32
    })
    .collect();
  let (brightest, peak) = cells
    .iter()
    .copied()
    .enumerate()
    .max_by(|a, b| a.1.total_cmp(&b.1))
    .expect("the picture has cells");
  let mut in_patch = vec![false; cells.len()];
  let mut open = vec![brightest];
  in_patch[brightest] = true;
  while let Some(cell) = open.pop() {
    let (cx, cy) = (cell % across, cell / across);
    // the picture wraps around left to right
    let next = [
      Some((cx + 1) % across + cy * across),
      Some((cx + across - 1) % across + cy * across),
      (cy + 1 < down).then(|| cx + (cy + 1) * across),
      cy.checked_sub(1).map(|up| cx + up * across),
    ];
    for neighbour in next.into_iter().flatten() {
      if !in_patch[neighbour] && cells[neighbour] >= peak * PATCH_EDGE {
        in_patch[neighbour] = true;
        open.push(neighbour);
      }
    }
  }
  let inside: Vec<bool> = (0..picture.pixels.len())
    .map(|index| {
      let (x, y) = (index % picture.width, index / picture.width);
      let (cx, cy) = ((x / CELL).min(across - 1), (y / CELL).min(down - 1));
      in_patch[cy * across + cx]
    })
    .collect();
  let mut middle = Vec3::ZERO;
  for (index, out, angle) in latitude_longitude(picture.width, picture.height) {
    if inside[index] {
      middle += out * luminance(picture.pixels[index]) * angle;
    }
  }
  let middle = middle.normalize();
  let irradiance = latitude_longitude(picture.width, picture.height)
    .filter(|&(index, _, _)| inside[index])
    .map(|(index, out, angle)| picture.pixels[index] * out.dot(middle).max(0.0) * angle)
    .sum();
  Patch {
    inside,
    middle,
    irradiance,
  }
}

/// Returns each pixel of a latitude-longitude picture with the way out through its middle and
/// the solid angle it covers.
///
/// The picture's top row looks straight up (+y), and its middle column along +z.
fn latitude_longitude(width: usize, height: usize) -> impl Iterator<Item = (usize, Vec3, f32)> {
  (0..width * height).map(move |index| {
    let (x, y) = (index % width, index / width);
    let longitude = ((x as f32 + 0.5) / width as f32 - 0.5) * 2.0 * PI;
    let latitude = (0.5 - (y as f32 + 0.5) / height as f32) * PI;
    let top = (0.5 - y as f32 / height as f32) * PI;
    let foot = (0.5 - (y + 1) as f32 / height as f32) * PI;
    let angle = 2.0 * PI / width as f32 * (top.sin() - foot.sin());
    let out = Vec3::new(
      latitude.cos() * longitude.sin(),
      latitude.sin(),
      latitude.cos() * longitude.cos(),
    );
    (index, out, angle)
  })
}

/// Returns the light of a latitude-longitude picture along a way out, mixed from the four
/// nearest pixels.
fn sampled(picture: &Picture, out: Vec3) -> Vec3 {
  let out = out.normalize();
  let longitude = out.x.atan2(out.z);
  let latitude = out.y.clamp(-1.0, 1.0).asin();
  let x = (longitude / (2.0 * PI) + 0.5) * picture.width as f32 - 0.5;
  let y =
    ((0.5 - latitude / PI) * picture.height as f32 - 0.5).clamp(0.0, (picture.height - 1) as f32);
  let (x0, y0) = (x.floor(), y.floor());
  let (fx, fy) = (x - x0, y - y0);
  let wrap = |x: f32| (x as isize).rem_euclid(picture.width as isize) as usize;
  let row = |y: f32| (y as usize).min(picture.height - 1) * picture.width;
  let pixel = |x: f32, y: f32| picture.pixels[row(y) + wrap(x)];
  let top = pixel(x0, y0).lerp(pixel(x0 + 1.0, y0), fx);
  let foot = pixel(x0, y0 + 1.0).lerp(pixel(x0 + 1.0, y0 + 1.0), fx);
  top.lerp(foot, fy)
}

/// Returns the Rust source of the baked studio.
fn generated_source(diffuse: &[Vec3; 9]) -> String {
  let mut harmonics = String::new();
  for coefficient in diffuse {
    let [r, g, b] = coefficient.to_array();
    writeln!(harmonics, "  [{r:?}, {g:?}, {b:?}],").expect("a string takes any write");
  }
  format!(
    "//! The studio the panel is lit by, as `cargo xtask env` bakes it from\n\
     //! `{SOURCE}`. The file is written by the bake.\n\
     \n\
     /// The width and height of each face of the specular map's first mip, in texels.\n\
     pub(super) const SIDE: u32 = {SIDE};\n\
     /// The mips of the specular map, for roughness from zero to one in fifths.\n\
     pub(super) const MIPS: u32 = {MIPS};\n\
     /// The light a white Lambert surface gives back, as nine spherical harmonics.\n\
     ///\n\
     /// Each is linear rgb, in the order and basis of `scene.wgsl`'s `all_around`. The key\n\
     /// light's patch of the studio is left out, since the key light gives that light itself.\n\
     pub(super) const HARMONICS: [[f32; 3]; 9] = [\n{harmonics}];\n\
     /// The specular map's texels as `Rgb9e5Ufloat`, mip after mip, each mip's {FACES} faces in\n\
     /// turn, each face in rows from the top.\n\
     pub(super) const SPECULAR: &[u8] = include_bytes!(\"../../assets/specular.bin\");\n"
  )
}
