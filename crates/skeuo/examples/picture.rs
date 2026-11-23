//! renders a panel of every knob style to a png without a window:
//! `cargo run --example picture -- out.png`.
//! a second argument is the pixel size and a third the color steps, for the crude looks

use std::sync::Arc;

use skeuo::glam::{Mat4, Vec2, Vec3};
use skeuo::parts::{self, Knob};
use skeuo::{Light, Look, Material, Object, Place, Renderer, Scene, wgpu};

const SIZE: [u32; 2] = [768, 320];

/// how a knob of the gallery is marked
#[derive(Clone, Copy)]
enum Marker {
  Line,
  Dot,
  /// its own shape says where it points
  None,
}

/// every knob style in two rows, each over a scale
fn scene(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let (radius, tall) = (44.0, 32.0);
  let dark = Vec3::new(0.09, 0.09, 0.10);
  let knobs = [
    (Knob::round(radius, tall), Marker::Line, dark),
    (
      Knob::fluted(radius, tall),
      Marker::Line,
      Vec3::new(0.16, 0.17, 0.19),
    ),
    (Knob::skirted(radius, tall), Marker::Dot, dark),
    (
      Knob::knurled(radius, tall),
      Marker::Line,
      Vec3::new(0.62, 0.63, 0.66),
    ),
    (
      Knob::dished(radius, tall),
      Marker::Dot,
      Vec3::new(0.55, 0.12, 0.10),
    ),
    (
      Knob::dimpled(radius, tall * 0.6),
      Marker::None,
      Vec3::new(0.70, 0.68, 0.62),
    ),
    (
      Knob::faceted(radius, tall),
      Marker::Line,
      Vec3::new(0.12, 0.25, 0.42),
    ),
    (Knob::chicken_head(radius, tall), Marker::Line, dark),
    (
      Knob::bar(radius, tall * 1.3),
      Marker::Line,
      Vec3::new(0.20, 0.21, 0.23),
    ),
    (
      Knob::lever(radius, tall * 0.8),
      Marker::Line,
      Vec3::new(0.78, 0.45, 0.08),
    ),
    (
      Knob::ridge(radius, tall * 1.25),
      Marker::Line,
      Vec3::new(0.22, 0.23, 0.26),
    ),
  ];

  let mut meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(7.0),
    parts::ticks(11, parts::KNOB_SWEEP, radius + 5.0, radius + 11.0, 5),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material {
      specular: 0.15,
      shininess: 20.0,
      ..Material::matte(Vec3::new(0.30, 0.33, 0.38))
    },
  }];
  for (index, (knob, marker, color)) in knobs.into_iter().enumerate() {
    let (x, y) = (
      72.0 + 125.0 * (index % 6) as f32,
      88.0 + 148.0 * (index / 6) as f32,
    );
    let turn = 0.15 + 0.08 * index as f32;
    let transform = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    objects.push(Object {
      mesh: 2,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: Material::matte(Vec3::splat(0.85)),
    });
    objects.push(Object {
      mesh: meshes.len(),
      transform,
      material: Material::glossy(color),
    });
    meshes.push(knob.body());
    let marker = match marker {
      Marker::Line => knob.line(),
      Marker::Dot => knob.dot(),
      Marker::None => continue,
    };
    let white = Material::matte(Vec3::splat(0.95));
    objects.push(Object {
      mesh: meshes.len(),
      transform,
      material: white,
    });
    meshes.push(marker);
  }
  let inset = 16.0;
  for (x, y) in [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ] {
    objects.push(Object {
      mesh: 1,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::glossy(Vec3::splat(0.6)),
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 48.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    meshes: Arc::from(meshes),
    objects,
  }
}

fn main() {
  let mut arguments = std::env::args().skip(1);
  let path = arguments.next().unwrap_or_else(|| "picture.png".to_owned());
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

  // a row of 768 rgba pixels is already a multiple of the 256 bytes wgpu copies rows by
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
