//! push switches: a sprung plunger in a collar

use glam::{Mat4, Vec3};
pub use potpot_controls::interaction::Push;

use super::button::Button;
use super::outline::band;
use crate::mesh::Mesh;

/// a push switch: a plunger standing in a collar on the panel, sprung so that it comes back
/// out by itself
#[derive(Clone, Copy, Debug)]
pub struct PushSwitch {
  /// the plunger, as tall as it stands out of the panel when it is out
  pub plunger: Button,
  /// how far the plunger goes in when it is pushed all the way
  pub travel: f32,
  /// how wide the collar around the plunger is
  pub collar: f32,
  /// how high the collar stands
  pub collar_height: f32,
}

impl PushSwitch {
  /// a square plunger in a square collar
  pub fn square(side: f32, height: f32) -> Self {
    Self {
      plunger: Button {
        corner: 0.18,
        roll: 0.12,
        crown: -0.04,
        ..Button::pad(side, height)
      },
      travel: height * 0.45,
      collar: side * 0.16,
      collar_height: height * 0.4,
    }
  }

  /// a round plunger in a round collar
  pub fn round(diameter: f32, height: f32) -> Self {
    Self {
      plunger: Button {
        roll: 0.14,
        crown: 0.05,
        ..Button::round(diameter, height)
      },
      ..Self::square(diameter, height)
    }
  }

  /// the plunger, out
  pub fn plunger(&self) -> Mesh {
    self.plunger.body()
  }

  /// the collar, a ring around the plunger with a hole for it to move in
  pub fn collar(&self) -> Mesh {
    // a hair of room around the plunger
    let cut = self.collar_height.min(self.collar) * 0.25;
    let plunger = self.plunger.outline();
    band(
      plunger.grown(self.collar),
      plunger.grown(self.collar * 0.08),
      self.collar_height,
      cut,
    )
  }

  /// where the plunger sits for a switch at (x, y) that is `depth` of the way in, which
  /// [`Push::depth`] tells
  pub fn transform(&self, x: f32, y: f32, depth: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, -self.travel * depth.clamp(0.0, 1.0)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_plunger_all_the_way_in_still_stands_over_its_collar() {
    let switch = PushSwitch::square(24.0, 14.0);
    let top = switch
      .transform(0.0, 0.0, 1.0)
      .transform_point3(Vec3::Z * switch.plunger.height);
    assert!(
      top.z > switch.collar_height,
      "{} under {}",
      top.z,
      switch.collar_height
    );
  }

  #[test]
  fn a_collar_is_a_ring_around_its_plunger() {
    for switch in [
      PushSwitch::square(24.0, 14.0),
      PushSwitch::round(24.0, 14.0),
    ] {
      let outline = |mesh: &Mesh| {
        let reach = |vertex: &crate::Vertex| Vec3::from(vertex.position).truncate().length();
        let reaches: Vec<f32> = mesh.vertices.iter().map(reach).collect();
        (
          reaches.iter().copied().fold(f32::MAX, f32::min),
          reaches.iter().copied().fold(f32::MIN, f32::max),
        )
      };
      let collar = switch.collar();
      let (inside, outside) = outline(&collar);
      // nothing of it is where the plunger's middle is, and it reaches past the plunger
      assert!(inside > 11.9, "{switch:?} collar comes in to {inside}");
      assert!(outside > outline(&switch.plunger()).1 + switch.collar * 0.9);
      for vertex in &collar.vertices {
        assert!(Vec3::from(vertex.normal).is_normalized());
        assert!((0.0..=switch.collar_height + 1e-4).contains(&vertex.position[2]));
      }
    }
  }
}
