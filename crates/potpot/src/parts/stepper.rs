//! Steppers, a value in a window between two arrow keys.

use glam::{Mat4, Vec2, Vec3};

use super::INK;
use super::button::Button;
use super::legend::{END, add_stroke};
use super::panel::frame;
use crate::mesh::Mesh;

/// One of the two keys of a [`Stepper`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
  /// The key to the left, which moves the value back.
  Back,
  /// The key to the right, which moves the value on.
  On,
}

/// A stepper: a value in a window between two arrow keys.
///
/// The window's middle is at the origin. The key to its left moves the value back and the one to
/// its right moves it on, and each goes in while it is held.
#[derive(Clone, Copy, Debug)]
pub struct Stepper {
  /// Each of the two keys.
  pub key: Button,
  /// The width and height of the window's glass, in points.
  pub window: Vec2,
  /// The width of the frame around the glass, in points.
  pub border: f32,
  /// The room between the frame and each key, in points.
  pub gap: f32,
}

impl Stepper {
  /// Creates a stepper whose window is `width` across.
  ///
  /// The keys are square, as long down the panel as the window and its frame, and `height`
  /// tall. The frame is half as wide as the keys are tall, and the keys stand as far off it.
  pub fn new(width: f32, length: f32, height: f32) -> Self {
    let border = height * 0.5;
    let side = length + border * 2.0;
    Self {
      key: Button::key(side, side, height),
      window: Vec2::new(width, length),
      border,
      gap: height,
    }
  }

  /// Returns the middle of a key, from the window's middle.
  pub fn middle(&self, arrow: Arrow) -> Vec2 {
    let apart = self.window.x / 2.0 + self.border + self.gap + self.key.width / 2.0;
    match arrow {
      Arrow::Back => Vec2::X * -apart,
      Arrow::On => Vec2::X * apart,
    }
  }

  /// Returns the width of the whole stepper in points.
  pub fn width(&self) -> f32 {
    (self.middle(Arrow::On).x + self.key.width / 2.0) * 2.0
  }

  /// Returns the mesh of a key.
  pub fn body(&self) -> Mesh {
    self.key.body()
  }

  /// Returns the dark gap around a key as a strip.
  ///
  /// The strip is `gap` wider than the key all around.
  pub fn seat(&self, gap: f32) -> Mesh {
    self.key.seat(gap)
  }

  /// Returns the glass of the window, a thin pane on the panel.
  pub fn glass(&self) -> Mesh {
    let half = self.window / 2.0;
    Mesh::cuboid((-half).extend(0.0), half.extend(0.4))
  }

  /// Returns the frame around the glass.
  pub fn frame(&self) -> Mesh {
    let outside = self.window + Vec2::splat(self.border * 2.0);
    frame(
      outside.x,
      outside.y,
      self.border,
      self.key.height * 0.4,
      self.border,
    )
  }

  /// Returns the transform of a key.
  ///
  /// The stepper's window is at (`x`, `y`), and the key goes in while it is `held`.
  pub fn transform(&self, x: f32, y: f32, arrow: Arrow, held: bool) -> Mat4 {
    let at = Vec2::new(x, y) + self.middle(arrow);
    self.key.transform(at.x, at.y, f32::from(u8::from(held)))
  }

  /// Returns the minus and the plus printed on the keys.
  ///
  /// They come in that order. Each is printed as lettering is, two fifths of a key wide, with
  /// its middle at the origin.
  pub fn signs(&self) -> [Mesh; 2] {
    let reach = self.key.width.min(self.key.length) * 0.2;
    let weight = reach * 0.45;
    let mut minus = Mesh::default();
    add_stroke(
      &mut minus,
      [Vec2::X * -reach, Vec2::X * reach],
      weight,
      INK,
      END,
    );
    let mut plus = minus.clone();
    add_stroke(
      &mut plus,
      [Vec2::Y * -reach, Vec2::Y * reach],
      weight,
      INK,
      END,
    );
    [minus, plus]
  }

  /// Returns the transform of the sign printed on a key.
  ///
  /// The sign is one of [`Stepper::signs`], the minus for the key that moves the value back and
  /// the plus for the other, which goes on the key's top and in with it.
  pub fn sign(&self, x: f32, y: f32, arrow: Arrow, held: bool) -> Mat4 {
    self.transform(x, y, arrow, held) * Mat4::from_translation(Vec3::Z * self.key.top())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_keys_stand_clear_of_the_frame_to_either_side() {
    let stepper = Stepper::new(60.0, 20.0, 6.0);
    let frame_edge = stepper.window.x / 2.0 + stepper.border;
    let key_edge = stepper.middle(Arrow::On).x - stepper.key.width / 2.0;
    assert!((key_edge - frame_edge - stepper.gap).abs() < 1e-5);
    // the two keys mirror each other, and the whole is as wide as from one's edge to the other's
    let back = stepper
      .transform(0.0, 0.0, Arrow::Back, false)
      .w_axis
      .truncate();
    let on = stepper
      .transform(0.0, 0.0, Arrow::On, false)
      .w_axis
      .truncate();
    assert_eq!(back, -on);
    assert_eq!(stepper.width(), (on.x + stepper.key.width / 2.0) * 2.0);
    // a held key goes in, and its sign with it
    let held = stepper.sign(0.0, 0.0, Arrow::On, true).w_axis.z;
    assert!(held < stepper.key.top() && held > 0.0);
  }

  #[test]
  fn each_sign_fills_between_a_third_and_a_half_of_its_key() {
    let stepper = Stepper::new(60.0, 22.0, 7.0);
    let [minus, plus] = stepper.signs();
    let reach = |mesh: &Mesh, axis: usize| {
      (mesh.vertices.iter())
        .map(|vertex| vertex.position[axis].abs())
        .fold(0.0, f32::max)
    };
    let share = reach(&plus, 0) * 2.0 / stepper.key.width;
    assert!((1.0 / 3.0..=0.5).contains(&share), "{share}");
    // the plus is as tall as it is wide, and the minus as wide as the plus
    assert!((reach(&plus, 1) - reach(&plus, 0)).abs() < 1e-5);
    assert_eq!(reach(&minus, 0), reach(&plus, 0));
  }
}
