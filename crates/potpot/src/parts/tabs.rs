//! A row of tabs, each a key that shows a page.

use glam::{Mat4, Vec2, Vec3};
use potpot_controls::interaction::Push;

use super::button::Button;
use crate::mesh::Mesh;

/// A row of tabs, each a low key that shows a page.
///
/// The keys stand side by side along x, and the row's middle is at the origin. The key of the
/// page shown stays in, as a latching button that is on does.
#[derive(Clone, Copy, Debug)]
pub struct Tabs {
  /// The key of each tab.
  pub key: Button,
  /// The count of tabs.
  pub count: usize,
  /// The room between two keys, in points.
  pub gap: f32,
}

impl Tabs {
  /// Creates a row of `count` keys.
  ///
  /// Each key is `width` across, `length` down and `height` tall, and the keys are as far apart
  /// as they are tall.
  pub fn new(count: usize, width: f32, length: f32, height: f32) -> Self {
    Self {
      key: Button::key(width, length, height),
      count,
      gap: height,
    }
  }

  /// Returns the width of the whole row in points.
  pub fn width(&self) -> f32 {
    let count = self.count as f32;
    count * self.key.width + (count - 1.0).max(0.0) * self.gap
  }

  /// Returns the middle of a tab's key, from the row's middle.
  pub fn middle(&self, tab: usize) -> Vec2 {
    let left = (self.key.width - self.width()) / 2.0;
    Vec2::new(left + (self.key.width + self.gap) * tab as f32, 0.0)
  }

  /// Returns the mesh of a key.
  pub fn body(&self) -> Mesh {
    self.key.body()
  }

  /// Returns the gap around a key as a dark strip.
  ///
  /// The strip is `gap` wider than the key all around.
  pub fn seat(&self, gap: f32) -> Mesh {
    self.key.seat(gap)
  }

  /// Returns the transform of a tab's key.
  ///
  /// The row's middle is at (`x`, `y`), and the key stays in while its page is `shown`.
  pub fn transform(&self, x: f32, y: f32, tab: usize, shown: bool) -> Mat4 {
    let at = Vec2::new(x, y) + self.middle(tab);
    self
      .key
      .transform(at.x, at.y, Push::Latching.depth(shown, false))
  }

  /// Returns the transform of the name printed on a tab's key.
  ///
  /// The name is lettering made as [`legend`](super::legend) makes it, which goes on the key's
  /// top and in with it. The key is in a row whose middle is at (`x`, `y`).
  pub fn name(&self, x: f32, y: f32, tab: usize, shown: bool) -> Mat4 {
    self.transform(x, y, tab, shown) * Mat4::from_translation(Vec3::Z * self.key.top())
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::material::Material;
  use crate::parts::INK;
  use crate::scene::{Light, Look, Object, Scene};

  /// Returns a scene of a row of tabs under a leaning view.
  ///
  /// Each tab is its seat and then its key, and the row's middle is at `middle`.
  fn row(tabs: &Tabs, middle: Vec2, shown: usize) -> Scene {
    let (body, seat) = (Arc::new(tabs.body()), Arc::new(tabs.seat(1.5)));
    let objects = (0..tabs.count).flat_map(|tab| {
      let at = middle + tabs.middle(tab);
      [
        Object {
          mesh: Arc::clone(&seat),
          transform: Mat4::from_translation(at.extend(0.0)),
          material: Material::matte(Vec3::splat(0.01)),
        },
        Object {
          mesh: Arc::clone(&body),
          transform: tabs.transform(middle.x, middle.y, tab, tab == shown),
          material: Material::silicone(Vec3::splat(0.6)),
        },
      ]
    });
    Scene {
      size: Vec2::new(600.0, 100.0),
      heights: [-8.0, 64.0],
      lean: Vec2::new(0.0, -0.25),
      light: Light::default(),
      look: Look::default(),
      objects: objects.collect(),
      lamps: Vec::new(),
    }
  }

  #[test]
  fn the_row_is_as_wide_as_its_keys_and_gaps_and_centered() {
    let tabs = Tabs::new(4, 80.0, 28.0, 6.0);
    assert_eq!(tabs.width(), 4.0 * 80.0 + 3.0 * 6.0);
    assert_eq!(tabs.middle(0).x, -tabs.width() / 2.0 + 40.0);
    assert_eq!(tabs.middle(3).x, tabs.width() / 2.0 - 40.0);
    assert_eq!(Tabs::new(1, 80.0, 28.0, 6.0).middle(0), Vec2::ZERO);
  }

  #[test]
  fn each_tab_is_picked_where_its_key_is_drawn_and_no_tab_between_them() {
    let tabs = Tabs::new(5, 80.0, 28.0, 6.0);
    let middle = Vec2::new(300.0, 50.0);
    for shown in [0, 2, 4] {
      let scene = row(&tabs, middle, shown);
      for tab in 0..tabs.count {
        let key = middle + tabs.middle(tab);
        // its middle, and in from each end, in and out alike: the key is its second object
        let inside = tabs.key.width / 2.0 - 3.0;
        for off in [0.0, -inside, inside] {
          let at = key + Vec2::new(off, 0.0);
          assert_eq!(scene.pick(at), Some(tab * 2 + 1), "tab {tab} at {at}");
        }
        // the gap after it, which no key covers
        if tab + 1 < tabs.count {
          let gap = key + Vec2::new((tabs.key.width + tabs.gap) / 2.0, 0.0);
          assert_eq!(scene.pick(gap), None, "after tab {tab}");
        }
      }
    }
  }

  #[test]
  fn the_shown_tab_stays_in_and_its_name_on_its_top() {
    let tabs = Tabs::new(3, 80.0, 28.0, 6.0);
    let depth = |tab: usize, shown| tabs.transform(0.0, 0.0, tab, shown).w_axis.z;
    assert_eq!(depth(1, false), 0.0);
    assert!(depth(1, true) < 0.0 && depth(1, true) > -tabs.key.height * 0.25);
    let top = tabs.key.top();
    for shown in [false, true] {
      let name = tabs
        .name(0.0, 0.0, 1, shown)
        .transform_point3(Vec3::Z * INK);
      assert!((name.z - (depth(1, shown) + top + INK)).abs() < 1e-5);
      assert_eq!(name.truncate(), tabs.middle(1));
    }
  }
}
