//! Scenes drawn in part, against the same scenes drawn whole.
//!
//! Most of these draw with a graphics device, so a plain test run leaves them out. `just
//! gpu-test` runs them.

use std::time::Duration;

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
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
      required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
      ..Default::default()
    }))
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
    self.drawn_at(renderer, scene, SIZE)
  }

  /// Returns the target's pixels with a scene on its top left.
  ///
  /// The scene is drawn `size` pixels wide and high, which the target has room for.
  fn drawn_at(&self, renderer: &mut Renderer, scene: &Scene, size: [u32; 2]) -> Vec<u8> {
    renderer.prepare(&self.device, &self.queue, scene, size);
    let mut encoder =
      (self.device).create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let place = Place {
      x: 0.0,
      y: 0.0,
      width: size[0] as f32,
      height: size[1] as f32,
    };
    let view = (self.texture).create_view(&wgpu::TextureViewDescriptor::default());
    renderer.render(&mut encoder, &view, place, [0, 0, size[0], size[1]]);
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
    finish(&self.device, "the pixels drawn on the target");
    let pixels = self.read.slice(..).get_mapped_range().to_vec();
    self.read.unmap();
    pixels
  }

  /// Draws a scene again until the renderer has settled on it.
  fn settle(&self, renderer: &mut Renderer, scene: &Scene) {
    self.drawn(renderer, scene);
    while !renderer.settled() {
      renderer.prepare(&self.device, &self.queue, scene, SIZE);
    }
  }

  /// Returns the picture a renderer drew last, as its mean stands.
  ///
  /// It is the light and the emission of each pixel, in rows.
  fn picture(&self, renderer: &Renderer) -> Vec<f32> {
    let picture = renderer.picture.as_ref().expect("a picture was drawn");
    let texture = picture.means[picture.side].0.texture();
    let size = texture.size();
    // a texel is four half floats
    let row = size.width * 8;
    let read = self.device.create_buffer(&wgpu::BufferDescriptor {
      label: None,
      size: u64::from(row * size.height),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    let mut encoder =
      (self.device).create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
      texture.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &read,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(row),
          rows_per_image: None,
        },
      },
      size,
    );
    self.queue.submit([encoder.finish()]);
    (read.slice(..)).map_async(wgpu::MapMode::Read, |mapped| {
      mapped.expect("the picture maps")
    });
    finish(&self.device, "the picture's mean read back");
    let texels = read.slice(..).get_mapped_range();
    (texels.as_chunks::<2>().0.iter())
      .map(|bits| f32::from(half::f16::from_le_bytes(*bits)))
      .collect()
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
  /// `kept` draws it after the ones it drew before, and `shown` holds the pixels it drew last,
  /// which this scene's take the place of. Outside the parts `kept` names as new, the pixels
  /// have to be those.
  fn assert_same(
    &self,
    kept: &mut Renderer,
    whole: &mut Renderer,
    shown: &mut Vec<u8>,
    scene: &Scene,
    step: &str,
  ) {
    let in_part = self.drawn(kept, scene);
    self.drawn_whole(whole, scene);
    assert_alike(&self.picture(kept), &self.picture(whole), step);
    if let Some(parts) = kept.changed() {
      let named = |across: u32, down: u32| {
        parts.iter().any(|&[left, top, width, height]| {
          (left..left + width).contains(&across) && (top..top + height).contains(&down)
        })
      };
      let pixels = (0..SIZE[1]).flat_map(|down| (0..SIZE[0]).map(move |across| (across, down)));
      let unnamed = (pixels.zip(in_part.as_chunks::<4>().0))
        .zip(shown.as_chunks::<4>().0)
        .filter(|(((across, down), now), before)| now != before && !named(*across, *down))
        .count();
      assert_eq!(
        (shown.len(), unnamed),
        (in_part.len(), 0),
        "pixels differ from the frame before outside the parts named, {step}"
      );
    }
    *shown = in_part;
  }
}

/// The longest a test waits on the device, which a frame of these scenes takes a small part of.
const LONGEST_WAIT: Duration = Duration::from_secs(30);

/// Waits for the device to finish what it was given, `what` naming the wait.
///
/// A wait that runs out fails the test with its name. This is to tell where a run that stalls
/// stalls: one run of these tests once hung in a wait on the device, and it has not happened
/// again or been explained.
fn finish(device: &wgpu::Device, what: &str) {
  let waited = device.poll(wgpu::PollType::Wait {
    submission_index: None,
    timeout: Some(LONGEST_WAIT),
  });
  if let Err(error) = waited {
    panic!("the device did not finish {what} within {LONGEST_WAIT:?}: {error}");
  }
}

/// Asserts that two pictures hold the same light.
///
/// Each value has to be within a thousandth of itself.
fn assert_alike(one: &[f32], other: &[f32], step: &str) {
  assert_eq!(one.len(), other.len(), "{step}");
  let differing = (one.iter().zip(other))
    .filter(|&(&one, &other)| (one - other).abs() > 1e-3 * one.abs().max(other.abs()))
    .count();
  assert_eq!(
    differing, 0,
    "values differ from the scene drawn whole, {step}"
  );
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
      material: Material::powder_coat(Vec3::splat(0.3)),
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
    _ => Material::powder_coat(color),
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
fn a_part_started_over_refines_back_to_the_scene_settled_whole() {
  let target = Target::new();
  let shapes = [
    Arc::new(Mesh::cuboid(
      Vec3::new(-14.0, -8.0, 0.0),
      Vec3::new(14.0, 8.0, 9.0),
    )),
    Arc::new(Mesh::lathe(
      &[[11.0, 0.0], [9.0, 22.0], [0.0, 24.0]],
      24,
      true,
    )),
  ];
  let mut number = numbers(11);
  let mut scene = panel(&mut number, &shapes);
  let mut kept = Renderer::new(&target.device, FORMAT);
  target.settle(&mut kept, &scene);
  // one thing moved a little, as a drag moves it, which starts its part of the picture over
  scene.objects[3].transform *= Mat4::from_translation(Vec3::new(6.0, -4.0, 0.0));
  target.drawn(&mut kept, &scene);
  assert!(
    kept.changed().is_some(),
    "the change is drawn in part, which this test is about"
  );
  // refining stays in the part started over: outside it every value is kept as it was, and
  // the target is said to change inside it alone
  let started = target.picture(&kept);
  let [x, y, width, height] = kept.unsettled;
  assert!(
    width * height < SIZE[0] * SIZE[1] / 2,
    "the part is {width} by {height}"
  );
  kept.prepare(&target.device, &target.queue, &scene, SIZE);
  assert_eq!(kept.changed(), Some(&kept.parts[..]));
  assert_eq!(kept.parts.len(), 1);
  while !kept.settled() {
    kept.prepare(&target.device, &target.queue, &scene, SIZE);
  }
  let refined = target.picture(&kept);
  let outside = |index: usize| {
    let (across, down) = ((index / 4) as u32 % SIZE[0], (index / 4) as u32 / SIZE[0]);
    !((x..x + width).contains(&across) && (y..y + height).contains(&down))
  };
  let moved = (0..started.len())
    .filter(|&index| outside(index) && started[index].to_bits() != refined[index].to_bits())
    .count();
  assert_eq!(
    moved, 0,
    "values outside the part moved while it was refined"
  );
  assert!(
    (0..started.len()).any(|index| !outside(index) && started[index] != refined[index]),
    "the part is refined"
  );
  target.settle(&mut kept, &scene);
  let mut whole = Renderer::new(&target.device, FORMAT);
  target.settle(&mut whole, &scene);
  assert_alike(
    &target.picture(&kept),
    &target.picture(&whole),
    "with the part started over and refined again",
  );
}

#[test]
#[ignore = "needs a graphics device"]
fn a_picture_settles_at_its_sixty_fourth_sample_and_any_change_unsettles_it() {
  let target = Target::new();
  let shapes = [Arc::new(Mesh::cuboid(
    Vec3::new(-14.0, -8.0, 0.0),
    Vec3::new(14.0, 8.0, 9.0),
  ))];
  let mut number = numbers(5);
  let mut scene = panel(&mut number, &shapes);
  let mut renderer = Renderer::new(&target.device, FORMAT);
  for sample in 1..=SETTLED {
    target.drawn(&mut renderer, &scene);
    assert_eq!(
      renderer.settled(),
      sample == SETTLED,
      "after sample {sample}"
    );
  }
  // a settled picture draws nothing more
  target.drawn(&mut renderer, &scene);
  assert!(renderer.settled() && renderer.changed() == Some(&[][..]));
  for change in 0..2 {
    match change {
      0 => scene.objects[2].material.color.x += 0.1,
      _ => scene.lamps[0].color *= 0.5,
    }
    target.drawn(&mut renderer, &scene);
    assert!(!renderer.settled(), "after change {change}");
    target.settle(&mut renderer, &scene);
  }
}

#[test]
#[ignore = "needs a graphics device"]
fn a_scene_that_holds_a_nan_settles() {
  let target = Target::new();
  let shapes = [Arc::new(Mesh::cuboid(
    Vec3::new(-14.0, -8.0, 0.0),
    Vec3::new(14.0, 8.0, 9.0),
  ))];
  let mut number = numbers(9);
  let mut scene = panel(&mut number, &shapes);
  scene.objects[1].material.roughness = f32::NAN;
  scene.lamps[0].at.z = f32::NAN;
  let mut renderer = Renderer::new(&target.device, FORMAT);
  for _ in 0..SETTLED {
    target.drawn(&mut renderer, &scene);
  }
  assert!(renderer.settled());
}

#[test]
fn the_baked_studio_holds_every_mip() {
  let texels: u32 = (0..environment::MIPS)
    .map(|mip| (environment::SIDE >> mip).pow(2) * 6)
    .sum();
  assert_eq!(environment::STUDIO.len(), texels as usize * 4);
  assert_eq!(environment::SOFTBOX.len(), texels as usize * 4);
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
  // under a view that leans one way, both ways and not at all, with and without bounced light,
  // and with a glow that reaches past a band
  let rounds = [
    (Vec2::new(0.0, -0.25), 0.0, Look::default()),
    (Vec2::new(0.0, -0.25), 3.0, Look::default()),
    (Vec2::new(0.2, 0.3), 0.0, Look::default()),
    (Vec2::ZERO, 2.0, Look::default()),
    (
      Vec2::new(0.1, -0.2),
      0.0,
      Look {
        bloom: 2.0,
        bloom_radius: 40.0,
        ..Look::default()
      },
    ),
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
    let (mut kept, mut shown) = (Renderer::new(&target.device, FORMAT), Vec::new());
    for frame in 0..48 {
      let object = 1 + (number() * 12.0) as usize;
      let kind = (number() * 14.0) as u32;
      match kind {
        0..=2 => scene.objects[object].transform = place(&mut number),
        // a little way, as a drag takes something
        3..=4 => scene.objects[object].transform *= Mat4::from_rotation_z(number() * 0.2),
        5..=6 => scene.objects[object].material = material(&mut number),
        // another shape, since a scene that comes in unchanged is refined one more sample
        7 => {
          let held = &scene.objects[object].mesh;
          let other = (frame..).map(|turn| &shapes[turn % shapes.len()]);
          let other = other
            .take(shapes.len())
            .find(|shape| !Arc::ptr_eq(shape, held));
          scene.objects[object].mesh = Arc::clone(other.expect("there is more than one shape"));
        }
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
        // a thing without a shadow made again, as a picture on a screen is, somewhere else
        11 => {
          let wide = 4.0 + number() * 20.0;
          let flat = Mesh::cuboid(Vec3::new(-wide, -3.0, 0.0), Vec3::new(wide, 3.0, 0.5));
          scene.objects[object].mesh = Arc::new(flat);
          scene.objects[object].transform = place(&mut number);
          scene.objects[object].material.shadowless = true;
        }
        // something without a shadow throws one, or the other way around
        12 => scene.objects[object].material.shadowless ^= true,
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
      target.assert_same(&mut kept, &mut whole, &mut shown, &scene, &step);
      in_part += usize::from(kept.changed().is_some());
      // and refined until settled, against a renderer that has drawn nothing before refined
      // as far. the next frame starts again from a renderer that has drawn nothing, since the
      // whole one compared with draws its first sample alone
      if frame % 16 == 15 {
        let mut first = Renderer::new(&target.device, FORMAT);
        target.settle(&mut first, &scene);
        target.settle(&mut kept, &scene);
        assert_alike(&target.picture(&first), &target.picture(&kept), &step);
        (kept, shown) = (Renderer::new(&target.device, FORMAT), Vec::new());
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
  let (mut kept, mut shown) = (Renderer::new(&target.device, FORMAT), Vec::new());
  for step in 0..16 {
    let around = PANEL / 2.0 + Vec2::from_angle(step as f32 * 0.7) * 26.0;
    scene.objects[1].transform = Mat4::from_translation(around.extend(0.0));
    let step = format!("with the post at step {step} of its way around the lamp");
    target.assert_same(&mut kept, &mut whole, &mut shown, &scene, &step);
    in_part += usize::from(kept.changed().is_some());
  }
  // most of those frames have to have been drawn in part for this to have tested anything
  assert!(in_part > 120, "{in_part} frames were drawn in part");
  // a tall post under a low light with wide shadow edges, whose long shadow is looked up far
  // to each side of where it falls
  let post = Arc::new(Mesh::cuboid(
    Vec3::new(-4.0, -4.0, 0.0),
    Vec3::new(4.0, 4.0, 36.0),
  ));
  let mut low = 0;
  for (round, (height, softness)) in [(0.25, 8.0), (0.3, 25.0), (0.27, 16.0)]
    .into_iter()
    .enumerate()
  {
    let mut number = numbers(11 + round as u32);
    let mut scene = panel(&mut number, &shapes);
    scene.objects.truncate(2);
    scene.objects[1].mesh = Arc::clone(&post);
    scene.lamps.clear();
    scene.light.toward.z = height;
    scene.light.softness = softness;
    let (mut kept, mut shown) = (Renderer::new(&target.device, FORMAT), Vec::new());
    for step in 0..12 {
      scene.objects[1].transform = place(&mut number);
      let step = format!("with the post at step {step} under low light {round}");
      target.assert_same(&mut kept, &mut whole, &mut shown, &scene, &step);
      low += usize::from(kept.changed().is_some());
    }
  }
  assert!(
    low > 24,
    "{low} frames under a low light were drawn in part"
  );
}

/// Returns the rectangle around where two pictures differ.
///
/// It is its left, top, right and foot, the last two one past it.
fn differing(one: &[u8], other: &[u8]) -> [i32; 4] {
  let pixels = (0..SIZE[1]).flat_map(|down| (0..SIZE[0]).map(move |across| (across, down)));
  (pixels.zip(one.as_chunks::<4>().0))
    .zip(other.as_chunks::<4>().0)
    .filter(|((_, one), other)| one != other)
    .fold([i32::MAX, i32::MAX, 0, 0], |around, (((x, y), _), _)| {
      let (x, y) = (x as i32, y as i32);
      [
        around[0].min(x),
        around[1].min(y),
        around[2].max(x + 1),
        around[3].max(y + 1),
      ]
    })
}

#[test]
#[ignore = "needs a graphics device"]
fn the_haze_shows_as_far_from_what_lights_itself_as_its_reach_and_no_further() {
  use super::damage::{HAZE_CELL, Spread};
  use super::picture::cells;

  let target = Target::new();
  let block = Arc::new(Mesh::cuboid(
    Vec3::new(-11.0, -7.0, 0.0),
    Vec3::new(11.0, 7.0, 4.0),
  ));
  // a block, dark and then lighting itself, without a haze and then with one far brighter than
  // any look has it, which shows the least of it. rough edges keep the block to whole pixels
  let scene = |level: f32, bloom: f32, bloom_radius: f32| {
    let mut scene = panel(&mut numbers(1), &[Arc::clone(&block)]);
    scene.objects.truncate(2);
    scene.objects[1].transform = Mat4::from_translation((PANEL / 2.0).extend(0.0));
    scene.objects[1].material = Material::matte(Vec3::splat(0.5)).lit(Vec3::ONE, level, 0.0);
    scene.lamps.clear();
    scene.look = Look {
      antialias: false,
      bloom,
      bloom_radius,
      ..Look::default()
    };
    scene
  };
  let mut renderer = Renderer::new(&target.device, FORMAT);
  // the panel is as many points as the target is pixels, so a radius in points is one in pixels
  assert_eq!(PANEL, Vec2::new(SIZE[0] as f32, SIZE[1] as f32));
  // the radius of the look by default, one past a band, and one in no whole count of cells
  for radius in [Look::default().bloom_radius, 40.0, 9.0] {
    let mut drawn = |level, bloom| target.drawn(&mut renderer, &scene(level, bloom, radius));
    let lit = differing(&drawn(0.0, 0.0), &drawn(1.0, 0.0));
    let hazed = differing(&drawn(0.0, 1e4), &drawn(1.0, 1e4));
    // a cell holds the pixels of what lights itself together, wherever in the cell they are
    let reach = Spread::new(Vec2::splat(radius), cells(SIZE)).reach();
    let (reach, cell) = (reach.x as i32, HAZE_CELL as i32);
    let further = [
      lit[0] - hazed[0],
      lit[1] - hazed[1],
      hazed[2] - lit[2],
      hazed[3] - lit[3],
    ];
    assert!(
      further
        .iter()
        .all(|side| (reach - cell..=reach).contains(side)),
      "the haze of radius {radius} shows {further:?} pixels past what lights itself, and \
       reaches {reach}"
    );
  }
}

#[test]
#[ignore = "needs a graphics device"]
fn what_lights_itself_on_an_edge_spills_as_it_does_with_the_dark_beside_it() {
  let target = Target::new();
  let mut renderer = Renderer::new(&target.device, FORMAT);
  // the haze alone, of a picture with one bar that lights itself: the picture with a haze less
  // the picture without. it is each pixel's red, green and blue together
  let mut haze = |size: [u32; 2], [left, top, right, foot]: [u32; 4]| {
    let [wide, high] = size.map(|side| side as f32);
    let [left, top, right, foot] = [left, top, right, foot].map(|side| side as f32);
    let bar = Mesh::cuboid(Vec3::new(left, top, 0.0), Vec3::new(right, foot, 1.0));
    let mut scene = Scene {
      size: Vec2::new(wide, high),
      heights: [-8.0, 48.0],
      lean: Vec2::ZERO,
      // no light falls on the panel and it is black, so that the bar and its haze are nearly
      // all that shows
      light: Light {
        color: Vec3::ZERO,
        environment: 0.0,
        ..Light::default()
      },
      // the ordered dither of the color steps repeats every four pixels, so it is the same at
      // the places compared, which a dither that differs at every pixel is not
      look: Look {
        antialias: false,
        vignette: 0.0,
        color_steps: 256,
        ..Look::default()
      },
      objects: vec![
        Object {
          mesh: Arc::new(crate::parts::plate(wide, high, 6.0)),
          transform: Mat4::IDENTITY,
          material: Material::matte(Vec3::ZERO),
        },
        Object {
          mesh: Arc::new(bar),
          transform: Mat4::IDENTITY,
          material: Material::matte(Vec3::splat(0.5)).lit(Vec3::ONE, 1.0, 0.0),
        },
      ],
      lamps: Vec::new(),
    };
    let mut drawn = |bloom: f32| {
      scene.look.bloom = bloom;
      target.drawn_at(&mut renderer, &scene, size)
    };
    let (plain, hazed) = (drawn(0.0), drawn(4.0));
    let sum = |pixel: &[u8; 4]| pixel[..3].iter().map(|&channel| i32::from(channel)).sum();
    let sums = hazed.as_chunks::<4>().0.iter().map(sum);
    let alone: Vec<i32> = (sums.zip(plain.as_chunks::<4>().0.iter().map(sum)))
      .map(|(hazed, plain): (i32, i32)| hazed - plain)
      .collect();
    move |across: u32, down: u32| alone[(down * SIZE[0] + across) as usize]
  };
  // a bar one pixel wide on an edge, and the same bar with `MORE` pixels of the picture past
  // it. the haze on the picture's side of the bar is to be the same, which is looked at as
  // far from the bar as `MORE`
  const MORE: u32 = 16;
  let mut assert_alike = |name: &str, edge: ([u32; 2], [u32; 4]), inner: ([u32; 2], [u32; 4])| {
    let (on_edge, inside) = (haze(edge.0, edge.1), haze(inner.0, inner.1));
    let [left, top, right, foot] = edge.1;
    let shift = [inner.1[0] - left, inner.1[1] - top];
    let around =
      |least: u32, most: u32, side: u32| least.saturating_sub(MORE)..(most + MORE).min(side);
    let mut most = 0;
    for across in around(left, right, edge.0[0]) {
      for down in around(top, foot, edge.0[1]) {
        let (here, there) = (
          on_edge(across, down),
          inside(across + shift[0], down + shift[1]),
        );
        assert!(
          (here - there).abs() <= 3,
          "the haze at {across} {down} is {here} with the bar on the {name} edge of {:?} and \
           {there} with it inside",
          edge.0
        );
        most = most.max(here);
      }
    }
    assert!(most > 60, "the haze of the bar is {most} at most");
  };
  // at each count of pixels over a whole count of cells, for the edges the cells end at
  for over in 0..4 {
    let [wide, high] = [200 + over, 100 + over];
    assert_alike(
      "right",
      ([wide, 100], [wide - 1, 30, wide, 70]),
      ([wide + MORE, 100], [wide - 1, 30, wide, 70]),
    );
    assert_alike(
      "bottom",
      ([200, high], [80, high - 1, 120, high]),
      ([200, high + MORE], [80, high - 1, 120, high]),
    );
  }
  assert_alike(
    "left",
    ([200, 100], [0, 30, 1, 70]),
    ([200 + MORE, 100], [MORE, 30, MORE + 1, 70]),
  );
  assert_alike(
    "top",
    ([200, 100], [80, 0, 120, 1]),
    ([200, 100 + MORE], [80, MORE, 120, MORE + 1]),
  );
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
  let mut shown = Vec::new();
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
    target.assert_same(
      &mut kept,
      &mut whole,
      &mut shown,
      &scene,
      "before anything is odd",
    );
    breaks(&mut scene);
    target.assert_same(&mut kept, &mut whole, &mut shown, &scene, name);
    // and something else moved while it is odd, and once it is plain again
    scene.objects[1].transform = place(&mut number);
    target.assert_same(&mut kept, &mut whole, &mut shown, &scene, name);
    let mut scene = plain.clone();
    scene.objects[2].material = material(&mut number);
    target.assert_same(&mut kept, &mut whole, &mut shown, &scene, name);
  }
}

/// Returns the share of the target named as new.
///
/// It is the pixels of the parts together over those of the target.
fn share(parts: &[[u32; 4]]) -> f32 {
  let named: u32 = parts.iter().map(|part| part[2] * part[3]).sum();
  named as f32 / (SIZE[0] * SIZE[1]) as f32
}

#[test]
#[ignore = "needs a graphics device"]
fn a_screen_whose_picture_is_made_again_is_drawn_again_alone() {
  use potpot_controls::flat::Flat;

  let target = Target::new();
  let screen = crate::parts::Screen::new(120.0, 70.0);
  let middle = Vec2::new(330.0, 120.0);
  let face = Material::display_glass(Vec3::splat(0.02), 1.5);
  let knob = crate::parts::Knob::fluted(16.0, 14.0).body();
  // a curve over a filled region, as often made again as it is shown
  let picture = |level: f32| {
    let mut flat = Flat::default();
    let points = [
      [280.0, 130.0],
      [330.0, 100.0 + level * 30.0],
      [380.0, 125.0],
    ];
    flat.area(&points, 150.0, [1.0, 0.6, 0.1, 0.5]);
    flat.curve(&points, 2.0, [0.2, 0.9, 1.0, 1.0]);
    crate::parts::picture(&flat, middle.x, middle.y, face, 0)
  };
  for bounce in [0.0, 3.0] {
    let mut scene = panel(&mut numbers(5), &[Arc::new(knob.clone())]);
    scene.objects.truncate(1);
    scene.light.bounce = bounce;
    scene.lamps = (screen.lamps(middle.x, middle.y, Vec3::new(0.2, 0.9, 1.0), 1.0)).collect();
    let on_panel = Mat4::from_translation(middle.extend(0.0));
    for (mesh, material) in [
      (screen.face(), face),
      (screen.bezel(), Material::glossy(Vec3::splat(0.04))),
    ] {
      scene.objects.push(Object {
        mesh: Arc::new(mesh),
        transform: on_panel,
        material,
      });
    }
    // a knob in the screen's rows, well to the left of it
    let turned =
      |turn: f32| Mat4::from_translation(Vec3::new(60.0, 120.0, 0.0)) * Mat4::from_rotation_z(turn);
    scene.objects.push(Object {
      mesh: Arc::new(knob.clone()),
      transform: turned(0.0),
      material: Material::spun(Vec3::splat(0.7)),
    });
    let fixed = scene.objects.len();
    scene.objects.extend(picture(0.0));
    let [mut kept, mut whole] = [(); 2].map(|()| Renderer::new(&target.device, FORMAT));
    let mut shown = Vec::new();
    target.assert_same(&mut kept, &mut whole, &mut shown, &scene, "at first");
    for frame in 1..12 {
      let level = (frame as f32 * 0.9).sin();
      scene.objects.truncate(fixed);
      scene.objects.extend(picture(level));
      // on every third frame the knob is turned too
      if frame % 3 == 0 {
        scene.objects[fixed - 1].transform = turned(frame as f32 * 0.3);
      }
      let step = format!("with picture {frame} and light bouncing {bounce}");
      target.assert_same(&mut kept, &mut whole, &mut shown, &scene, &step);
      // the frame after the first tells whether a bound holds, and is drawn whole
      if frame == 1 {
        continue;
      }
      let parts = kept.changed().expect("the picture is drawn in part");
      if frame % 3 == 0 {
        // nothing of the panel between the knob's shadow and the screen. with light bouncing,
        // what the shadow falls on throws its light that far
        let between = |&[left, _, width, _]: &[u32; 4]| left < 210 && left + width > 210;
        assert!(
          bounce > 0.0 || !parts.iter().any(between),
          "{parts:?} {step}"
        );
      } else {
        // the picture alone and the bands its glow reaches: the screen, and with light bouncing
        // what its light is thrown on
        let most = if bounce > 0.0 { 0.4 } else { 0.25 };
        assert!(share(parts) < most, "{parts:?} {step}");
      }
    }
  }
}

#[test]
#[ignore = "needs a graphics device"]
fn light_off_what_changes_color_shows_on_everything_near_it_at_any_height() {
  let target = Target::new();
  // a low block that lights itself between two very tall walls that face it from near by, with
  // far more of its light thrown on them than any look has. under a view that leans, the top
  // of a wall is seen far off from where the wall stands
  let shapes = [
    Mesh::cuboid(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 3.0)),
    Mesh::cuboid(Vec3::new(-14.0, -2.0, 0.0), Vec3::new(14.0, 2.0, 190.0)),
  ]
  .map(Arc::new);
  let at = |x: f32, y: f32| Mat4::from_translation(Vec3::new(x, y, 0.0));
  for (round, lean) in [Vec2::new(0.0, -0.5), Vec2::new(0.5, 0.5), Vec2::ZERO]
    .into_iter()
    .enumerate()
  {
    let mut number = numbers(21 + round as u32);
    let mut scene = panel(&mut number, &shapes);
    scene.objects.truncate(4);
    scene.lamps.clear();
    (scene.lean, scene.light.bounce) = (lean, 40.0);
    scene.heights = [-8.0, 200.0];
    let places = [at(224.0, 128.0), at(224.0, 104.0), at(224.0, 152.0)];
    let meshes = [&shapes[0], &shapes[1], &shapes[1]];
    for ((object, shape), place) in scene.objects[1..].iter_mut().zip(meshes).zip(places) {
      (object.mesh, object.transform) = (Arc::clone(shape), place);
      object.material = Material::matte(Vec3::splat(0.8));
    }
    let [mut kept, mut whole] = [(); 2].map(|()| Renderer::new(&target.device, FORMAT));
    let mut shown = Vec::new();
    for frame in 0..10 {
      let color = Vec3::new(number(), number(), number());
      scene.objects[1].material = Material::matte(color).lit(color, number(), 0.0);
      let step = format!("with the block in color {frame} under lean {round}");
      target.assert_same(&mut kept, &mut whole, &mut shown, &scene, &step);
      // the first frame is drawn whole, and the one after it tells whether a bound holds
      if frame > 1 {
        let parts = kept.changed().expect("the color is drawn in part");
        assert!(share(parts) < 0.5, "{parts:?} {step}");
      }
    }
  }
}

#[test]
#[ignore = "needs a graphics device"]
fn light_off_what_changes_color_shows_on_a_sliver_where_the_sliver_is() {
  let target = Target::new();
  let block = Arc::new(Mesh::cuboid(
    Vec3::new(-10.0, -10.0, 0.0),
    Vec3::new(10.0, 10.0, 3.0),
  ));
  // six walls 40 high that lean nearly along the line of sight, so each is a sliver of a pixel
  // across. the plane of one runs on along that line to any height, over the block too
  let (lean, off) = (0.3, 0.004);
  let mut slivers = Mesh::default();
  let normal = Vec3::new(-1.0, 0.0, off - lean).normalize();
  for foot in [154.04, 156.07, 158.10, 160.05, 162.08, 164.03] {
    let top = foot + (off - lean) * 40.0;
    let corners = [
      [foot, 100.0, 0.0],
      [foot, 156.0, 0.0],
      [top, 156.0, 40.0],
      [top, 100.0, 40.0],
    ];
    slivers.quad(corners.map(Vec3::from), normal);
  }
  let mut number = numbers(77);
  let mut scene = panel(&mut number, &[Arc::clone(&block)]);
  scene.objects.truncate(3);
  scene.lamps.clear();
  (scene.lean, scene.light.bounce) = (Vec2::new(lean, 0.0), 40.0);
  scene.objects[1].transform = Mat4::from_translation(Vec3::new(100.0, 128.0, 0.0));
  scene.objects[2] = Object {
    mesh: Arc::new(slivers),
    transform: Mat4::IDENTITY,
    material: Material::matte(Vec3::splat(0.8)),
  };
  let [mut kept, mut whole] = [(); 2].map(|()| Renderer::new(&target.device, FORMAT));
  let mut shown = Vec::new();
  for frame in 0..10 {
    let color = Vec3::new(number(), number(), number());
    scene.objects[1].material = Material::matte(color).lit(color, 1.0, 0.0);
    let step = format!("with the block in color {frame} beside the slivers");
    target.assert_same(&mut kept, &mut whole, &mut shown, &scene, &step);
    // the first frame is drawn whole, and the one after it tells whether a bound holds
    if frame > 1 {
      let parts = kept.changed().expect("the color is drawn in part");
      assert!(share(parts) < 0.5, "{parts:?} {step}");
    }
  }
}

/// Returns what a function of WGSL makes of each of `inputs`, run in a compute shader.
///
/// `source` goes ahead of the shader, and `of_value` is an expression of `value`, one of the
/// inputs, that comes to a `vec4<f32>`.
fn computed(source: &str, of_value: &str, inputs: &[[f32; 4]]) -> Vec<[f32; 4]> {
  let target = Target::new();
  let device = &target.device;
  let source = format!(
    "{source}
     @group(0) @binding(0) var<storage, read_write> values: array<vec4<f32>>;
     @compute @workgroup_size(1)
     fn main(@builtin(global_invocation_id) at: vec3<u32>) {{
       let value = values[at.x];
       values[at.x] = {of_value};
     }}"
  );
  let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: None,
    source: wgpu::ShaderSource::Wgsl(source.into()),
  });
  let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
    label: None,
    layout: None,
    module: &module,
    entry_point: Some("main"),
    compilation_options: wgpu::PipelineCompilationOptions::default(),
    cache: None,
  });
  let values = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: None,
    contents: bytemuck::cast_slice(inputs),
    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
  });
  let read = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: values.size(),
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: None,
    layout: &pipeline.get_bind_group_layout(0),
    entries: &[wgpu::BindGroupEntry {
      binding: 0,
      resource: values.as_entire_binding(),
    }],
  });
  let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
  {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
  }
  encoder.copy_buffer_to_buffer(&values, 0, &read, 0, values.size());
  target.queue.submit([encoder.finish()]);
  (read.slice(..)).map_async(wgpu::MapMode::Read, |mapped| {
    mapped.expect("the values map")
  });
  finish(device, "the values a compute shader wrote");
  let mapped = read.slice(..).get_mapped_range();
  bytemuck::cast_slice(&mapped).to_vec()
}

#[test]
#[ignore = "needs a graphics device"]
fn a_display_shows_the_mean_of_its_cells_over_each_pixel() {
  // the lit square takes 0.78 of a cell each way, and the gap shows 0.3 of the light
  let mean = 0.3 + 0.7 * 0.78 * 0.78;
  let shown = |inputs: &[[f32; 4]]| {
    let source = pipelines::CELLS_SOURCE;
    let out = computed(source, "vec4<f32>(cells_over(value.xy, value.zw))", inputs);
    out.into_iter().map(|value| value[0]).collect::<Vec<f32>>()
  };

  // a pixel over a great many cells shows their mean, wherever it is
  let wide = [[0.0, 0.0], [0.31, 7.9], [12.5, 3.25]].map(|[x, y]| [x, y, 1000.5, 1000.5]);
  for (input, got) in wide.iter().zip(shown(&wide)) {
    assert!(
      (got - mean).abs() < 1e-3,
      "{input:?} shows {got}, not {mean}"
    );
  }

  // pixels a third of a cell wide, whose gap is narrower than a pixel: over the nine places a
  // pixel can stand in a cell they show the mean, from any start and either way round
  let third = 1.0 / 3.0;
  for start in [0.0, 0.1, 0.37, 0.71] {
    let places: Vec<[f32; 4]> = (0..9)
      .map(|place| {
        let (across, down) = ((place % 3) as f32, (place / 3) as f32);
        [start + across * third, 0.2 + down * third, third, third]
      })
      .collect();
    let flipped: Vec<[f32; 4]> = (places.iter()).map(|&[x, y, w, h]| [y, x, h, w]).collect();
    for inputs in [places, flipped] {
      let got = shown(&inputs).iter().sum::<f32>() / 9.0;
      assert!(
        (got - mean).abs() < 1e-4,
        "from {start} the pixels show {got}, not {mean}"
      );
    }
  }

  // a pixel far finer than the gap shows all the light in the lit square and 0.3 in the gap,
  // which is at a cell's edges
  let fine = [
    [0.5, 0.5, 1e-3, 1e-3],
    [3.4, 7.6, 1e-3, 1e-3],
    [0.0, 0.5, 1e-3, 1e-3],
    [0.5, 0.02, 1e-3, 1e-3],
    [2.97, 4.03, 1e-3, 1e-3],
  ];
  let wanted = [1.0, 1.0, 0.3, 0.3, 0.3];
  for ((input, got), wanted) in fine.iter().zip(shown(&fine)).zip(wanted) {
    assert!(
      (got - wanted).abs() < 1e-3,
      "{input:?} shows {got}, not {wanted}"
    );
  }
}

#[test]
#[ignore = "needs a graphics device"]
fn the_tone_map_meets_the_khronos_reference_points() {
  // inputs and what Khronos's sample code, pbrNeutral.glsl, makes of them
  let points: [([f32; 3], [f32; 3]); 10] = [
    ([0.0; 3], [0.0; 3]),
    ([0.05; 3], [0.015625; 3]),
    ([0.18; 3], [0.14; 3]),
    ([0.5; 3], [0.46; 3]),
    ([1.0; 3], [0.869091; 3]),
    ([4.0; 3], [0.983256; 3]),
    ([16.0; 3], [0.996269; 3]),
    ([1.0, 0.5, 0.1], [0.869091, 0.422529, 0.065279]),
    ([4.0, 2.0, 0.5], [0.983256, 0.639951, 0.382473]),
    ([0.02, 0.3, 0.9], [0.007559, 0.272777, 0.841103]),
  ];
  let inputs: Vec<[f32; 4]> = (points.iter())
    .map(|([red, green, blue], _)| [*red, *green, *blue, 1.0])
    .collect();
  let mapped = computed(
    pipelines::TONEMAP_SOURCE,
    "vec4<f32>(tonemap(value.rgb), 1.0)",
    &inputs,
  );
  for ((input, wanted), got) in points.iter().zip(mapped) {
    for (wanted, got) in wanted.iter().zip(got) {
      assert!(
        (wanted - got).abs() < 1e-5,
        "{input:?} maps to {got:?}, not {wanted:?}"
      );
    }
  }
}
