//! a small lit 3d scene renderer on wgpu, for plugin panels that look like rendered hardware.
//!
//! a [`Scene`] is a panel seen from above with [`Object`]s on it, one light casting shadows,
//! and a [`Look`] that can make the picture as blocky and banded as an old render. a
//! [`Renderer`] draws it onto any wgpu texture, and again only when it has changed, and
//! [`Scene::pick`] tells which object is seen at a point of the picture. [`Flat`] shapes are
//! for what is not part of the panel, like a menu: a [`FlatRenderer`] draws them over the
//! picture as plain colors.

mod flat;
mod material;
mod mesh;
pub mod parts;
mod pick;
mod renderer;
mod scene;

pub use flat::{Color, Flat, FlatRenderer};
pub use glam;
pub use material::Material;
pub use mesh::{Mesh, Vertex};
pub use renderer::{Place, Renderer};
pub use scene::{LAMPS, Lamp, Light, Look, Object, Scene};
pub use wgpu;
