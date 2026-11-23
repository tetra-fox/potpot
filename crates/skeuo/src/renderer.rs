//! draws a [`Scene`] with wgpu: a shadow map, the lit picture at its own resolution, and that
//! picture stretched into place on a target

use std::mem::size_of;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::mesh::{Mesh, Vertex};
use crate::scene::{LAMPS, Scene};

/// the shadow map's width and height
const SHADOW_SIZE: u32 = 2048;
/// the width and height of the map of how high everything on the panel is
const HEIGHT_SIZE: u32 = 1024;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// the picture holds srgb-encoded color in a format that does not encode, see scene.wgsl
const PICTURE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// a rectangle of a target, in its pixels
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
}

/// everything on the gpu that drawing scenes takes. [`Renderer::prepare`] loads a scene and
/// [`Renderer::render`] draws the one last prepared
pub struct Renderer {
  shadow_pipeline: wgpu::RenderPipeline,
  /// for one sample a pixel, and for [`SAMPLES`]
  scene_pipelines: [wgpu::RenderPipeline; 2],
  present_pipeline: wgpu::RenderPipeline,
  globals: wgpu::Buffer,
  globals_group: wgpu::BindGroup,
  shadow_map: wgpu::TextureView,
  /// how high everything on the panel is, seen from straight above
  height_map: wgpu::TextureView,
  height_pipeline: wgpu::RenderPipeline,
  shadow_group: wgpu::BindGroup,
  present_layout: wgpu::BindGroupLayout,
  present_options: wgpu::Buffer,
  /// whether the target encodes srgb itself
  target_encodes: bool,
  picture: Option<Picture>,
  meshes: Option<Loaded>,
  placed: wgpu::Buffer,
  /// how many objects the placed buffer holds
  placed_room: usize,
  /// the mesh of each object of the prepared scene, in order
  draws: Vec<usize>,
}

/// the picture's textures at one size
struct Picture {
  size: [u32; 2],
  /// what the scene is drawn into when edges are smoothed, to be resolved into `color`
  smoothed: Option<wgpu::TextureView>,
  color: wgpu::TextureView,
  depth: wgpu::TextureView,
  group: wgpu::BindGroup,
}

/// a scene's meshes on the gpu, and the allocation they came from
struct Loaded {
  from: Arc<[Mesh]>,
  meshes: Vec<LoadedMesh>,
}

struct LoadedMesh {
  vertices: wgpu::Buffer,
  indices: wgpu::Buffer,
  count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
  view: [[f32; 4]; 4],
  light_view: [[f32; 4]; 4],
  toward_light: [f32; 4],
  toward_viewer: [f32; 4],
  light_color: [f32; 4],
  ambient: [f32; 4],
  shadows: [f32; 4],
  overhead: [[f32; 4]; 4],
  /// the panel's width and height, and the lowest and highest anything on it reaches
  panel: [f32; 4],
  /// each lamp's place with its reach, and then its color
  lamps: [[[f32; 4]; 2]; LAMPS],
}

/// one object's place and material, read once per instance
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Placed {
  model: [[f32; 4]; 4],
  color: [f32; 4],
  highlight: [f32; 4],
  finish: [f32; 4],
  texture: [f32; 4],
}

/// the samples of each pixel when edges are smoothed
const SAMPLES: u32 = 4;

impl Renderer {
  /// `target` is the format of the textures [`Renderer::render`] will draw onto
  pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("skeuo scene"),
      source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
    });
    let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("skeuo present"),
      source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
    });

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
    let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("skeuo globals"),
      entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
    });
    let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("skeuo shadow map"),
      entries: &[
        wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
          },
          count: None,
        },
        wgpu::BindGroupLayoutEntry {
          binding: 1,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
          count: None,
        },
        wgpu::BindGroupLayoutEntry {
          binding: 2,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
          },
          count: None,
        },
      ],
    });
    let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("skeuo present"),
      entries: &[
        wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
          },
          count: None,
        },
        wgpu::BindGroupLayoutEntry {
          binding: 1,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
          count: None,
        },
        uniform(2, wgpu::ShaderStages::FRAGMENT),
      ],
    });

    let vertex_buffers = [
      wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
      },
      wgpu::VertexBufferLayout {
        array_stride: size_of::<Placed>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
          2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
          6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4
        ],
      },
    ];
    // the panel's y runs down, which flips every triangle's winding, and parts are solids
    // whose insides never show, so nothing is culled
    let primitive = wgpu::PrimitiveState::default();
    let depth = |bias| wgpu::DepthStencilState {
      format: DEPTH,
      depth_write_enabled: true,
      depth_compare: wgpu::CompareFunction::Less,
      stencil: wgpu::StencilState::default(),
      bias,
    };

    let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("skeuo shadow"),
      layout: Some(
        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
          label: Some("skeuo shadow"),
          bind_group_layouts: &[&globals_layout],
          push_constant_ranges: &[],
        }),
      ),
      vertex: wgpu::VertexState {
        module: &scene_shader,
        entry_point: Some("shadow_vertex"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &vertex_buffers,
      },
      fragment: None,
      primitive,
      depth_stencil: Some(depth(wgpu::DepthBiasState::default())),
      multisample: wgpu::MultisampleState::default(),
      multiview: None,
      cache: None,
    });
    let height_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("skeuo heights"),
      layout: Some(
        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
          label: Some("skeuo heights"),
          bind_group_layouts: &[&globals_layout],
          push_constant_ranges: &[],
        }),
      ),
      vertex: wgpu::VertexState {
        module: &scene_shader,
        entry_point: Some("height_vertex"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &vertex_buffers,
      },
      fragment: None,
      primitive,
      depth_stencil: Some(depth(wgpu::DepthBiasState::default())),
      multisample: wgpu::MultisampleState::default(),
      multiview: None,
      cache: None,
    });
    let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("skeuo scene"),
      bind_group_layouts: &[&globals_layout, &shadow_layout],
      push_constant_ranges: &[],
    });
    // one for each count of samples, since a pipeline draws at only one
    let scene_pipelines = [1, SAMPLES].map(|count| {
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
        depth_stencil: Some(depth(wgpu::DepthBiasState::default())),
        multisample: wgpu::MultisampleState {
          count,
          ..Default::default()
        },
        multiview: None,
        cache: None,
      })
    });
    let present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("skeuo present"),
      layout: Some(
        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
          label: Some("skeuo present"),
          bind_group_layouts: &[&present_layout],
          push_constant_ranges: &[],
        }),
      ),
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

    let globals = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("skeuo globals"),
      size: size_of::<Globals>() as u64,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("skeuo globals"),
      layout: &globals_layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: globals.as_entire_binding(),
      }],
    });

    let shadow_map = device
      .create_texture(&wgpu::TextureDescriptor {
        label: Some("skeuo shadow map"),
        size: wgpu::Extent3d {
          width: SHADOW_SIZE,
          height: SHADOW_SIZE,
          depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
      })
      .create_view(&wgpu::TextureViewDescriptor::default());
    let height_map = device
      .create_texture(&wgpu::TextureDescriptor {
        label: Some("skeuo heights"),
        size: wgpu::Extent3d {
          width: HEIGHT_SIZE,
          height: HEIGHT_SIZE,
          depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
      })
      .create_view(&wgpu::TextureViewDescriptor::default());
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("skeuo shadow map"),
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      compare: Some(wgpu::CompareFunction::LessEqual),
      ..Default::default()
    });
    let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("skeuo shadow map"),
      layout: &shadow_layout,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&shadow_map),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: wgpu::BindingResource::Sampler(&shadow_sampler),
        },
        wgpu::BindGroupEntry {
          binding: 2,
          resource: wgpu::BindingResource::TextureView(&height_map),
        },
      ],
    });

    let present_options = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("skeuo present"),
      size: size_of::<[f32; 8]>() as u64,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });

    Self {
      shadow_pipeline,
      scene_pipelines,
      present_pipeline,
      globals,
      globals_group,
      shadow_map,
      height_map,
      height_pipeline,
      shadow_group,
      present_layout,
      present_options,
      target_encodes: target.is_srgb(),
      picture: None,
      meshes: None,
      placed: placed_buffer(device, 0),
      placed_room: 0,
      draws: Vec::new(),
    }
  }

  /// loads a scene to be drawn over `size` pixels of a target
  pub fn prepare(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &Scene,
    size: [u32; 2],
  ) {
    let look = scene.look;
    let picture_size =
      size.map(|side| ((side as f32 / look.pixel_size.max(1.0)).ceil() as u32).max(1));
    if self.picture.as_ref().is_none_or(|picture| {
      picture.size != picture_size || picture.smoothed.is_some() != look.antialias
    }) {
      self.picture = Some(self.picture_of(device, picture_size, look.antialias));
    }
    if self
      .meshes
      .as_ref()
      .is_none_or(|loaded| !Arc::ptr_eq(&loaded.from, &scene.meshes))
    {
      self.meshes = Some(Loaded {
        from: Arc::clone(&scene.meshes),
        meshes: scene.meshes.iter().map(|mesh| load(device, mesh)).collect(),
      });
    }

    let globals = Globals {
      view: scene.view().to_cols_array_2d(),
      light_view: scene.light_view().to_cols_array_2d(),
      toward_light: (scene.light.toward.normalize())
        .extend(scene.light.softness)
        .into(),
      toward_viewer: scene.toward_viewer().extend(0.0).into(),
      light_color: scene.light.color.extend(scene.light.falloff).into(),
      ambient: scene.light.ambient.extend(0.0).into(),
      shadows: [
        f32::from(u8::from(look.soft_shadows)),
        1.0 / SHADOW_SIZE as f32,
        scene.lamps.len().min(LAMPS) as f32,
        scene.light.occlusion,
      ],
      overhead: scene.overhead().to_cols_array_2d(),
      panel: [
        scene.size.x,
        scene.size.y,
        scene.heights[0],
        scene.heights[1],
      ],
      lamps: std::array::from_fn(|index| {
        scene.lamps.get(index).map_or([[0.0; 4]; 2], |lamp| {
          [
            lamp.at.extend(lamp.reach).into(),
            lamp.color.extend(0.0).into(),
          ]
        })
      }),
    };
    queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
    let options = [
      look.color_steps as f32,
      f32::from(u8::from(self.target_encodes)),
      look.bloom,
      look.vignette,
      look.soften,
      0.0,
      0.0,
      0.0,
    ];
    queue.write_buffer(&self.present_options, 0, bytemuck::bytes_of(&options));

    let mesh_count = scene.meshes.len();
    let objects = || {
      scene
        .objects
        .iter()
        .filter(|object| object.mesh < mesh_count)
    };
    let placed: Vec<Placed> = objects()
      .map(|object| Placed {
        model: object.transform.to_cols_array_2d(),
        color: object.material.color.extend(object.material.glow).into(),
        highlight: [
          object.material.specular,
          object.material.shininess,
          object.material.glow_radius,
          0.0,
        ],
        finish: [
          object.material.reflect,
          object.material.metal,
          object.material.spun,
          object.material.glaze,
        ],
        texture: [object.material.grain, object.material.brushed, 0.0, 0.0],
      })
      .collect();
    if placed.len() > self.placed_room {
      self.placed_room = placed.len().next_power_of_two();
      self.placed = placed_buffer(device, self.placed_room);
    }
    queue.write_buffer(&self.placed, 0, bytemuck::cast_slice(&placed));
    self.draws.clear();
    self.draws.extend(objects().map(|object| object.mesh));
  }

  /// draws the prepared scene over `place` on the target, leaving everything outside `clip`
  /// as it was. `clip` is x, y, width and height in the target's pixels
  pub fn render(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    place: Place,
    clip: [u32; 4],
  ) {
    let (Some(picture), Some(loaded)) = (&self.picture, &self.meshes) else {
      return;
    };
    if clip[2] == 0 || clip[3] == 0 || place.width <= 0.0 || place.height <= 0.0 {
      return;
    }

    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("skeuo shadow"),
        color_attachments: &[],
        depth_stencil_attachment: Some(depth_attachment(&self.shadow_map)),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_pipeline(&self.shadow_pipeline);
      pass.set_bind_group(0, &self.globals_group, &[]);
      self.draw_objects(&mut pass, loaded);
    }
    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("skeuo heights"),
        color_attachments: &[],
        depth_stencil_attachment: Some(depth_attachment(&self.height_map)),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_pipeline(&self.height_pipeline);
      pass.set_bind_group(0, &self.globals_group, &[]);
      self.draw_objects(&mut pass, loaded);
    }
    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("skeuo scene"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: picture.smoothed.as_ref().unwrap_or(&picture.color),
          depth_slice: None,
          resolve_target: picture.smoothed.as_ref().map(|_| &picture.color),
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: Some(depth_attachment(&picture.depth)),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_pipeline(&self.scene_pipelines[usize::from(picture.smoothed.is_some())]);
      pass.set_bind_group(0, &self.globals_group, &[]);
      pass.set_bind_group(1, &self.shadow_group, &[]);
      self.draw_objects(&mut pass, loaded);
    }
    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("skeuo present"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: target,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_pipeline(&self.present_pipeline);
      pass.set_bind_group(0, &picture.group, &[]);
      pass.set_viewport(place.x, place.y, place.width, place.height, 0.0, 1.0);
      pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
      pass.draw(0..3, 0..1);
    }
  }

  fn draw_objects(&self, pass: &mut wgpu::RenderPass<'_>, loaded: &Loaded) {
    pass.set_vertex_buffer(1, self.placed.slice(..));
    for (object, &mesh) in self.draws.iter().enumerate() {
      let Some(mesh) = loaded.meshes.get(mesh) else {
        continue;
      };
      let object = object as u32;
      pass.set_vertex_buffer(0, mesh.vertices.slice(..));
      pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
      pass.draw_indexed(0..mesh.count, 0, object..object + 1);
    }
  }

  fn picture_of(&self, device: &wgpu::Device, size: [u32; 2], smooth: bool) -> Picture {
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
      "skeuo picture",
      PICTURE,
      wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
      1,
    );
    let samples = if smooth { SAMPLES } else { 1 };
    let smoothed = smooth.then(|| {
      texture(
        "skeuo picture samples",
        PICTURE,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        samples,
      )
    });
    let depth = texture(
      "skeuo picture depth",
      DEPTH,
      wgpu::TextureUsages::RENDER_ATTACHMENT,
      samples,
    );
    // each rendered pixel lands on the window as a hard-edged block
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("skeuo picture"),
      mag_filter: wgpu::FilterMode::Nearest,
      min_filter: wgpu::FilterMode::Nearest,
      ..Default::default()
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("skeuo picture"),
      layout: &self.present_layout,
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
          resource: self.present_options.as_entire_binding(),
        },
      ],
    });
    Picture {
      size,
      smoothed,
      color,
      depth,
      group,
    }
  }
}

fn depth_attachment(view: &wgpu::TextureView) -> wgpu::RenderPassDepthStencilAttachment<'_> {
  wgpu::RenderPassDepthStencilAttachment {
    view,
    depth_ops: Some(wgpu::Operations {
      load: wgpu::LoadOp::Clear(1.0),
      store: wgpu::StoreOp::Store,
    }),
    stencil_ops: None,
  }
}

fn placed_buffer(device: &wgpu::Device, room: usize) -> wgpu::Buffer {
  device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("skeuo placed"),
    size: (room.max(1) * size_of::<Placed>()) as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
  })
}

fn load(device: &wgpu::Device, mesh: &Mesh) -> LoadedMesh {
  LoadedMesh {
    vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("skeuo vertices"),
      contents: bytemuck::cast_slice(&mesh.vertices),
      usage: wgpu::BufferUsages::VERTEX,
    }),
    indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("skeuo indices"),
      contents: bytemuck::cast_slice(&mesh.indices),
      usage: wgpu::BufferUsages::INDEX,
    }),
    count: mesh.indices.len() as u32,
  }
}
