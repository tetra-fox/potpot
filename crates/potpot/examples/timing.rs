//! The GPU time of each frame of a knob's drag.
//!
//! `just timing` turns the first knob of the gallery's knobs panel through 120 frames at scale
//! 2, and prints the median and the 95th percentile of the time each frame takes on the GPU, in
//! milliseconds. The device has to have `Features::TIMESTAMP_QUERY`.
//!
//! Each frame's work is queued behind a copy that keeps the GPU busy for longer than the frame
//! takes to prepare on the CPU, so the time measured is the GPU's alone.

#[path = "gallery/panels/mod.rs"]
mod panels;

use std::time::{Duration, Instant};

use potpot::{Look, Place, Renderer, wgpu};

use crate::panels::{SIZE, knobs};

/// The frames of the drag.
const FRAMES: usize = 120;
/// The scale the panel is drawn at.
const SCALE: u32 = 2;
/// The size of each copy that keeps the GPU busy, in bytes.
const HOLD: u64 = 64 << 20;
/// The copies that keep the GPU busy ahead of each frame.
const HOLDS: usize = 16;

fn main() {
  let instance = wgpu::Instance::default();
  let adapter =
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
      .expect("a graphics adapter is found");
  eprintln!("timing on {}", adapter.get_info().name);
  let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
    required_features: wgpu::Features::TIMESTAMP_QUERY,
    ..Default::default()
  }))
  .expect("the adapter opens a device with timestamp queries");

  let size = SIZE.map(|side| side * SCALE);
  let format = wgpu::TextureFormat::Rgba8UnormSrgb;
  let target = device
    .create_texture(&wgpu::TextureDescriptor {
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
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
      view_formats: &[],
    })
    .create_view(&wgpu::TextureViewDescriptor::default());
  let place = Place {
    x: 0.0,
    y: 0.0,
    width: size[0] as f32,
    height: size[1] as f32,
  };

  let hold = [(); 2].map(|()| {
    device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("hold"),
      size: HOLD,
      usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    })
  });
  // when the hold starts, when the frame starts and when it ends
  let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
    label: Some("timing"),
    ty: wgpu::QueryType::Timestamp,
    count: 3,
  });
  let stamps = 3 * size_of::<u64>() as u64;
  let resolved = device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("timing resolved"),
    size: stamps,
    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
    mapped_at_creation: false,
  });
  let read = device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("timing read"),
    size: stamps,
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  // an empty pass that writes one timestamp as it starts
  let stamp = |encoder: &mut wgpu::CommandEncoder, index: u32| {
    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
      label: None,
      timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
        query_set: &queries,
        beginning_of_pass_write_index: Some(index),
        end_of_pass_write_index: None,
      }),
    });
  };

  let mut scene = knobs::knobs(Look::default());
  // the parts of the first knob, which turn with it
  let first = knobs::turned(0, knobs::turn(0));
  let turning: Vec<usize> = (scene.objects.iter().enumerate())
    .filter(|(_, object)| object.transform == first)
    .map(|(index, _)| index)
    .collect();
  let mut renderer = Renderer::new(&device, format);
  // the first picture is drawn whole, as a window's first frame is
  renderer.prepare(&device, &queue, &scene, size);

  let period = f64::from(queue.get_timestamp_period());
  let mut frames = Vec::with_capacity(FRAMES);
  for frame in 1..=FRAMES {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    stamp(&mut encoder, 0);
    for _ in 0..HOLDS {
      encoder.copy_buffer_to_buffer(&hold[0], 0, &hold[1], 0, HOLD);
    }
    stamp(&mut encoder, 1);
    queue.submit([encoder.finish()]);

    let turned = knobs::turned(0, knobs::turn(0) + frame as f32 * 0.005);
    for &index in &turning {
      scene.objects[index].transform = turned;
    }
    let preparing = Instant::now();
    renderer.prepare(&device, &queue, &scene, size);
    let prepared = preparing.elapsed();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render(&mut encoder, &target, place, [0, 0, size[0], size[1]]);
    stamp(&mut encoder, 2);
    encoder.resolve_query_set(&queries, 0..3, &resolved, 0);
    encoder.copy_buffer_to_buffer(&resolved, 0, &read, 0, stamps);
    queue.submit([encoder.finish()]);
    read.slice(..).map_async(wgpu::MapMode::Read, |result| {
      result.expect("the timestamps map")
    });
    device
      .poll(wgpu::PollType::wait_indefinitely())
      .expect("the device finishes");
    let [held, started, ended] = {
      let mapped = read.slice(..).get_mapped_range();
      let mut stamps = [0; 3];
      for (stamp, bytes) in stamps.iter_mut().zip(mapped.as_chunks::<8>().0) {
        *stamp = u64::from_le_bytes(*bytes);
      }
      stamps
    };
    read.unmap();
    let nanoseconds = |ticks: u64| Duration::from_nanos((ticks as f64 * period) as u64);
    let hold_time = nanoseconds(started - held);
    assert!(
      hold_time > prepared,
      "the gpu was held for {hold_time:?}, less than the {prepared:?} the frame took to prepare"
    );
    frames.push(nanoseconds(ended - started).as_secs_f64() * 1e3);
  }

  frames.sort_by(f64::total_cmp);
  let at = |share: f64| frames[((frames.len() - 1) as f64 * share).round() as usize];
  println!(
    "gpu time per drag frame over {FRAMES} frames: median {:.3} ms, 95th percentile {:.3} ms",
    at(0.5),
    at(0.95)
  );
}
