//! A baseview window for a panel of controls, drawn with wgpu by a given skin.
//!
//! [`open()`] opens the window with baseview, in a host's window or on its own. A panel is
//! its [`Settings`], its [`Param`]s and the [`controls`] that turn them, drawn by a [`Skin`].
//! The window draws the skin with wgpu and sees to everything between the pointer, the keys
//! and the panel's parameters. The menu, the grip and a value being typed work the same
//! under any skin. A [`Feed`] carries numbers no parameter holds, such as a meter's level, from
//! an audio thread to the skin. A [`Notes`] carries the notes played on a keybed of the panel
//! to an audio thread, and the notes a host plays to the skin. [`picture()`] draws one frame of a
//! panel to a [`Picture`] of plain pixels, with no window.
//!
//! # Examples
//!
//! ```no_run
//! use potpot_window::controls::flat::Flat;
//! use potpot_window::controls::interaction::{Control, Dial, Readout, Target};
//! use potpot_window::{Gpu, Opening, Param, Settings, Skin, View, wgpu};
//!
//! // one control, drawn as a bar as long as its value
//! struct Bar;
//!
//! impl Skin for Bar {
//!   fn prepare(&mut self, _: &Gpu, view: &View, over: &mut Flat) {
//!     over.rect([20.0, 40.0], [20.0 + 160.0 * view.controls[0].value, 60.0], [1.0; 4]);
//!   }
//!   fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {}
//!   fn target_at(&self, at: [f32; 2]) -> Option<Target> {
//!     (40.0..60.0).contains(&at[1]).then_some(Target::Control(0))
//!   }
//! }
//!
//! let settings = Settings::new("Bar", [200.0, 100.0]);
//! let params = vec![Param::free(Dial::linear([0.0, 1.0], 0.5, None, 2, ""))];
//! let controls = vec![Control::new(0, Readout::new(4))];
//! let opened = potpot_window::open(settings, params, controls, Opening::alone(), |_| Bar)?;
//! opened.window.run_until_closed()?;
//! # Ok::<(), potpot_window::baseview::Error>(())
//! ```
//!
//! `examples/flat.rs` is a whole panel written this way, and [`nice_plug`] makes one the
//! editor of a plugin.
//!
//! # Drawing
//!
//! The picture is drawn off the window and put in it as plain pixels, so nothing of the
//! graphics driver's own window code runs in a host's process. Where a skin names the parts of
//! its picture that a frame differs in, with [`Skin::changed`], those parts alone are read back
//! and put in the window.
//!
//! The windows of one plugin draw with one graphics device: every window that one loaded copy
//! of this crate has open, and every [`picture()`] it draws, and each plugin binary has a copy
//! of its own. Their frames run one after another on the device, so a window's frame can wait
//! for a neighbor's. The device is let go of when no window is open on it, no picture is being
//! drawn and no [`Shared`] holds it.
//!
//! A fault of the device stops the drawing of every window on it, and the next window to open
//! draws with a new one. An error while a window opens fails that window alone. wgpu reports an
//! error to the device, so an error of one window's drawing at that moment can fail the opening
//! of another.

mod feed;
mod graphics;
#[cfg(feature = "nice-plug")]
pub mod nice_plug;
mod notes;
mod open;
mod picture;
mod screen;
mod skin;
mod window;

pub use feed::Feed;
pub use notes::{Heard, Notes, Played, Pressed};
pub use open::{Opened, Opening, Param, Remembered, Settings, Shared, Tell, open};
pub use picture::{Picture, picture};
pub use skin::{Gpu, Skin, View};
// what the types above are made of, so that a skin is built on the same versions
pub use potpot_controls as controls;
pub use {baseview, wgpu};

// builds the example in the workspace's README as a doc test
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
struct Readme;
