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
//! - `industrial`: dark metal hardware lit from within, and a glow on its own
//!
//! After the panel's name, a whole number is the scale the picture is drawn at. After it, one
//! number is the pixel size and a second the color steps, for the crude looks.

mod panels;

use potpot::{Look, Place, Renderer, wgpu};

use crate::panels::{PANELS, SIZE};

fn main() {
  // the first sample alone, before the picture is refined
  let first = std::env::args().any(|argument| argument == panels::shared::window::FIRST);
  let mut arguments = std::env::args()
    .skip(1)
    .filter(|argument| argument != panels::shared::window::FIRST);
  let (Some(path), Some(panel)) = (arguments.next(), arguments.next()) else {
    panic!("gallery: expected a png file and then one of {PANELS}")
  };
  let Some(scene) = panels::scene(&panel) else {
    panic!("gallery: unknown panel {panel}: expected one of {PANELS}")
  };
  // a whole number, so that a row of the picture stays a multiple of the 256 bytes wgpu
  // copies rows by
  let scale: u32 = arguments.next().map_or(1, |scale| {
    scale.parse().expect("the scale is a whole number")
  });
  let size = SIZE.map(|side| side * scale);
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
  let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
    required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
    ..Default::default()
  }))
  .expect("the adapter opens a device");

  let format = wgpu::TextureFormat::Rgba8UnormSrgb;
  let target = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d {
      width: size[0],
      height: size[1],
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
  let scene = scene(look);
  renderer.prepare(&device, &queue, &scene, size);
  while !first && !renderer.settled() {
    renderer.prepare(&device, &queue, &scene, size);
  }
  let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
  renderer.render(
    &mut encoder,
    &target.create_view(&wgpu::TextureViewDescriptor::default()),
    Place {
      x: 0.0,
      y: 0.0,
      width: size[0] as f32,
      height: size[1] as f32,
    },
    [0, 0, size[0], size[1]],
  );

  let row = size[0] * 4;
  let read = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: u64::from(row * size[1]),
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
  panels::shared::window::write_png(path.as_ref(), size, &pixels).expect("the png file is written");
  eprintln!("wrote {path}");
}
