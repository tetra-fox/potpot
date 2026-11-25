//! The marks printed around a knob or beside a fader.

use glam::{Mat4, Vec3};

use crate::mesh::Mesh;

/// Returns the reach and the half width of one mark of a scale.
///
/// Every `long_every`th mark is half as long again as the others, and wider.
fn mark(index: u32, long_every: u32, inner: f32, outer: f32) -> (f32, f32) {
  if long_every > 0 && index.is_multiple_of(long_every) {
    (outer + (outer - inner) * 0.5, 1.0)
  } else {
    (outer, 0.6)
  }
}

/// Returns the scale around a knob.
///
/// The scale is `count` thin marks lying on the panel, spread over `sweep` radians the way
/// [`knob_transform`](super::knob_transform) turns, from `inner` to `outer` away from the center.
/// Every `long_every`th mark is half as long again.
pub fn ticks(count: u32, sweep: f32, inner: f32, outer: f32, long_every: u32) -> Mesh {
  let mut ticks = Mesh::default();
  for tick in 0..count {
    let turn = if count > 1 {
      tick as f32 / (count - 1) as f32
    } else {
      0.5
    };
    let (reach, half_width) = mark(tick, long_every, inner, outer);
    let mark = Mesh::cuboid(
      Vec3::new(-half_width, -reach, 0.0),
      Vec3::new(half_width, -inner, 0.3),
    );
    ticks = ticks.merged(&mark.transformed(Mat4::from_rotation_z((turn - 0.5) * sweep)));
  }
  ticks
}

/// Returns the scale beside a fader.
///
/// The scale is `count` thin marks lying on the panel down the length of the fader's `travel`, on
/// both sides of the slot from `inner` to `outer` away from it. Every `long_every`th mark is half
/// as long again.
pub fn fader_ticks(count: u32, travel: f32, inner: f32, outer: f32, long_every: u32) -> Mesh {
  let mut ticks = Mesh::default();
  for tick in 0..count {
    let along = if count > 1 {
      tick as f32 / (count - 1) as f32 - 0.5
    } else {
      0.0
    };
    let (reach, half) = mark(tick, long_every, inner, outer);
    let y = along * travel;
    for side in [-1.0_f32, 1.0] {
      let (from, to) = (side * inner, side * reach);
      ticks = ticks.merged(&Mesh::cuboid(
        Vec3::new(from.min(to), y - half, 0.0),
        Vec3::new(from.max(to), y + half, 0.3),
      ));
    }
  }
  ticks
}

/// Returns the dashed ring around a knob that old soft synths draw.
///
/// The ring is `count` short bars lying on the panel end to end around the origin at `radius`, over
/// `sweep` radians the way [`knob_transform`](super::knob_transform) turns. Each bar is `width`
/// across. `fill` is the share of its part of the ring a bar takes, and the gap to the next bar
/// takes what is left.
pub fn dashes(count: u32, sweep: f32, radius: f32, width: f32, fill: f32) -> Mesh {
  let mut ring = Mesh::default();
  let share = sweep / count.max(1) as f32;
  let half_length = radius * (share * fill.clamp(0.0, 1.0) / 2.0).tan();
  for dash in 0..count {
    let turn = (dash as f32 + 0.5) * share - sweep / 2.0;
    let bar = Mesh::cuboid(
      Vec3::new(-half_length, -radius - width / 2.0, 0.0),
      Vec3::new(half_length, -radius + width / 2.0, 0.3),
    );
    ring = ring.merged(&bar.transformed(Mat4::from_rotation_z(turn)));
  }
  ring
}
