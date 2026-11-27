//! The studio the panel is lit by, as `cargo xtask env` bakes it.
//!
//! The bake writes this file from `crates/potpot/assets/source/wooden_studio_16_1k.hdr`.

/// The width and height of each face of a specular map's first mip, in texels.
pub(super) const SIDE: u32 = 128;
/// The mips of each specular map, for roughness from zero to one in fifths.
pub(super) const MIPS: u32 = 6;
/// The light a white Lambert surface gives back, as nine spherical harmonics.
///
/// Each is linear rgb, in the order and basis of `scene.wgsl`'s `all_around`. The key
/// light's patch of the studio is left out, since the key light gives that light itself.
pub(super) const HARMONICS: [[f32; 3]; 9] = [
  [1.0427783, 1.0690334, 1.071771],
  [0.68920493, 0.70989054, 0.71421254],
  [0.4154448, 0.42228624, 0.42145798],
  [-0.27828574, -0.28647953, -0.2859917],
  [-0.1427448, -0.14752355, -0.14752668],
  [0.16929226, 0.17241529, 0.17162527],
  [-0.051350914, -0.055516127, -0.05778912],
  [-0.0852329, -0.086993545, -0.08655316],
  [-0.077580675, -0.08036428, -0.0806981],
];
/// The specular map of the studio without its softbox.
///
/// Its texels are `Rgb9e5Ufloat`, mip after mip, each mip's 6 faces in turn, each
/// face in rows from the top.
pub(super) const STUDIO: &[u8] = include_bytes!("../../assets/studio.bin");
/// The specular map of the softbox alone, laid out as [`STUDIO`] is.
pub(super) const SOFTBOX: &[u8] = include_bytes!("../../assets/softbox.bin");
