//! A renderer of [`Flat`] shapes.
//!
//! The shapes go straight onto a target, over whatever is drawn there already. The same
//! pipeline draws a window's shapes and the pictures of a scene.

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

/// The way the shapes' points land on the pixels of their place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mapping {
  /// The pixels to a point, across and down.
  pub(crate) scale: [f32; 2],
  /// The pixel the panel's top left lands on, from the place's top left.
  pub(crate) shift: [f32; 2],
  /// Whether the edges of lines and spans are left rough.
  ///
  /// A pixel that an edge covers in part is then drawn whole or not at all.
  pub(crate) rough: bool,
}

impl Mapping {
  /// Returns the mapping of a panel of `size` points over the whole of a place.
  fn over(size: Point, place: [u32; 4]) -> Self {
    Self {
      scale: [0, 1].map(|axis| place[axis + 2] as f32 / size[axis]),
      shift: [0.0; 2],
      rough: false,
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Options {
  /// The pixels to a point, then the pixel the panel's top left lands on.
  mapping: [f32; 4],
  /// The width and height of the shapes' place in pixels.
  pixels: [f32; 2],
  /// The left and top of the shapes' place in the target.
  ///
  /// They are in the target's pixels.
  origin: [f32; 2],
  /// One as its first number when edges are left rough.
  options: [f32; 4],
}

impl Options {
  fn new(mapping: Mapping, place: [u32; 4]) -> Self {
    let [left, top, width, height] = place.map(|side| side as f32);
    let ([across, down], [x, y]) = (mapping.scale, mapping.shift);
    Self {
      mapping: [across, down, x, y],
      pixels: [width, height],
      origin: [left, top],
      options: [f32::from(u8::from(mapping.rough)), 0.0, 0.0, 0.0],
    }
  }
}

/// The pipeline that draws shapes onto targets of one format.
#[derive(Debug)]
pub(crate) struct Pipeline {
  pipeline: wgpu::RenderPipeline,
  layout: wgpu::BindGroupLayout,
}

/// What one list of shapes is drawn from: its corners and its options.
#[derive(Debug)]
pub(crate) struct Drawing {
  options: wgpu::Buffer,
  group: wgpu::BindGroup,
  corners: wgpu::Buffer,
  /// The number of corners the corner buffer has room for.
  room: usize,
  /// The number of corners it holds.
  count: u32,
  /// The contents of the options buffer.
  ///
  /// The buffer is written only when the options differ from them.
  held_options: Options,
  /// The place on the target the shapes go on.
  place: [u32; 4],
}

impl Pipeline {
  /// Creates the pipeline for targets of `target`'s format.
  ///
  /// # Panics
  ///
  /// Panics if `target` encodes sRGB.
  pub(crate) fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    // colors are blended as they are to look, which a target that encodes them itself undoes
    assert!(
      !target.is_srgb(),
      "the target holds colors as they are to look"
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("potpot flat"),
      source: wgpu::ShaderSource::Wgsl(source().into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("potpot flat"),
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
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("potpot flat"),
      layout: Some(
        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
          label: Some("potpot flat"),
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
    Self { pipeline, layout }
  }

  /// Creates a drawing with no shapes, to be drawn by this pipeline.
  pub(crate) fn drawing(&self, device: &wgpu::Device) -> Drawing {
    let options = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("potpot flat"),
      size: size_of::<Options>() as u64,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot flat"),
      layout: &self.layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: options.as_entire_binding(),
      }],
    });
    Drawing {
      options,
      group,
      corners: corner_buffer(device, 0),
      room: 0,
      count: 0,
      // a new buffer is all zeros
      held_options: Options::zeroed(),
      place: [0; 4],
    }
  }

  /// Draws a drawing onto its place on the target.
  ///
  /// Everything outside the place stays as it was. With `clear`, the place is cleared to no
  /// color first.
  pub(crate) fn draw(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    drawing: &Drawing,
    clear: bool,
  ) {
    let [x, y, width, height] = drawing.place;
    if (drawing.count == 0 && !clear) || width == 0 || height == 0 {
      return;
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("potpot flat"),
      color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: target,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
          load: if clear {
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
          } else {
            wgpu::LoadOp::Load
          },
          store: wgpu::StoreOp::Store,
        },
      })],
      depth_stencil_attachment: None,
      timestamp_writes: None,
      occlusion_query_set: None,
    });
    if drawing.count == 0 {
      return;
    }
    pass.set_pipeline(&self.pipeline);
    pass.set_bind_group(0, &drawing.group, &[]);
    pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
    pass.set_scissor_rect(x, y, width, height);
    pass.set_vertex_buffer(0, drawing.corners.slice(..));
    pass.draw(0..drawing.count, 0..1);
  }
}

impl Drawing {
  /// Takes the options the shapes are drawn with next and the place they go on.
  fn set_options(&mut self, queue: &wgpu::Queue, options: Options, place: [u32; 4]) {
    self.place = place;
    if options != self.held_options {
      queue.write_buffer(&self.options, 0, bytemuck::bytes_of(&options));
      self.held_options = options;
    }
  }

  /// Takes the shapes to draw next, first drawn first.
  fn set_shapes<'a>(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shapes: impl IntoIterator<Item = &'a Shape>,
  ) {
    let corners: Vec<Corner> = shapes.into_iter().flat_map(Corner::of).collect();
    if corners.len() > self.room {
      self.room = corners.len().next_power_of_two();
      self.corners = corner_buffer(device, self.room);
    }
    queue.write_buffer(&self.corners, 0, bytemuck::cast_slice(&corners));
    self.count = corners.len() as u32;
  }

  /// Takes shapes and the way they are drawn on the place they go on.
  ///
  /// `place` is `[left, top, width, height]` in the pixels of the target the drawing is drawn
  /// on, and has to lie on it.
  pub(crate) fn set<'a>(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shapes: impl IntoIterator<Item = &'a Shape>,
    mapping: Mapping,
    place: [u32; 4],
  ) {
    self.set_options(queue, Options::new(mapping, place), place);
    self.set_shapes(device, queue, shapes);
  }
}

/// The GPU resources that drawing [`Flat`] shapes over a panel takes.
///
/// The shapes are drawn with their edges smoothed, in colors blended over what the target
/// shows as they are to look, as a browser blends them. [`FlatRenderer::prepare`] takes the
/// shapes and [`FlatRenderer::render`] draws them.
#[derive(Debug)]
pub struct FlatRenderer {
  pipeline: Pipeline,
  drawing: Drawing,
  /// The shapes whose corners the drawing holds.
  ///
  /// [`FlatRenderer::prepare`] writes them only when it is given other shapes.
  held: Flat,
}

impl FlatRenderer {
  /// Creates a renderer for targets of one format.
  ///
  /// `target` is the format of the textures [`FlatRenderer::render`] draws onto. It holds colors
  /// as they are to look, as `Bgra8Unorm` does, and the shapes are blended in those colors. A
  /// texture of an sRGB format is drawn onto through a view of its format without the encoding.
  ///
  /// # Panics
  ///
  /// Panics if `target` encodes sRGB.
  pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    let pipeline = Pipeline::new(device, target);
    Self {
      drawing: pipeline.drawing(device),
      pipeline,
      held: Flat::default(),
    }
  }

  /// Returns the rectangles of the target where shapes are new.
  ///
  /// `flat`, `size` and `place` are as [`FlatRenderer::prepare`] takes them, and each rectangle
  /// is given as `place` is. Together they hold every pixel that the shapes are drawn otherwise
  /// in than the ones last prepared, and there is none if they are drawn the same.
  pub fn differing(&self, flat: &Flat, size: Point, place: [u32; 4]) -> Vec<[u32; 4]> {
    let options = Options::new(Mapping::over(size, place), place);
    if options == self.drawing.held_options {
      differing(self.held.shapes(), flat.shapes(), &options)
    } else {
      vec![place]
    }
  }

  /// Takes the shapes to draw next and the place they go on.
  ///
  /// `size` is the panel's width and height in points. `place` is `[left, top, width, height]`
  /// in the pixels of the target [`FlatRenderer::render`] is given, and has to lie on it.
  pub fn prepare(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    flat: &Flat,
    size: Point,
    place: [u32; 4],
  ) {
    let options = Options::new(Mapping::over(size, place), place);
    self.drawing.set_options(queue, options, place);
    if *flat != self.held {
      self.drawing.set_shapes(device, queue, flat.shapes());
      self.held.clone_from(flat);
    }
  }

  /// Draws the shapes last prepared onto the target.
  ///
  /// They go on the place they were prepared for, and everything outside it stays as it was.
  pub fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    self.pipeline.draw(encoder, target, &self.drawing, false);
  }
}

/// The most rectangles two lists of shapes differ in.
///
/// More than that are told as the one rectangle around them.
const MOST_DIFFERING: usize = 8;
/// The most pixels between shapes that one rectangle holds.
///
/// Two shapes that differ and are that near one another are told as one rectangle. The strokes
/// of one piece of lettering are that near.
const TOGETHER: f32 = 16.0;

/// Returns the rectangles of a target that two lists differ in.
///
/// They are around the shapes that are not the same in both, each as `[left, top, width,
/// height]` in the target's pixels, with those near one another in one rectangle. The shapes
/// that both lists start with and those that both end with are the same, however many lie
/// between them in each, and so are those between them at the same place in lists as long.
fn differing(before: &[Shape], now: &[Shape], options: &Options) -> Vec<[u32; 4]> {
  let starts = (before.iter().zip(now))
    .take_while(|(before, now)| before == now)
    .count();
  let (before, now) = (&before[starts..], &now[starts..]);
  let ends = (before.iter().rev().zip(now.iter().rev()))
    .take_while(|(before, now)| before == now)
    .count();
  let (before, now) = (&before[..before.len() - ends], &now[..now.len() - ends]);
  let alike = |index: usize| before.len() == now.len() && before[index] == now[index];
  let differing = (before.iter().enumerate())
    .chain(now.iter().enumerate())
    .filter(|(index, _)| !alike(*index));
  let around = |one: [f32; 4], other: [f32; 4]| {
    [
      one[0].min(other[0]),
      one[1].min(other[1]),
      one[2].max(other[2]),
      one[3].max(other[3]),
    ]
  };
  let near = |one: &[f32; 4], other: &[f32; 4]| {
    one[0] - TOGETHER <= other[2]
      && other[0] <= one[2] + TOGETHER
      && one[1] - TOGETHER <= other[3]
      && other[1] <= one[3] + TOGETHER
  };
  let mut apart: Vec<[f32; 4]> = Vec::new();
  for (_, shape) in differing {
    let mut shape = reach(shape, options);
    while let Some(held) = apart.iter().position(|held| near(held, &shape)) {
      shape = around(shape, apart.swap_remove(held));
    }
    apart.push(shape);
  }
  if apart.len() > MOST_DIFFERING {
    apart = apart.into_iter().reduce(around).into_iter().collect();
  }
  let [x, y] = options.origin;
  let [width, height] = options.pixels;
  (apart.into_iter())
    .filter_map(|[left, top, right, foot]| {
      let (left, top) = (left.floor().max(0.0), top.floor().max(0.0));
      let (right, foot) = (right.ceil().min(width), foot.ceil().min(height));
      (left < right && top < foot)
        .then(|| [x + left, y + top, right - left, foot - top].map(|side| side as u32))
    })
    .collect()
}

/// Returns the rectangle of pixels a shape is drawn in.
///
/// It is its left, top, right and foot in pixels of the shapes' place, [`PAST`] past the shape
/// as `flat.wgsl` draws it.
fn reach(shape: &Shape, options: &Options) -> [f32; 4] {
  let [across, down, x, y] = options.mapping;
  let (scale, shift) = ([across, down], [x, y]);
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
    min[0] * scale[0] + shift[0] - past,
    min[1] * scale[1] + shift[1] - past,
    max[0] * scale[0] + shift[0] + past,
    max[1] * scale[1] + shift[1] + past,
  ]
}

fn corner_buffer(device: &wgpu::Device, room: usize) -> wgpu::Buffer {
  device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("potpot flat"),
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
    let place = [10, 20, 200, 100];
    let options = Options::new(Mapping::over([100.0, 50.0], place), place);
    let ink = [1.0; 4];
    let mut before = Flat::default();
    before.rect([0.0, 0.0], [100.0, 50.0], ink);
    before.line([20.0, 10.0], [30.0, 10.0], 2.0, ink);
    before.area(&[[50.0, 30.0], [60.0, 40.0]], 45.0, ink);
    let differ = |now: &Flat| differing(before.shapes(), now.shapes(), &options);
    assert_eq!(differ(&before), [[0; 4]; 0]);

    // the line somewhere else: around where it was and where it is, as far as it is drawn
    let mut moved = Flat::default();
    moved.rect([0.0, 0.0], [100.0, 50.0], ink);
    moved.line([20.0, 15.0], [35.0, 15.0], 2.0, ink);
    moved.area(&[[50.0, 30.0], [60.0, 40.0]], 45.0, ink);
    let [[left, top, width, height]] = differ(&moved)[..] else {
      panic!("the lines differ in one rectangle");
    };
    // its ends at 40 and 70 across and 20 and 30 down, each a pixel of half its width past them
    assert!((10 + 34..=10 + 37).contains(&left), "{left}");
    assert!((20 + 14..=20 + 17).contains(&top), "{top}");
    assert!((10 + 73..=10 + 76).contains(&(left + width)), "{width}");
    assert!((20 + 33..=20 + 36).contains(&(top + height)), "{height}");

    // a shape more or a shape less: around that shape, whichever list has it
    let mut fewer = Flat::default();
    fewer.rect([0.0, 0.0], [100.0, 50.0], ink);
    fewer.line([20.0, 10.0], [30.0, 10.0], 2.0, ink);
    let [around_area] = differ(&fewer)[..] else {
      panic!("one has a shape more, in one rectangle");
    };
    assert_eq!(
      differing(fewer.shapes(), before.shapes(), &options),
      [around_area]
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
    assert_eq!(differ(&ground), [[10, 20, 200, 100]]);
  }

  #[test]
  fn shapes_that_differ_far_apart_differ_in_rectangles_of_their_own() {
    let place = [0, 0, 400, 200];
    let options = Options::new(Mapping::over([400.0, 200.0], place), place);
    let ink = [1.0; 4];
    // two values written far apart with a line between them, the first of them a stroke longer
    let written = |first: &[f32], second: f32| {
      let mut flat = Flat::default();
      for across in first {
        flat.line([20.0 + across, 20.0], [20.0 + across, 30.0], 1.0, ink);
      }
      flat.line([100.0, 100.0], [300.0, 100.0], 1.0, ink);
      flat.line([350.0, 170.0], [350.0 + second, 180.0], 1.0, ink);
      flat
    };
    let before = written(&[0.0, 6.0], 0.0);
    let differ = |now: &Flat| differing(before.shapes(), now.shapes(), &options);
    let holds = |part: &[u32; 4], [across, down]: [u32; 2]| {
      (part[0]..part[0] + part[2]).contains(&across) && (part[1]..part[1] + part[3]).contains(&down)
    };
    // a stroke more in the first: around that value alone, though every shape after it is at
    // another place in the list
    let longer = differ(&written(&[0.0, 6.0, 12.0], 0.0));
    assert_eq!(longer.len(), 1, "{longer:?}");
    assert!(holds(&longer[0], [32, 25]) && longer[0][2] < 40 && longer[0][3] < 30);
    // both values differ: a rectangle around each, and the line between them in neither
    let both = differ(&written(&[0.0, 9.0], 5.0));
    assert_eq!(both.len(), 2, "{both:?}");
    assert!(both.iter().any(|part| holds(part, [29, 25])));
    assert!(both.iter().any(|part| holds(part, [353, 175])));
    assert!(!both.iter().any(|part| holds(part, [200, 100])));
    // more places than are told apart are one rectangle around them all
    let mut many = Flat::default();
    for place in 0..MOST_DIFFERING + 1 {
      let across = 20.0 + place as f32 * 40.0;
      many.line([across, 20.0], [across, 30.0], 1.0, ink);
    }
    assert_eq!(differing(&[], many.shapes(), &options).len(), 1);
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
