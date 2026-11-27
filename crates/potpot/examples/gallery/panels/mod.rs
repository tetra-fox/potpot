//! The gallery's panels, each a scene of potpot's parts.
//!
//! The gallery draws one of them to a file, and the timing example turns a knob of one.

#![allow(dead_code, reason = "the timing example draws one panel of these")]

mod faders;
mod industrial;
pub mod knobs;
mod lamp;
mod lights;
mod materials;
mod retro;
mod rings;
mod screens;
#[path = "../../shared/mod.rs"]
pub mod shared;

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts;
use potpot::{Lamp, Light, Look, Material, Mesh, Object, Scene};

use self::shared::{ALUMINUM, BLACK, SEAT, lettering};

pub const SIZE: [u32; 2] = [1024, 640];
/// The names of the panels, as a message lists them.
pub const PANELS: &str =
  "knobs, faders, lights, lamp, rings, screens, retro, industrial, materials";

/// Returns a screw for each corner, each turned its own way.
fn screws() -> [Object; 4] {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let screw = Arc::new(parts::screw(8.0));
  let inset = 18.0;
  [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ]
  .map(|(x, y)| Object {
    mesh: Arc::clone(&screw),
    transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
    material: Material::chrome(Vec3::splat(0.75)),
  })
}

/// Returns the scene of a panel that fills the picture.
///
/// The panel is seen leaning a little, under the plain light and with no lamps.
fn framed(look: Look, objects: Vec<Object>) -> Scene {
  Scene {
    size: Vec2::new(SIZE[0] as f32, SIZE[1] as f32),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    objects,
    lamps: Vec::new(),
  }
}

/// A panel being put together.
///
/// It holds what stands where and what is lit.
#[derive(Default)]
struct Build {
  objects: Vec<Object>,
  lamps: Vec<Lamp>,
}

impl Build {
  /// Adds a mesh that is placed as often as wanted.
  fn place(&mut self, mesh: &Arc<Mesh>, transform: Mat4, material: Material) {
    self.objects.push(Object {
      mesh: Arc::clone(mesh),
      transform,
      material,
    });
  }

  /// Adds a mesh that goes in one place only.
  fn put(&mut self, mesh: Mesh, transform: Mat4, material: Material) {
    self.place(&Arc::new(mesh), transform, material);
  }

  /// Adds lettering centered on a point at a height off the panel.
  fn letters(&mut self, text: &str, at: Vec3, tall: f32, material: Material) {
    self.put(lettering(text, tall), Mat4::from_translation(at), material);
  }
}

/// Returns the panel of a name, as a scene under a look.
pub fn scene(name: &str) -> Option<fn(Look) -> Scene> {
  Some(match name {
    "knobs" => knobs::knobs,
    "faders" => faders::faders,
    "lights" => lights::lights,
    "lamp" => lamp::lamp,
    "rings" => rings::rings,
    "screens" => screens::screens,
    "retro" => retro::retro,
    "industrial" => industrial::industrial,
    "materials" => materials::materials,
    _ => return None,
  })
}
