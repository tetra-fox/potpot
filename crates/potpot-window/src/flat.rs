//! A renderer of [`Flat`] shapes.
//!
//! The shapes go straight onto a target, over whatever a skin has drawn there.

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use potpot_controls::Point;
use potpot_controls::flat::{Color, Flat, Shape};

/// One corner of a shape.
///
/// A shape is a rectangle about a middle: lying along the pixels and drawn onto their edges,
/// or any way round with its edges smoothed.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Corner {
  middle: [f32; 2],
  /// The direction the shape's length runs in.
  along: [f32; 2],
  /// The shape's reach from its middle along its length and across it.
  ///
  /// For a line the first is to its two points, which its round ends are past.
  half: [f32; 2],
  /// The corner this is: one or minus one along, and across.
  side: [f32; 2],
  color: Color,
  /// One for a rectangle whose corners go on the nearest pixel edges.
  snap: f32,
}

impl Corner {
  const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4, 5 => Float32
  ];

  /// Returns the six corners of the two triangles a shape is drawn as.
  fn of(shape: &Shape) -> [Self; 6] {
    let corner = |side: [f32; 2]| Self {
      middle: shape.middle,
      along: shape.along,
      half: shape.half,
      side,
      color: shape.color,
      snap: f32::from(u8::from(shape.snap)),
    };
    let sides = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    [0, 1, 2, 0, 2, 3].map(|side: usize| corner(sides[side]))
  }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Options {
  size: [f32; 2],
  pixels: [f32; 2],
  /// One as its first number when the target encodes sRGB itself.
  options: [f32; 4],
}

/// The GPU resources that drawing [`Flat`] shapes takes.
#[derive(Debug)]
pub(crate) struct FlatRenderer {
  pipeline: wgpu::RenderPipeline,
  options: wgpu::Buffer,
  group: wgpu::BindGroup,
  corners: wgpu::Buffer,
  /// The number of corners the buffer has room for.
  room: usize,
  /// The shapes whose corners the corner buffer holds.
  ///
  /// [`FlatRenderer::prepare`] writes the buffer only when it is given other shapes.
  held: Flat,
  /// The contents of the options buffer.
  ///
  /// [`FlatRenderer::prepare`] writes the buffer only when the options differ from them.
  held_options: Options,
  target_encodes: bool,
}

impl FlatRenderer {
  /// Creates a renderer for targets of one format.
  ///
  /// `target` is the format of the textures [`FlatRenderer::render`] draws onto.
  pub(crate) fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("controls flat"),
      source: wgpu::ShaderSource::Wgsl(include_str!("flat.wgsl").into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("controls flat"),
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
      label: Some("controls flat"),
      size: size_of::<Options>() as u64,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("controls flat"),
      layout: &layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: options.as_entire_binding(),
      }],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("controls flat"),
      layout: Some(
        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
          label: Some("controls flat"),
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
      held: Flat::default(),
      // a new buffer is all zeros
      held_options: Options::zeroed(),
      target_encodes: target.is_srgb(),
    }
  }

  /// Takes the shapes to draw next.
  ///
  /// `size` is the panel's width and height in points, and `pixels` is the width and height
  /// of the place the shapes are drawn in.
  pub(crate) fn prepare(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    flat: &Flat,
    size: Point,
    pixels: [u32; 2],
  ) {
    let options = Options {
      size,
      pixels: pixels.map(|side| side as f32),
      options: [f32::from(u8::from(self.target_encodes)), 0.0, 0.0, 0.0],
    };
    if options != self.held_options {
      queue.write_buffer(&self.options, 0, bytemuck::bytes_of(&options));
      self.held_options = options;
    }
    if *flat != self.held {
      let corners: Vec<Corner> = flat.shapes().iter().flat_map(Corner::of).collect();
      if corners.len() > self.room {
        self.room = corners.len().next_power_of_two();
        self.corners = corner_buffer(device, self.room);
      }
      queue.write_buffer(&self.corners, 0, bytemuck::cast_slice(&corners));
      self.held.clone_from(flat);
    }
  }

  /// Draws the shapes last prepared over `place` on the target.
  ///
  /// Everything outside `place` stays as it was. `place` is
  /// `[left, top, width, height]` in the target's pixels, and has to lie on the target.
  pub(crate) fn render(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    place: [u32; 4],
  ) {
    let count = self.held.shapes().len() as u32 * 6;
    let [x, y, width, height] = place;
    if count == 0 || width == 0 || height == 0 {
      return;
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("controls flat"),
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
    pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
    pass.set_scissor_rect(x, y, width, height);
    pass.set_vertex_buffer(0, self.corners.slice(..));
    pass.draw(0..count, 0..1);
  }
}

fn corner_buffer(device: &wgpu::Device, room: usize) -> wgpu::Buffer {
  device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("controls flat"),
    size: (room.max(1) * size_of::<Corner>()) as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
  })
}

#[cfg(test)]
mod tests {
  #[test]
  fn the_shader_can_be_written_for_every_backend() {
    use naga::back::{hlsl, msl, spv};

    // a backend is handed the shader in a language of its own, which naga writes it in when
    // the pipeline is made. one it cannot be written in fails there, on the user's machine
    let module =
      naga::front::wgsl::parse_str(include_str!("flat.wgsl")).expect("the shader parses");
    let info = naga::valid::Validator::new(
      naga::valid::ValidationFlags::all(),
      naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the shader is valid");
    let mut direct3d = String::new();
    let pipeline = hlsl::PipelineOptions::default();
    hlsl::Writer::new(&mut direct3d, &hlsl::Options::default(), &pipeline)
      .write(&module, &info, None)
      .expect("the shader is written for direct3d");
    msl::write_string(
      &module,
      &info,
      &msl::Options::default(),
      &msl::PipelineOptions::default(),
    )
    .expect("the shader is written for metal");
    spv::write_vec(&module, &info, &spv::Options::default(), None)
      .expect("the shader is written for vulkan");
  }
}
