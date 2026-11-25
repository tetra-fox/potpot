//! the gallery of knob styles

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Knob, Top};
use potpot::{Look, Material, Object, Scene};

use crate::{ALUMINUM, BLACK, SIZE, framed, lettering, screws};

/// how a knob of the gallery is marked
#[derive(Clone, Copy)]
enum Marker {
  Line,
  Dot,
  /// its own shape says where it points
  None,
}

/// a knob of the gallery: what it is called, its shape, what it is made of, what marks it
/// and in which color, and what its face is inlaid with if anything
struct Shown {
  name: &'static str,
  knob: Knob,
  material: Material,
  marker: Marker,
  mark: Vec3,
  cap: Option<Material>,
}

/// every knob style in rows of five, each over a scale
pub(super) fn knobs(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
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
      Material::glossy(BLACK),
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
      "LEVER",
      Knob::lever(radius, tall * 0.8),
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
  let mut objects = vec![Object {
    mesh: Arc::new(parts::plate(width, height, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::brushed(Vec3::new(0.30, 0.32, 0.36)),
  }];
  for (index, shown) in knobs.into_iter().enumerate() {
    let (x, y) = (
      112.0 + 200.0 * (index % 5) as f32,
      112.0 + 208.0 * (index / 5) as f32,
    );
    let turn = 0.12 + 0.055 * index as f32;
    let transform = parts::knob_transform(x, y, turn, parts::KNOB_SWEEP);
    objects.push(Object {
      mesh: Arc::clone(&scale),
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: Material::matte(Vec3::splat(0.85)),
    });
    let mut part = |mesh: potpot::Mesh, material: Material| {
      objects.push(Object {
        mesh: Arc::new(mesh),
        transform,
        material,
      });
    };
    part(shown.knob.body(), shown.material);
    if let Some(cap) = shown.cap {
      part(shown.knob.cap(0.86), cap);
    }
    match shown.marker {
      Marker::Line => part(shown.knob.line(), Material::matte(shown.mark)),
      Marker::Dot => part(shown.knob.dot(), Material::matte(shown.mark)),
      Marker::None => {}
    }
    // its name engraved under it, and the ends of its scale numbered
    let engraved = Material::matte(Vec3::splat(0.08));
    let mut letters = |text: &str, x: f32, y: f32, tall: f32| {
      objects.push(Object {
        mesh: Arc::new(lettering(text, tall)),
        transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
        material: engraved,
      });
    };
    letters(shown.name, x, y + radius + 34.0, 9.0);
    letters("0", x - radius - 4.0, y + radius + 14.0, 7.0);
    letters("10", x + radius + 6.0, y + radius + 14.0, 7.0);
  }
  objects.extend(screws());
  framed(look, objects)
}
