//! push switches: a sprung plunger in a collar

use glam::{Mat4, Vec3};

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

/// what a push switch does when it is pushed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Push {
  /// each push turns it over: in and on, then out and off. it stays most of the way in
  /// while it is on
  Latching,
  /// it is on only while it is held, like any button
  Momentary,
}

impl Push {
  /// how far in the plunger of such a switch is, from zero for out to one for all the way:
  /// all the way while it is `held`, and most of the way while a latching switch is `on`
  pub fn depth(self, on: bool, held: bool) -> f32 {
    match (held, self, on) {
      (true, ..) => 1.0,
      (false, Push::Latching, true) => 0.7,
      (false, Push::Latching, false) | (false, Push::Momentary, _) => 0.0,
    }
  }

  /// whether such a switch is on after a push is let go, given whether it was on before. a
  /// momentary switch is never on once it is let go
  pub fn released(self, was_on: bool) -> bool {
    self == Push::Latching && !was_on
  }
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
  fn a_latching_switch_stays_in_and_a_momentary_one_comes_back() {
    let switch = PushSwitch::square(24.0, 14.0);
    // held, either is all the way in
    assert_eq!(Push::Latching.depth(false, true), 1.0);
    assert_eq!(Push::Momentary.depth(false, true), 1.0);
    // let go, a latching one that is on stays most of the way in and the rest are out
    let latched = Push::Latching.depth(true, false);
    assert!(latched > 0.5 && latched < 1.0);
    assert_eq!(Push::Latching.depth(false, false), 0.0);
    assert_eq!(Push::Momentary.depth(true, false), 0.0);
    // each push turns a latching one over
    assert!(Push::Latching.released(false));
    assert!(!Push::Latching.released(true));
    assert!(!Push::Momentary.released(false));
    // even all the way in, the plunger still stands over its collar
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
