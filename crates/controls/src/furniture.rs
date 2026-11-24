//! what every panel has besides its controls, whatever it looks like: the menu a control is
//! asked for, and the grip the window is pulled larger and smaller by. each is a place to
//! hit and flat shapes to draw, with a plain look that a panel may change the measures and
//! colors of

use crate::Point;
use crate::flat::{Color, Flat};
use crate::interaction::{Item, Menu, Target};

/// the menu: a flat list drawn over the picture, with a row for each item. it starts as
/// [`MenuLook::default`] and is changed a field at a time
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MenuLook {
  /// how wide and tall a row is
  pub row: Point,
  /// the edge around the rows
  pub edge: f32,
  /// the room between every two pairs of rows, which a line across divides them in
  pub divide: f32,
  /// how tall the letters are
  pub letters: f32,
  /// how wide their strokes are
  pub weight: f32,
  /// how far in from a row's left they start
  pub inset: f32,
  /// what the menu is filled with
  pub ground: Color,
  /// its outline and the lines across it
  pub line: Color,
  /// its letters
  pub ink: Color,
  /// the row the pointer is on
  pub mark: Color,
  /// the letters of that row
  pub marked_ink: Color,
  /// what the rows say, in the order of [`Item::ALL`]
  pub labels: [&'static str; 4],
}

impl Default for MenuLook {
  fn default() -> Self {
    Self {
      row: [84.0, 13.0],
      edge: 3.0,
      divide: 5.0,
      letters: 6.0,
      weight: 1.0,
      inset: 6.0,
      ground: [0.075, 0.075, 0.085, 0.97],
      line: [0.4, 0.4, 0.43, 1.0],
      ink: [0.9, 0.9, 0.9, 1.0],
      mark: [0.95, 0.62, 0.14, 1.0],
      marked_ink: [0.06, 0.05, 0.04, 1.0],
      labels: ["EDIT VALUE", "RESET", "COPY VALUE", "PASTE VALUE"],
    }
  }
}

impl MenuLook {
  /// how far down from the first row's top a row's top is
  fn row_down(&self, row: usize) -> f32 {
    row as f32 * self.row[1] + (row / 2) as f32 * self.divide
  }

  /// how wide and tall the whole menu is
  pub fn size(&self) -> Point {
    [
      self.row[0] + 2.0 * self.edge,
      self.row_down(Item::ALL.len() - 1) + self.row[1] + 2.0 * self.edge,
    ]
  }

  /// where the menu's top left corner goes when it is asked for at a point of a panel of
  /// `panel` size: at the point, or with the menu to the other side of the point where it
  /// would run off the panel
  pub fn corner(&self, at: Point, panel: Point) -> Point {
    let size = self.size();
    let turned = |axis: usize| {
      if at[axis] + size[axis] <= panel[axis] {
        at[axis]
      } else {
        at[axis] - size[axis]
      }
    };
    [turned(0), turned(1)]
  }

  /// what of a menu with its corner at a place is at a point, if any of it is
  pub fn target_at(&self, corner: Point, at: Point) -> Option<Target> {
    let (size, within) = (self.size(), [at[0] - corner[0], at[1] - corner[1]]);
    if !(0..2).all(|axis| (0.0..size[axis]).contains(&within[axis])) {
      return None;
    }
    let row = (Item::ALL.into_iter().zip(0..)).find(|&(_, row)| {
      let down = within[1] - self.edge - self.row_down(row);
      (0.0..self.row[1]).contains(&down)
        && (self.edge..self.edge + self.row[0]).contains(&within[0])
    });
    Some(row.map_or(Target::Menu, |(item, _)| Target::Item(item)))
  }

  /// the menu as flat shapes on a panel of `panel` size, with lines as wide as `pixel`,
  /// which is one of the window's
  pub fn shapes(&self, flat: &mut Flat, menu: &Menu, panel: Point, pixel: f32) {
    let (corner, size) = (self.corner(menu.at, panel), self.size());
    let far = [corner[0] + size[0], corner[1] + size[1]];
    flat.rect(corner, far, self.line);
    flat.rect(
      [corner[0] + pixel, corner[1] + pixel],
      [far[0] - pixel, far[1] - pixel],
      self.ground,
    );
    for ((item, label), row) in Item::ALL.into_iter().zip(self.labels).zip(0..) {
      let top_left = [
        corner[0] + self.edge,
        corner[1] + (self.edge + self.row_down(row)),
      ];
      let marked = menu.hovered == Some(item);
      if marked {
        flat.rect(
          top_left,
          [top_left[0] + self.row[0], top_left[1] + self.row[1]],
          self.mark,
        );
      }
      let letters = [
        top_left[0] + self.inset,
        top_left[1] + (self.row[1] - self.letters) / 2.0,
      ];
      flat.legend(
        label,
        // onto the middles of pixels, where the upright and level strokes are sharpest
        letters.map(|along| ((along / pixel).floor() + 0.5) * pixel),
        self.letters,
        self.weight,
        if marked { self.marked_ink } else { self.ink },
      );
      // a line across the middle of the room before each pair of rows after the first
      if row > 0 && row % 2 == 0 {
        let across = [top_left[0], top_left[1] + -self.divide / 2.0];
        flat.rect(
          across,
          [across[0] + self.row[0], across[1] + pixel],
          self.line,
        );
      }
    }
  }
}

/// the grip the window is pulled larger and smaller by, drawn flat in its far corner. its
/// measures are in the screen's points and not the panel's, so that it is the same size
/// however large the window is. it starts as [`GripLook::default`] and is changed a field at
/// a time
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct GripLook {
  /// how wide and tall the corner it takes up is
  pub reach: f32,
  /// how wide its lines are
  pub weight: f32,
  /// what they are drawn in
  pub ink: Color,
}

impl Default for GripLook {
  fn default() -> Self {
    Self {
      reach: 13.0,
      weight: 1.2,
      ink: [1.0, 1.0, 1.0, 0.5],
    }
  }
}

impl GripLook {
  /// whether a point of a panel of `panel` size, shown at so many times its size, is on the
  /// grip
  pub fn holds(&self, at: Point, panel: Point, scale: f32) -> bool {
    (0..2).all(|axis| at[axis] > panel[axis] - self.reach / scale && at[axis] <= panel[axis])
  }

  /// the grip: three lines slanting across the corner, each shorter than the last toward it
  pub fn shapes(&self, flat: &mut Flat, panel: Point, scale: f32) {
    for nth in 1..=3u8 {
      // from the foot of the window to its side, the same way in from the corner on both
      let inward = (f32::from(nth) * 3.5 + 0.5) / scale;
      let clear = 1.5 / scale;
      flat.line(
        [panel[0] - inward, panel[1] - clear],
        [panel[0] - clear, panel[1] - inward],
        self.weight / scale,
        self.ink,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const PANEL: Point = [500.0, 250.0];

  #[test]
  fn the_menu_opens_at_the_pointer_and_turns_from_the_panels_edges() {
    let menu = MenuLook::default();
    let size = menu.size();
    // on the panel, whatever is drawn there, its corner is the pointer
    for at in [
      [150.0, 150.0],
      [86.0, 94.0],
      [380.0, 156.0],
      [0.0, 0.0],
      [PANEL[0] - size[0], PANEL[1] - size[1]],
    ] {
      assert_eq!(menu.corner(at, PANEL), at);
    }
    // where it would run off the right or the foot it lies to the other side of the pointer
    let right = [PANEL[0] - 10.0, 100.0];
    assert_eq!(menu.corner(right, PANEL), [right[0] - size[0], right[1]]);
    let foot = [100.0, PANEL[1] - 10.0];
    assert_eq!(menu.corner(foot, PANEL), [foot[0], foot[1] - size[1]]);
    assert_eq!(
      menu.corner(PANEL, PANEL),
      [PANEL[0] - size[0], PANEL[1] - size[1]]
    );
    // and wherever it is asked for all of it is on the panel
    for x in 0..=PANEL[0] as u16 {
      for y in 0..=PANEL[1] as u16 {
        let corner = menu.corner([f32::from(x), f32::from(y)], PANEL);
        assert!(
          (0..2).all(|axis| corner[axis] >= 0.0 && corner[axis] + size[axis] <= PANEL[axis]),
          "asked for at {x} {y}, the menu at {corner:?} is off the panel"
        );
      }
    }
  }

  #[test]
  fn a_point_of_the_menu_is_a_row_the_plate_or_not_the_menu() {
    let menu = MenuLook::default();
    let (corner, size) = ([100.0, 50.0], menu.size());
    let at = |point: Point| menu.target_at(corner, point);
    let row = |row: usize| {
      [
        corner[0] + size[0] / 2.0,
        corner[1] + menu.edge + menu.row_down(row),
      ]
    };
    for (item, nth) in Item::ALL.into_iter().zip(0..) {
      // from its first line of points to its last
      assert_eq!(at(row(nth)), Some(Target::Item(item)));
      let last = [row(nth)[0], row(nth)[1] + (menu.row[1] - 0.01)];
      assert_eq!(at(last), Some(Target::Item(item)));
    }
    // the edge around the rows and the room between the pairs are the menu and no row
    assert_eq!(at([corner[0] + 1.0, corner[1] + 1.0]), Some(Target::Menu));
    assert_eq!(at([row(2)[0], row(2)[1] - 2.0]), Some(Target::Menu));
    assert_eq!(
      at([row(0)[0] - (menu.row[0] / 2.0 + 1.0), row(0)[1]]),
      Some(Target::Menu)
    );
    // and past its sides is not the menu
    assert_eq!(at([corner[0] - 0.5, corner[1] - 0.5]), None);
    assert_eq!(at([corner[0] + size[0], corner[1] + size[1]]), None);
    assert_eq!(
      at([corner[0] + size[0] - 0.01, corner[1] + size[1] - 0.01]),
      Some(Target::Menu)
    );
  }

  #[test]
  fn the_grip_is_the_windows_far_corner_and_as_large_on_the_screen_at_any_size() {
    let grip = GripLook::default();
    for scale in [0.75, 1.0, 3.0] {
      // so many of the screen's points in from the corner, whatever that is of the panel's
      let inside = PANEL.map(|side| side - (grip.reach - 0.5) / scale);
      let outside = PANEL.map(|side| side - (grip.reach + 0.5) / scale);
      assert!(
        grip.holds(inside, PANEL, scale) && grip.holds(PANEL, PANEL, scale),
        "{scale}"
      );
      assert!(!grip.holds(outside, PANEL, scale), "{scale}");
      assert!(
        !grip.holds([inside[0], outside[1]], PANEL, scale),
        "{scale}"
      );
      assert!(
        !grip.holds([outside[0], inside[1]], PANEL, scale),
        "{scale}"
      );
    }
  }
}
