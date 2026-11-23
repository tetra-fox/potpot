//! renders a panel of every knob style, of every fader, or of lit buttons and meters, to a
//! png without a window: `cargo run --example picture -- out.png`, `... -- out.png faders`
//! and `... -- out.png lights`. after those, one number is the pixel size and a second the
//! color steps, for the crude looks

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use skeuo::glam::{Mat4, Vec2, Vec3};
use skeuo::parts::{self, Button, Fader, Knob, Meter, Push, PushSwitch, Top};
use skeuo::{Light, Look, Material, Object, Place, Renderer, Scene, wgpu};

const SIZE: [u32; 2] = [1024, 640];

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

/// every knob style in rows of five, each over a scale
fn knobs(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let (radius, tall) = (58.0, 42.0);
  let black = Vec3::new(0.035, 0.035, 0.04);
  let white = Vec3::splat(0.86);
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
    shown(
      Knob::hex(radius, tall * 1.1),
      Material::glossy(black),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: black,
      ..shown(
        Knob::ribbed(radius * 0.8, tall * 1.1),
        Material::satin(black),
        Marker::Line,
      )
    },
    Shown {
      cap: Some(Material::spun(Vec3::new(0.80, 0.66, 0.36))),
      mark: black,
      ..shown(
        Knob::conical(radius, tall),
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
    material: Material::brushed(Vec3::new(0.30, 0.32, 0.36)),
  }];
  for (index, shown) in knobs.into_iter().enumerate() {
    let (x, y) = (
      112.0 + 200.0 * (index % 5) as f32,
      112.0 + 208.0 * (index / 5) as f32,
    );
    let turn = 0.12 + 0.055 * index as f32;
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
    lamps: Vec::new(),
  }
}

/// every fader cap in its slot, the last few a dj mixer's in its colors
fn faders(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let black = Vec3::new(0.035, 0.035, 0.04);
  let (white, dark) = (Vec3::splat(0.86), Vec3::splat(0.05));
  let (wide, long, tall) = (34.0, 62.0, 26.0);
  // a dj mixer's cap is much wider across than it is long
  let block = Fader::block(50.0, 24.0, 22.0);
  let caps = [
    (
      Fader::console(wide, long, tall),
      Material::glossy(black),
      white,
    ),
    (
      Fader::terraced(wide, long, tall),
      Material::glossy(black),
      white,
    ),
    (
      Fader::terraced(wide, long, tall),
      Material::spun(Vec3::new(0.78, 0.79, 0.81)),
      dark,
    ),
    (
      Fader::stepped(wide, long, tall),
      Material::satin(Vec3::splat(0.2)),
      white,
    ),
    (
      Fader::wedge(wide, long, tall * 1.2),
      Material::glossy(black),
      white,
    ),
    (
      Fader::barrel(wide * 0.8, long * 0.8, tall),
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      dark,
    ),
    (
      Fader::fin(wide * 0.8, long * 0.7, tall),
      Material::satin(Vec3::new(0.62, 0.10, 0.08)),
      white,
    ),
    (block, Material::glossy(Vec3::new(0.86, 0.72, 0.05)), dark),
    (block, Material::glossy(Vec3::new(0.70, 0.06, 0.07)), white),
    (block, Material::glossy(Vec3::new(0.10, 0.55, 0.62)), dark),
  ];

  let (travel, middle) = (380.0, height / 2.0);
  let mut meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(8.0),
    parts::fader_slot(travel, 5.0),
    parts::fader_ticks(11, travel, 26.0, 34.0, 5),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.16, 0.17, 0.19)),
  }];
  for (index, (fader, material, stripe)) in caps.into_iter().enumerate() {
    let x = 62.0 + 100.0 * index as f32;
    let track = Mat4::from_translation(Vec3::new(x, middle, 0.0));
    objects.push(Object {
      mesh: 2,
      transform: track,
      material: Material::matte(Vec3::splat(0.02)),
    });
    objects.push(Object {
      mesh: 3,
      transform: track,
      material: Material::matte(Vec3::splat(0.85)),
    });
    // each a little higher than the last, and then around again
    let value = (0.15 + 0.23 * index as f32) % 1.0;
    let transform = parts::fader_transform(x, middle, travel, value);
    objects.push(Object {
      mesh: meshes.len(),
      transform,
      material,
    });
    meshes.push(fader.body());
    objects.push(Object {
      mesh: meshes.len(),
      transform,
      material: Material::matte(stripe),
    });
    meshes.push(fader.line());
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
    lamps: Vec::new(),
  }
}

/// rubber buttons, some of them lit, and level meters at a few levels, on a dark panel
/// under a dimmed light so that what glows shows
fn lights(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let rubber = Material::silicone(Vec3::new(0.62, 0.62, 0.60));
  let colors = [
    Vec3::new(1.0, 0.25, 0.05),
    Vec3::new(0.1, 0.9, 0.3),
    Vec3::new(0.1, 0.5, 1.0),
    Vec3::new(1.0, 0.75, 0.1),
    Vec3::new(0.9, 0.1, 0.7),
    Vec3::new(0.95, 0.95, 0.9),
  ];

  let pad = Button::pad(70.0, 12.0);
  let round = Button::round(46.0, 10.0);
  let key = Button::key(70.0, 28.0, 7.0);
  let pill = Button::pill(26.0, 60.0, 8.0);
  let wide = Meter::ladder(16, 18.0, 420.0);
  let thin = Meter::ladder(24, 9.0, 420.0);
  let lying = Meter::ladder(14, 20.0, 250.0);
  let meshes = vec![
    parts::plate(width, height, 6.0),
    parts::screw(8.0),
    pad.body(),
    pad.seat(3.0),
    round.body(),
    round.seat(2.5),
    key.body(),
    key.seat(2.5),
    pill.body(),
    pill.seat(2.5),
    wide.segment(),
    wide.seat(),
    thin.segment(),
    thin.seat(),
    PushSwitch::square(30.0, 18.0).plunger(),
    PushSwitch::square(30.0, 18.0).collar(),
    PushSwitch::round(30.0, 18.0).plunger(),
    PushSwitch::round(30.0, 18.0).collar(),
    lying.segment(),
    lying.seat(),
  ];
  let mut objects = vec![Object {
    mesh: 0,
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.075, 0.08, 0.09)),
  }];
  let mut lamps = Vec::new();
  let seat = Material::matte(Vec3::splat(0.012));

  // each button with the mesh of its body, which its seat's follows, where it is, how far
  // it is lit and in which color, and how far it is held down
  let mut button =
    |shape: Button, mesh: usize, x: f32, y: f32, level: f32, color: Vec3, held: f32| {
      objects.push(Object {
        mesh: mesh + 1,
        transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
        material: seat,
      });
      objects.push(Object {
        mesh,
        transform: parts::button_transform(x, y, shape.height, held),
        material: rubber.lit(color, level, shape.glow_radius()),
      });
      if level > 0.0 {
        lamps.push(shape.lamp(x, y, color, level));
      }
    };
  // a grid of pads, lit here and there, one of them dimly and one held down
  for index in 0..16 {
    let (x, y) = (
      72.0 + 86.0 * (index % 4) as f32,
      82.0 + 86.0 * (index / 4) as f32,
    );
    let level = match index {
      0 | 5 | 6 | 11 | 13 => 1.0,
      9 => 0.35,
      _ => 0.0,
    };
    let held = if index == 6 { 1.0 } else { 0.0 };
    button(pad, 2, x, y, level, colors[index % colors.len()], held);
  }
  for index in 0..5 {
    let level = if index == 1 || index == 4 { 1.0 } else { 0.0 };
    button(
      round,
      4,
      72.0 + 64.5 * index as f32,
      470.0,
      level,
      colors[index + 1],
      0.0,
    );
  }
  for index in 0..4 {
    let level = if index == 2 { 1.0 } else { 0.0 };
    button(
      key,
      6,
      72.0 + 86.0 * index as f32,
      560.0,
      level,
      colors[3],
      0.0,
    );
  }
  for index in 0..12 {
    let (x, y) = (
      440.0 + 44.0 * (index % 6) as f32,
      110.0 + 84.0 * (index / 6) as f32,
    );
    let level = if index % 6 == index / 6 * 3 + 1 {
      1.0
    } else {
      0.0
    };
    button(
      pill,
      8,
      x,
      y,
      level,
      colors[(index + 2) % colors.len()],
      0.0,
    );
  }

  // push switches: out, latched in and lit, and held all the way in, square and then round
  let black = Material::glossy(Vec3::new(0.05, 0.05, 0.055));
  let collar = Material::chrome(Vec3::splat(0.7));
  let switches = [
    (PushSwitch::square(30.0, 18.0), Push::Latching, false, false),
    (PushSwitch::square(30.0, 18.0), Push::Latching, true, false),
    (PushSwitch::square(30.0, 18.0), Push::Momentary, true, true),
    (PushSwitch::round(30.0, 18.0), Push::Latching, false, false),
    (PushSwitch::round(30.0, 18.0), Push::Latching, true, false),
    (PushSwitch::round(30.0, 18.0), Push::Momentary, true, true),
  ];
  for (index, (switch, push, on, held)) in switches.into_iter().enumerate() {
    let (x, y) = (452.0 + 48.0 * index as f32, 300.0);
    let round = index >= 3;
    let (plunger, ring) = if round { (16, 17) } else { (14, 15) };
    objects.push(Object {
      mesh: ring,
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: collar,
    });
    let amber = Vec3::new(1.0, 0.55, 0.05);
    let level = if on { 1.0 } else { 0.0 };
    objects.push(Object {
      mesh: plunger,
      transform: switch.transform(x, y, switch.depth(push, on, held)),
      material: if round {
        black
      } else {
        rubber.lit(amber, level, switch.plunger.glow_radius())
      },
    });
    if on && !round {
      lamps.push(switch.plunger.lamp(x, y, amber, 0.6));
    }
  }

  // two pairs of meters, each at its own level, the first pair holding a peak
  let foot = 540.0;
  let meters = [
    (wide, 10, 760.0, 0.78, Some(0.92)),
    (wide, 10, 792.0, 0.64, Some(0.80)),
    (thin, 12, 870.0, 0.42, None),
    (thin, 12, 888.0, 1.0, None),
    (thin, 12, 940.0, 0.0, None),
    (thin, 12, 958.0, 0.18, None),
  ];
  for (meter, mesh, x, level, peak) in meters {
    objects.extend(meter.seats(mesh + 1, x, foot));
    objects.extend(meter.lights(mesh, x, foot, level, peak));
    lamps.extend(meter.lamp(x, foot, level));
  }
  // and a row of lights lying on its side, which is a meter turned a quarter
  let across =
    Mat4::from_translation(Vec3::new(440.0, 420.0, 0.0)) * Mat4::from_rotation_z(FRAC_PI_2);
  let turned = |object: Object| Object {
    transform: across * object.transform,
    ..object
  };
  objects.extend(lying.seats(19, 0.0, 0.0).into_iter().map(turned));
  objects.extend(
    lying
      .lights(18, 0.0, 0.0, 0.7, None)
      .into_iter()
      .map(turned),
  );

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
    light: Light {
      color: Vec3::splat(0.6),
      ambient: Vec3::splat(0.2),
      ..Light::default()
    },
    look,
    meshes: Arc::from(meshes),
    objects,
    lamps,
  }
}

fn main() {
  let mut arguments = std::env::args().skip(1).peekable();
  let path = arguments.next().unwrap_or_else(|| "picture.png".to_owned());
  let scene: fn(Look) -> Scene = match arguments.next_if(|panel| panel.parse::<f32>().is_err()) {
    None => knobs,
    Some(panel) if panel == "knobs" => knobs,
    Some(panel) if panel == "faders" => faders,
    Some(panel) if panel == "lights" => lights,
    Some(panel) => panic!("no panel called {panel}, only knobs, faders and lights"),
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
