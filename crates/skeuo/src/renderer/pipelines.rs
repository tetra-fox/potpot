//! the bind group layouts and render pipelines, made once for a target's format

use std::mem::size_of;

use super::uniforms::Placed;
use crate::mesh::Vertex;
use crate::scene::LAMPS;

pub(super) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// the picture holds srgb-encoded color in a format that does not encode, see scene.wgsl
pub(super) const PICTURE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// the samples of each pixel when edges are smoothed
pub(super) const SAMPLES: u32 = 4;

pub(super) const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
  wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

pub(super) const PRESENT_SOURCE: &str = include_str!("present.wgsl");

/// scene.wgsl with the length of its array of lamps ahead of it, which is a scene's
pub(super) fn scene_source() -> String {
  format!(
    "const LAMPS: u32 = {LAMPS};\n{}",
    include_str!("scene.wgsl")
  )
}

#[derive(Debug)]
pub(super) struct Layouts {
  pub(super) globals: wgpu::BindGroupLayout,
  /// the shadow map with its sampler, and the height map
  pub(super) maps: wgpu::BindGroupLayout,
  /// the picture with its sampler, and how it is degraded
  pub(super) present: wgpu::BindGroupLayout,
}

impl Layouts {
  pub(super) fn new(device: &wgpu::Device) -> Self {
    let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry {
      binding,
      visibility,
      ty: wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
      },
      count: None,
    };
    let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
      binding,
      visibility: wgpu::ShaderStages::FRAGMENT,
      ty: wgpu::BindingType::Texture {
        sample_type,
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
      },
      count: None,
    };
    let sampler = |binding, kind| wgpu::BindGroupLayoutEntry {
      binding,
      visibility: wgpu::ShaderStages::FRAGMENT,
      ty: wgpu::BindingType::Sampler(kind),
      count: None,
    };
    Self {
      globals: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("skeuo globals"),
        entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
      }),
      maps: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("skeuo shadow map"),
        entries: &[
          texture(0, wgpu::TextureSampleType::Depth),
          sampler(1, wgpu::SamplerBindingType::Comparison),
          texture(2, wgpu::TextureSampleType::Depth),
        ],
      }),
      present: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("skeuo present"),
        entries: &[
          texture(0, wgpu::TextureSampleType::Float { filterable: true }),
          sampler(1, wgpu::SamplerBindingType::Filtering),
          uniform(2, wgpu::ShaderStages::FRAGMENT),
        ],
      }),
    }
  }
}

#[derive(Debug)]
pub(super) struct Pipelines {
  pub(super) shadow: wgpu::RenderPipeline,
  pub(super) height: wgpu::RenderPipeline,
  /// for one sample a pixel, and for [`SAMPLES`]
  pub(super) scene: [wgpu::RenderPipeline; 2],
  pub(super) present: wgpu::RenderPipeline,
}

impl Pipelines {
  /// `target` is the format of what the present pipeline draws onto
  pub(super) fn new(device: &wgpu::Device, layouts: &Layouts, target: wgpu::TextureFormat) -> Self {
    let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("skeuo scene"),
      source: wgpu::ShaderSource::Wgsl(scene_source().into()),
    });
    let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("skeuo present"),
      source: wgpu::ShaderSource::Wgsl(PRESENT_SOURCE.into()),
    });

    let vertex_buffers = [
      wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRIBUTES,
      },
      Placed::layout(),
    ];
    // the panel's y runs down, which flips every triangle's winding, and parts are solids
    // whose insides never show, so nothing is culled
    let primitive = wgpu::PrimitiveState::default();
    let depth = || wgpu::DepthStencilState {
      format: DEPTH,
      depth_write_enabled: true,
      depth_compare: wgpu::CompareFunction::Less,
      stencil: wgpu::StencilState::default(),
      bias: wgpu::DepthBiasState::default(),
    };
    let layout = |label, bind_group_layouts: &[&wgpu::BindGroupLayout]| {
      device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts,
        push_constant_ranges: &[],
      })
    };

    // a pipeline that draws the objects' depth only, as seen through one of the globals' views
    let depth_only = |label, entry_point| {
      device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout(label, &[&layouts.globals])),
        vertex: wgpu::VertexState {
          module: &scene_shader,
          entry_point: Some(entry_point),
          compilation_options: wgpu::PipelineCompilationOptions::default(),
          buffers: &vertex_buffers,
        },
        fragment: None,
        primitive,
        depth_stencil: Some(depth()),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
      })
    };
    let scene_layout = layout("skeuo scene", &[&layouts.globals, &layouts.maps]);
    // one for each count of samples, since a pipeline draws at only one
    let scene = [1, SAMPLES].map(|count| {
      device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("skeuo scene"),
        layout: Some(&scene_layout),
        vertex: wgpu::VertexState {
          module: &scene_shader,
          entry_point: Some("vertex"),
          compilation_options: wgpu::PipelineCompilationOptions::default(),
          buffers: &vertex_buffers,
        },
        fragment: Some(wgpu::FragmentState {
          module: &scene_shader,
          entry_point: Some("fragment"),
          compilation_options: wgpu::PipelineCompilationOptions::default(),
          targets: &[Some(PICTURE.into())],
        }),
        primitive,
        depth_stencil: Some(depth()),
        multisample: wgpu::MultisampleState {
          count,
          ..Default::default()
        },
        multiview: None,
        cache: None,
      })
    });
    let present = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("skeuo present"),
      layout: Some(&layout("skeuo present", &[&layouts.present])),
      vertex: wgpu::VertexState {
        module: &present_shader,
        entry_point: Some("vertex"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &[],
      },
      fragment: Some(wgpu::FragmentState {
        module: &present_shader,
        entry_point: Some("fragment"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        targets: &[Some(target.into())],
      }),
      primitive,
      depth_stencil: None,
      multisample: wgpu::MultisampleState::default(),
      multiview: None,
      cache: None,
    });

    Self {
      shadow: depth_only("skeuo shadow", "shadow_vertex"),
      height: depth_only("skeuo heights", "height_vertex"),
      scene,
      present,
    }
  }
}
