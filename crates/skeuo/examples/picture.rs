//! renders a panel of every knob style to a png without a window:
//! `cargo run --example picture -- out.png`.
//! a second argument is the pixel size and a third the color steps, for the crude looks

use std::sync::Arc;

use skeuo::glam::{Mat4, Vec2, Vec3};
use skeuo::parts::{self, Knob, Top};
use skeuo::{Light, Look, Material, Object, Place, Renderer, Scene, wgpu};

const SIZE: [u32; 2] = [1024, 448];

/// how a knob of the gallery is marked
#[derive(Clone, Copy)]
enum Marker {
  Line,
  Dot,
  /// its own shape says where it points
  None,
}

/// a knob of the gallery: its shape, what it is made of, what marks it and in which color,
/// and what its face is inlaid with if anything
struct Shown {
  knob: Knob,
  material: Material,
  marker: Marker,
  mark: Vec3,
  cap: Option<Material>,
}

/// every knob style in two rows, each over a scale
fn scene(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let (radius, tall) = (58.0, 42.0);
  let black = Vec3::new(0.035, 0.035, 0.04);
  let white = Vec3::splat(0.93);
  let aluminum = Material::spun(Vec3::new(0.78, 0.79, 0.81));
  let shown = |knob, material, marker| Shown {
    knob,
    material,
    marker,
    mark: white,
    cap: None,
  };
  let knobs = [
    shown(
      Knob::round(radius, tall),
      Material::glossy(black),
      Marker::Line,
    ),
    shown(
      Knob::fluted(radius, tall),
      Material::glossy(black),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: black,
      ..shown(
        Knob::skirted(radius, tall),
        Material::glossy(black),
        Marker::Line,
      )
    },
    Shown {
      mark: black,
      ..shown(Knob::knurled(radius * 0.85, tall), aluminum, Marker::Line)
    },
    shown(
      Knob::dished(radius, tall),
      Material::satin(Vec3::new(0.50, 0.08, 0.07)),
      Marker::Dot,
    ),
    shown(
      Knob::dimpled(radius, tall * 0.55),
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      Marker::None,
    ),
    shown(
      Knob::faceted(radius, tall),
      Material::glossy(Vec3::new(0.06, 0.13, 0.26)),
      Marker::Line,
    ),
    shown(
      Knob::chicken_head(radius, tall),
      Material::glossy(black),
      Marker::Line,
    ),
    shown(
      Knob::bar(radius, tall * 1.2),
      Material::glossy(black),
      Marker::Line,
    ),
    shown(
      Knob::lever(radius, tall * 0.8),
      Material::glossy(Vec3::new(0.70, 0.36, 0.04)),
      Marker::Line,
    ),
    shown(
      Knob::ridge(radius, tall * 1.25),
      Material::glossy(black),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: black,
      ..shown(
        Knob {
          skirt: 0.0,
          top: Top::Flat,
          ..Knob::knurled(radius * 0.8, tall * 1.1)
        },
        Material::glossy(black),
        Marker::Line,
      )
    },
  ];

  let mut meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(8.0),
    parts::ticks(11, parts::KNOB_SWEEP, radius + 6.0, radius + 13.0, 5),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material::satin(Vec3::new(0.27, 0.29, 0.33)),
  }];
  for (index, shown) in knobs.into_iter().enumerate() {
    let (x, y) = (
      90.0 + 169.0 * (index % 6) as f32,
      112.0 + 216.0 * (index / 6) as f32,
    );
    let turn = 0.12 + 0.07 * index as f32;
    let transform = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    objects.push(Object {
      mesh: 2,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: Material::matte(Vec3::splat(0.85)),
    });
    let mut part = |mesh: skeuo::Mesh, material: Material| {
      objects.push(Object {
        mesh: meshes.len(),
        transform,
        material,
      });
      meshes.push(mesh);
    };
    part(shown.knob.body(), shown.material);
    if let Some(cap) = shown.cap {
      part(shown.knob.cap(0.86), cap);
    }
    match shown.marker {
      Marker::Line => part(shown.knob.line(), Material::matte(shown.mark)),
      Marker::Dot => part(shown.knob.dot(), Material::matte(shown.mark)),
      Marker::None => {}
    }
  }
  let inset = 18.0;
  for (x, y) in [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ] {
    objects.push(Object {
      mesh: 1,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::chrome(Vec3::splat(0.75)),
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 64.0],
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
