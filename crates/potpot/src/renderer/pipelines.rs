//! The bind group layouts and render pipelines, made once for a target's format.

use std::mem::size_of;

use super::damage::{
  BOUNCE_REACHES, HAZE_REACH, OPEN_CLEAR, OPEN_REACHES, SHADOW_BIAS, SHADOW_SPREAD,
};
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

/// Returns `scene.wgsl` with what it is given ahead of it.
///
/// That is [`LAMPS`], the length of its array of lamps, how far around a point it looks things
/// up, and the sRGB curve.
pub(super) fn scene_source() -> String {
  // a row of numbers as the shader language writes it
  let listed = |numbers: &[f32]| {
    let numbers: Vec<String> = numbers.iter().map(|number| format!("{number:?}")).collect();
    numbers.join(", ")
  };
  format!(
    "const LAMPS: u32 = {LAMPS};\n\
     const SHADOW_SPREAD: i32 = {SHADOW_SPREAD};\n\
     const SHADOW_BIAS: f32 = {SHADOW_BIAS:?};\n\
     const OPEN_RINGS: i32 = {};\n\
     const OPEN_REACHES = array<f32, {}>({});\n\
     const OPEN_CLEAR: f32 = {OPEN_CLEAR:?};\n\
     const BOUNCE_RINGS: i32 = {};\n\
     const BOUNCE_REACHES = array<f32, {}>({});\n\
     {SRGB_SOURCE}{}",
    OPEN_REACHES.len(),
    OPEN_REACHES.len(),
    listed(&OPEN_REACHES),
    BOUNCE_REACHES.len(),
    BOUNCE_REACHES.len(),
    listed(&BOUNCE_REACHES),
    include_str!("scene.wgsl")
  )
}

/// Returns `present.wgsl` with what it is given ahead of it.
///
/// That is the sRGB curve and the reach of its haze.
pub(super) fn present_source() -> String {
  format!(
    "const HAZE_REACH: f32 = {HAZE_REACH:?};\n{SRGB_SOURCE}{}",
    include_str!("present.wgsl")
  )
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
  /// The pipelines that wipe triangles of the picture.
  ///
  /// What they cover is put back to what a picture starts from. They are for one sample a pixel
  /// and for [`SAMPLES`].
  pub(super) wipe: [wgpu::RenderPipeline; 2],
  /// The pipeline that wipes the shadow map or the height map.
  ///
  /// It puts the map back to what it starts from, as far as the scissor lets it.
  pub(super) wipe_depth: wgpu::RenderPipeline,
  /// The pipeline that wipes the bounce map the same way.
  pub(super) wipe_bounce: wgpu::RenderPipeline,
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
    // whatever is there is overwritten
    let overwritten = || wgpu::DepthStencilState {
      depth_compare: wgpu::CompareFunction::Always,
      ..depth()
    };
    let far = wgpu::VertexState {
      module: &scene_shader,
      entry_point: Some("far_vertex"),
      compilation_options: wgpu::PipelineCompilationOptions::default(),
      buffers: &[],
    };
    let wipe_depth = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("potpot wipe depth"),
      layout: Some(&layout("potpot wipe depth", &[])),
      vertex: far.clone(),
      fragment: None,
      primitive,
      depth_stencil: Some(overwritten()),
      multisample: wgpu::MultisampleState::default(),
      multiview: None,
      cache: None,
    });
    let wipe_bounce = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("potpot wipe bounce"),
      layout: Some(&layout("potpot wipe bounce", &[&layouts.globals])),
      vertex: far,
      fragment: Some(wgpu::FragmentState {
        module: &scene_shader,
        entry_point: Some("no_bounce_fragment"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        targets: &[Some(wgpu::ColorTargetState::from(BOUNCE))],
      }),
      primitive,
      depth_stencil: Some(overwritten()),
      multisample: wgpu::MultisampleState {
        count: SAMPLES,
        ..Default::default()
      },
      multiview: None,
      cache: None,
    });
    let wipe_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("potpot wipe"),
      source: wgpu::ShaderSource::Wgsl(include_str!("wipe.wgsl").into()),
    });
    let wipe_layout = layout("potpot wipe", &[]);
    let wipe = [1, SAMPLES].map(|count| {
      device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("potpot wipe"),
        layout: Some(&wipe_layout),
        vertex: wgpu::VertexState {
          module: &wipe_shader,
          entry_point: Some("vertex"),
          compilation_options: wgpu::PipelineCompilationOptions::default(),
          buffers: &[wgpu::VertexBufferLayout {
            array_stride: size_of::<[f32; 2]>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
          }],
        },
        fragment: Some(wgpu::FragmentState {
          module: &wipe_shader,
          entry_point: Some("fragment"),
          compilation_options: wgpu::PipelineCompilationOptions::default(),
          targets: &[Some(wgpu::ColorTargetState::from(PICTURE))],
        }),
        primitive,
        depth_stencil: Some(overwritten()),
        multisample: wgpu::MultisampleState {
          count,
          ..Default::default()
        },
        multiview: None,
        cache: None,
      })
    });
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
      wipe,
      wipe_depth,
      wipe_bounce,
      present,
    }
  }
}
