//! renders a panel to a png without a window: `cargo run --example picture -- out.png`
//! draws every knob style, and `faders`, `lights` or `synth` after the file's name draws
//! every fader, lit buttons and meters, or a whole soft synth's front in an old manner.
//! after those, one number is the pixel size and a second the color steps, for the crude
//! looks

mod faders;
mod knobs;
mod lights;
mod synth;

use skeuo::{Look, Place, Renderer, Scene, wgpu};

use crate::faders::faders;
use crate::knobs::knobs;
use crate::lights::lights;
use crate::synth::synth;

const SIZE: [u32; 2] = [1024, 640];

fn main() {
  let mut arguments = std::env::args().skip(1).peekable();
  let path = arguments.next().unwrap_or_else(|| "picture.png".to_owned());
  let scene: fn(Look) -> Scene = match arguments.next_if(|panel| panel.parse::<f32>().is_err()) {
    None => knobs,
    Some(panel) if panel == "knobs" => knobs,
    Some(panel) if panel == "faders" => faders,
    Some(panel) if panel == "lights" => lights,
    Some(panel) if panel == "synth" => synth,
    Some(panel) => panic!("no panel called {panel}, only knobs, faders, lights and synth"),
  };
  let look = Look {
    pixel_size: arguments
      .next()
      .map_or(1.0, |size| size.parse().expect("a pixel size")),
    color_steps: arguments
      .next()
      .map_or(0, |steps| steps.parse().expect("a count of steps")),
    ..Look::default()
  };

  let instance = wgpu::Instance::default();
  let adapter =
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
      .expect("no graphics adapter");
  eprintln!("drawing on {}", adapter.get_info().name);
  let (device, queue) =
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
      .expect("no graphics device");

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
    result.expect("the picture to map")
  });
  device
    .poll(wgpu::PollType::wait_indefinitely())
    .expect("the device to finish");

  let file = std::fs::File::create(&path).expect("the png to open");
  let mut png = png::Encoder::new(std::io::BufWriter::new(file), SIZE[0], SIZE[1]);
  png.set_color(png::ColorType::Rgba);
  let mut writer = png.write_header().expect("the png to start");
  writer
    .write_image_data(&read.slice(..).get_mapped_range())
    .expect("the png to write");
  eprintln!("wrote {path}");
}
