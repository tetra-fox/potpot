//! the textures the scene is drawn into before it is stretched onto the target

use super::pipelines::{DEPTH, PICTURE, SAMPLES};

/// the picture's textures at one size
#[derive(Debug)]
pub(super) struct Picture {
  size: [u32; 2],
  /// what the scene is drawn into when edges are smoothed, to be resolved into `color`
  pub(super) smoothed: Option<wgpu::TextureView>,
  pub(super) color: wgpu::TextureView,
  pub(super) depth: wgpu::TextureView,
  /// the color with its sampler and the present options, for the present pass to read
  pub(super) group: wgpu::BindGroup,
}

impl Picture {
  /// `layout` is the present layout, and `options` the buffer its third binding reads
  pub(super) fn new(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    options: &wgpu::Buffer,
    size: [u32; 2],
    smooth: bool,
  ) -> Self {
    let texture = |label, format, usage, sample_count| {
      device
        .create_texture(&wgpu::TextureDescriptor {
          label: Some(label),
          size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
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
    );
    let samples = if smooth { SAMPLES } else { 1 };
    let smoothed = smooth.then(|| {
      texture(
        "potpot picture samples",
        PICTURE,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        samples,
      )
    });
    let depth = texture(
      "potpot picture depth",
      DEPTH,
      wgpu::TextureUsages::RENDER_ATTACHMENT,
      samples,
    );
    // each rendered pixel lands on the window as a hard-edged block
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot picture"),
      mag_filter: wgpu::FilterMode::Nearest,
      min_filter: wgpu::FilterMode::Nearest,
      ..Default::default()
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot picture"),
      layout,
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
      ],
    });
    Self {
      size,
      smoothed,
      color,
      depth,
      group,
    }
  }

  /// whether this is the picture a frame of this size and smoothing draws into
  pub(super) fn fits(&self, size: [u32; 2], smooth: bool) -> bool {
    self.size == size && self.smoothed.is_some() == smooth
  }
}
