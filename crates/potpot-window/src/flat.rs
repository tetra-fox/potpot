//! A renderer of [`Flat`] shapes.
//!
//! The shapes go straight onto a target, over whatever a skin has drawn there.

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use potpot_controls::Point;
use potpot_controls::flat::{Color, Flat, Shape};

/// The kind of shape a corner is of, as the shader is told it.
const LINE: f32 = 0.0;
const RECT: f32 = 1.0;
const SPAN: f32 = 2.0;

/// The pixels past its edge that a line or span is drawn over.
///
/// They are for the pixels the edge only partly covers. A rectangle's sides go on pixel edges
/// nearer than this.
const PAST: f32 = 1.0;

/// Returns `flat.wgsl` with what it is given ahead of it.
///
/// That is the kinds of shape and [`PAST`].
fn source() -> String {
  format!(
    "const RECT: f32 = {RECT:?};\nconst SPAN: f32 = {SPAN:?};\nconst PAST: f32 = {PAST:?};\n{}",
    include_str!("flat.wgsl")
  )
}

/// One corner of a shape.
///
/// A rectangle and a line are each drawn about a middle: the rectangle along the pixels and
/// onto their edges, the line any way round with its edges smoothed. A span has no middle. It
/// keeps its left and right sides in `middle`, how far down its sloping edge is at the two in
/// `along`, and how far down its level line is as the first of `half`.
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
  /// A line's two points, the one it runs from first.
  ends: [f32; 4],
  /// The way a curve runs through a line's two points.
  ///
  /// The first point's comes first, and a round end has zero. Two lines that meet in a point hold
  /// the same numbers for it here and in `ends`, which is what the shader parts them by.
  joins: [f32; 4],
  /// [`LINE`], [`RECT`] or [`SPAN`].
  kind: f32,
}

impl Corner {
  const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4,
    5 => Float32x4, 6 => Float32x4, 7 => Float32
  ];

  /// Returns the six corners of the two triangles a shape is drawn as.
  fn of(shape: &Shape) -> [Self; 6] {
    let all = match *shape {
      Shape::Rect { min, max, .. } => Self {
        middle: [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0],
        along: [1.0, 0.0],
        half: [(max[0] - min[0]) / 2.0, (max[1] - min[1]) / 2.0],
        kind: RECT,
        ..Self::zeroed()
      },
      Shape::Line {
        from,
        to,
        weight,
        joins,
        ..
      } => {
        let reach = [to[0] - from[0], to[1] - from[1]];
        let length = (reach[0] * reach[0] + reach[1] * reach[1]).sqrt();
        let each = 1.0 / length;
        // a line of no length is a dot, and lies across
        let along = if each.is_finite() && each > 0.0 {
          [reach[0] * each, reach[1] * each]
        } else {
          [1.0, 0.0]
        };
        let [start, end] = joins;
        Self {
          middle: [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0],
          along,
          half: [length / 2.0, weight / 2.0],
          ends: [from[0], from[1], to[0], to[1]],
          joins: [start[0], start[1], end[0], end[1]],
          kind: LINE,
          ..Self::zeroed()
        }
      }
      Shape::Span {
        sides, edge, level, ..
      } => Self {
        middle: sides,
        along: edge,
        half: [level, 0.0],
        kind: SPAN,
        ..Self::zeroed()
      },
    };
    let sides = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    [0, 1, 2, 0, 2, 3].map(|side: usize| Self {
      side: sides[side],
      color: shape.color(),
      ..all
    })
  }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Options {
  size: [f32; 2],
  pixels: [f32; 2],
  /// The left and top of the shapes' place in the target.
  ///
  /// They are in the target's pixels.
  origin: [f32; 2],
  /// One as its first number when the target encodes sRGB itself.
  options: [f32; 2],
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
  /// The place on the target the shapes last prepared go on.
  place: [u32; 4],
  target_encodes: bool,
}

impl FlatRenderer {
  /// Creates a renderer for targets of one format.
  ///
  /// `target` is the format of the textures [`FlatRenderer::render`] draws onto.
  pub(crate) fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("controls flat"),
      source: wgpu::ShaderSource::Wgsl(source().into()),
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
      place: [0; 4],
      // a new buffer is all zeros
      held_options: Options::zeroed(),
      target_encodes: target.is_srgb(),
    }
  }

  /// Returns the options that shapes are drawn with on a place.
  ///
  /// `size` and `place` are as [`FlatRenderer::prepare`] takes them.
  fn options(&self, size: Point, place: [u32; 4]) -> Options {
    let [left, top, width, height] = place.map(|side| side as f32);
    Options {
      size,
      pixels: [width, height],
      origin: [left, top],
      options: [f32::from(u8::from(self.target_encodes)), 0.0],
    }
  }

  /// Returns the rectangle of the target where shapes are new.
  ///
  /// `flat`, `size` and `place` are as [`FlatRenderer::prepare`] takes them, and the rectangle
  /// is given as `place` is. It holds every pixel that the shapes are drawn otherwise in than
  /// the ones last prepared, and is `None` if they are drawn the same.
  pub(crate) fn differing(&self, flat: &Flat, size: Point, place: [u32; 4]) -> Option<[u32; 4]> {
    let options = self.options(size, place);
    if options == self.held_options {
      differing(self.held.shapes(), flat.shapes(), &options)
    } else {
      Some(place)
    }
  }

  /// Takes the shapes to draw next and the place they go on.
  ///
  /// `size` is the panel's width and height in points. `place` is `[left, top, width, height]`
  /// in the pixels of the target [`FlatRenderer::render`] is given, and has to lie on it.
  pub(crate) fn prepare(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    flat: &Flat,
    size: Point,
    place: [u32; 4],
  ) {
    let options = self.options(size, place);
    self.place = place;
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

  /// Draws the shapes last prepared onto the target.
  ///
  /// They go on the place they were prepared for, and everything outside it stays as it was.
  pub(crate) fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    let count = self.held.shapes().len() as u32 * 6;
    let [x, y, width, height] = self.place;
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

/// Returns the rectangle of a target that two lists differ in.
///
/// It is around every shape of either list that the other does not have at the same place in
/// it, as `[left, top, width, height]` in the target's pixels, and `None` for the same lists.
fn differing(before: &[Shape], now: &[Shape], options: &Options) -> Option<[u32; 4]> {
  let (shorter, longer) = if before.len() <= now.len() {
    (before, now)
  } else {
    (now, before)
  };
  let changed = (before.iter().zip(now)).filter(|(before, now)| before != now);
  let all = changed
    .flat_map(|(before, now)| [before, now])
    .chain(&longer[shorter.len()..])
    .map(|shape| reach(shape, options));
  let [left, top, right, foot] = all.reduce(|[left, top, right, foot], shape| {
    [
      left.min(shape[0]),
      top.min(shape[1]),
      right.max(shape[2]),
      foot.max(shape[3]),
    ]
  })?;
  let [x, y] = options.origin;
  let [width, height] = options.pixels;
  let (left, top) = (left.floor().max(0.0), top.floor().max(0.0));
  let (right, foot) = (right.ceil().min(width), foot.ceil().min(height));
  (left < right && top < foot)
    .then(|| [x + left, y + top, right - left, foot - top].map(|side| side as u32))
}

/// Returns the rectangle of pixels a shape is drawn in.
///
/// It is its left, top, right and foot in pixels of the shapes' place, [`PAST`] past the shape
/// as `flat.wgsl` draws it.
fn reach(shape: &Shape, options: &Options) -> [f32; 4] {
  let scale = [0, 1].map(|axis| options.pixels[axis] / options.size[axis]);
  let (min, max, past) = match *shape {
    Shape::Rect { min, max, .. } => (min, max, PAST),
    Shape::Line {
      from, to, weight, ..
    } => {
      let past = weight / 2.0 * scale[0] + PAST;
      let ends = |pick: fn(f32, f32) -> f32| [0, 1].map(|axis| pick(from[axis], to[axis]));
      (ends(f32::min), ends(f32::max), past)
    }
    Shape::Span {
      sides, edge, level, ..
    } => (
      [sides[0].min(sides[1]), edge[0].min(edge[1]).min(level)],
      [sides[0].max(sides[1]), edge[0].max(edge[1]).max(level)],
      PAST,
    ),
  };
  [
    min[0] * scale[0] - past,
    min[1] * scale[1] - past,
    max[0] * scale[0] + past,
    max[1] * scale[1] + past,
  ]
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
  use super::*;

  #[test]
  fn shapes_differ_in_a_rectangle_around_those_that_are_not_the_same() {
    // a panel of 100 by 50 points on 200 by 100 pixels, 10 and 20 into the target
    let options = Options {
      size: [100.0, 50.0],
      pixels: [200.0, 100.0],
      origin: [10.0, 20.0],
      options: [0.0; 2],
    };
    let ink = [1.0; 4];
    let mut before = Flat::default();
    before.rect([0.0, 0.0], [100.0, 50.0], ink);
    before.line([20.0, 10.0], [30.0, 10.0], 2.0, ink);
    before.area(&[[50.0, 30.0], [60.0, 40.0]], 45.0, ink);
    let differ = |now: &Flat| differing(before.shapes(), now.shapes(), &options);
    assert_eq!(differ(&before), None);

    // the line somewhere else: around where it was and where it is, as far as it is drawn
    let mut moved = Flat::default();
    moved.rect([0.0, 0.0], [100.0, 50.0], ink);
    moved.line([20.0, 15.0], [35.0, 15.0], 2.0, ink);
    moved.area(&[[50.0, 30.0], [60.0, 40.0]], 45.0, ink);
    let [left, top, width, height] = differ(&moved).expect("the lines differ");
    // its ends at 40 and 70 across and 20 and 30 down, each a pixel of half its width past them
    assert!((10 + 34..=10 + 37).contains(&left), "{left}");
    assert!((20 + 14..=20 + 17).contains(&top), "{top}");
    assert!((10 + 73..=10 + 76).contains(&(left + width)), "{width}");
    assert!((20 + 33..=20 + 36).contains(&(top + height)), "{height}");

    // a shape more or a shape less: around that shape, whichever list has it
    let mut fewer = Flat::default();
    fewer.rect([0.0, 0.0], [100.0, 50.0], ink);
    fewer.line([20.0, 10.0], [30.0, 10.0], 2.0, ink);
    let around_area = differ(&fewer).expect("one has a shape more");
    assert_eq!(
      differing(fewer.shapes(), before.shapes(), &options),
      Some(around_area)
    );
    let [left, top, width, height] = around_area;
    assert!(
      left <= 10 + 100 && left + width >= 10 + 120,
      "{around_area:?}"
    );
    assert!(top <= 20 + 60 && top + height >= 20 + 90, "{around_area:?}");
    assert!(width < 40 && height < 50, "{around_area:?}");

    // the ground in another color is the whole place and no more
    let mut ground = Flat::default();
    ground.rect([0.0, 0.0], [100.0, 50.0], [0.5; 4]);
    ground.line([20.0, 10.0], [30.0, 10.0], 2.0, ink);
    ground.area(&[[50.0, 30.0], [60.0, 40.0]], 45.0, ink);
    assert_eq!(differ(&ground), Some([10, 20, 200, 100]));
  }

  #[test]
  fn the_shader_can_be_written_for_every_backend() {
    use naga::back::{hlsl, msl, spv};

    // a backend is handed the shader in a language of its own, which naga writes it in when
    // the pipeline is made. one it cannot be written in fails there, on the user's machine
    let module = naga::front::wgsl::parse_str(&source()).expect("the shader parses");
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
