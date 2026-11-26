//! The textures the scene is drawn into before it is stretched onto the target.

use super::damage::HAZE_CELL;
use super::pipelines::{DEPTH, GLOW, Layouts, PICTURE, SAMPLES};

/// The picture's textures at one size.
#[derive(Debug)]
pub(super) struct Picture {
  pub(super) size: [u32; 2],
  /// The texture the scene is drawn into when edges are smoothed.
  ///
  /// It is resolved into `color`.
  pub(super) smoothed: Option<wgpu::TextureView>,
  pub(super) color: wgpu::TextureView,
  pub(super) depth: wgpu::TextureView,
  /// The glow map, of what lights itself in the picture.
  ///
  /// It has a texel for each cell of [`HAZE_CELL`] pixels by as many, with the mean over the
  /// cell of each pixel's color times how much it lights itself. Around those it has one more
  /// row and column on every side, which stay dark as everything past the picture's edge is.
  pub(super) glow: wgpu::TextureView,
  /// The glow map blurred across, and then down.
  ///
  /// The haze is read from the last. Each is the glow map's size, and its cells around the
  /// picture hold the glow spread past the edge.
  pub(super) spread: [wgpu::TextureView; 2],
  /// The color alone, for the glow pass to read.
  pub(super) lit: wgpu::BindGroup,
  /// What the passes that blur across and then down read.
  ///
  /// That is the present options and the map each blurs.
  pub(super) unspread: [wgpu::BindGroup; 2],
  /// What the present pass reads.
  ///
  /// That is the color with its sampler, the present options, and the blurred glow map with
  /// its sampler.
  pub(super) group: wgpu::BindGroup,
}

impl Picture {
  /// Creates the textures of a picture.
  ///
  /// `options` is the buffer the present layout's third binding reads.
  pub(super) fn new(
    device: &wgpu::Device,
    layouts: &Layouts,
    options: &wgpu::Buffer,
    size: [u32; 2],
    smooth: bool,
  ) -> Self {
    let texture = |label, format, usage, sample_count, [width, height]: [u32; 2]| {
      device
        .create_texture(&wgpu::TextureDescriptor {
          label: Some(label),
          size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
          },
          mip_level_count: 1,
          sample_count,
          dimension: wgpu::TextureDimension::D2,
          format,
          usage,
          view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let color = texture(
      "potpot picture",
      PICTURE,
      wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
      1,
      size,
    );
    let map = |label| {
      texture(
        label,
        GLOW,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        1,
        cells(size),
      )
    };
    let glow = map("potpot glow");
    let spread = [map("potpot spread across"), map("potpot spread down")];
    let samples = if smooth { SAMPLES } else { 1 };
    let smoothed = smooth.then(|| {
      texture(
        "potpot picture samples",
        PICTURE,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        samples,
        size,
      )
    });
    let depth = texture(
      "potpot picture depth",
      DEPTH,
      wgpu::TextureUsages::RENDER_ATTACHMENT,
      samples,
      size,
    );
    // each rendered pixel lands on the window as a hard-edged block
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot picture"),
      mag_filter: wgpu::FilterMode::Nearest,
      min_filter: wgpu::FilterMode::Nearest,
      ..Default::default()
    });
    // a place between the middles of four cells is given a mix of them
    let haze_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot haze"),
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      ..Default::default()
    });
    let lit = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot glow"),
      layout: &layouts.glow,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::TextureView(&color),
      }],
    });
    let unspread = [&glow, &spread[0]].map(|unspread| {
      device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("potpot spread"),
        layout: &layouts.spread,
        entries: &[
          wgpu::BindGroupEntry {
            binding: 2,
            resource: options.as_entire_binding(),
          },
          wgpu::BindGroupEntry {
            binding: 5,
            resource: wgpu::BindingResource::TextureView(unspread),
          },
        ],
      })
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot picture"),
      layout: &layouts.present,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&color),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: wgpu::BindingResource::Sampler(&sampler),
        },
        wgpu::BindGroupEntry {
          binding: 2,
          resource: options.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
          binding: 3,
          resource: wgpu::BindingResource::TextureView(&spread[1]),
        },
        wgpu::BindGroupEntry {
          binding: 4,
          resource: wgpu::BindingResource::Sampler(&haze_sampler),
        },
      ],
    });
    Self {
      size,
      smoothed,
      color,
      depth,
      glow,
      spread,
      lit,
      unspread,
      group,
    }
  }

  /// Returns the width and height of the glow map and of the maps blurred from it.
  pub(super) fn cells(&self) -> [u32; 2] {
    cells(self.size)
  }

  /// Returns whether a frame of this size and smoothing draws into this picture.
  pub(super) fn fits(&self, size: [u32; 2], smooth: bool) -> bool {
    self.size == size && self.smoothed.is_some() == smooth
  }
}

/// Returns the width and height of the glow map of a picture of `size` pixels.
///
/// The cells at the right and the foot reach past a picture that is no whole count of them, and
/// there is one more all around.
fn cells(size: [u32; 2]) -> [u32; 2] {
  size.map(|side| side.div_ceil(HAZE_CELL) + 2)
}
