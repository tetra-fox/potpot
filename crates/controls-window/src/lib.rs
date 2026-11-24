//! a window for a panel of [`controls`]: it opens with baseview, in a host's window or on
//! its own, draws with wgpu whatever [`Skin`] it is given, and sees to everything between
//! the pointer and the keys and the panel's parameters. the menu, the grip and a value being
//! typed work the same under any skin.
//!
//! the picture is drawn off the window and put in it as plain pixels, so nothing of the
//! graphics driver's own window code runs in a host's process.
//!
//! a panel is its [`Settings`], a [`Param`] for each control, and a [`Skin`]:
//!
//! ```no_run
//! use controls_window::controls::flat::Flat;
//! use controls_window::controls::interaction::{Control, Dial, Readout, Target};
//! use controls_window::{Gpu, Opening, Param, Settings, Skin, View, wgpu};
//!
//! /// one control, drawn as a bar as long as its value
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
//! let opened = controls_window::open(settings, params, controls, Opening::alone(), |_| Bar)?;
//! opened.window.run_until_closed()?;
//! # Ok::<(), controls_window::baseview::Error>(())
//! ```
//!
//! `examples/flat.rs` is a whole panel written this way, and [`nice_plug`] makes one the
//! editor of a plugin

#![warn(missing_docs, missing_debug_implementations)]

mod flat;
#[cfg(feature = "nice-plug")]
pub mod nice_plug;
mod open;
mod screen;
mod skin;
mod window;

pub use open::{Opened, Opening, Param, Remembered, Settings, Shared, Tell, open};
pub use skin::{Gpu, Skin, View};
// what the types above are made of, so that a skin is built on the same versions
pub use {baseview, controls, wgpu};
