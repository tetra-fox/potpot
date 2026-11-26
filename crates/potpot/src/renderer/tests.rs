//! Scenes drawn in part, against the same scenes drawn whole.
//!
//! These draw with a graphics device, so a plain test run leaves them out. `just gpu-test` runs
//! them.

use glam::{Mat4, Vec2, Vec3};

use super::*;
use crate::material::Material;
use crate::scene::{Lamp, Light, Look};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// The target's width and height in pixels.
///
/// Its rows are of a length wgpu copies as they are.
const SIZE: [u32; 2] = [448, 256];

/// A target that scenes are drawn on and read back from.
struct Target {
  device: wgpu::Device,
  queue: wgpu::Queue,
  texture: wgpu::Texture,
  read: wgpu::Buffer,
}

impl Target {
  fn new() -> Self {
    let instance = wgpu::Instance::default();
    let adapter =
      pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a graphics adapter is found");
    let (device, queue) =
      pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("the adapter opens a device");
    let texture = device.create_texture(&wgpu::TextureDescriptor {
      label: None,
      size: wgpu::Extent3d {
        width: SIZE[0],
        height: SIZE[1],
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: FORMAT,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
      view_formats: &[],
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
      label: None,
      size: u64::from(SIZE[0] * SIZE[1] * 4),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    Self {
      device,
      queue,
      texture,
      read,
    }
  }

  /// Returns the target's pixels with a scene drawn on it.
  fn drawn(&self, renderer: &mut Renderer, scene: &Scene) -> Vec<u8> {
    renderer.prepare(&self.device, &self.queue, scene, SIZE);
    let mut encoder =
      (self.device).create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let place = Place {
      x: 0.0,
      y: 0.0,
      width: SIZE[0] as f32,
      height: SIZE[1] as f32,
    };
    let view = (self.texture).create_view(&wgpu::TextureViewDescriptor::default());
    renderer.render(&mut encoder, &view, place, [0, 0, SIZE[0], SIZE[1]]);
    encoder.copy_texture_to_buffer(
      self.texture.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &self.read,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(SIZE[0] * 4),
          rows_per_image: None,
        },
      },
      self.texture.size(),
    );
    self.queue.submit([encoder.finish()]);
    (self.read.slice(..)).map_async(wgpu::MapMode::Read, |mapped| {
      mapped.expect("the pixels map")
    });
    (self.device.poll(wgpu::PollType::wait_indefinitely())).expect("the device finishes");
    let pixels = self.read.slice(..).get_mapped_range().to_vec();
    self.read.unmap();
    pixels
  }

  /// Returns the target's pixels with a scene drawn afresh.
  ///
  /// Nothing is kept from a scene before. `whole` draws another scene first, under another
  /// view, which leaves it nothing of this one's to keep.
  fn drawn_whole(&self, whole: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let other = Scene {
      size: scene.size + 1.0,
      objects: Vec::new(),
      lamps: Vec::new(),
      ..scene.clone()
    };
    whole.prepare(&self.device, &self.queue, &other, SIZE);
    self.drawn(whole, scene)
  }

  /// Asserts that a scene drawn in part is as it is drawn whole.
  ///
  /// `kept` draws it after the ones it drew before.
  fn assert_same(&self, kept: &mut Renderer, whole: &mut Renderer, scene: &Scene, step: &str) {
    let (in_part, in_whole) = (self.drawn(kept, scene), self.drawn_whole(whole, scene));
    let differing = (in_part.as_chunks::<4>().0.iter())
      .zip(in_whole.as_chunks::<4>().0)
      .filter(|(part, whole)| part != whole)
      .count();
    assert_eq!(
      differing, 0,
      "pixels differ from the scene drawn whole, {step}"
    );
  }
}

/// Returns numbers from zero to one, the same ones for a seed.
fn numbers(seed: u32) -> impl FnMut() -> f32 {
  let mut state = seed | 1;
  move || {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    (state >> 8) as f32 / (1 << 24) as f32
  }
}

const PANEL: Vec2 = Vec2::new(448.0, 256.0);

/// Returns a panel with a plate, a few things and two lamps.
fn panel(number: &mut impl FnMut() -> f32, shapes: &[Arc<Mesh>]) -> Scene {
  let mut scene = Scene {
    size: PANEL,
    heights: [-8.0, 48.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look: Look::default(),
    objects: vec![Object {
      mesh: Arc::new(crate::parts::plate(PANEL.x, PANEL.y, 6.0)),
      transform: Mat4::IDENTITY,
      material: Material::textured(Vec3::splat(0.3)),
    }],
    lamps: vec![lamp(number), lamp(number)],
  };
  for index in 0..12 {
    scene.objects.push(Object {
      mesh: Arc::clone(&shapes[index % shapes.len()]),
      transform: place(number),
      material: material(number),
    });
  }
  scene
}

/// Returns a place well inside the panel, turned some way.
fn place(number: &mut impl FnMut() -> f32) -> Mat4 {
  let at =
    Vec2::new(60.0, 50.0) + (PANEL - Vec2::new(120.0, 100.0)) * Vec2::new(number(), number());
  Mat4::from_translation(at.extend(0.0)) * Mat4::from_rotation_z(number() * 6.0)
}

fn material(number: &mut impl FnMut() -> f32) -> Material {
  let color = Vec3::new(number(), number(), number());
  let made = match (number() * 5.0) as u32 {
    0 => Material::matte(color),
    1 => Material::glossy(color),
    2 => Material::chrome(color),
    3 => Material::spun(color),
    _ => Material::textured(color),
  };
  if number() < 0.3 {
    made.lit(color, number(), 8.0)
  } else {
    made
  }
}

fn lamp(number: &mut impl FnMut() -> f32) -> Lamp {
  Lamp {
    at: (PANEL * Vec2::new(number(), number())).extend(4.0 + number() * 20.0),
    color: Vec3::new(number(), number(), number()) * 2.0,
    reach: 20.0 + number() * 70.0,
    radius: 1.0 + number() * 8.0,
  }
}

#[test]
#[ignore = "needs a graphics device"]
fn a_scene_drawn_in_part_is_the_scene_drawn_whole() {
  let target = Target::new();
  let shapes = [
    Mesh::cuboid(Vec3::new(-14.0, -8.0, 0.0), Vec3::new(14.0, 8.0, 9.0)),
    Mesh::lathe(&[[11.0, 0.0], [9.0, 22.0], [0.0, 24.0]], 24, true),
    Mesh::cuboid(Vec3::new(-3.0, -3.0, 0.0), Vec3::new(3.0, 3.0, 40.0)),
    crate::parts::Knob::fluted(16.0, 14.0).body(),
  ]
  .map(Arc::new);
  let mut whole = Renderer::new(&target.device, FORMAT);
  let mut in_part = 0;
  // under a view that leans one way, both ways and not at all, with and without bounced light
  let rounds = [
    (Vec2::new(0.0, -0.25), 0.0, Look::default()),
    (Vec2::new(0.0, -0.25), 3.0, Look::default()),
    (Vec2::new(0.2, 0.3), 0.0, Look::default()),
    (Vec2::ZERO, 2.0, Look::default()),
    (
      Vec2::new(-0.1, -0.3),
      1.0,
      Look {
        antialias: false,
        pixel_size: 2.0,
        soft_shadows: false,
        ..Look::default()
      },
    ),
  ];
  for (round, (lean, bounce, look)) in rounds.into_iter().enumerate() {
    let mut number = numbers(0x2545_f491 + round as u32);
    let mut scene = panel(&mut number, &shapes);
    (scene.lean, scene.light.bounce, scene.look) = (lean, bounce, look);
    scene.light.softness = 1.0 + number() * 4.0;
    let mut kept = Renderer::new(&target.device, FORMAT);
    for frame in 0..48 {
      let object = 1 + (number() * 12.0) as usize;
      let kind = (number() * 12.0) as u32;
      match kind {
        0..=2 => scene.objects[object].transform = place(&mut number),
        // a little way, as a drag takes something
        3..=4 => scene.objects[object].transform *= Mat4::from_rotation_z(number() * 0.2),
        5..=6 => scene.objects[object].material = material(&mut number),
        7 => scene.objects[object].mesh = Arc::clone(&shapes[frame % shapes.len()]),
        // a mesh made again, as a picture on a screen is
        8 => {
          let wide = 4.0 + number() * 20.0;
          let flat = Mesh::cuboid(Vec3::new(-wide, -3.0, 0.0), Vec3::new(wide, 3.0, 0.5));
          scene.objects[object].mesh = Arc::new(flat);
        }
        9..=10 => {
          let slot = frame % scene.lamps.len();
          scene.lamps[slot] = lamp(&mut number);
        }
        // two things at once: something moved and a lamp gone or come back
        _ => {
          scene.objects[object].transform = place(&mut number);
          if scene.lamps.len() == 2 {
            scene.lamps.pop();
          } else {
            scene.lamps.push(lamp(&mut number));
          }
        }
      }
      let step = format!("at frame {frame} of round {round} after a change of kind {kind}");
      target.assert_same(&mut kept, &mut whole, &scene, &step);
      in_part += usize::from(kept.changed().is_some());
      // and against a renderer that has drawn nothing before
      if frame % 16 == 15 {
        let first = target.drawn(&mut Renderer::new(&target.device, FORMAT), &scene);
        assert!(first == target.drawn(&mut kept, &scene), "{step}");
      }
    }
  }
  // a post that goes around a lamp, and the shadow it throws out from the lamp around with it
  let mut number = numbers(3);
  let mut scene = panel(&mut number, &shapes);
  scene.objects.truncate(2);
  scene.objects[1].mesh = Arc::clone(&shapes[2]);
  scene.lamps = vec![Lamp {
    at: (PANEL / 2.0).extend(6.0),
    color: Vec3::splat(3.0),
    reach: 120.0,
    radius: 2.0,
  }];
  let mut kept = Renderer::new(&target.device, FORMAT);
  for step in 0..16 {
    let around = PANEL / 2.0 + Vec2::from_angle(step as f32 * 0.7) * 26.0;
    scene.objects[1].transform = Mat4::from_translation(around.extend(0.0));
    let step = format!("with the post at step {step} of its way around the lamp");
    target.assert_same(&mut kept, &mut whole, &scene, &step);
    in_part += usize::from(kept.changed().is_some());
  }
  // most of those frames have to have been drawn in part for this to have tested anything
  assert!(in_part > 120, "{in_part} frames were drawn in part");
}

/// A name for what no bound takes, and what breaks a scene so.
type Odd = (&'static str, fn(&mut Scene));

#[test]
#[ignore = "needs a graphics device"]
fn a_scene_that_no_bound_holds_for_is_drawn_whole() {
  let target = Target::new();
  let shapes = [Mesh::cuboid(
    Vec3::new(-14.0, -8.0, 0.0),
    Vec3::new(14.0, 8.0, 9.0),
  )]
  .map(Arc::new);
  let mut number = numbers(7);
  let plain = panel(&mut number, &shapes);
  let [mut kept, mut whole] = [(); 2].map(|()| Renderer::new(&target.device, FORMAT));
  // each of these is drawn in one way only, which a part of the picture drawn again would miss
  let odd: [Odd; 6] = [
    ("with a lamp that reaches less than nowhere", |scene| {
      scene.lamps[0].reach = -50.0
    }),
    ("with a lamp smaller than nothing", |scene| {
      scene.lamps[1].radius = -30.0
    }),
    ("with an object that is not flat", |scene| {
      scene.objects[3].transform.x_axis.w = 0.002
    }),
    ("with an object nowhere", |scene| {
      scene.objects[4].transform.w_axis.x = f32::NAN
    }),
    ("with heights the wrong way up", |scene| {
      scene.heights = [48.0, -8.0]
    }),
    ("under a light level with the panel", |scene| {
      scene.light.toward = Vec3::new(1.0, 0.2, 0.0)
    }),
  ];
  for (name, breaks) in odd {
    let mut scene = plain.clone();
    target.assert_same(&mut kept, &mut whole, &scene, "before anything is odd");
    breaks(&mut scene);
    target.assert_same(&mut kept, &mut whole, &scene, name);
    // and something else moved while it is odd, and once it is plain again
    scene.objects[1].transform = place(&mut number);
    target.assert_same(&mut kept, &mut whole, &scene, name);
    let mut scene = plain.clone();
    scene.objects[2].material = material(&mut number);
    target.assert_same(&mut kept, &mut whole, &scene, name);
  }
}
