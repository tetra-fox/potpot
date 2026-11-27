//! A panel's look, as the window asks it of the skin that draws the panel.

use potpot_controls::Point;
use potpot_controls::flat::Flat;
use potpot_controls::interaction::{Edit, Menu, Reading, Target};

use crate::notes::Pressed;

/// The graphics device the window draws with.
///
/// A [`Skin`] makes what it draws from it. Every window of the plugin draws with the same device
/// and queue, on a thread of its own on some systems, so a skin leaves alone what would reach
/// the other windows:
///
/// - [`wgpu::Device::destroy`]
/// - [`wgpu::Device::on_uncaptured_error`] and [`wgpu::Device::set_device_lost_callback`], which
///   the window sets
/// - [`wgpu::Device::poll`], which the window calls for all of them
/// - an error scope left pushed, which takes the errors of every window
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Gpu<'a> {
  /// The device the window draws with.
  pub device: &'a wgpu::Device,
  /// The queue of the device.
  pub queue: &'a wgpu::Queue,
  /// The format of the targets a skin is given to draw on.
  pub format: wgpu::TextureFormat,
}

/// Everything a frame shows that can change between frames.
///
/// A skin's picture is made from this alone: the same view gives the same picture. The window
/// draws a frame when the view has changed, when the window is given a size, and after
/// [`Tell::shown`](crate::Tell::shown).
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct View<'a> {
  /// The page shown, by its place among the panel's pages.
  ///
  /// A skin draws this page and the controls on it, and finds targets on it alone.
  pub page: usize,
  /// The [`Reading`] each control shows, in the order of the panel's controls.
  pub controls: &'a [Reading],
  /// The value of each parameter, in the order of the panel's parameters.
  ///
  /// A skin draws from these whatever belongs to no control.
  pub values: &'a [f32],
  /// The number in each slot of the [`Feed`](crate::Feed).
  ///
  /// They are in the order of the slots.
  pub feed: &'a [f32],
  /// The value being typed, if there is one.
  pub edit: Option<&'a Edit>,
  /// The menu, if it is up.
  ///
  /// The window draws it over the skin's picture.
  pub menu: Option<&'a Menu>,
  /// The control held down by the left button, if any.
  ///
  /// The control is a button being pressed, one being dragged, or one whose arrow is held,
  /// given by its place among the panel's controls.
  pub held: Option<usize>,
  /// The arrow held down by the left button, if any.
  ///
  /// It is a [`Target::Decrement`] or a [`Target::Increment`] of the control `held` names.
  pub arrow: Option<Target>,
  /// The notes whose keys are down.
  ///
  /// They are the one a key of the panel plays and those the audio thread shows through the
  /// [`Notes`](crate::Notes).
  pub notes: Pressed,
  /// The scale the panel is drawn at, as a multiple of its own size.
  pub scale: f32,
  /// The size of a pixel in the panel's points.
  pub pixel: f32,
  /// The width and height in pixels of the place the panel is drawn on.
  pub pixels: [u32; 2],
}

/// The look of a panel.
///
/// A skin says what is drawn for the panel's controls and where on the panel they are. The
/// window owns the target, the menu and the grip, and handles the pointer and the keys.
///
/// # Examples
///
/// ```
/// use potpot_window::controls::flat::Flat;
/// use potpot_window::controls::interaction::Target;
/// use potpot_window::{Gpu, Skin, View, wgpu};
///
/// // one control, drawn as a bar as long as its value
/// struct Bar;
///
/// impl Skin for Bar {
///   fn prepare(&mut self, _: &Gpu, view: &View, over: &mut Flat) {
///     over.rect([20.0, 40.0], [20.0 + 160.0 * view.controls[0].value, 60.0], [1.0; 4]);
///   }
///   fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {}
///   fn target_at(&self, at: [f32; 2]) -> Option<Target> {
///     (40.0..60.0).contains(&at[1]).then_some(Target::Control(0))
///   }
/// }
/// ```
pub trait Skin: 'static {
  /// Lays out the panel as `view` has it, ready to draw.
  ///
  /// The window calls this once before it asks anything else of the skin, and then once for
  /// each frame that is drawn, before [`Skin::draw`]. A frame is drawn when the [`View`]
  /// differs from the last one drawn, when the window is given a size, and after
  /// [`Tell::shown`](crate::Tell::shown). When the system wants the window painted and none of
  /// these has happened, the window puts the last picture back and asks nothing of the skin.
  ///
  /// [`Flat`] shapes for the window to draw over the skin's own picture go in `over`, in the
  /// panel's points.
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat);

  /// Draws what was last prepared onto `place` of a target.
  ///
  /// `place` is `[left, top, width, height]` in the target's pixels. The skin touches
  /// nothing outside `place`.
  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]);

  /// Returns the parts of the place where the picture is new.
  ///
  /// Each part is `[left, top, width, height]` in pixels from the top left of the place the
  /// panel is drawn on, which is [`View::pixels`] wide and high. `None` is the whole place.
  ///
  /// The window calls this after each [`Skin::prepare`] that it draws a frame for. The parts
  /// are those of that call: they hold every pixel that [`Skin::draw`] draws otherwise after it
  /// than after the [`Skin::prepare`] before it, for a place of the same size. A pixel that
  /// differs and is in no part stays in the window as it was.
  ///
  /// The window copies only the parts back from the graphics device and into the window, in
  /// place of the whole frame. A skin that leaves this as it is has every frame copied whole.
  /// The window sees for itself where the flat shapes differ, so a skin that draws nothing else
  /// names no parts.
  fn changed(&self) -> Option<&[[u32; 4]]> {
    None
  }

  /// Returns whether the skin's picture is settled.
  ///
  /// A skin whose picture is refined while the panel stands still returns false until it is
  /// done. The window draws frames while the skin is not settled, though nothing else asks
  /// for one, and none once it is. A skin that leaves this as it is is always settled.
  fn settled(&self) -> bool {
    true
  }

  /// Returns the [`Target`] at a point of the panel as last prepared.
  ///
  /// The window sees to the menu and the grip itself.
  fn target_at(&self, at: Point) -> Option<Target>;
}
