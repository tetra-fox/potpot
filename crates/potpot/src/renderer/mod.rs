//! The drawing of a [`Scene`] with `wgpu`.
//!
//! A scene is drawn as a few maps, then the lit picture at its own resolution with a map of what
//! lights itself in it, then that picture stretched into place on a target. The picture is kept.
//! For a scene that differs from the one it shows, the maps that the difference is in are drawn
//! again, and of the picture as much as the difference shows in.

mod damage;
mod dfg;
mod environment;
mod picture;
mod pipelines;
mod textures;
mod uniforms;

use std::mem::size_of;
use std::sync::Arc;

use bytemuck::Zeroable;
use glam::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

use self::damage::{Bounds, HAZE_CELL, Reach, Redrawn, Spread};
use self::picture::Picture;
use self::pipelines::{BOUNCE, DEPTH, DFG, Layouts, Pipelines, SAMPLES, STUDIO};
use self::uniforms::{Globals, Placed, Present};
use crate::mesh::Mesh;
use crate::scene::{Object, Scene};

/// The shadow map's width and height in texels.
const SHADOW_SIZE: u32 = 2048;
/// The height map's width and height in texels.
///
/// The map holds how high everything on the panel is.
const HEIGHT_SIZE: u32 = 1024;
/// The samples of a pixel its mean is refined to while the scene stands still.
const SETTLED: u32 = 64;
/// The bounce map's width and height in texels.
///
/// The map holds the light that comes off everything on the panel. It is small, since a point
/// catches that light from a few of its texels around it and so from further off the larger
/// they are.
const BOUNCE_SIZE: u32 = 64;

/// A rectangle of a target, in the target's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
  /// The distance from the target's left edge to the rectangle's.
  pub x: f32,
  /// The distance down from the target's top edge to the rectangle's.
  pub y: f32,
  /// The rectangle's width.
  pub width: f32,
  /// The rectangle's height.
  pub height: f32,
}

/// Everything on the GPU that drawing scenes takes.
///
/// [`Renderer::prepare`] draws a scene's picture and [`Renderer::render`] puts the last one
/// drawn on a target.
///
/// # Examples
///
/// ```no_run
/// use potpot::{Place, Renderer, Scene, wgpu};
///
/// fn draw(
///   device: &wgpu::Device,
///   queue: &wgpu::Queue,
///   target: &wgpu::TextureView,
///   scene: &Scene,
/// ) {
///   let mut renderer = Renderer::new(device, wgpu::TextureFormat::Bgra8UnormSrgb);
///   renderer.prepare(device, queue, scene, [800, 400]);
///
///   let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
///   let place = Place {
///     x: 0.0,
///     y: 0.0,
///     width: 800.0,
///     height: 400.0,
///   };
///   renderer.render(&mut encoder, target, place, [0, 0, 800, 400]);
///   queue.submit([encoder.finish()]);
/// }
/// ```
#[derive(Debug)]
pub struct Renderer {
  pipelines: Pipelines,
  globals: wgpu::Buffer,
  /// Whether what is drawn next is each pixel's first sample, for the scene pass.
  refine: wgpu::Buffer,
  globals_group: wgpu::BindGroup,
  shadow_map: wgpu::TextureView,
  /// The height of everything on the panel, seen from straight above.
  height_map: wgpu::TextureView,
  /// The shadow map alone, for the bounce pass to read.
  shadow_group: wgpu::BindGroup,
  /// The texture the bounce pass draws into.
  ///
  /// The pass draws the light that comes off everything on the panel, seen from straight above.
  bounce_samples: wgpu::TextureView,
  bounce_depth: wgpu::TextureView,
  /// The texture the bounce pass is resolved into to be read.
  bounce_map: wgpu::TextureView,
  /// Every map, for the scene pass to read.
  maps_group: wgpu::BindGroup,
  /// The DFG table.
  dfg_table: wgpu::Texture,
  /// The DFG table's texels, until the table holds them.
  dfg_texels: Option<wgpu::Buffer>,
  /// The studio without its softbox as mirrors of each roughness show it, a cube map.
  studio_map: wgpu::Texture,
  /// The softbox alone, as [`Renderer::studio_map`] holds the rest.
  softbox_map: wgpu::Texture,
  /// The color, normal and roughness maps of the texture sets, each an array of the sets.
  texture_maps: [wgpu::Texture; 3],
  /// Whether the maps of the studio and the texture sets are yet to be given their texels.
  unfilled: bool,
  layouts: Layouts,
  present_options: wgpu::Buffer,
  /// The fewest samples any pixel's mean of the picture holds.
  least: u32,
  /// Whether the target encodes sRGB itself.
  target_encodes: bool,
  /// The greatest width and height of a texture of the device.
  max_side: u32,
  picture: Option<Picture>,
  placed: wgpu::Buffer,
  /// The number of objects the placed buffer has room for.
  placed_room: usize,
  /// The contents of the globals buffer.
  ///
  /// The held contents of the three buffers tell a scene that differs from the one drawn.
  held_globals: Globals,
  /// The contents of the present options buffer.
  held_present: Present,
  /// The blur of the glow map that the blurred maps hold.
  held_spread: Spread,
  /// The contents of the placed buffer.
  held_placed: Vec<Placed>,
  /// The placed objects of a scene, worked out to compare with the held ones.
  ///
  /// The vector is kept for its room.
  next_placed: Vec<Placed>,
  /// Each mesh of the drawn scene, once.
  meshes: Vec<Loaded>,
  /// The place in `meshes` of the mesh of each object of the drawn scene, in order.
  draws: Vec<usize>,
  /// The rectangles each object of the drawn scene is drawn in.
  ///
  /// They are those only while `spanned` is set.
  spans: Vec<Spans>,
  /// Whether `spans` holds the drawn scene's.
  ///
  /// They are worked out only for a scene that may be drawn in part.
  spanned: bool,
  /// Whether a change of the drawn scene can be bounded.
  ///
  /// [`damage::bounded`] tells, and is asked only of a scene that few objects differ in.
  sound: bool,
  /// The objects that differ from the drawn scene's, if few do.
  ///
  /// The vector is kept for its room.
  differing: Vec<Differing>,
  /// The boxes around what differs in its look alone.
  ///
  /// Each is around objects that differ and throw the shadows they threw, or none. The vector is
  /// kept for its room.
  painted: Vec<Bounds>,
  /// The corners of the triangles over what is drawn again.
  ///
  /// They are in the picture's clip space. The vector is kept for its room.
  wiped: Vec<[f32; 2]>,
  /// Where what is drawn again lies across each band.
  ///
  /// The bands are of the picture. The vector is kept for its room.
  bands: Vec<damage::Band>,
  /// The corners as the wipe pipeline is handed them.
  wipe: wgpu::Buffer,
  /// The number of corners the wipe buffer has room for.
  wipe_room: usize,
  /// The size of the target the picture was last prepared for.
  held_size: [u32; 2],
  /// The parts of that target where the picture is new.
  ///
  /// The picture shows otherwise there than the one before it. The vector is kept for its room.
  parts: Vec<[u32; 4]>,
  /// Whether the picture may be new anywhere on the target.
  ///
  /// It may whatever `parts` holds.
  anywhere: bool,
}

/// The rectangles one object is drawn in.
///
/// Each is its left, top, right and foot, one past the object on every side.
#[derive(Clone, Copy, Debug)]
struct Spans {
  /// The rectangle of the picture, in pixels.
  seen: [f32; 4],
  /// The rectangle of the shadow map, in texels.
  shadowed: [f32; 4],
  /// The rectangle of the height map, in texels.
  overhead: [f32; 4],
}

/// What a pass draws the objects on.
///
/// It is one of the three that an object's [`Spans`] are of.
#[derive(Clone, Copy, Debug)]
enum Map {
  Picture,
  Shadows,
  Heights,
}

/// An object that differs from the drawn scene's at its place.
#[derive(Clone, Copy, Debug)]
struct Differing {
  /// The object's place among the scene's.
  index: usize,
  change: Change,
  /// The box around it in the drawn scene, in the panel's space.
  was: Option<Bounds>,
  /// The box around it in the scene that is being prepared.
  now: Option<Bounds>,
}

/// The way an object differs from the drawn scene's.
///
/// The object is compared with the one at its place among the drawn scene's.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Change {
  /// Another material alone.
  Recolored,
  /// Another shape or place of an object without a shadow.
  ///
  /// It throws none in either scene, so the shadow map and the height map hold nothing of it.
  Repainted,
  /// Another shape or place, or a shadow in one scene alone.
  Reshaped,
}

/// The objects a pass draws.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Objects {
  /// Every object.
  All,
  /// The objects that throw a shadow.
  Shadowing,
}

/// A mesh on the GPU, and the allocation it came from.
#[derive(Debug)]
struct Loaded {
  from: Arc<Mesh>,
  vertices: wgpu::Buffer,
  indices: wgpu::Buffer,
  count: u32,
  /// The box around the mesh in its own space.
  ///
  /// A mesh without a vertex has none.
  bounds: Option<Bounds>,
}

impl Renderer {
  /// Creates a renderer for targets of one format.
  ///
  /// `target` is the format of the textures [`Renderer::render`] draws onto. The device needs
  /// [`wgpu::Features::TEXTURE_COMPRESSION_BC`], which the texture sets are stored in.
  ///
  /// # Panics
  ///
  /// Panics if the device was made without [`wgpu::Features::TEXTURE_COMPRESSION_BC`].
  pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
    assert!(
      device
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
      "the device has bc texture compression"
    );
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
    let refine = uniform("potpot refine", size_of::<[u32; 4]>());
    let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("potpot globals"),
      layout: &layouts.globals,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: globals.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: refine.as_entire_binding(),
        },
      ],
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
    let dfg_table = device.create_texture(&wgpu::TextureDescriptor {
      label: Some("potpot dfg"),
      size: wgpu::Extent3d {
        width: dfg::SIDE,
        height: dfg::SIDE,
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: DFG,
      usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
      view_formats: &[],
    });
    let dfg_texels = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("potpot dfg"),
      contents: bytemuck::cast_slice(&dfg::table()),
      usage: wgpu::BufferUsages::COPY_SRC,
    });
    let dfg_view = dfg_table.create_view(&wgpu::TextureViewDescriptor::default());
    let cube = |label| {
      device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
          width: environment::SIDE,
          height: environment::SIDE,
          depth_or_array_layers: 6,
        },
        mip_level_count: environment::MIPS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: STUDIO,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
      })
    };
    let (studio_map, softbox_map) = (cube("potpot studio"), cube("potpot softbox"));
    let cube_view = |map: &wgpu::Texture| {
      map.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
      })
    };
    let (studio_view, softbox_view) = (cube_view(&studio_map), cube_view(&softbox_map));
    let texture_maps = TEXTURE_FORMATS.map(|(label, format)| {
      device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
          width: textures::SIDE,
          height: textures::SIDE,
          depth_or_array_layers: textures::SETS,
        },
        mip_level_count: textures::MIPS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
      })
    });
    let texture_views = texture_maps.each_ref().map(|map| {
      map.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
      })
    });
    // the sets tile, and are seen at a glancing angle on the sides of things
    let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot textures"),
      address_mode_u: wgpu::AddressMode::Repeat,
      address_mode_v: wgpu::AddressMode::Repeat,
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      mipmap_filter: wgpu::FilterMode::Linear,
      anisotropy_clamp: 8,
      ..Default::default()
    });
    let studio_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("potpot studio"),
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      mipmap_filter: wgpu::FilterMode::Linear,
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
        view(5, &dfg_view),
        view(6, &studio_view),
        sampler(7, &studio_sampler),
        view(8, &softbox_view),
        view(9, &texture_views[0]),
        view(10, &texture_views[1]),
        view(11, &texture_views[2]),
        sampler(12, &texture_sampler),
      ],
    });

    Self {
      pipelines,
      globals,
      refine,
      globals_group,
      shadow_map,
      height_map,
      shadow_group,
      bounce_samples,
      bounce_depth,
      bounce_map,
      maps_group,
      dfg_table,
      dfg_texels: Some(dfg_texels),
      studio_map,
      softbox_map,
      texture_maps,
      unfilled: true,
      present_options: uniform("potpot present", size_of::<Present>()),
      least: SETTLED,
      target_encodes: target.is_srgb(),
      max_side: device.limits().max_texture_dimension_2d,
      picture: None,
      placed: placed_buffer(device, 0),
      placed_room: 0,
      // a new buffer is all zeros
      held_globals: Globals::zeroed(),
      held_present: Present::zeroed(),
      // the first picture is drawn whole, which blurs all of it whatever this holds
      held_spread: Spread {
        taps: [0; 2],
        falloff: [0.0; 2],
      },
      held_placed: Vec::new(),
      next_placed: Vec::new(),
      meshes: Vec::new(),
      draws: Vec::new(),
      spans: Vec::new(),
      spanned: false,
      sound: false,
      differing: Vec::new(),
      painted: Vec::new(),
      wiped: Vec::new(),
      bands: Vec::new(),
      wipe: wipe_buffer(device, 0),
      wipe_room: 0,
      held_size: [0; 2],
      parts: Vec::new(),
      anywhere: true,
      layouts,
    }
  }

  /// Draws the picture of a scene to go over `size` pixels of a target.
  ///
  /// A scene that is in every part the one prepared last has its picture already, and costs no
  /// drawing. One that differs from it in the places and materials of some objects and in some
  /// lamps costs the drawing of the part of the picture those show in.
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
    let fresh = kept.is_none();
    let picture = kept.unwrap_or_else(|| {
      Picture::new(
        device,
        &self.layouts,
        &self.present_options,
        picture_size,
        look.antialias,
      )
    });

    // the radius is in points, and the glow is as wide on the panel at any size
    let radius = look.bloom_radius * Vec2::from(picture_size.map(|side| side as f32)) / scene.size;
    let spread = Spread::new(radius, picture.cells());
    let reblur = spread != self.held_spread;
    self.held_spread = spread;
    // the present pass reads these every time it runs, so new ones need no new picture
    let present = Present::of(look, &scene.light, self.target_encodes, spread);
    let presented_alike = present == self.held_present && size == self.held_size;
    if present != self.held_present {
      queue.write_buffer(&self.present_options, 0, bytemuck::bytes_of(&present));
      self.held_present = present;
    }
    self.held_size = size;
    let globals = Globals::of(scene);
    let before = std::mem::replace(&mut self.held_globals, globals);
    let lit_alike = globals.lit_alike(&before);
    if globals != before {
      queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
    }

    let objects = &scene.objects;
    self.next_placed.clear();
    self.next_placed.extend(objects.iter().map(Placed::of));
    // two scenes of as many objects are compared object by object, and any others in whole.
    // the objects that differ are listed while their bound may still be worth building
    let counted_alike = objects.len() == self.draws.len();
    let (mut same_meshes, mut alike, mut few) = (counted_alike, counted_alike, counted_alike);
    let mut shapes = 0;
    self.differing.clear();
    if counted_alike {
      let held = self.held_placed.iter().zip(&self.draws);
      for (index, ((object, placed), (held, &mesh))) in
        (objects.iter().zip(&self.next_placed).zip(held)).enumerate()
      {
        let loaded = &self.meshes[mesh];
        let same_mesh = Arc::ptr_eq(&object.mesh, &loaded.from);
        same_meshes &= same_mesh;
        if !few {
          continue;
        }
        let moved = !same_mesh || placed.model != held.model;
        if moved || placed != held {
          alike = false;
          let change = match (moved, placed.shadowless(), held.shadowless()) {
            (true, true, true) => Change::Repainted,
            (false, shadowless, was) if shadowless == was => Change::Recolored,
            _ => Change::Reshaped,
          };
          shapes += if change == Change::Reshaped {
            damage::MOVED_SHAPES
          } else {
            1
          };
          few = shapes <= damage::MOST_SHAPES;
          self.differing.push(Differing {
            index,
            change,
            was: None,
            now: None,
          });
        }
      }
    }
    // where each was, while the drawn scene's meshes and places are still held
    for differing in self.differing.iter_mut().filter(|_| few) {
      let loaded = &self.meshes[self.draws[differing.index]];
      differing.was = loaded.bounds_as(&self.held_placed[differing.index]);
    }
    if !same_meshes {
      self.load(device, objects);
    }
    if !alike {
      std::mem::swap(&mut self.held_placed, &mut self.next_placed);
      if self.held_placed.len() > self.placed_room {
        self.placed_room = self.held_placed.len().next_power_of_two();
        self.placed = placed_buffer(device, self.placed_room);
      }
      queue.write_buffer(&self.placed, 0, bytemuck::cast_slice(&self.held_placed));
    }
    for differing in self.differing.iter_mut().filter(|_| few) {
      differing.now =
        self.meshes[self.draws[differing.index]].bounds_as(&self.held_placed[differing.index]);
    }
    let differs = globals != before || !alike;

    // a scene is drawn in part only if what differs can be bounded: in this scene and the one
    // before it, under the same light and view, for objects few enough to be worth it
    let boxes = self.meshes.iter().map(|loaded| loaded.bounds);
    let sound = few && damage::bounded(&globals, &self.held_placed, boxes);
    let bounded = sound && self.sound && lit_alike && few;
    self.sound = sound;
    let changed =
      |change: Change| (self.differing.iter()).filter(move |differing| differing.change == change);
    let reshaped = || changed(Change::Reshaped);

    // where each object is drawn, which tells the objects to draw in a part. a map is stale
    // where an object that is elsewhere was and where it is
    let [wide, high] = picture_size.map(|side| side as f32);
    let onto = [
      damage::to_pixels(&globals.view, Vec2::new(wide, high)),
      damage::to_pixels(&globals.light_view, Vec2::splat(SHADOW_SIZE as f32)),
      damage::to_pixels(&globals.overhead, Vec2::splat(HEIGHT_SIZE as f32)),
    ];
    let spans =
      |index: usize| self.meshes[self.draws[index]].spans(&self.held_placed[index], &onto);
    let gather = |stale: [[f32; 4]; 2], spans: &[Spans]| {
      reshaped().fold(stale, |[shadows, heights], differing| {
        let spans = spans[differing.index];
        [
          around(shadows, spans.shadowed),
          around(heights, spans.overhead),
        ]
      })
    };
    let kept = self.spanned && !fresh;
    let mut stale = None;
    if bounded && differs {
      if kept {
        let before = gather([NOWHERE; 2], &self.spans);
        for differing in reshaped().chain(changed(Change::Repainted)) {
          self.spans[differing.index] = spans(differing.index);
        }
        stale = Some(gather(before, &self.spans));
      } else {
        self.spans.clear();
        self.spans.extend((0..objects.len()).map(spans));
      }
      self.spanned = true;
    } else {
      self.spanned = kept && !differs;
    }
    let shapes = match stale {
      _ if lit_alike && few && reshaped().next().is_none() => [Redrawn::Nothing; 2],
      Some([shadows, heights]) => [
        damage::texels(corners_of(shadows), SHADOW_SIZE),
        damage::texels(corners_of(heights), HEIGHT_SIZE),
      ],
      None => [Redrawn::Whole; 2],
    };

    // the bounce map holds every part of the scene, and the picture all of that as it is seen
    let (mut wiped, mut bands) = (
      std::mem::take(&mut self.wiped),
      std::mem::take(&mut self.bands),
    );
    let mut painted = std::mem::take(&mut self.painted);
    painted.clear();
    let (redrawn, light) = if !differs {
      (Redrawn::Nothing, Redrawn::Nothing)
    } else if bounded && !fresh {
      let mut reach = Reach::new(&before, &globals, picture_size, &mut wiped, &mut bands);
      // where each was and where it is, once if that is the same box
      let moved = reshaped().flat_map(|differing| {
        let was = differing.was.filter(|was| Some(*was) != differing.now);
        was.into_iter().chain(differing.now)
      });
      // what shows otherwise and throws the shadows it threw, or none. boxes over one another
      // are taken as one, so that the pictures of one screen are one box
      let shown = (changed(Change::Repainted).flat_map(|differing| [differing.was, differing.now]))
        .chain(changed(Change::Recolored).map(|differing| differing.now));
      for bounds in shown.flatten() {
        damage::add_box(&mut painted, bounds);
      }
      if reach.takes(&before, &globals, moved.clone(), painted.len()) <= damage::MOST_SHAPES {
        reach.relight(&before, &globals);
        moved.for_each(|bounds| reach.moved(bounds));
        (painted.iter()).for_each(|bounds| reach.recolored(*bounds));
        (reach.redrawn(), reach.relit())
      } else {
        (Redrawn::Whole, Redrawn::Whole)
      }
    } else {
      (Redrawn::Whole, Redrawn::Whole)
    };
    let redrawn = if fresh { Redrawn::Whole } else { redrawn };
    // a scene that stands still is drawn one more sample, until every pixel is settled
    let refining = redrawn == Redrawn::Nothing && self.least < SETTLED;
    self.least = match redrawn {
      Redrawn::Nothing => (self.least + u32::from(refining)).min(SETTLED),
      _ => 1,
    };
    (self.wiped, self.bands, self.painted) = (wiped, bands, painted);
    if let Redrawn::Within(_) = redrawn {
      if self.wiped.len() > self.wipe_room {
        self.wipe_room = self.wiped.len().next_power_of_two();
        self.wipe = wipe_buffer(device, self.wipe_room);
      }
      queue.write_buffer(&self.wipe, 0, bytemuck::cast_slice(&self.wiped));
    }
    self.anywhere = !presented_alike || redrawn == Redrawn::Whole || refining;
    self.parts.clear();
    if let Redrawn::Within(_) = redrawn {
      // the haze shows only with a bloom, and the soften reaches a pixel
      let reach = if look.bloom > 0.0 {
        spread.reach()
      } else {
        Vec2::ONE
      };
      damage::parts(&self.bands, picture_size, size, reach, &mut self.parts);
    }
    if let Some(texels) = self.dfg_texels.take() {
      let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("potpot dfg"),
      });
      encoder.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
          buffer: &texels,
          layout: wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(dfg::SIDE * size_of::<[half::f16; 2]>() as u32),
            rows_per_image: None,
          },
        },
        self.dfg_table.as_image_copy(),
        self.dfg_table.size(),
      );
      queue.submit([encoder.finish()]);
    }
    if self.unfilled {
      fill_cube(queue, &self.studio_map, environment::STUDIO);
      fill_cube(queue, &self.softbox_map, environment::SOFTBOX);
      let baked = [textures::ALBEDO, textures::NORMAL, textures::ROUGHNESS];
      for ((map, texels), (_, format)) in self.texture_maps.iter().zip(baked).zip(TEXTURE_FORMATS) {
        fill_sets(queue, map, format, texels);
      }
      self.unfilled = false;
    }
    let mut picture = picture;
    self.draw_picture(
      device,
      queue,
      &mut picture,
      shapes,
      light,
      redrawn,
      refining,
      reblur,
    );
    self.picture = Some(picture);
  }

  /// Returns whether the picture is settled.
  ///
  /// A picture is refined one more sample at each [`Renderer::prepare`] of the scene it was
  /// last drawn of, which smooths its soft shadows and shading. It is settled when every pixel
  /// has 64 samples, and from then on such a prepare draws nothing. Any change of the scene
  /// starts the part it changes over, and the picture is not settled until that part is.
  pub fn settled(&self) -> bool {
    self.least >= SETTLED
  }

  /// Returns the parts of the target where the picture is new.
  ///
  /// They are where the picture of the last [`Renderer::prepare`] differs from that of the one
  /// before it. Each part is x, y, width and height in pixels of the `size` the last was given,
  /// from its top left. Outside the parts [`Renderer::render`] draws every pixel as it did
  /// after the earlier one, when it is given a place of that size both times. It is `None` when
  /// any pixel may differ, as after the first picture or one of another size.
  pub fn changed(&self) -> Option<&[[u32; 4]]> {
    (!self.anywhere).then_some(&self.parts)
  }

  /// Returns the object seen at a point of the last picture.
  ///
  /// The object is of the scene last prepared. It is given as its place in the scene's
  /// [`Scene::objects`], and it is the one [`Scene::pick`] finds in that scene at `at`. This
  /// looks only at the meshes whose bounds the line of sight passes through. Before any scene
  /// is prepared it is `None`.
  ///
  /// # Panics
  ///
  /// Panics if an index of an object's mesh is past the end of the mesh's vertices.
  pub fn pick(&self, at: Vec2) -> Option<usize> {
    let [width, height, ..] = self.held_globals.panel;
    let objects = (self.held_placed.iter().zip(&self.draws)).map(|(placed, &mesh)| {
      let loaded = &self.meshes[mesh];
      let transform = Mat4::from_cols_array_2d(&placed.model);
      (transform, &*loaded.from, loaded.bounds)
    });
    let view = Mat4::from_cols_array_2d(&self.held_globals.view);
    crate::pick::seen(view, Vec2::new(width, height), at, objects)
  }

  /// Puts every mesh the objects use on the GPU once.
  ///
  /// The meshes that are there already are kept, and those no object uses are dropped.
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

  /// Draws what is stale of the maps and then of the picture, and the picture's mean.
  ///
  /// `shapes` is the parts of the shadow map and of the height map to draw again, and `light`
  /// the part of the bounce map, which is left alone unless light bounces. `redrawn` is the
  /// part of the picture to draw again, where its mean starts over. `refining` draws one more
  /// sample of the whole picture into each pixel's mean instead. `reblur` blurs all of the
  /// glow map again, as a new blur takes.
  #[expect(clippy::too_many_arguments, reason = "each is a part of what is stale")]
  fn draw_picture(
    &self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    picture: &mut Picture,
    shapes: [Redrawn; 2],
    light: Redrawn,
    redrawn: Redrawn,
    refining: bool,
    reblur: bool,
  ) {
    // the scene pass draws all of a sample that refines, and the mean takes it in
    let (drawn, first) = if refining {
      (Redrawn::Whole, 0)
    } else {
      (redrawn, 1)
    };
    let redrawn = drawn;
    let light = if self.held_globals.bounces() {
      light
    } else {
      Redrawn::Nothing
    };
    if [shapes[0], shapes[1], light, redrawn] == [Redrawn::Nothing; 4] && !reblur {
      return;
    }
    queue.write_buffer(&self.refine, 0, bytemuck::bytes_of(&[first, 0, 0, 0]));
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
      label: Some("potpot picture"),
    });
    let [shadows, heights] = shapes;
    let depth_passes = [
      (
        "potpot shadow",
        &self.pipelines.shadow,
        &self.shadow_map,
        shadows,
        Map::Shadows,
      ),
      (
        "potpot heights",
        &self.pipelines.height,
        &self.height_map,
        heights,
        Map::Heights,
      ),
    ];
    for (label, pipeline, map, part, span) in depth_passes {
      if part == Redrawn::Nothing {
        continue;
      }
      // a part is drawn over the map as it is kept, once the part is as far off as can be
      let load = match part {
        Redrawn::Within(_) => wgpu::LoadOp::Load,
        _ => wgpu::LoadOp::Clear(1.0),
      };
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[],
        depth_stencil_attachment: Some(depth_attachment(map, load)),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      let mut within = None;
      if let Redrawn::Within(part @ [x, y, width, height]) = part {
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(&self.pipelines.wipe_depth);
        pass.draw(0..3, 0..1);
        within = Some((part.map(|side| side as f32), span));
      }
      pass.set_pipeline(pipeline);
      pass.set_bind_group(0, &self.globals_group, &[]);
      self.draw_objects(&mut pass, within, Objects::Shadowing);
    }
    if light != Redrawn::Nothing {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("potpot bounce"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &self.bounce_samples,
          depth_slice: None,
          resolve_target: Some(&self.bounce_map),
          // all of it is kept, for a part to be drawn over later
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        // with everything as near as can be, nothing is drawn but where the wipe puts the map
        // back as far off as can be
        depth_stencil_attachment: Some(depth_attachment(
          &self.bounce_depth,
          wgpu::LoadOp::Clear(0.0),
        )),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      let mut within = None;
      if let Redrawn::Within(part @ [x, y, width, height]) = light {
        pass.set_scissor_rect(x, y, width, height);
        // an object is known by its place on the height map, which is as much finer
        let finer = (HEIGHT_SIZE / BOUNCE_SIZE) as f32;
        within = Some((part.map(|side| side as f32 * finer), Map::Heights));
      }
      pass.set_bind_group(0, &self.globals_group, &[]);
      pass.set_pipeline(&self.pipelines.wipe_bounce);
      pass.draw(0..3, 0..1);
      pass.set_pipeline(&self.pipelines.bounce);
      pass.set_bind_group(1, &self.shadow_group, &[]);
      self.draw_objects(&mut pass, within, Objects::All);
    }
    // the cells of the glow map drawn again, as left, top, right and foot
    let mut glowing = None;
    if redrawn != Redrawn::Nothing {
      let whole = redrawn == Redrawn::Whole;
      let smooth = usize::from(picture.smoothed.is_some());
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("potpot scene"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: (picture.smoothed.as_ref()).map_or(&picture.color, |(samples, _)| samples),
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: if whole {
              wgpu::LoadOp::Clear(wgpu::Color::BLACK)
            } else {
              wgpu::LoadOp::Load
            },
            store: wgpu::StoreOp::Store,
          },
        })],
        // a part is drawn over the picture as it is kept. with everything as near as can be,
        // nothing is drawn but where the wipe puts it back as far off as can be
        depth_stencil_attachment: Some(depth_attachment(
          &picture.depth,
          wgpu::LoadOp::Clear(if whole { 1.0 } else { 0.0 }),
        )),
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      let mut within = None;
      if let Redrawn::Within(part @ [x, y, width, height]) = redrawn {
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(&self.pipelines.wipe[smooth]);
        pass.set_vertex_buffer(0, self.wipe.slice(..));
        pass.draw(0..self.wiped.len() as u32, 0..1);
        within = Some((part.map(|side| side as f32), Map::Picture));
      }
      pass.set_pipeline(&self.pipelines.scene[smooth]);
      pass.set_bind_group(0, &self.globals_group, &[]);
      pass.set_bind_group(1, &self.maps_group, &[]);
      pass.set_bind_group(2, &picture.counted[picture.side], &[]);
      self.draw_objects(&mut pass, within, Objects::All);
      drop(pass);

      // the samples resolved into the picture where they were drawn
      if let Some((_, samples)) = &picture.smoothed {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
          label: Some("potpot resolve"),
          color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &picture.color,
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
        if let Redrawn::Within([x, y, width, height]) = redrawn {
          pass.set_scissor_rect(x, y, width, height);
        }
        pass.set_pipeline(&self.pipelines.resolve);
        pass.set_bind_group(0, samples, &[]);
        pass.draw(0..3, 0..1);
      }

      // the next mean: the one standing with the sample added, or with the sample in place of
      // it where the sample starts it over
      let next = 1 - picture.side;
      let (mean, count) = &picture.means[next];
      let target = |view| {
        Some(wgpu::RenderPassColorAttachment {
          view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })
      };
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("potpot accumulate"),
        color_attachments: &[target(mean), target(count)],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
      });
      pass.set_bind_group(0, &picture.accumulate[picture.side], &[]);
      match (refining, redrawn) {
        (true, _) => {
          pass.set_pipeline(&self.pipelines.add);
          pass.draw(0..3, 0..1);
        }
        (false, Redrawn::Within([x, y, width, height])) => {
          pass.set_pipeline(&self.pipelines.keep);
          pass.draw(0..3, 0..1);
          pass.set_scissor_rect(x, y, width, height);
          pass.set_pipeline(&self.pipelines.first[1]);
          pass.set_vertex_buffer(0, self.wipe.slice(..));
          pass.draw(0..self.wiped.len() as u32, 0..1);
        }
        (false, _) => {
          pass.set_pipeline(&self.pipelines.first[0]);
          pass.draw(0..3, 0..1);
        }
      }
      drop(pass);
      picture.side = next;

      // the glow map follows the picture, in the cells that what was drawn again lies in. the
      // dark cells around them are cleared with the whole map and never drawn
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("potpot glow"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &picture.glow,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: if whole {
              wgpu::LoadOp::Clear(wgpu::Color::BLACK)
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
      let [x, y, width, height] = match redrawn {
        Redrawn::Within(part) => part,
        _ => [0, 0, picture.size[0], picture.size[1]],
      };
      let (left, top) = (x / HAZE_CELL, y / HAZE_CELL);
      let (right, foot) = (
        (x + width).div_ceil(HAZE_CELL),
        (y + height).div_ceil(HAZE_CELL),
      );
      pass.set_scissor_rect(left + 1, top + 1, right - left, foot - top);
      pass.set_pipeline(&self.pipelines.glow);
      pass.set_bind_group(0, &picture.lit[picture.side], &[]);
      pass.draw(0..3, 0..1);
      glowing = Some([left + 1, top + 1, right + 1, foot + 1]);
    }

    // the blurred maps follow the glow map as far as the blur carries what differs in it, each
    // pass that far along its axis. a picture drawn whole or a new blur takes all of them
    let cells = picture.cells();
    if redrawn == Redrawn::Whole || reblur {
      glowing = Some([0, 0, cells[0], cells[1]]);
    }
    if let Some([left, top, right, foot]) = glowing {
      let [across, down] = self.held_spread.taps;
      let (left, right) = (left.saturating_sub(across), (right + across).min(cells[0]));
      let parts = [
        [left, top, right, foot],
        [
          left,
          top.saturating_sub(down),
          right,
          (foot + down).min(cells[1]),
        ],
      ];
      let passes = (self.pipelines.spread.iter())
        .zip(&picture.spread)
        .zip(&picture.unspread);
      for (((pipeline, view), unspread), [left, top, right, foot]) in passes.zip(parts) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
          label: Some("potpot spread"),
          color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
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
        pass.set_scissor_rect(left, top, right - left, foot - top);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, unspread, &[]);
        pass.draw(0..3, 0..1);
      }
    }
    queue.submit([encoder.finish()]);
  }

  /// Stretches the picture last drawn over `place` on the target.
  ///
  /// Everything outside `clip` stays as it was. Both are in the target's pixels, `clip` as x, y,
  /// width and height, and `clip` has to lie on the target. Before any scene is prepared there
  /// is no picture and nothing is drawn.
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
    pass.set_bind_group(0, &picture.group[picture.side], &[]);
    pass.set_viewport(place.x, place.y, place.width, place.height, 0.0, 1.0);
    pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
    pass.draw(0..3, 0..1);
  }

  /// Draws the objects of the scene.
  ///
  /// Each is drawn with its place and material. `within` is a rectangle of what is drawn on, as
  /// x, y, width and height in its pixels, and what that is. With it, only the objects drawn in
  /// the rectangle are drawn, and of those the ones `objects` names.
  fn draw_objects(
    &self,
    pass: &mut wgpu::RenderPass<'_>,
    within: Option<([f32; 4], Map)>,
    objects: Objects,
  ) {
    pass.set_vertex_buffer(1, self.placed.slice(..));
    for ((object, &mesh), placed) in (0..).zip(&self.draws).zip(&self.held_placed) {
      let mesh = &self.meshes[mesh];
      // wgpu has no slice of an empty buffer, and such a mesh has nothing to draw
      if mesh.count == 0 || (placed.shadowless() && objects == Objects::Shadowing) {
        continue;
      }
      if within.is_some_and(|([x, y, width, height], on)| {
        let spans = self.spans[object as usize];
        let [left, top, right, foot] = match on {
          Map::Picture => spans.seen,
          Map::Shadows => spans.shadowed,
          Map::Heights => spans.overhead,
        };
        right < x || left > x + width || foot < y || top > y + height
      }) {
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
      bounds: mesh.bounds(),
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

  /// Returns the rectangles the mesh is drawn in as an object.
  ///
  /// `placed` is the object's place. `onto` is the matrix from panel points to the pixels of the
  /// picture, to the texels of the shadow map and to those of the height map.
  fn spans(&self, placed: &Placed, onto: &[Mat4; 3]) -> Spans {
    let model = Mat4::from_cols_array_2d(&placed.model);
    let [seen, shadowed, overhead] = onto.map(|onto| {
      let onto = onto * model;
      let corners = (self.bounds.into_iter()).flat_map(damage::corners);
      corners.fold(NOWHERE, |span, corner| {
        let at = onto.transform_point3(corner);
        around(span, [at.x - 1.0, at.y - 1.0, at.x + 1.0, at.y + 1.0])
      })
    });
    Spans {
      seen,
      shadowed,
      overhead,
    }
  }

  /// Returns the box around the mesh as an object.
  ///
  /// The box is in the panel's space. `placed` is the object's place. A mesh without a vertex
  /// has none.
  fn bounds_as(&self, placed: &Placed) -> Option<Bounds> {
    let model = Mat4::from_cols_array_2d(&placed.model);
    let corners = damage::corners(self.bounds?).map(|corner| model.transform_point3(corner));
    Some(
      corners.fold([Vec3::MAX, Vec3::MIN], |[least, most], corner| {
        [least.min(corner), most.max(corner)]
      }),
    )
  }
}

/// A rectangle that holds nothing.
///
/// It is its left, top, right and foot, as the other rectangles of an object are.
const NOWHERE: [f32; 4] = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];

/// Returns the rectangle around two.
///
/// Each is its left, top, right and foot.
fn around(one: [f32; 4], other: [f32; 4]) -> [f32; 4] {
  [
    one[0].min(other[0]),
    one[1].min(other[1]),
    one[2].max(other[2]),
    one[3].max(other[3]),
  ]
}

/// Returns the least and greatest corner of a rectangle.
///
/// The rectangle is given as its left, top, right and foot.
fn corners_of([left, top, right, foot]: [f32; 4]) -> [Vec2; 2] {
  [Vec2::new(left, top), Vec2::new(right, foot)]
}

/// Creates a square texture that a pass draws into.
///
/// `also` is the texture's other usage.
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

fn depth_attachment(
  view: &wgpu::TextureView,
  load: wgpu::LoadOp<f32>,
) -> wgpu::RenderPassDepthStencilAttachment<'_> {
  wgpu::RenderPassDepthStencilAttachment {
    view,
    depth_ops: Some(wgpu::Operations {
      load,
      store: wgpu::StoreOp::Store,
    }),
    stencil_ops: None,
  }
}

fn wipe_buffer(device: &wgpu::Device, room: usize) -> wgpu::Buffer {
  device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("potpot wipe"),
    size: (room.max(1) * size_of::<[f32; 2]>()) as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
  })
}

fn placed_buffer(device: &wgpu::Device, room: usize) -> wgpu::Buffer {
  device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("potpot placed"),
    size: (room.max(1) * size_of::<Placed>()) as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
  })
}

/// The label and format of the color, normal and roughness maps of the texture sets.
const TEXTURE_FORMATS: [(&str, wgpu::TextureFormat); 3] = [
  ("potpot albedo", wgpu::TextureFormat::Bc7RgbaUnormSrgb),
  ("potpot normal", wgpu::TextureFormat::Bc5RgUnorm),
  ("potpot roughness", wgpu::TextureFormat::Bc4RUnorm),
];

/// Writes baked blocks into an array of the texture sets, mip after mip.
///
/// A mip narrower than a block takes one whole block, as the bake stores it.
fn fill_sets(
  queue: &wgpu::Queue,
  map: &wgpu::Texture,
  format: wgpu::TextureFormat,
  mut blocks: &[u8],
) {
  let block = format
    .block_copy_size(None)
    .expect("a compressed format has a block size");
  for mip in 0..textures::MIPS {
    let across = (textures::SIDE >> mip).div_ceil(4);
    let layer = (across * across * block) as usize;
    let (level, rest) = blocks.split_at(layer * textures::SETS as usize);
    blocks = rest;
    queue.write_texture(
      wgpu::TexelCopyTextureInfo {
        texture: map,
        mip_level: mip,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
      },
      level,
      wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(across * block),
        rows_per_image: Some(across),
      },
      wgpu::Extent3d {
        width: across * 4,
        height: across * 4,
        depth_or_array_layers: textures::SETS,
      },
    );
  }
}

/// Writes baked texels into a cube map of the studio, mip after mip.
fn fill_cube(queue: &wgpu::Queue, map: &wgpu::Texture, mut texels: &[u8]) {
  for mip in 0..environment::MIPS {
    let side = environment::SIDE >> mip;
    let (level, rest) = texels.split_at((side * side * 6 * 4) as usize);
    texels = rest;
    queue.write_texture(
      wgpu::TexelCopyTextureInfo {
        texture: map,
        mip_level: mip,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
      },
      level,
      wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(side * 4),
        rows_per_image: Some(side),
      },
      wgpu::Extent3d {
        width: side,
        height: side,
        depth_or_array_layers: 6,
      },
    );
  }
}

#[cfg(test)]
mod tests;
