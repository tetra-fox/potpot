//! what a panel looks like, as the window asks it of whoever draws the panel

use potpot_controls::Point;
use potpot_controls::flat::Flat;
use potpot_controls::interaction::{Edit, Menu, Reading, Target};

/// the graphics device the window draws with, for a skin to make what it draws from
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Gpu<'a> {
  /// the device
  pub device: &'a wgpu::Device,
  /// and its queue
  pub queue: &'a wgpu::Queue,
  /// the format of the targets a skin is given to draw on
  pub format: wgpu::TextureFormat,
}

/// everything a frame shows that can change from one to the next. a skin's picture is made
/// from this alone: with the same view it is the same picture, and the window draws a frame
/// only when the view has changed
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct View<'a> {
  /// what each control shows, in the order of the panel's controls
  pub controls: &'a [Reading],
  /// where each parameter stands, in the order of the panel's parameters, for whatever a
  /// skin draws that is not a control's
  pub values: &'a [f32],
  /// the value being typed, if one is
  pub edit: Option<&'a Edit>,
  /// the menu, if it is up. the window draws it, over the skin's picture
  pub menu: Option<&'a Menu>,
  /// the control that is pressed and held down, if one is, by its place among the panel's
  pub held: Option<usize>,
  /// how many times its own size the panel is drawn at
  pub scale: f32,
  /// how much of one of the panel's points a pixel is
  pub pixel: f32,
  /// the width and height in pixels of the place the panel is drawn on
  pub pixels: [u32; 2],
}

/// a panel's look: what is drawn for its controls, and where on it they are. the window owns
/// the target, the menu, the grip and everything about the pointer and the keys
pub trait Skin: 'static {
  /// lays out the panel as a view has it, ready to draw. called once before anything else
  /// is asked of the skin, and then once for each frame that is drawn, before
  /// [`Skin::draw`]. a frame is drawn only when the view differs from the last one drawn:
  /// when the system wants the window painted and nothing has changed, the last picture is
  /// put back and the skin is not asked for anything. flat shapes for
  /// the window to draw over the skin's own picture go in `over`, in the panel's points
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat);

  /// draws what was last prepared onto `place` of a target: its left, top, width and height
  /// in the target's pixels. nothing outside the place is to be touched
  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]);

  /// what of the panel as last prepared is at one of its points, apart from the menu and
  /// the grip, which the window sees to
  fn target_at(&self, at: Point) -> Option<Target>;
}
