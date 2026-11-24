//! flat shapes drawn straight onto a target, over the picture and apart from it: rectangles
//! and lettering in plain colors, with no light on them and nothing of a [`crate::Look`]

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::parts::legend;
use crate::renderer::Place;

/// a color as it is to look on the target, with how much of what is under it it covers
pub type Color = [f32; 4];

/// one corner of a shape, which is a rectangle about a middle: lying along the pixels and
/// drawn onto their edges, or any way round with its edges smoothed
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Corner {
  middle: [f32; 2],
  /// the way the shape's length runs
  along: [f32; 2],
  /// how far it reaches from its middle along its length and across it. for a line the
  /// first is to its two points, which its round ends are past
  half: [f32; 2],
  /// which corner this is: one or minus one along, and across
  side: [f32; 2],
  color: Color,
  /// one for a rectangle whose corners go on the nearest pixel edges
  snap: f32,
}

impl Corner {
  const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4, 5 => Float32
  ];
}

/// shapes to draw, in the order they were added, the later over the earlier. their places
/// are in the same points as a scene's panel, from its top left
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flat {
  corners: Vec<Corner>,
}

impl Flat {
  fn shape(&mut self, middle: Vec2, along: Vec2, half: Vec2, color: Color, snap: bool) {
    let corner = |side: [f32; 2]| Corner {
      middle: middle.into(),
      along: along.into(),
      half: half.into(),
      side,
      color,
      snap: f32::from(u8::from(snap)),
    };
    let sides = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    (self.corners).extend([0, 1, 2, 0, 2, 3].map(|side: usize| corner(sides[side])));
  }

  /// a filled rectangle between two corners. its sides are drawn on pixel edges, so they
  /// are sharp whatever the scale
  pub fn rect(&mut self, min: Vec2, max: Vec2, color: Color) {
    self.shape((min + max) / 2.0, Vec2::X, (max - min) / 2.0, color, true);
  }

  /// a line `weight` wide from one point to another, with round ends half its width past
  /// them. its edges are smoothed, whichever way it runs
  pub fn line(&mut self, from: Vec2, to: Vec2, weight: f32, color: Color) {
    let along = (to - from).try_normalize().unwrap_or(Vec2::X);
    let half = Vec2::new(from.distance(to), weight) / 2.0;
    self.shape((from + to) / 2.0, along, half, color, false);
  }

  /// lettering in the face of [`crate::parts::legend`], with the top left of its first letter at
  /// `from`: letters `height` tall with strokes `weight` wide
  pub fn legend(&mut self, text: &str, from: Vec2, height: f32, weight: f32, color: Color) {
    let offset = from - legend::top_left(text.chars().count(), height);
    for [start, end] in legend::lines(text, height) {
      self.line(offset + start, offset + end, weight, color);
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Options {
  size: [f32; 2],
  pixels: [f32; 2],
  /// one when the target encodes srgb itself
  options: [f32; 4],
}

/// what drawing [`Flat`] shapes takes on the gpu
#[derive(Debug)]
pub struct FlatRenderer {
  pipeline: wgpu::RenderPipeline,
  options: wgpu::Buffer,
  group: wgpu::BindGroup,
  corners: wgpu::Buffer,
  /// how many corners the buffer has room for
  room: usize,
  /// what the buffers hold, to tell shapes and a place that differ from them
  held: Flat,
  held_options: Options,
  target_encodes: bool,
}

impl FlatRenderer {
  /// `target` is the format of the textures [`FlatRenderer::render`] will draw onto
  pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("skeuo flat"),
      source: wgpu::ShaderSource::Wgsl(include_str!("flat.wgsl").into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("skeuo flat"),
      entries: &[wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
          ty: wgpu::BufferBindingType::Uniform,
          has_dynamic_offset: false,
          min_binding_size: None,
        },
        count: None,
      }],
    });
    let options = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("skeuo flat"),
      size: size_of::<Options>() as u64,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("skeuo flat"),
      layout: &layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: options.as_entire_binding(),
      }],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("skeuo flat"),
      layout: Some(
        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
          label: Some("skeuo flat"),
          bind_group_layouts: &[&layout],
          push_constant_ranges: &[],
        }),
      ),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vertex"),
        buffers: &[wgpu::VertexBufferLayout {
          array_stride: size_of::<Corner>() as u64,
          step_mode: wgpu::VertexStepMode::Vertex,
          attributes: &Corner::ATTRIBUTES,
        }],
        compilation_options: Default::default(),
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fragment"),
        targets: &[Some(wgpu::ColorTargetState {
          format: target,
          blend: Some(wgpu::BlendState::ALPHA_BLENDING),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: Default::default(),
      }),
      // shapes are flat and seen from either side, so nothing is culled
      primitive: wgpu::PrimitiveState::default(),
      depth_stencil: None,
      multisample: wgpu::MultisampleState::default(),
      multiview: None,
      cache: None,
    });
    Self {
      pipeline,
      options,
      group,
      corners: corner_buffer(device, 0),
      room: 0,
      // a new buffer is all zeros
      held: Flat::default(),
      held_options: Options::zeroed(),
      target_encodes: target.is_srgb(),
    }
  }

  /// takes the shapes to draw next. `size` is the width and height their points run over,
  /// which is a scene's panel, and `pixels` those of the place they will be drawn in
  pub fn prepare(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    flat: &Flat,
    size: Vec2,
    pixels: [u32; 2],
  ) {
    let options = Options {
      size: size.into(),
      pixels: pixels.map(|side| side as f32),
      options: [f32::from(u8::from(self.target_encodes)), 0.0, 0.0, 0.0],
    };
    if options != self.held_options {
      queue.write_buffer(&self.options, 0, bytemuck::bytes_of(&options));
      self.held_options = options;
    }
    if *flat != self.held {
      if flat.corners.len() > self.room {
        self.room = flat.corners.len().next_power_of_two();
        self.corners = corner_buffer(device, self.room);
      }
      queue.write_buffer(&self.corners, 0, bytemuck::cast_slice(&flat.corners));
      self.held.clone_from(flat);
    }
  }

  /// draws the shapes last prepared over `place` on the target, leaving everything outside
  /// `clip` as it was. `clip` is x, y, width and height in the target's pixels, and has to
  /// lie on the target
  pub fn render(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    place: Place,
    clip: [u32; 4],
  ) {
    let count = self.held.corners.len() as u32;
    if count == 0 || clip[2] == 0 || clip[3] == 0 || place.width <= 0.0 || place.height <= 0.0 {
      return;
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("skeuo flat"),
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
    pass.set_pipeline(&self.pipeline);
    pass.set_bind_group(0, &self.group, &[]);
    pass.set_viewport(place.x, place.y, place.width, place.height, 0.0, 1.0);
    pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
    pass.set_vertex_buffer(0, self.corners.slice(..));
    pass.draw(0..count, 0..1);
  }
}

fn corner_buffer(device: &wgpu::Device, room: usize) -> wgpu::Buffer {
  device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("skeuo flat"),
    size: (room.max(1) * size_of::<Corner>()) as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
  })
}
