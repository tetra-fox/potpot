//! The texture sets a material can be drawn with, as `cargo xtask assets` bakes them.
//!
//! The bake writes this file from the pictures in `crates/potpot/assets/source`.

/// The place of ambientCG's Wood027 among the sets.
pub(super) const WALNUT: u32 = 0;
/// The place of the bake's hammered paint among the sets.
pub(super) const HAMMERTONE: u32 = 1;
/// The place of ambientCG's Metal010 among the sets.
pub(super) const BRUSHED_ALUMINIUM: u32 = 2;
/// The place of ambientCG's Metal027 among the sets.
pub(super) const POWDER_COAT: u32 = 3;
/// The count of sets, the layers of each map.
pub(super) const SETS: u32 = 4;
/// The width and height of each map's first mip, in texels.
pub(super) const SIDE: u32 = 512;
/// The mips of each map, down to one texel.
pub(super) const MIPS: u32 = 10;
/// The mean of each set's color, in linear light.
pub(super) const MEAN_COLOR: [[f32; 3]; 4] = [
  [0.05037755, 0.016580101, 0.004772253],
  [0.9522974, 0.9522974, 0.9522974],
  [0.23380458, 0.26117778, 0.2894069],
  [0.027468812, 0.026291125, 0.03521369],
];
/// The mean of each set's roughness.
pub(super) const ROUGHNESS_MEAN: [f32; 4] = [0.48566517, 0.43933177, 0.4297225, 0.6491278];
/// The color maps as [`wgpu::TextureFormat::Bc7RgbaUnormSrgb`].
///
/// They are mip after mip, each mip's layers in turn, each layer in rows of blocks from the
/// top. A mip narrower than a block fills one block, repeated across it.
pub(super) const ALBEDO: &[u8] = include_bytes!("../../assets/albedo.bin");
/// The normal maps as [`wgpu::TextureFormat::Bc5RgUnorm`], laid out as [`ALBEDO`] is.
///
/// They hold x and y of the way the surface faces, with y toward the top of the picture.
pub(super) const NORMAL: &[u8] = include_bytes!("../../assets/normal.bin");
/// The roughness maps as [`wgpu::TextureFormat::Bc4RUnorm`], laid out as [`ALBEDO`] is.
pub(super) const ROUGHNESS: &[u8] = include_bytes!("../../assets/roughness.bin");
