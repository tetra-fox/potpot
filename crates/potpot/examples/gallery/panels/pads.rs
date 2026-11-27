//! XY pads, a puck on a surface in a rim.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::Pad;
use potpot::{Look, Material, Object, Scene};

use super::{Catalog, button_rubber, dim};

/// Returns a wide pad and a square one on a dark panel.
///
/// The wide pad's puck is lit and the square one's dark, each off its pad's middle. The light
/// is dimmed so that what glows shows.
pub(super) fn pads(look: Look) -> Scene {
  let mut catalog = Catalog::dark("PADS", [2, 1], 280.0);
  let cells = catalog.cells.clone();
  let blue = Vec3::new(0.1, 0.5, 1.0);
  // each pad's name and size, where its puck is across and up, and its puck's level
  let pads = [
    ("WIDE", Pad::new(320.0, 210.0), [0.8, 0.7], 1.0),
    ("SQUARE", Pad::new(240.0, 240.0), [0.3, 0.6], 0.0),
  ];
  for ((name, pad, [across, up], level), cell) in pads.into_iter().zip(&cells) {
    let (x, y) = (cell.middle.x, cell.middle.y);
    let on_panel = Mat4::from_translation(Vec3::new(x, y, 0.0));
    let on_pad = pad.transform(x, y, across, up);
    catalog.build.objects.extend([
      Object {
        mesh: Arc::new(pad.surface()),
        transform: on_panel,
        material: Material::glossy(Vec3::splat(0.02)),
      },
      Object {
        mesh: Arc::new(pad.rim()),
        transform: on_panel,
        material: Material::chrome(Vec3::splat(0.7)),
      },
      Object {
        mesh: Arc::new(pad.puck()),
        transform: on_pad,
        material: button_rubber().lit(blue, level, pad.puck.emission_radius()),
      },
    ]);
    if level > 0.0 {
      let puck = on_pad.transform_point3(Vec3::ZERO);
      catalog
        .build
        .lamps
        .push(pad.puck.lamp(puck.x, puck.y, blue, level));
    }
    catalog.label(*cell, name);
  }

  catalog.finish(look, dim())
}
