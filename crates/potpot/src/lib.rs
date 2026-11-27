//! A small lit 3D scene renderer on wgpu, for skeuomorphic plugin panels.
//!
//! A [`Scene`] is a panel seen from above with [`Object`]s on it, one [`Light`] casting shadows,
//! and a [`Look`] that can make the picture as blocky and banded as an old render. A
//! [`Renderer`] draws it onto a `wgpu` texture, and of a scene that has changed draws again
//! only what the change shows in. [`Scene::pick`] returns the object seen at a point of the
//! picture, and [`Renderer::pick`] the same of the scene it drew last, with less work.
//!
//! # Examples
//!
//! ```
//! use std::sync::Arc;
//!
//! use potpot::glam::{Mat4, Vec2, Vec3};
//! use potpot::{Light, Look, Material, Mesh, Object, Scene};
//!
//! // a cylinder 10 high, standing at (100, 60)
//! let knob = Object {
//!   mesh: Arc::new(Mesh::lathe(&[[12.0, 0.0], [12.0, 10.0], [0.0, 10.0]], 32, true)),
//!   transform: Mat4::from_translation(Vec3::new(100.0, 60.0, 0.0)),
//!   material: Material::glossy(Vec3::splat(0.1)),
//! };
//! let scene = Scene {
//!   size: Vec2::new(200.0, 120.0),
//!   heights: [0.0, 20.0],
//!   lean: Vec2::ZERO,
//!   light: Light::default(),
//!   look: Look::default(),
//!   objects: vec![knob],
//!   lamps: Vec::new(),
//! };
//! assert_eq!(scene.pick(Vec2::new(104.0, 63.0)), Some(0));
//! assert_eq!(scene.pick(Vec2::new(20.0, 20.0)), None);
//! ```

mod material;
mod mesh;
pub mod parts;
mod pick;
mod renderer;
mod scene;

pub use glam;
pub use material::{Anisotropy, Material};
pub use mesh::{Mesh, Vertex};
pub use renderer::{Place, Renderer};
pub use scene::{LAMPS, Lamp, Light, Look, Object, Scene};
pub use wgpu;
