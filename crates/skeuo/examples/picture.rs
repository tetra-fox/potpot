//! renders a small panel to a png without a window: `cargo run --example picture -- out.png`.
//! a second argument is the pixel size and a third the color steps, for the crude looks

use std::sync::Arc;

use skeuo::glam::{Mat4, Vec2, Vec3};
use skeuo::{Light, Look, Material, Object, Place, Renderer, Scene, parts, wgpu};

const SIZE: [u32; 2] = [640, 320];

fn scene(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let meshes = Arc::from([
    parts::plate(width, height, 6.0),
    parts::knob_body(44.0, 34.0, 48, true),
    parts::knob_pointer(44.0, 34.0),
    parts::knob_body(30.0, 26.0, 9, false),
    parts::knob_pointer(30.0, 26.0),
    parts::screw(7.0),
    skeuo::Mesh::lathe(&[[9.0, 0.0], [9.0, 3.0], [6.0, 6.0], [0.0, 7.0]], 20, true),
  ]);

  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material {
      specular: 0.15,
      shininess: 20.0,
      ..Material::matte(Vec3::new(0.30, 0.33, 0.38))
    },
  }];
  let mut knob = |body: usize, x: f32, y: f32, turn: f32, color: Vec3| {
    let transform = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    objects.push(Object {
      mesh: body,
      transform,
      material: Material::glossy(color),
    });
    objects.push(Object {
      mesh: body + 1,
      transform,
      material: Material::matte(Vec3::splat(0.95)),
    });
  };
  knob(1, 130.0, 150.0, 0.2, Vec3::new(0.08, 0.08, 0.09));
  knob(1, 300.0, 150.0, 0.65, Vec3::new(0.08, 0.08, 0.09));
  knob(3, 440.0, 110.0, 0.5, Vec3::new(0.55, 0.12, 0.10));
  knob(3, 540.0, 110.0, 0.9, Vec3::new(0.75, 0.72, 0.65));
  for (x, y) in [
    (18.0, 18.0),
    (width - 18.0, 18.0),
    (18.0, height - 18.0),
    (width - 18.0, height - 18.0),
  ] {
    objects.push(Object {
      mesh: 5,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::glossy(Vec3::splat(0.6)),
    });
  }
  for (x, lit) in [(440.0, true), (490.0, false), (540.0, true)] {
    let color = Vec3::new(1.0, 0.25, 0.1);
    objects.push(Object {
      mesh: 6,
      transform: Mat4::from_translation(Vec3::new(x, 230.0, 0.0)),
      material: if lit {
        Material::lamp(color)
      } else {
        Material::glossy(color * 0.25)
      },
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 48.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    meshes,
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

  // a row of 640 rgba pixels is already a multiple of the 256 bytes wgpu copies rows by
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
