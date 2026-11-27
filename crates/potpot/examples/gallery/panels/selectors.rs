//! Segmented selectors and steppers.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{Arrow, Selector, Stepper};
use potpot::{Look, Material, Scene};

use super::{BLACK, Catalog, KEY_LETTERS, SEAT, at, button_rubber, dim, key, key_ink, lettering};

/// Returns selectors and steppers on a dark panel.
///
/// The selectors have four, three and two choices. A selector's chosen key is in and lit. The last
/// selector is lit in no color, and the wide stepper has its right key held. The light is dimmed so
/// that what glows shows.
pub(super) fn selectors(look: Look) -> Scene {
  let mut catalog = Catalog::dark("SELECTORS", [3, 2], 70.0);
  let cells = catalog.cells.clone();
  let chrome = Material::chrome(Vec3::splat(0.7));

  // each selector's name, its choices, the one chosen, and whether that one is lit
  let selectors = [
    ("FOUR WAY", &["1X", "2X", "4X", "8X"][..], 2, true),
    ("THREE WAY", &["LOW", "MID", "HIGH"][..], 1, true),
    ("TWO WAY", &["OFF", "ON"][..], 1, true),
    ("UNLIT", &["1X", "2X", "4X", "8X"][..], 0, false),
  ];
  let places = [cells[0], cells[1], cells[2], cells[5]];
  for ((name, choices, chosen, lit), cell) in selectors.into_iter().zip(places) {
    let selector = Selector::new(choices.len(), 64.0, 30.0, 6.0);
    let (x, y) = (cell.middle.x, cell.middle.y);
    let row = selector.row;
    let build = &mut catalog.build;
    build.put(selector.frame(), at(cell.middle), chrome);
    let body = Arc::new(row.body());
    for (choice, written) in choices.iter().enumerate() {
      let chosen = choice == chosen;
      let material = if chosen && !lit {
        button_rubber()
      } else {
        key(chosen)
      };
      build.place(&body, row.transform(x, y, choice, chosen), material);
      let letters = lettering(written, KEY_LETTERS);
      build.put(letters, row.name(x, y, choice, chosen), key_ink());
    }
    catalog.label(cell, name);
  }

  // each stepper's name, the width of its window, its value, and the key held, if one is
  let steppers = [
    ("STEPPER", 56.0, "8", None),
    ("WIDE STEPPER", 112.0, "16 VOICES", Some(Arrow::On)),
  ];
  let amber = Vec3::new(1.0, 0.66, 0.14);
  for ((name, width, value, held), cell) in steppers.into_iter().zip(&cells[3..5]) {
    let stepper = Stepper::new(width, 24.0, 7.0);
    let (x, y) = (cell.middle.x, cell.middle.y);
    let build = &mut catalog.build;
    build.put(stepper.frame(), at(cell.middle), Material::glossy(BLACK));
    build.put(
      stepper.glass(),
      at(cell.middle),
      Material::pane(Vec3::splat(0.01)),
    );
    // the value lit on the glass, as a display shows it
    let shown = Mat4::from_translation(Vec3::new(x, y, 0.4));
    build.put(lettering(value, 10.0), shown, Material::lamp(amber));
    let (body, seat) = (Arc::new(stepper.body()), Arc::new(stepper.seat(2.0)));
    let signs = stepper.signs().map(Arc::new);
    for (arrow, sign) in [Arrow::Back, Arrow::On].into_iter().zip(&signs) {
      let pressed = held == Some(arrow);
      let middle = cell.middle + stepper.middle(arrow);
      build.place(&seat, at(middle), Material::matte(SEAT));
      build.place(
        &body,
        stepper.transform(x, y, arrow, pressed),
        button_rubber(),
      );
      build.place(sign, stepper.sign(x, y, arrow, pressed), key_ink());
    }
    catalog.label(*cell, name);
  }

  catalog.finish(look, dim())
}
