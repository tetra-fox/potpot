//! small round indicator lights

use glam::Vec3;

use super::button::Button;
use super::outline::Outline;
use crate::material::Material;
use crate::mesh::Mesh;

/// a small round light with a domed top, like the indicator beside a switch
pub fn led(diameter: f32) -> Mesh {
  Button {
    roll: 0.9,
    crown: 0.25,
    ..Button::round(diameter, diameter * 0.32)
  }
  .body()
}

/// the dark ring a small round light sits in, lying on the panel
pub fn led_seat(diameter: f32) -> Mesh {
  Outline::new(diameter, diameter, diameter / 2.0)
    .grown(diameter * 0.12)
    .strip()
}

/// what a small light of a color is made of at a level from zero for dark to one for lit:
/// dark tinted plastic that glows from its middle
pub fn led_material(color: Vec3, level: f32, diameter: f32) -> Material {
  let dark = Material::pipe();
  Material {
    color: dark.color * 0.6 + color * 0.07,
    ..dark
  }
  .lit(color, level, diameter * 0.45)
}
