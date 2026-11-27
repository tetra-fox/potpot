//! Rings of lights around knobs.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Knob, LedRing};
use potpot::{Light, Look, Material, Object, Scene};

use super::{BLACK, SIZE, framed, lettering, screws};

/// Returns knobs in rings of lights on a dark panel.
///
/// The first ring shows a level from the start of the turn, the second a level either way of the
/// middle, the third how far another value is from where its knob points, and the last is dark.
/// The light is dimmed so that what glows shows.
pub(super) fn rings(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let knob = Knob::round(40.0, 30.0);
  let ring = LedRing::new(21, knob.radius + 16.0, 6.0);
  let (body, line) = (Arc::new(knob.body()), Arc::new(knob.line()));
  let (segment, seat) = (Arc::new(ring.segment()), Arc::new(ring.seat()));
  let mut objects = vec![Object {
    mesh: Arc::new(parts::plate(width, height, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.075, 0.08, 0.09)),
  }];
  let ink = Material::ink(Vec3::splat(0.8));
  // each knob's name, where it points and the run its ring lights, in what color
  let shown = [
    ("LEVEL", 0.7, [0.0, 0.7], Vec3::new(0.1, 0.95, 0.3)),
    ("PAN", 0.3, [0.5, 0.3], Vec3::new(1.0, 0.6, 0.1)),
    ("SEND", 0.4, [0.4, 0.65], Vec3::new(0.2, 0.6, 1.0)),
    ("OFF", 0.6, [0.6, 0.6], Vec3::new(1.0, 0.2, 0.1)),
  ];
  for (index, (name, turn, run, color)) in shown.into_iter().enumerate() {
    let (x, y) = (182.0 + 220.0 * index as f32, height / 2.0);
    objects.extend(ring.seats(&seat, x, y));
    objects.extend(ring.lights(&segment, x, y, run, color));
    let turned = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    objects.extend([
      Object {
        mesh: Arc::clone(&body),
        transform: turned,
        material: Material::glossy(BLACK),
      },
      Object {
        mesh: Arc::clone(&line),
        transform: turned,
        material: Material::matte(Vec3::splat(0.86)),
      },
      Object {
        mesh: Arc::new(lettering(name, 12.0)),
        transform: Mat4::from_translation(Vec3::new(x, y + ring.radius + 40.0, 0.0)),
        material: ink,
      },
    ]);
  }

  objects.extend(screws());
  Scene {
    light: Light {
      color: Vec3::splat(0.6),
      ambient: Vec3::splat(0.2),
      ..Light::default()
    },
    ..framed(look, objects)
  }
}
