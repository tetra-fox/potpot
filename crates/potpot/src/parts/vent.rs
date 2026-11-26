//! Air vents cut through a panel.

use glam::{Mat4, Vec2, Vec3};

use super::PAINT;
use super::legend::{END, add_stroke};
use super::panel::slot;
use crate::mesh::Mesh;

/// A row of slots through the panel, with a perforated sheet behind them.
///
/// The slots run along y and stand side by side along x, centered on the origin. Through each
/// one the sheet shows small round holes in rows, for light from behind the panel to come
/// through.
#[derive(Clone, Copy, Debug)]
pub struct Vent {
  /// The count of slots.
  pub slots: u32,
  /// The width of each slot along x, in points.
  pub width: f32,
  /// The length of each slot along y, in points.
  pub length: f32,
  /// The distance between the middles of two neighboring slots, in points.
  pub pitch: f32,
  /// The width of each hole in the sheet, in points.
  pub hole: f32,
  /// The distance between the middles of two neighboring holes, in points.
  pub hole_pitch: f32,
}

impl Vent {
  /// Creates a row of `slots` slots, each `width` by `length` with round ends.
  pub fn new(slots: u32, width: f32, length: f32) -> Self {
    let hole = width * 0.12;
    Self {
      slots,
      width,
      length,
      pitch: width * 2.0,
      hole,
      hole_pitch: hole * 1.6,
    }
  }

  /// Returns the middle of each slot along x.
  fn middles(&self) -> impl Iterator<Item = f32> {
    let first = -(self.slots.saturating_sub(1) as f32) * self.pitch / 2.0;
    (0..self.slots).map(move |index| first + index as f32 * self.pitch)
  }

  /// Returns the slots, as dark strips lying on the panel.
  ///
  /// They are the sheet behind the panel, seen through it.
  pub fn slots(&self) -> Mesh {
    let one = slot(self.width, self.length);
    (self.middles()).fold(Mesh::default(), |slots, x| {
      slots.merged(&one.clone().transformed(Mat4::from_translation(Vec3::X * x)))
    })
  }

  /// Returns the middles of the holes that show through one slot centered on the origin.
  ///
  /// The holes are packed in staggered rows, and each one is clear of the slot's edge by the
  /// web of sheet between two holes.
  fn holes_in_slot(&self) -> Vec<Vec2> {
    let row = self.hole_pitch * 3.0_f32.sqrt() / 2.0;
    let web = self.hole_pitch - self.hole;
    // the slot is the round-ended strip around this line, half its width away
    let ends = (self.length - self.width).max(0.0) / 2.0;
    let room = self.width / 2.0 - self.hole / 2.0 - web;
    let rows = (self.length / 2.0 / row).ceil() as i32;
    let columns = (self.width / 2.0 / self.hole_pitch).ceil() as i32 + 1;
    let mut holes = Vec::new();
    for line in -rows..=rows {
      let y = line as f32 * row;
      // every other row is set over by half a hole's pitch
      let shift = if line % 2 == 0 { 0.0 } else { 0.5 };
      for column in -columns..=columns {
        let at = Vec2::new((column as f32 + shift) * self.hole_pitch, y);
        let nearest = Vec2::new(0.0, y.clamp(-ends, ends));
        if at.distance(nearest) <= room {
          holes.push(at);
        }
      }
    }
    holes
  }

  /// Returns the holes in the sheet that show through the slots.
  ///
  /// The holes lie just over [`Vent::slots`], to be drawn in a material that lights itself.
  pub fn holes(&self) -> Mesh {
    let mut holes = Mesh::default();
    let one = self.holes_in_slot();
    for x in self.middles() {
      for at in &one {
        let at = *at + Vec2::X * x;
        // a stroke of no length is a round dot
        add_stroke(&mut holes, [at, at], self.hole, PAINT, END);
      }
    }
    holes
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_holes_of_a_slot_are_inside_it_and_apart() {
    for vent in [Vent::new(6, 8.0, 40.0), Vent::new(3, 10.0, 10.0)] {
      let holes = vent.holes_in_slot();
      assert!(holes.len() > 4, "{vent:?} shows {} holes", holes.len());
      let ends = (vent.length - vent.width).max(0.0) / 2.0;
      for hole in &holes {
        // the edge of the hole is inside the slot's round-ended outline
        let nearest = Vec2::new(0.0, hole.y.clamp(-ends, ends));
        let reach = hole.distance(nearest) + vent.hole / 2.0;
        assert!(reach < vent.width / 2.0, "{vent:?} has a hole at {hole}");
      }
      for (index, hole) in holes.iter().enumerate() {
        for other in &holes[index + 1..] {
          let apart = hole.distance(*other);
          assert!(apart > vent.hole_pitch - 1e-4, "{hole} and {other}");
        }
      }
    }
  }

  #[test]
  fn the_slots_of_a_vent_are_centered_and_a_pitch_apart() {
    let vent = Vent::new(4, 6.0, 30.0);
    let middles: Vec<f32> = vent.middles().collect();
    assert_eq!(middles, [-18.0, -6.0, 6.0, 18.0]);
    let slots = vent.slots();
    let reach = (slots.vertices.iter())
      .map(|vertex| vertex.position[0].abs())
      .fold(0.0, f32::max);
    assert!((reach - 21.0).abs() < 1e-3, "{reach}");
  }
}
