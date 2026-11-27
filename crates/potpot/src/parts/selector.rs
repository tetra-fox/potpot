//! Segmented selectors, a row of keys of which one is chosen.

use super::panel::frame;
use super::tabs::Tabs;
use crate::mesh::Mesh;

/// A segmented selector, of which one key is chosen.
///
/// It is a row of keys in a frame. The keys stand side by side along x with a hair between them,
/// and the row's middle is at the origin. The key chosen stays in, as the key of the page shown
/// does in [`Tabs`], whose methods lay the keys out.
#[derive(Clone, Copy, Debug)]
pub struct Selector {
  /// The row of keys, one for each choice.
  pub row: Tabs,
  /// The width of the frame around the row, in points.
  pub border: f32,
}

impl Selector {
  /// Creates a selector of `count` choices.
  ///
  /// Each key is `width` across, `length` down and `height` tall. The keys are a point apart,
  /// and the frame is as wide as the keys are tall.
  pub fn new(count: usize, width: f32, length: f32, height: f32) -> Self {
    Self {
      row: Tabs {
        gap: 1.0,
        ..Tabs::new(count, width, length, height)
      },
      border: height,
    }
  }

  /// Returns the frame around the row, standing on the panel.
  ///
  /// The frame leaves the row's gap between it and the keys, and is half as tall as they are.
  pub fn frame(&self) -> Mesh {
    let Self { row, border } = *self;
    let inside = [row.width(), row.key.length].map(|side| side + row.gap * 2.0);
    let [width, length] = inside.map(|side| side + border * 2.0);
    let corner = row.key.length.min(row.key.width) * row.key.corner / 2.0 + border;
    frame(width, length, border, row.key.height * 0.5, corner)
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use glam::{Mat4, Vec2, Vec3};

  use super::*;
  use crate::material::Material;
  use crate::scene::{Light, Look, Object, Scene};

  #[test]
  fn each_segment_is_picked_where_its_key_is_drawn_and_the_frame_is_none() {
    let selector = Selector::new(4, 56.0, 26.0, 5.0);
    let middle = Vec2::new(200.0, 50.0);
    let row = selector.row;
    let (body, frame) = (Arc::new(row.body()), Arc::new(selector.frame()));
    for chosen in 0..row.count {
      // the frame and then each key, the chosen one in
      let keys = (0..row.count).map(|choice| Object {
        mesh: Arc::clone(&body),
        transform: row.transform(middle.x, middle.y, choice, choice == chosen),
        material: Material::silicone(Vec3::splat(0.6)),
      });
      let frame = Object {
        mesh: Arc::clone(&frame),
        transform: Mat4::from_translation(middle.extend(0.0)),
        material: Material::chrome(Vec3::splat(0.7)),
      };
      let scene = Scene {
        size: Vec2::new(400.0, 100.0),
        heights: [-8.0, 64.0],
        lean: Vec2::new(0.0, -0.25),
        light: Light::default(),
        look: Look::default(),
        objects: std::iter::once(frame).chain(keys).collect(),
        lamps: Vec::new(),
      };
      for choice in 0..row.count {
        let key = middle + row.middle(choice);
        // its middle and in from each end: a key is its own choice's, after the frame
        let inside = row.key.width / 2.0 - 2.0;
        for off in [0.0, -inside, inside] {
          let at = key + Vec2::new(off, 0.0);
          assert_eq!(scene.pick(at), Some(1 + choice), "{choice} at {at}");
        }
      }
      // the frame, past the end of the first key and past the foot of the row
      let first = middle + row.middle(0);
      let edge = row.key.width / 2.0 + row.gap + selector.border / 2.0;
      assert_eq!(scene.pick(first - Vec2::X * edge), Some(0));
      let foot = row.key.length / 2.0 + row.gap + selector.border / 2.0;
      assert_eq!(scene.pick(first + Vec2::Y * foot), Some(0));
    }
  }

  #[test]
  fn the_frame_holds_the_row_with_the_gap_all_around() {
    let selector = Selector::new(3, 40.0, 20.0, 4.0);
    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    for vertex in &selector.frame().vertices {
      let position = Vec3::from(vertex.position);
      (min, max) = (min.min(position), max.max(position));
    }
    let row = selector.row;
    let half =
      Vec2::new(row.width(), row.key.length) / 2.0 + Vec2::splat(row.gap + selector.border);
    assert!((max.truncate() - half).length() < 1e-3, "to {max}");
    assert!((min.truncate() + half).length() < 1e-3, "from {min}");
    assert!((max.z - row.key.height * 0.5).abs() < 1e-3);
  }
}
