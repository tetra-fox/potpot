//! The fit of a panel of one shape in windows of any size.
//!
//! The functions return where the panel is drawn, where the pointer is on it, and what sizes a
//! host may give it. A panel's `size` is its width and height in its own points, whole numbers
//! of them.

use crate::Point;

/// Returns the part of a window that the panel is drawn on.
///
/// `window` is the window's width and height in pixels. The part is `[left, top, width, height]`:
/// the largest of the panel's own shape that fits, in the middle. A window of that shape is all
/// of it, and one of any other has room left over at two sides.
///
/// # Panics
///
/// Panics if `size` is less than one point wide and either `window` is no pixels wide or
/// `size` is less than one point tall.
pub fn fitted(window: [u32; 2], size: Point) -> [u32; 4] {
  let [wide, tall] = window.map(u64::from);
  let (across, down) = (size[0] as u64, size[1] as u64);
  let (width, height) = if wide * down <= tall * across {
    (wide, wide * down / across)
  } else {
    (tall * across / down, tall)
  };
  [(wide - width) / 2, (tall - height) / 2, width, height].map(|side| side as u32)
}

/// Returns the point of the panel at a pixel of the window.
///
/// `place` is the part of the window the panel is drawn on.
pub fn point_at(place: [u32; 4], pixel: Point, size: Point) -> Point {
  let [left, top, width, height] = place.map(|side| side as f32);
  [
    (pixel[0] - left) / width * size[0],
    (pixel[1] - top) / height * size[1],
  ]
}

/// Returns the size of the panel's shape nearest one asked about.
///
/// `asked` is the size a host asks about, and it, `current` and the least and the most are
/// all in the same units. The size is as wide as `asked` if the width has changed from
/// `current`, the size the window has, at least as much as the height, and as tall as `asked`
/// otherwise. Each change is measured as a part of the panel's own side. The width is held
/// between the least and the most.
///
/// # Panics
///
/// Panics if the least width is larger than the most.
///
/// Panics if `size` is less than one point wide.
///
/// Panics if `size` is less than one point tall and `asked` and `current` differ in height.
pub fn shaped(
  asked: [u32; 2],
  current: [u32; 2],
  [least, most]: [[u32; 2]; 2],
  size: Point,
) -> [u32; 2] {
  let (across, down) = (size[0] as u64, size[1] as u64);
  // each change as a part of the panel's own side, cross multiplied
  let wider = u64::from(asked[0].abs_diff(current[0])) * down;
  let taller = u64::from(asked[1].abs_diff(current[1])) * across;
  let width = if wider >= taller {
    asked[0]
  } else {
    (u64::from(asked[1]) * across / down) as u32
  };
  let width = width.clamp(least[0], most[0]);
  [width, (u64::from(width) * down / across) as u32]
}

/// Returns the scale a size from the host is the user's choice of.
///
/// `scales` is the least and the most scale the panel is drawn at. The sizes are in the
/// host's units, `per_point` of them to a point. The result is `None` if `given` is the size
/// the host was last told, which is the host saying it back, or one the panel is not drawn as
/// small or as large as.
///
/// # Panics
///
/// Panics if `given` differs from `told`, the first of `scales` is larger than the second
/// and the scale of `given` is within 0.01 of both.
pub fn chosen(
  given: [u32; 2],
  told: [u32; 2],
  per_point: f64,
  size: Point,
  scales: [f32; 2],
) -> Option<f32> {
  if given == told {
    return None;
  }
  let [wide, tall] = given.map(|side| f64::from(side) / per_point);
  let scale = (wide / f64::from(size[0])).min(tall / f64::from(size[1])) as f32;
  // the least and the most in whole pixels come out a little to either side of themselves
  let near = 0.01;
  (scales[0] - near..=scales[1] + near)
    .contains(&scale)
    .then(|| scale.clamp(scales[0], scales[1]))
}

#[cfg(test)]
mod tests {
  use super::*;

  // a panel twice as wide as tall, drawn from three quarters to three times its size
  const SIZE: Point = [500.0, 250.0];
  const SCALES: [f32; 2] = [0.75, 3.0];

  fn fits(window: [u32; 2]) -> [u32; 4] {
    fitted(window, SIZE)
  }

  #[test]
  fn the_panel_is_fitted_into_a_window_of_any_shape_at_its_own() {
    // the panel covers all of a window of its own shape
    assert_eq!(fits([750, 375]), [0, 0, 750, 375]);
    assert_eq!(fits([500, 250]), [0, 0, 500, 250]);
    // one too wide has the room left over at its sides, and one too tall above and below
    assert_eq!(fits([1100, 375]), [175, 0, 750, 375]);
    assert_eq!(fits([750, 640]), [0, 132, 750, 375]);
    assert_eq!(fits([1908, 1077]), [0, 61, 1908, 954]);
    // odd sizes round down, and what is left over is never more than a pixel off the middle
    assert_eq!(fits([751, 375]), [0, 0, 750, 375]);
    assert_eq!(fits([750, 376]), [0, 0, 750, 375]);
    assert_eq!(fits([3, 100]), [0, 49, 3, 1]);
    for window in [[640, 480], [333, 777], [2250, 1125], [1, 1], [0, 0]] {
      let [left, top, width, height] = fits(window);
      assert!(
        left + width <= window[0] && top + height <= window[1],
        "{window:?}"
      );
      // the panel's shape, to within the pixel that rounding takes
      assert!(width.abs_diff(height * 2) <= 1, "{window:?}");
      // and as large as fits: one of the two reaches from side to side
      assert!(width == window[0] || height == window[1], "{window:?}");
    }
  }

  #[test]
  fn the_pointer_is_on_the_panel_where_the_panel_is_drawn() {
    // a wide window with the panel in its middle at one and a half pixels to a point
    let place = fits([1100, 375]);
    assert_eq!(point_at(place, [175.0, 0.0], SIZE), [0.0, 0.0]);
    assert_eq!(point_at(place, [925.0, 375.0], SIZE), SIZE);
    assert_eq!(point_at(place, [550.0, 187.5], SIZE), [250.0, 125.0]);
    // on the room left over it is off the panel, where nothing is picked
    assert!(point_at(place, [100.0, 100.0], SIZE)[0] < 0.0);
    assert!(point_at(place, [1000.0, 100.0], SIZE)[0] > SIZE[0]);
    // and in a tall one
    let place = fits([750, 640]);
    assert_eq!(point_at(place, [375.0, 132.0], SIZE), [250.0, 0.0]);
    assert!(point_at(place, [375.0, 100.0], SIZE)[1] < 0.0);
  }

  /// Returns the size the host is told for a scale.
  ///
  /// `per_point` is the number of pixels to a point.
  fn told(scale: f32, per_point: f64) -> [u32; 2] {
    SIZE.map(|side| (f64::from(side * scale) * per_point).round() as u32)
  }

  fn choice(given: [u32; 2], told: [u32; 2], per_point: f64) -> Option<f32> {
    chosen(given, told, per_point, SIZE, SCALES)
  }

  #[test]
  fn only_a_size_of_the_hosts_own_that_the_panel_can_have_is_the_users_choice() {
    for per_point in [1.0, 1.25, 1.5, 2.0] {
      for scale in [SCALES[0], 1.0, 1.1, 2.0, SCALES[1]] {
        // the host saying back what it was told changes nothing, whatever that rounds to
        let same = told(scale, per_point);
        assert_eq!(choice(same, same, per_point), None);
        // any other size the panel is drawn at is taken, as the scale it is
        let other = choice(same, told(1.3, per_point), per_point)
          .expect("a size the panel is drawn at is a choice");
        assert!((other - scale).abs() < 0.005, "{other} for {scale}");
        assert!((SCALES[0]..=SCALES[1]).contains(&other));
      }
      // a view smaller or larger than the panel is ever drawn is not one to remember
      let was = told(1.0, per_point);
      assert_eq!(choice(told(0.6, per_point), was, per_point), None);
      assert_eq!(choice(told(3.5, per_point), was, per_point), None);
    }
    // a size that is not the panel's shape is the scale of the panel that fits in it
    assert_eq!(choice([1350, 375], told(2.0, 1.5), 1.5), Some(1.0));
  }

  #[test]
  fn the_size_live_gives_a_view_it_sized_for_one_pixel_to_a_point_is_not_remembered() {
    // live made its window from a size reported at one pixel to a point and gives it to the
    // view, whose display has one and a half: two thirds of the panel, smaller than it goes
    let given = told(1.0, 1.0);
    assert_eq!(choice(given, told(1.0, 1.5), 1.5), None);
  }

  /// Returns the answer to a size a host asks about.
  ///
  /// The host asks from the window as it opens, and the least and the most are those at one
  /// and a half pixels to a point.
  fn asked(width: u32, height: u32) -> [u32; 2] {
    let limits = [[563, 281], [2250, 1125]];
    shaped([width, height], [750, 375], limits, SIZE)
  }

  #[test]
  fn a_size_a_host_asks_about_is_answered_with_the_panels_shape() {
    // the shape itself is left alone
    assert_eq!(asked(750, 375), [750, 375]);
    assert_eq!(asked(1000, 500), [1000, 500]);
    // pulled by its side the window follows the width, and by its foot the height
    assert_eq!(asked(900, 375), [900, 450]);
    assert_eq!(asked(600, 375), [600, 300]);
    assert_eq!(asked(750, 450), [900, 450]);
    assert_eq!(asked(750, 300), [600, 300]);
    // by its corner, whichever side has gone further for its length
    assert_eq!(asked(900, 500), [1000, 500]);
    assert_eq!(asked(1000, 400), [1000, 500]);
    // and no smaller or larger than the window is let be
    assert_eq!(asked(100, 375), [563, 281]);
    assert_eq!(asked(750, 50), [563, 281]);
    assert_eq!(asked(5000, 375), [2250, 1125]);
    assert_eq!(asked(750, 3000), [2250, 1125]);
  }
}
