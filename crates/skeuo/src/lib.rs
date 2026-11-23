//! a small lit 3d scene renderer on wgpu, for plugin panels that look like rendered hardware.
//!
//! a [`Scene`] is a panel seen from above with [`Object`]s on it, one light casting shadows,
//! and a [`Look`] that can make the picture as blocky and banded as an old render. a
//! [`Renderer`] draws it onto any wgpu texture. with the `iced` feature, [`iced::panel`] is
//! the same thing as a widget for an iced_baseview editor, where it draws under whatever
//! widgets take the input.

mod mesh;
pub mod parts;
mod renderer;
mod scene;

#[cfg(feature = "iced")]
pub mod iced;

pub use glam;
pub use mesh::{Mesh, Vertex};
pub use renderer::{Place, Renderer};
pub use scene::{LAMPS, Lamp, Light, Look, Material, Object, Scene};
pub use wgpu;
