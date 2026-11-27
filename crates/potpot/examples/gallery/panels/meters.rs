//! Level meters: ladders of lights and a needle meter.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{Meter, Vu};
use potpot::{Light, Look, Material, Object, Scene};

use super::Catalog;

/// Returns each kind of level meter on a dark panel.
///
/// The meters stand at a few levels. The ladders stand in pairs, the first holding a peak, and one
/// more lies on its side. The light is dimmed so that what glows shows.
pub(super) fn meters(look: Look) -> Scene {
  let ink = Material::ink(Vec3::splat(0.8));
  let plate = Material::powder_coat(Vec3::new(0.075, 0.08, 0.09));
  let mut catalog = Catalog::new("METERS", plate, ink, [5, 1], 480.0);
  let cells = catalog.cells.clone();
  let length = 420.0;

  // each kind in a pair, and the level and the peak of each of the pair
  let ladders = [
    (
      "LADDER",
      Meter::ladder(16, 18.0, length),
      [(0.78, Some(0.92)), (0.64, Some(0.80))],
    ),
    (
      "THIN LADDER",
      Meter::ladder(24, 9.0, length),
      [(0.42, None), (1.0, None)],
    ),
    (
      "DOTS",
      Meter::dots(16, 12.0, length),
      [(0.55, None), (0.3, None)],
    ),
  ];
  for ((name, meter, pair), cell) in ladders.into_iter().zip(&cells) {
    let (segment, seat) = (Arc::new(meter.segment()), Arc::new(meter.seat()));
    let foot = cell.middle.y + length / 2.0;
    let apart = meter.width + 14.0;
    for (side, (level, peak)) in [-0.5, 0.5].into_iter().zip(pair) {
      let x = cell.middle.x + side * apart;
      let build = &mut catalog.build;
      build.objects.extend(meter.seats(&seat, x, foot));
      build
        .objects
        .extend(meter.lights(&segment, x, foot, level, peak));
      build.lamps.extend(meter.lamp(x, foot, level));
    }
    catalog.label(*cell, name);
  }

  // a ladder lying on its side, which is a meter turned a quarter, its foot to the left
  let lying = Meter::ladder(14, 20.0, 160.0);
  let (segment, seat) = (Arc::new(lying.segment()), Arc::new(lying.seat()));
  let middle = cells[3].middle;
  let across = Mat4::from_translation(Vec3::new(middle.x - 80.0, middle.y, 0.0))
    * Mat4::from_rotation_z(FRAC_PI_2);
  let turned = |object: Object| Object {
    transform: across * object.transform,
    ..object
  };
  catalog
    .build
    .objects
    .extend(lying.seats(&seat, 0.0, 0.0).map(turned));
  catalog
    .build
    .objects
    .extend(lying.lights(&segment, 0.0, 0.0, 0.7, None).map(turned));
  catalog.label(cells[3], "LYING LADDER");

  // a needle meter, its lamp on
  let middle = cells[4].middle;
  catalog
    .build
    .objects
    .extend(Vu::new(140.0, 76.0).objects(middle.x, middle.y, 0.62, 1.0));
  catalog.label(cells[4], "VU");

  let light = Light {
    color: Vec3::splat(0.6),
    ..Light::default()
  };
  catalog.finish(look, light)
}
