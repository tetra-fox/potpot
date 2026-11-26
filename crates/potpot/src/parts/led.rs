//! Small round indicator lights.

use glam::{Vec2, Vec3};

use super::PAINT;
use super::button::Button;
use super::legend::{END, add_stroke};
use super::outline::{Outline, STRIP};
use crate::material::Material;
use crate::mesh::Mesh;
use crate::scene::Lamp;

/// Returns a small round light with a domed top.
///
/// It is like the indicator beside a switch.
pub fn led(diameter: f32) -> Mesh {
  Button {
    roll: 0.9,
    crown: 0.25,
    ..Button::round(diameter, diameter * 0.32)
  }
  .body()
}

/// Returns the dark ring a small round light sits in.
///
/// The ring lies on the panel.
pub fn led_seat(diameter: f32) -> Mesh {
  Outline::new(diameter, diameter, diameter / 2.0)
    .grown(diameter * 0.12)
    .strip()
}

/// Returns the material of a small light of a color at a level.
///
/// `level` runs from zero for dark to one for lit. The [`Material`] is dark tinted plastic that
/// glows from its middle.
pub fn led_material(color: Vec3, level: f32, diameter: f32) -> Material {
  let dark = Material::pipe();
  Material {
    color: dark.color * 0.6 + color * 0.07,
    ..dark
  }
  .lit(color, level, diameter * 0.45)
}

/// Returns the light a small light throws on what is around it.
///
/// The [`Lamp`] is for a light of `color` at `level`. It shines from `at`, over the light's middle,
/// out to `reach` away.
pub fn led_lamp(at: Vec3, color: Vec3, level: f32, diameter: f32, reach: f32) -> Lamp {
  Lamp {
    at,
    color: color * level * 2.5,
    reach,
    radius: diameter / 2.0,
  }
}

/// Returns a thin line of light inlaid flush with the panel.
///
/// The line runs through the points of `path` in turn and is `weight` wide, with round ends. It
/// lies on the panel, to be drawn in a material that lights itself evenly.
pub fn light_line(path: &[Vec2], weight: f32) -> Mesh {
  strokes(path, weight, PAINT)
}

/// Returns the dark groove a line of light is set in.
///
/// The groove follows the line [`light_line`] makes along `path`, `margin` wider to each side.
pub fn light_line_seat(path: &[Vec2], weight: f32, margin: f32) -> Mesh {
  strokes(path, weight + margin * 2.0, STRIP)
}

/// Returns strokes `weight` wide between each point of `path` and the next, at `height`.
fn strokes(path: &[Vec2], weight: f32, height: f32) -> Mesh {
  let mut mesh = Mesh::default();
  for pair in path.windows(2) {
    add_stroke(&mut mesh, [pair[0], pair[1]], weight, height, END);
  }
  mesh
}
