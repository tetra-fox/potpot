//! The bind group layouts and render pipelines, made once for a target's format.

use std::mem::size_of;

use super::uniforms::Placed;
use crate::mesh::Vertex;
use crate::scene::LAMPS;

pub(super) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The format of the picture.
///
/// The picture holds sRGB-encoded color in a format that does not encode, for the reason given
/// in `scene.wgsl`.
pub(super) const PICTURE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The format of the bounce map.
///
/// The map holds linear light in rgb and a height in a, and a height lies outside zero to one.
pub(super) const BOUNCE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The samples of each pixel when edges are smoothed.
pub(super) const SAMPLES: u32 = 4;

pub(super) const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
  wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

/// The sRGB curve, which goes ahead of each shader that uses it.
const SRGB_SOURCE: &str = include_str!("srgb.wgsl");

/// Returns `scene.wgsl` with the length of its array of lamps ahead of it.
///
/// The length is [`LAMPS`], and the sRGB curve goes ahead of the shader too.
pub(super) fn scene_source() -> String {
  format!(
    "const LAMPS: u32 = {LAMPS};\n{SRGB_SOURCE}{}",
    include_str!("scene.wgsl")
  )
}

/// Returns `present.wgsl` with the sRGB curve ahead of it.
pub(super) fn present_source() -> String {
  format!("{SRGB_SOURCE}{}", include_str!("present.wgsl"))
}

#[derive(Debug)]
pub(super) struct Layouts {
  pub(super) globals: wgpu::BindGroupLayout,
  /// The shadow map with its sampler, for the pass that draws the bounce map.
  pub(super) shadow: wgpu::BindGroupLayout,
  /// Every map the scene pass reads.
  ///
  /// The shadow map comes with its sampler, then the height map, then the bounce map with its
  /// sampler.
  pub(super) maps: wgpu::BindGroupLayout,
  /// The picture with its sampler, and how it is degraded.
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
        label: Some("potpot globals"),
        entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
      }),
      shadow: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("potpot shadow map"),
        entries: &[
          texture(0, wgpu::TextureSampleType::Depth),
          sampler(1, wgpu::SamplerBindingType::Comparison),
        ],
      }),
      maps: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("potpot maps"),
        entries: &[
          texture(0, wgpu::TextureSampleType::Depth),
          sampler(1, wgpu::SamplerBindingType::Comparison),
          texture(2, wgpu::TextureSampleType::Depth),
          texture(3, wgpu::TextureSampleType::Float { filterable: true }),
          sampler(4, wgpu::SamplerBindingType::Filtering),
        ],
      }),
      present: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("potpot present"),
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
  pub(super) bounce: wgpu::RenderPipeline,
  /// The scene pipelines for one sample a pixel and for [`SAMPLES`].
  pub(super) scene: [wgpu::RenderPipeline; 2],
  pub(super) present: wgpu::RenderPipeline,
}

impl Pipelines {
  /// Creates every pipeline for a target's format.
  ///
  /// `target` is the format of what the present pipeline draws onto.
  pub(super) fn new(device: &wgpu::Device, layouts: &Layouts, target: wgpu::TextureFormat) -> Self {
    let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("potpot scene"),
      source: wgpu::ShaderSource::Wgsl(scene_source().into()),
    });
    let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("potpot present"),
      source: wgpu::ShaderSource::Wgsl(present_source().into()),
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
    // a pipeline that draws the objects lit, by a pair of the scene shader's entry points
    let lit =
      |label, layout: &wgpu::PipelineLayout, [vertex, fragment]: [&str; 2], format, count| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
          label: Some(label),
          layout: Some(layout),
          vertex: wgpu::VertexState {
            module: &scene_shader,
            entry_point: Some(vertex),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &vertex_buffers,
          },
          fragment: Some(wgpu::FragmentState {
            module: &scene_shader,
            entry_point: Some(fragment),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState::from(format))],
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
      };
    let scene_layout = layout("potpot scene", &[&layouts.globals, &layouts.maps]);
    // one for each count of samples, since a pipeline draws at only one
    let scene = [1, SAMPLES].map(|count| {
      lit(
        "potpot scene",
        &scene_layout,
        ["vertex", "fragment"],
        PICTURE,
        count,
      )
    });
    let bounce = lit(
      "potpot bounce",
      &layout("potpot bounce", &[&layouts.globals, &layouts.shadow]),
      ["bounce_vertex", "bounce_fragment"],
      BOUNCE,
      SAMPLES,
    );
    let present = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("potpot present"),
      layout: Some(&layout("potpot present", &[&layouts.present])),
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
      shadow: depth_only("potpot shadow", "shadow_vertex"),
      height: depth_only("potpot heights", "height_vertex"),
      bounce,
      scene,
      present,
    }
  }
}
