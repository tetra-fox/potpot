//! Panels of potpot's parts, drawn to a PNG file without a window.
//!
//! `just render gallery out.png knobs` draws every knob style. The name after the file's name
//! picks the panel:
//!
//! - `knobs`: every knob style
//! - `faders`: every fader
//! - `lights`: lit buttons, meters and an XY pad
//! - `lamp`: lamps beside what shows their light
//! - `rings`: rings of lights around knobs
//! - `screens`: screens that show an equalizer, a scope and an envelope
//! - `retro`: a whole soft synth's front in an old manner
//!
//! After the panel's name, one number is the pixel size and a second the color steps, for the
//! crude looks.

mod faders;
mod knobs;
mod lamp;
mod lights;
mod retro;
mod rings;
mod screens;
#[path = "../shared/mod.rs"]
mod shared;

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts;
use potpot::{Light, Look, Material, Object, Place, Renderer, Scene, wgpu};

use crate::faders::faders;
use crate::knobs::knobs;
use crate::lamp::lamp;
use crate::lights::lights;
use crate::retro::retro;
use crate::rings::rings;
use crate::screens::screens;
use crate::shared::{ALUMINUM, BLACK, SEAT, lettering};

const SIZE: [u32; 2] = [1024, 640];
/// The names of the panels, as a message lists them.
const PANELS: &str = "knobs, faders, lights, lamp, rings, screens, retro";

/// Returns a screw for each corner, each turned its own way.
fn screws() -> [Object; 4] {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let screw = Arc::new(parts::screw(8.0));
  let inset = 18.0;
  [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ]
  .map(|(x, y)| Object {
    mesh: Arc::clone(&screw),
    transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
    material: Material::chrome(Vec3::splat(0.75)),
  })
}

/// Returns the scene of a panel that fills the picture.
///
/// The panel is seen leaning a little, under the plain light and with no lamps.
fn framed(look: Look, objects: Vec<Object>) -> Scene {
  Scene {
    size: Vec2::new(SIZE[0] as f32, SIZE[1] as f32),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    objects,
    lamps: Vec::new(),
  }
}

fn main() {
  let mut arguments = std::env::args().skip(1);
  let (Some(path), Some(panel)) = (arguments.next(), arguments.next()) else {
    panic!("gallery: expected a png file and then one of {PANELS}")
  };
  let scene: fn(Look) -> Scene = match panel.as_str() {
    "knobs" => knobs,
    "faders" => faders,
    "lights" => lights,
    "lamp" => lamp,
    "rings" => rings,
    "screens" => screens,
    "retro" => retro,
    _ => panic!("gallery: unknown panel {panel}: expected one of {PANELS}"),
  };
  let look = Look {
    pixel_size: arguments.next().map_or(1.0, |size| {
      size.parse().expect("the pixel size is a number")
    }),
    color_steps: arguments.next().map_or(0, |steps| {
      steps.parse().expect("the color steps are a whole number")
    }),
    ..Look::default()
  };

  let instance = wgpu::Instance::default();
  let adapter =
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
      .expect("a graphics adapter is found");
  eprintln!("drawing on {}", adapter.get_info().name);
  let (device, queue) =
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
      .expect("the adapter opens a device");

  let format = wgpu::TextureFormat::Rgba8UnormSrgb;
  let target = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d {
      width: SIZE[0],
      height: SIZE[1],
      depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    view_formats: &[],
  });

  let mut renderer = Renderer::new(&device, format);
  renderer.prepare(&device, &queue, &scene(look), SIZE);
  let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
  renderer.render(
    &mut encoder,
    &target.create_view(&wgpu::TextureViewDescriptor::default()),
    Place {
      x: 0.0,
      y: 0.0,
      width: SIZE[0] as f32,
      height: SIZE[1] as f32,
    },
    [0, 0, SIZE[0], SIZE[1]],
  );

  // a row of 1024 rgba pixels is already a multiple of the 256 bytes wgpu copies rows by
  let row = SIZE[0] * 4;
  let read = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: u64::from(row * SIZE[1]),
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  encoder.copy_texture_to_buffer(
    target.as_image_copy(),
    wgpu::TexelCopyBufferInfo {
      buffer: &read,
      layout: wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(row),
        rows_per_image: None,
      },
    },
    target.size(),
  );
  queue.submit([encoder.finish()]);
  read.slice(..).map_async(wgpu::MapMode::Read, |result| {
    result.expect("the picture maps")
  });
  device
    .poll(wgpu::PollType::wait_indefinitely())
    .expect("the device finishes");

  let pixels = read.slice(..).get_mapped_range();
  shared::window::write_png(path.as_ref(), SIZE, &pixels).expect("the png file is written");
  eprintln!("wrote {path}");
}
