//! a small lit 3d scene renderer on wgpu, for plugin panels that look like rendered hardware.
//!
//! a [`Scene`] is a panel seen from above with [`Object`]s on it, one light casting shadows,
//! and a [`Look`] that can make the picture as blocky and banded as an old render. a
//! [`Renderer`] draws it onto any wgpu texture, and again only when it has changed, and
//! [`Scene::pick`] tells which object is seen at a point of the picture.

mod material;
mod mesh;
pub mod parts;
mod pick;
mod renderer;
mod scene;

pub use glam;
pub use material::Material;
pub use mesh::{Mesh, Vertex};
pub use renderer::{Place, Renderer};
pub use scene::{LAMPS, Lamp, Light, Look, Object, Scene};
pub use wgpu;
