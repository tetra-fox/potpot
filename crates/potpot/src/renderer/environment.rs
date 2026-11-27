//! The studio the panel is lit by, as `cargo xtask env` bakes it from
//! `crates/potpot/assets/source/wooden_studio_16_1k.hdr`. The file is written by the bake.

/// The width and height of each face of the specular map's first mip, in texels.
pub(super) const SIDE: u32 = 128;
/// The mips of the specular map, for roughness from zero to one in fifths.
pub(super) const MIPS: u32 = 6;
/// The light a white Lambert surface gives back, as nine spherical harmonics.
///
/// Each is linear rgb, in the order and basis of `scene.wgsl`'s `all_around`. The key
/// light's patch of the studio is left out, since the key light gives that light itself.
pub(super) const HARMONICS: [[f32; 3]; 9] = [
  [1.0427783, 1.0690334, 1.071771],
  [0.42465484, 0.4399646, 0.44289193],
  [0.58435714, 0.59704393, 0.5965259],
  [0.45094463, 0.4629289, 0.46654615],
  [0.09097253, 0.0946289, 0.09533875],
  [0.18310216, 0.18928304, 0.19024742],
  [0.06759766, 0.067110166, 0.06527221],
  [0.13541205, 0.13777775, 0.13669695],
  [-0.015707966, -0.017325656, -0.017473707],
];
/// The specular map's texels as `Rgb9e5Ufloat`, mip after mip, each mip's 6 faces in
/// turn, each face in rows from the top.
pub(super) const SPECULAR: &[u8] = include_bytes!("../../assets/specular.bin");
