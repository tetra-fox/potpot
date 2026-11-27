//! A panel's picture, drawn with no window.

use std::fmt;

use potpot_controls::interaction::{Control, Interaction};

use crate::graphics::Graphics;
use crate::open::{Param, Settings, check_size};
use crate::skin::{Gpu, Skin};
use crate::window::pictured;

/// A panel's picture as plain pixels.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Picture {
  /// The picture's width and height in pixels.
  pub size: [u32; 2],
  /// The bytes of the pixels, in rows from the top with no gaps.
  ///
  /// A pixel is four bytes: red, green, blue and alpha, the colors in sRGB. Every alpha is 255.
  /// It is the frame once the skin has settled, as [`Skin::settled`] has it.
  pub pixels: Vec<u8>,
  /// The bytes of the first frame drawn, before the skin refined its picture.
  ///
  /// They are laid out as [`Picture::pixels`] are.
  pub first: Vec<u8>,
}

impl fmt::Debug for Picture {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Picture")
      .field("size", &self.size)
      .finish_non_exhaustive()
  }
}

/// Draws one frame of a panel to pixels, with no window.
///
/// The frame is drawn again until the skin has settled on its picture.
///
/// The [`Picture`] is what a window of `settings` shows at `scale` on a display with one pixel
/// to a point: the skin's picture and over it its flat shapes and the grip. `scale` is a multiple
/// of the panel's own size, and each side of the picture is that many times the panel's, rounded
/// to whole pixels. `params`, `controls` and `skin` are as [`open()`](crate::open()) takes them.
///
/// `scale` is not held to [`Settings::scales`], which are the sizes a window is pulled to: a
/// picture can be a thumbnail or larger than any window.
///
/// The frame is a window's first. Each parameter stands where its [`Param::value`] has it, the
/// [`Feed`](crate::Feed) holds what was last written to it, and the notes shown with
/// [`Notes::show`](crate::Notes::show) are down. Nothing is typed, held or chosen from a menu.
///
/// No display server is asked anything. The frame is drawn with a graphics device of its own,
/// which is let go of before this returns.
///
/// # Errors
///
/// Returns an error if a side of the panel's size is not a finite number of points, one at
/// least.
///
/// Returns an error if no graphics adapter is found or the adapter opens no device.
///
/// Returns an error if a side of the picture is no pixels or more than the device draws on in
/// one piece, or the picture is more bytes than the device reads back at once.
///
/// Returns an error if the device reports one while the skin is made or the frame is drawn, is
/// lost, or does not draw the frame within a second.
///
/// # Panics
///
/// Panics if a control names a parameter that `params` has none for, or names one twice.
pub fn picture<S: Skin>(
  settings: &Settings,
  params: Vec<Param>,
  controls: Vec<Control>,
  scale: f32,
  skin: impl FnOnce(&Gpu) -> S,
) -> Result<Picture, baseview::Error> {
  check_size(settings.size)?;
  // a device of its own, so that nothing of this frame reaches an open window's
  let graphics = Graphics::new()?;
  // a scale that is no number, or under zero, comes out as no pixels, which is refused
  let size = settings.size.map(|side| (side * scale).round() as u32);
  let values = params.iter().map(|param| (param.value)()).collect();
  let dials = params.into_iter().map(|param| param.dial).collect();
  let interaction = Interaction::new(dials, controls, settings.size, settings.scales);
  let pictured = pictured(&graphics, settings, &interaction, values, size, skin)?;
  Ok(Picture {
    size,
    pixels: pictured.settled,
    first: pictured.first,
  })
}

#[cfg(test)]
mod tests {
  use potpot_controls::flat::Flat;
  use potpot_controls::interaction::{Dial, Readout, Target};

  use super::*;
  use crate::skin::View;

  /// A panel whose left half is red and whose right half is as blue as its one control stands.
  struct Halves;

  impl Skin for Halves {
    fn prepare(&mut self, _: &Gpu, view: &View, over: &mut Flat) {
      over.rect([0.0, 0.0], [100.0, 100.0], [1.0, 0.0, 0.0, 1.0]);
      let blue = [0.0, 0.0, view.controls[0].value, 1.0];
      over.rect([100.0, 0.0], [200.0, 100.0], blue);
    }
    fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {}
    fn target_at(&self, _: [f32; 2]) -> Option<Target> {
      None
    }
  }

  #[test]
  #[ignore = "needs a graphics device"]
  fn a_picture_is_the_frame_in_rows_of_its_own_width_with_red_first() {
    let settings = Settings::new("Halves", [200.0, 100.0]);
    let params = vec![Param::free(Dial::linear([0.0, 1.0], 1.0, None, 2, ""))];
    let controls = vec![Control::new(0, Readout::new(4))];
    let drawn = picture(&settings, params, controls, 1.5, |_| Halves).expect("the frame is drawn");
    // rows of 300 pixels are read back with a gap after each, which the picture leaves out
    assert_eq!(drawn.size, [300, 150]);
    assert_eq!(drawn.pixels.len(), 300 * 150 * 4);
    let at = |x: usize, y: usize| &drawn.pixels[(y * 300 + x) * 4..][..4];
    for y in [0, 75, 120] {
      assert_eq!(at(5, y), [255, 0, 0, 255], "row {y}");
      assert_eq!(at(250, y), [0, 0, 255, 255], "row {y}");
    }
    // the grip is over the skin's shapes, in the corner
    assert!((270..300).any(|x| at(x, 148) != [0, 0, 255, 255]));
  }
}
