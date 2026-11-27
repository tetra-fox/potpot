//! Rings, arcs and stops of light around knobs.

use std::sync::Arc;

use potpot::glam::Vec3;
use potpot::parts::{self, Knob, LedArc, LedRing, LedStops};
use potpot::{Light, Look, Material, Scene};

use super::{BLACK, Catalog};

/// Returns each kind of light around a knob.
///
/// The knobs are on a dark panel. The first ring shows a level from the start of the turn, the
/// second a level either way of the middle, and the third how far another value is from where its
/// knob points. After them are an unbroken arc lit from its start and a light at each of a knob's
/// stops. The light is dimmed so that what glows shows.
pub(super) fn rings(look: Look) -> Scene {
  let knob = Knob::round(40.0, 30.0);
  let ring = LedRing::new(21, knob.radius + 16.0, 6.0);
  let arc = LedArc::new(knob.radius + 16.0, 4.0);
  let stops = LedStops::new(5, knob.radius + 16.0, 7.0);
  let (body, line) = (Arc::new(knob.body()), Arc::new(knob.line()));
  let (segment, seat) = (Arc::new(ring.segment()), Arc::new(ring.seat()));
  let arc_meshes = [Arc::new(arc.seat()), Arc::new(arc.lens())];
  let (stop, stop_seat) = (Arc::new(stops.light()), Arc::new(stops.seat()));
  let ink = Material::ink(Vec3::splat(0.8));
  let plate = Material::powder_coat(Vec3::new(0.075, 0.08, 0.09));
  let mut catalog = Catalog::new("RINGS", plate, ink, [5, 1], 160.0);
  let cells = catalog.cells.clone();

  // each kind's name, where its knob points, and what it lights in what color
  let rings = [
    ("RING", 0.7, [0.0, 0.7], Vec3::new(0.1, 0.95, 0.3)),
    ("CENTERED RING", 0.3, [0.5, 0.3], Vec3::new(1.0, 0.6, 0.1)),
    ("RANGE RING", 0.4, [0.4, 0.65], Vec3::new(0.2, 0.6, 1.0)),
  ];
  for ((name, turn, run, color), cell) in rings.into_iter().zip(&cells) {
    let (x, y) = (cell.middle.x, cell.middle.y);
    catalog.build.objects.extend(ring.seats(&seat, x, y));
    catalog
      .build
      .objects
      .extend(ring.lights(&segment, x, y, run, color));
    knob_at(&mut catalog, [&body, &line], x, y, turn);
    catalog.label(*cell, name);
  }
  let cyan = Vec3::new(0.2, 0.85, 1.0);
  let (x, y) = (cells[3].middle.x, cells[3].middle.y);
  let lit = arc.lights(&arc_meshes, x, y, [0.0, 0.62], cyan);
  catalog.build.objects.extend(lit);
  knob_at(&mut catalog, [&body, &line], x, y, 0.62);
  catalog.label(cells[3], "ARC");
  let (x, y) = (cells[4].middle.x, cells[4].middle.y);
  catalog.build.objects.extend(stops.seats(&stop_seat, x, y));
  let at = stops.turn(1);
  catalog
    .build
    .objects
    .extend(stops.lights(&stop, x, y, at, Vec3::new(1.0, 0.2, 0.1)));
  knob_at(&mut catalog, [&body, &line], x, y, at);
  catalog.label(cells[4], "STOPS");

  let light = Light {
    color: Vec3::splat(0.6),
    ..Light::default()
  };
  catalog.finish(look, light)
}

/// Adds a glossy black knob with a white line.
///
/// The knob is at (`x`, `y`), turned to `turn`.
fn knob_at(
  catalog: &mut Catalog,
  [body, line]: [&Arc<potpot::Mesh>; 2],
  x: f32,
  y: f32,
  turn: f32,
) {
  let turned = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
  catalog.build.place(body, turned, Material::glossy(BLACK));
  catalog
    .build
    .place(line, turned, Material::matte(Vec3::splat(0.86)));
}
