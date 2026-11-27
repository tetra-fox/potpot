//! Every button and push switch, out, on and held.

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{Button, PushSwitch};
use potpot::{Look, Material, Scene};
use potpot_controls::interaction::Push;

use super::{Catalog, SEAT, dim, rubber};

/// The space between the middles of a cell's three specimens.
///
/// It is a share of the cell's width.
const APART: f32 = 0.3;

/// Returns every button shape and push switch, three of each.
///
/// The first of a cell is out and dark, the second on, and the third held all the way in. A
/// button is on while it is lit, and a switch while it is latched in. The light is dimmed so that
/// what glows shows.
pub(super) fn buttons(look: Look) -> Scene {
  let mut catalog = Catalog::dark("BUTTONS AND SWITCHES", [4, 2], 110.0);
  let cells = catalog.cells.clone();
  // each state's place in its cell, whether it is on, and whether it is held
  let states = [(-1.0, false, false), (0.0, true, false), (1.0, true, true)];

  let buttons = [
    ("PAD", Button::pad(60.0, 12.0), Vec3::new(1.0, 0.25, 0.05)),
    ("ROUND", Button::round(46.0, 10.0), Vec3::new(0.1, 0.5, 1.0)),
    (
      "PILL",
      Button::pill(26.0, 60.0, 8.0),
      Vec3::new(0.1, 0.9, 0.3),
    ),
    (
      "KEY",
      Button::key(60.0, 26.0, 7.0),
      Vec3::new(1.0, 0.75, 0.1),
    ),
  ];
  for ((name, shape, color), cell) in buttons.into_iter().zip(&cells) {
    let (body, seat) = (Arc::new(shape.body()), Arc::new(shape.seat(2.5)));
    for (place, on, held) in states {
      let at = cell.middle + Vec2::X * place * cell.room.x * APART;
      let build = &mut catalog.build;
      build.place(
        &seat,
        Mat4::from_translation(at.extend(0.0)),
        Material::matte(SEAT),
      );
      let level = f32::from(u8::from(on));
      let lit = rubber().lit(color, level, shape.emission_radius());
      build.place(
        &body,
        shape.transform(at.x, at.y, f32::from(u8::from(held))),
        lit,
      );
      if on {
        build.lamps.push(shape.lamp(at.x, at.y, color, level));
      }
    }
    catalog.label(*cell, name);
  }

  let amber = Vec3::new(1.0, 0.55, 0.05);
  let cyan = Vec3::new(0.2, 0.85, 1.0);
  let switches = [
    (
      "SQUARE SWITCH",
      PushSwitch::square(30.0, 18.0),
      Plunger::Rubber(amber),
    ),
    (
      "ROUND SWITCH",
      PushSwitch::round(30.0, 18.0),
      Plunger::Black,
    ),
    (
      "BACKLIT SWITCH",
      PushSwitch::backlit(56.0, 26.0, 8.0),
      Plunger::Backlit(cyan),
    ),
  ];
  let chrome = Material::chrome(Vec3::splat(0.7));
  for ((name, switch, plunger), cell) in switches.into_iter().zip(&cells[buttons.len()..]) {
    let (body, collar) = (Arc::new(switch.plunger()), Arc::new(switch.collar()));
    for (place, on, held) in states {
      let at = cell.middle + Vec2::X * place * cell.room.x * APART;
      let build = &mut catalog.build;
      build.place(&collar, Mat4::from_translation(at.extend(0.0)), chrome);
      // held all the way in is the way of a momentary one, and on the way of a latching one
      let push = if held {
        Push::Momentary
      } else {
        Push::Latching
      };
      let level = f32::from(u8::from(on));
      let (material, color) = match plunger {
        Plunger::Rubber(color) => {
          let radius = switch.plunger.emission_radius();
          (rubber().lit(color, level, radius), Some(color))
        }
        Plunger::Backlit(color) => (switch.backlight(color, level), Some(color)),
        Plunger::Black => (Material::glossy(Vec3::new(0.05, 0.05, 0.055)), None),
      };
      build.place(
        &body,
        switch.transform(at.x, at.y, push.depth(on, held)),
        material,
      );
      if let Some(color) = color.filter(|_| on) {
        build
          .lamps
          .push(switch.plunger.lamp(at.x, at.y, color, 0.6));
      }
    }
    catalog.label(*cell, name);
  }

  catalog.finish(look, dim())
}

/// The plunger of a push switch, and how it is lit while on.
#[derive(Clone, Copy)]
enum Plunger {
  /// Rubber that lights in a color.
  Rubber(Vec3),
  /// Clear plastic lit in a color from under it.
  Backlit(Vec3),
  /// Black plastic, which no light is under.
  Black,
}
