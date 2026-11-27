//! Every knob style.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Knob, Top};
use potpot::{Anisotropy, Light, Look, Material, Scene, Textures};

use super::{ALUMINUM, BLACK, Catalog, cells};

/// The way a knob of the gallery is marked.
#[derive(Clone, Copy)]
enum Marker {
  Line,
  Dot,
  /// No mark, for a knob whose shape says where it points.
  None,
}

/// A knob of the gallery.
struct Shown {
  name: &'static str,
  knob: Knob,
  material: Material,
  marker: Marker,
  /// The color of its mark.
  mark: Vec3,
  /// What its face is inlaid with, if anything.
  cap: Option<Material>,
}

/// The knobs' grid, in columns and rows.
const GRID: [usize; 2] = [5, 3];
/// The height of each knob's cell over its label.
///
/// Three rows of it fill the page.
const ROOM: f32 = 156.0;

/// Returns every knob style in rows of five, each over a scale.
pub fn knobs(look: Look) -> Scene {
  let (radius, tall) = (58.0, 42.0);
  let white = Vec3::splat(0.86);
  let aluminum = Material::spun(ALUMINUM);
  let shown = |name, knob, material, marker| Shown {
    name,
    knob,
    material,
    marker,
    mark: white,
    cap: None,
  };
  let knobs = [
    shown(
      "ROUND",
      Knob::round(radius, tall),
      // turned walnut, whose grain wraps around the knob
      Material {
        anisotropy: Anisotropy::Spun(0.0),
        textures: Some(Textures::Walnut(90.0)),
        ..Material::walnut()
      },
      Marker::Line,
    ),
    shown(
      "FLUTED",
      Knob::fluted(radius, tall),
      Material::glossy(BLACK),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: BLACK,
      ..shown(
        "SKIRTED",
        Knob::skirted(radius, tall),
        Material::glossy(BLACK),
        Marker::Line,
      )
    },
    Shown {
      mark: BLACK,
      ..shown(
        "KNURLED",
        Knob::knurled(radius * 0.85, tall),
        aluminum,
        Marker::Line,
      )
    },
    shown(
      "DISHED",
      Knob::dished(radius, tall),
      Material::satin(Vec3::new(0.50, 0.08, 0.07)),
      Marker::Dot,
    ),
    shown(
      "DIMPLED",
      Knob::dimpled(radius, tall * 0.55),
      Material::satin(Vec3::new(0.74, 0.72, 0.66)),
      Marker::None,
    ),
    shown(
      "FACETED",
      Knob::faceted(radius, tall),
      Material::glossy(Vec3::new(0.06, 0.13, 0.26)),
      Marker::Line,
    ),
    shown(
      "CHICKEN HEAD",
      Knob::chicken_head(radius, tall),
      Material::glossy(BLACK),
      Marker::Line,
    ),
    shown(
      "BAR",
      Knob::bar(radius, tall * 1.2),
      Material::glossy(BLACK),
      Marker::Line,
    ),
    shown(
      "BIG WALKER",
      Knob::big_walker(radius, tall * 0.8),
      Material::glossy(Vec3::new(0.70, 0.36, 0.04)),
      Marker::Line,
    ),
    shown(
      "RIDGE",
      Knob::ridge(radius, tall * 1.25),
      Material::glossy(BLACK),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: BLACK,
      ..shown(
        "CAPPED",
        Knob {
          skirt: 0.0,
          top: Top::Flat,
          ..Knob::knurled(radius * 0.8, tall * 1.1)
        },
        Material::glossy(BLACK),
        Marker::Line,
      )
    },
    shown(
      "HEX",
      Knob::hex(radius, tall * 1.1),
      Material::glossy(BLACK),
      Marker::Line,
    ),
    Shown {
      cap: Some(aluminum),
      mark: BLACK,
      ..shown(
        "RIBBED",
        Knob::ribbed(radius * 0.8, tall * 1.1),
        Material::satin(BLACK),
        Marker::Line,
      )
    },
    Shown {
      cap: Some(Material::spun(Vec3::new(0.80, 0.66, 0.36))),
      mark: BLACK,
      ..shown(
        "CONICAL",
        Knob::conical(radius, tall),
        Material::glossy(BLACK),
        Marker::Line,
      )
    },
  ];

  let scale = Arc::new(parts::ticks(
    11,
    parts::KNOB_SWEEP,
    radius + 6.0,
    radius + 13.0,
    5,
  ));
  let engraved = Material::ink(Vec3::splat(0.08));
  let plate = Material::brushed(Vec3::new(0.55, 0.57, 0.6));
  let mut catalog = Catalog::new("KNOBS", plate, engraved, GRID, ROOM);
  for (index, (shown, cell)) in knobs.into_iter().zip(catalog.cells.clone()).enumerate() {
    let (x, y) = (cell.middle.x, cell.middle.y);
    let transform = turned(index, turn(index));
    let build = &mut catalog.build;
    build.place(
      &scale,
      Mat4::from_translation(Vec3::new(x, y, 0.0)),
      Material::ink(Vec3::splat(0.85)),
    );
    build.put(shown.knob.body(), transform, shown.material);
    if let Some(cap) = shown.cap {
      build.put(shown.knob.cap(0.86), transform, cap);
    }
    match shown.marker {
      Marker::Line => build.put(shown.knob.line(), transform, Material::matte(shown.mark)),
      Marker::Dot => build.put(shown.knob.dot(), transform, Material::matte(shown.mark)),
      Marker::None => {}
    }
    // the ends of its scale numbered, and its name under it
    build.letters(
      "0",
      Vec3::new(x - radius - 4.0, y + radius + 14.0, 0.0),
      7.0,
      engraved,
    );
    build.letters(
      "10",
      Vec3::new(x + radius + 6.0, y + radius + 14.0, 0.0),
      7.0,
      engraved,
    );
    catalog.label(cell, shown.name);
  }
  catalog.finish(look, Light::default())
}

/// Returns the middle of a knob of the rows.
fn middle(index: usize) -> Vec2 {
  cells(GRID, ROOM)[index].middle
}

/// Returns the turn a knob of the rows is drawn at.
///
/// The turn runs from zero to one.
pub fn turn(index: usize) -> f32 {
  0.12 + 0.055 * index as f32
}

/// Returns the transform of a knob of the rows at a turn.
///
/// The turn runs from zero to one. The parts that turn with the knob have it, and nothing else
/// on the panel does.
pub fn turned(index: usize, turn: f32) -> Mat4 {
  let at = middle(index);
  parts::knob_transform(at.x, at.y, turn, parts::KNOB_SWEEP)
}
