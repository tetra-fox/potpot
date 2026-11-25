//! draws a [`Scene`] with wgpu: a few maps, the lit picture at its own resolution, and that
//! picture stretched into place on a target. the picture is kept, and drawn again only for a
//! scene that differs from the one it shows

mod picture;
mod pipelines;
mod uniforms;

use std::mem::size_of;
use std::sync::Arc;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use self::picture::Picture;
use self::pipelines::{BOUNCE, DEPTH, Layouts, Pipelines, SAMPLES};
use self::uniforms::{Globals, Placed, Present};
use crate::mesh::Mesh;
use crate::scene::{Object, Scene};

/// the shadow map's width and height
const SHADOW_SIZE: u32 = 2048;
/// the width and height of the map of how high everything on the panel is
const HEIGHT_SIZE: u32 = 1024;
/// the width and height of the map of the light that comes off everything on the panel.
/// small, since a point catches that light from a few of its texels around it and so from
/// further off the larger they are
const BOUNCE_SIZE: u32 = 64;

/// a rectangle of a target, in its pixels
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
}

/// everything on the gpu that drawing scenes takes. [`Renderer::prepare`] draws a scene's
/// picture and [`Renderer::render`] puts the last one drawn on a target
#[derive(Debug)]
pub struct Renderer {
  pipelines: Pipelines,
  globals: wgpu::Buffer,
  globals_group: wgpu::BindGroup,
  shadow_map: wgpu::TextureView,
  /// how high everything on the panel is, seen from straight above
  height_map: wgpu::TextureView,
  /// the shadow map alone, for the bounce pass to read
  shadow_group: wgpu::BindGroup,
  /// the light that comes off everything on the panel, seen from straight above: what the
  /// bounce pass draws into, its depth, and what that is resolved into to be read
  bounce_samples: wgpu::TextureView,
  bounce_depth: wgpu::TextureView,
  bounce_map: wgpu::TextureView,
  /// every map, for the scene pass to read
  maps_group: wgpu::BindGroup,
  present_layout: wgpu::BindGroupLayout,
  present_options: wgpu::Buffer,
  /// whether the target encodes srgb itself
  target_encodes: bool,
  /// the widest and tallest a texture of the device can be
  max_side: u32,
  picture: Option<Picture>,
  placed: wgpu::Buffer,
  /// how many objects the placed buffer has room for
  placed_room: usize,
  /// what the three buffers hold, to tell a scene that differs from the one drawn
  held_globals: Globals,
  held_present: Present,
  held_placed: Vec<Placed>,
  /// where a scene's objects are worked out to compare with what is held, kept for its room
  next_placed: Vec<Placed>,
  /// each mesh of the drawn scene, once
  meshes: Vec<Loaded>,
  /// which of the meshes each object of the drawn scene is, in order
  draws: Vec<usize>,
}

/// a mesh on the gpu, and the allocation it came from
#[derive(Debug)]
struct Loaded {
  from: Arc<Mesh>,
  vertices: wgpu::Buffer,
  indices: wgpu::Buffer,
  count: u32,
}

impl Renderer {
  /// `target` is the format of the textures [`Renderer::render`] will draw onto
  pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    let layouts = Layouts::new(device);
    let pipelines = Pipelines::new(device, &layouts, target);

    let uniform = |label, size| {
      device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
      })
    };
    let globals = uniform("potpot globals", size_of::<Globals>());
    let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot globals"),
      layout: &layouts.globals,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: globals.as_entire_binding(),
      }],
    });

    let read = wgpu::TextureUsages::TEXTURE_BINDING;
    let shadow_map = map(device, "potpot shadow map", SHADOW_SIZE, DEPTH, 1, read);
    let height_map = map(device, "potpot heights", HEIGHT_SIZE, DEPTH, 1, read);
    let none = wgpu::TextureUsages::empty();
    let bounce_samples = map(
      device,
      "potpot bounce samples",
      BOUNCE_SIZE,
      BOUNCE,
      SAMPLES,
      none,
    );
    let bounce_depth = map(
      device,
      "potpot bounce depth",
      BOUNCE_SIZE,
      DEPTH,
      SAMPLES,
      none,
    );
    let bounce_map = map(device, "potpot bounce", BOUNCE_SIZE, BOUNCE, 1, read);
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot shadow map"),
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      compare: Some(wgpu::CompareFunction::LessEqual),
      ..Default::default()
    });
    let bounce_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot bounce"),
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      ..Default::default()
    });
    let view = |binding, view| wgpu::BindGroupEntry {
      binding,
      resource: wgpu::BindingResource::TextureView(view),
    };
    let sampler = |binding, sampler| wgpu::BindGroupEntry {
      binding,
      resource: wgpu::BindingResource::Sampler(sampler),
    };
    let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot shadow map"),
      layout: &layouts.shadow,
      entries: &[view(0, &shadow_map), sampler(1, &shadow_sampler)],
    });
    let maps_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot maps"),
      layout: &layouts.maps,
      entries: &[
        view(0, &shadow_map),
        sampler(1, &shadow_sampler),
        view(2, &height_map),
        view(3, &bounce_map),
        sampler(4, &bounce_sampler),
      ],
    });

    Self {
      pipelines,
      globals,
      globals_group,
      shadow_map,
      height_map,
      shadow_group,
      bounce_samples,
      bounce_depth,
      bounce_map,
      maps_group,
      present_layout: layouts.present,
      present_options: uniform("potpot present", size_of::<Present>()),
      target_encodes: target.is_srgb(),
      max_side: device.limits().max_texture_dimension_2d,
      picture: None,
      placed: placed_buffer(device, 0),
      placed_room: 0,
      // a new buffer is all zeros
      held_globals: Globals::zeroed(),
      held_present: Present::zeroed(),
      held_placed: Vec::new(),
      next_placed: Vec::new(),
      meshes: Vec::new(),
      draws: Vec::new(),
    }
  }

  /// draws the picture of a scene to go over `size` pixels of a target, unless it is the
  /// picture already drawn: a scene that is in every part the one prepared last costs no
  /// drawing
  pub fn prepare(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &Scene,
    size: [u32; 2],
  ) {
    let look = scene.look;
    // the picture is stretched over its place whatever its size, so one that the device has
    // no texture large enough for is drawn as large as it has
    let picture_size = size
      .map(|side| ((side as f32 / look.pixel_size.max(1.0)).ceil() as u32).clamp(1, self.max_side));
    let kept = (self.picture.take()).filter(|picture| picture.fits(picture_size, look.antialias));
    let mut stale = kept.is_none();
    let picture = kept.unwrap_or_else(|| {
      Picture::new(
        device,
        &self.present_layout,
        &self.present_options,
        picture_size,
        look.antialias,
      )
    });

    // the present pass reads these every time it runs, so new ones need no new picture
    let present = Present::of(look, self.target_encodes);
    if present != self.held_present {
      queue.write_buffer(&self.present_options, 0, bytemuck::bytes_of(&present));
      self.held_present = present;
    }
    let globals = Globals::of(scene);
    if globals != self.held_globals {
      queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
      self.held_globals = globals;
      stale = true;
    }

    let objects = &scene.objects;
    let same_meshes = objects.len() == self.draws.len()
      && (objects.iter().zip(&self.draws))
        .all(|(object, &mesh)| Arc::ptr_eq(&object.mesh, &self.meshes[mesh].from));
    if !same_meshes {
      self.load(device, objects);
      stale = true;
    }
    self.next_placed.clear();
    self.next_placed.extend(objects.iter().map(Placed::of));
    if self.next_placed != self.held_placed {
      std::mem::swap(&mut self.held_placed, &mut self.next_placed);
      if self.held_placed.len() > self.placed_room {
        self.placed_room = self.held_placed.len().next_power_of_two();
        self.placed = placed_buffer(device, self.placed_room);
      }
      queue.write_buffer(&self.placed, 0, bytemuck::cast_slice(&self.held_placed));
      stale = true;
    }

    if stale {
      self.draw_picture(device, queue, &picture);
    }
    self.picture = Some(picture);
  }

  /// puts every mesh the objects use on the gpu once, keeping the ones that are there
  /// already and dropping the rest
  fn load(&mut self, device: &wgpu::Device, objects: &[Object]) {
    let mut before = std::mem::take(&mut self.meshes);
    let found = |meshes: &[Loaded], mesh: &Arc<Mesh>| {
      (meshes.iter()).position(|loaded| Arc::ptr_eq(&loaded.from, mesh))
    };
    self.draws.clear();
    for object in objects {
      let place = found(&self.meshes, &object.mesh).unwrap_or_else(|| {
        let loaded = match found(&before, &object.mesh) {
          Some(kept) => before.swap_remove(kept),
          None => Loaded::new(device, &object.mesh),
        };
        self.meshes.push(loaded);
        self.meshes.len() - 1
      });
      self.draws.push(place);
    }
  }

  /// the shadow map, the height map, the bounce map if light bounces, and then the lit
  /// scene into the picture
  fn draw_picture(&self, device: &wgpu::Device, queue: &wgpu::Queue, picture: &Picture) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
      label: Some("potpot picture"),
    });
    let depth_passes = [
      ("potpot shadow", &self.pipelines.shadow, &self.shadow_map),
      ("potpot heights", &self.pipelines.height, &self.height_map),
    ];
    for (label, pipeline, map) in depth_passes {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[],
        depth_stencil_attachment: Some(depth_attachment(map)),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_pipeline(pipeline);
      pass.set_bind_group(0, &self.globals_group, &[]);
      self.draw_objects(&mut pass);
    }
    if self.held_globals.bounces() {
      // where nothing is drawn no light comes off, from as low as anything gets
      let nothing = wgpu::Color {
        a: f64::from(self.held_globals.lowest()),
        ..wgpu::Color::TRANSPARENT
      };
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("potpot bounce"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &self.bounce_samples,
          depth_slice: None,
          resolve_target: Some(&self.bounce_map),
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(nothing),
            store: wgpu::StoreOp::Discard,
          },
        })],
        depth_stencil_attachment: Some(depth_attachment(&self.bounce_depth)),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_pipeline(&self.pipelines.bounce);
      pass.set_bind_group(0, &self.globals_group, &[]);
      pass.set_bind_group(1, &self.shadow_group, &[]);
      self.draw_objects(&mut pass);
    }
    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("potpot scene"),
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
      pass.set_pipeline(&self.pipelines.scene[usize::from(picture.smoothed.is_some())]);
      pass.set_bind_group(0, &self.globals_group, &[]);
      pass.set_bind_group(1, &self.maps_group, &[]);
      self.draw_objects(&mut pass);
    }
    queue.submit([encoder.finish()]);
  }

  /// stretches the picture last drawn over `place` on the target, leaving everything outside
  /// `clip` as it was. `clip` is x, y, width and height in the target's pixels, and has to
  /// lie on the target. before any scene is prepared there is no picture and nothing is drawn
  pub fn render(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    place: Place,
    clip: [u32; 4],
  ) {
    let Some(picture) = &self.picture else {
      return;
    };
    if clip[2] == 0 || clip[3] == 0 || place.width <= 0.0 || place.height <= 0.0 {
      return;
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("potpot present"),
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
    pass.set_pipeline(&self.pipelines.present);
    pass.set_bind_group(0, &picture.group, &[]);
    pass.set_viewport(place.x, place.y, place.width, place.height, 0.0, 1.0);
    pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
    pass.draw(0..3, 0..1);
  }

  fn draw_objects(&self, pass: &mut wgpu::RenderPass<'_>) {
    pass.set_vertex_buffer(1, self.placed.slice(..));
    for (object, &mesh) in (0..).zip(&self.draws) {
      let mesh = &self.meshes[mesh];
      // wgpu has no slice of an empty buffer, and such a mesh has nothing to draw
      if mesh.count == 0 {
        continue;
      }
      pass.set_vertex_buffer(0, mesh.vertices.slice(..));
      pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
      pass.draw_indexed(0..mesh.count, 0, object..object + 1);
    }
  }
}

impl Loaded {
  fn new(device: &wgpu::Device, mesh: &Arc<Mesh>) -> Self {
    Self {
      from: Arc::clone(mesh),
      vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("potpot vertices"),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
      }),
      indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("potpot indices"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
      }),
      // a mesh without vertices has no triangles, whatever its indices say
      count: if mesh.vertices.is_empty() {
        0
      } else {
        mesh.indices.len() as u32
      },
    }
  }
}

/// a square texture that a pass draws into, with whatever else is done with it
fn map(
  device: &wgpu::Device,
  label: &str,
  size: u32,
  format: wgpu::TextureFormat,
  sample_count: u32,
  also: wgpu::TextureUsages,
) -> wgpu::TextureView {
  device
    .create_texture(&wgpu::TextureDescriptor {
      label: Some(label),
      size: wgpu::Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count,
      dimension: wgpu::TextureDimension::D2,
      format,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | also,
      view_formats: &[],
    })
    .create_view(&wgpu::TextureViewDescriptor::default())
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
    label: Some("potpot placed"),
    size: (room.max(1) * size_of::<Placed>()) as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
  })
}
