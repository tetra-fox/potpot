//! a window for a panel of [`controls`]: it opens with baseview, in a host's window or on
//! its own, draws with wgpu whatever [`Skin`] it is given, and sees to everything between
//! the pointer and the keys and the panel's parameters. the menu, the grip and a value being
//! typed work the same under any skin.
//!
//! the picture is drawn off the window and put in it as plain pixels, so nothing of the
//! graphics driver's own window code runs in a host's process

#![warn(missing_docs, missing_debug_implementations)]

mod flat;
#[cfg(feature = "nice-plug")]
pub mod nice_plug;
mod open;
mod screen;
mod skin;
mod window;

pub use open::{Opened, Opening, Panel, Param, Remembered, Shared, Tell, open};
pub use skin::{Gpu, Skin, View};
// what the types above are made of, so that a skin is built on the same versions
pub use {baseview, controls, wgpu};
