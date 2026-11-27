//! The scene a frame shows: objects on a panel, its lights and its look.

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use potpot_controls::flat::Flat;

use crate::material::Material;
use crate::mesh::Mesh;

include!("default_light.rs");

/// A panel seen from straight above.
///
/// The x and y axes are the panel's own points with the origin at its top left and y running
/// down, the way a UI lays out, so an object at (x, y) sits under a widget at (x, y). The z axis
/// is the height off the panel toward the viewer.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use potpot::glam::{Mat4, Vec2, Vec3};
/// use potpot::{Light, Look, Material, Mesh, Object, Scene};
///
/// // a box 20 high under a view that leans, so that its front shows
/// let scene = Scene {
///   size: Vec2::new(400.0, 200.0),
///   heights: [-10.0, 40.0],
///   lean: Vec2::new(0.0, -0.5),
///   light: Light::default(),
///   look: Look::default(),
///   objects: vec![Object {
///     mesh: Arc::new(Mesh::cuboid(
///       Vec3::new(100.0, 50.0, 0.0),
///       Vec3::new(140.0, 90.0, 20.0),
///     )),
///     transform: Mat4::IDENTITY,
///     material: Material::matte(Vec3::ONE),
///     picture: Vec::new(),
///   }],
///   lamps: Vec::new(),
/// };
/// // the top is drawn 10 points up the picture from where the box stands
/// assert_eq!(scene.pick(Vec2::new(120.0, 45.0)), Some(0));
/// assert_eq!(scene.pick(Vec2::new(120.0, 95.0)), None);
/// ```
#[derive(Clone, Debug)]
pub struct Scene {
  /// The panel's width and height in points.
  pub size: Vec2,
  /// The lowest and highest z anything reaches.
  pub heights: [f32; 2],
  /// How far a point slides across the picture per unit of its height.
  ///
  /// Zero looks straight down. Any other value leans the view so that sides show.
  pub lean: Vec2,
  /// The light over the whole panel.
  pub light: Light,
  /// How crude the picture is.
  pub look: Look,
  /// Everything on the panel.
  ///
  /// [`Scene::pick`] names an object by its place here.
  pub objects: Vec<Object>,
  /// Small lights on the panel itself.
  ///
  /// A lit button throws such a light on what is around it. Only the first [`LAMPS`] shine.
  pub lamps: Vec<Lamp>,
}

/// The most lamps a scene lights at once.
pub const LAMPS: usize = 32;

/// A small light at a point, which shines on what is near it.
///
/// What stands between it and a surface shadows that surface, going by how high things are seen
/// from straight above. A surface catches less of its light the more it lights itself, so a
/// lamp that stands for a lit part does not light that part again.
#[derive(Clone, Copy, Debug)]
pub struct Lamp {
  /// The lamp's place, in the scene's x, y and z.
  pub at: Vec3,
  /// The lamp's light a quarter of its reach away.
  ///
  /// The light is linear, its color and brightness together, and may be more than one. The
  /// light falls with the square of the distance, and levels off within `radius` of the lamp's
  /// middle.
  pub color: Vec3,
  /// The distance at which its light has faded to nothing.
  pub reach: f32,
  /// How far from its middle the thing that shines reaches.
  ///
  /// The lamp shows that large in a mirror, cut off where the mirror's plane cuts it. Nothing
  /// that near the lamp shadows it.
  pub radius: f32,
}

/// The one light over the whole panel, which casts the shadows.
#[derive(Clone, Copy, Debug)]
pub struct Light {
  /// The direction from the panel toward the light.
  ///
  /// Its length does not matter, and it is not zero.
  pub toward: Vec3,
  /// The light's color and brightness, as linear light.
  pub color: Vec3,
  /// How much the studio's light is kept out of corners.
  ///
  /// The light from all around is kept out of corners and from under things, which grounds
  /// everything on the panel with a soft dark edge. Zero is none.
  pub occlusion: f32,
  /// How much light off surfaces lands on what is next to them.
  ///
  /// The light tints a thing with the color of its neighbors and lightens the sides of things
  /// that face away from the light. Zero is none and costs nothing, and it takes about three to
  /// show plainly.
  pub bounce: f32,
  /// The width of shadow edges, as if the light were that much bigger.
  ///
  /// One is nearly a point, and a studio's wide soft light is five or more.
  pub softness: f32,
  /// How bright the studio around the panel is, as a multiple of its own light.
  ///
  /// The studio lights every surface from all around and is what glossy surfaces mirror. Its
  /// softbox lies where the light of [`Light::default`] comes from, and all of it takes
  /// [`Light::color`] as the light does. Zero is none.
  pub environment: f32,
  /// How much brighter the picture is made, in stops.
  ///
  /// Each stop doubles the light before it is tone mapped.
  pub exposure: f32,
}

impl Default for Light {
  fn default() -> Self {
    Self {
      toward: Vec3::from_array(DEFAULT_TOWARD),
      color: Vec3::from_array(DEFAULT_COLOR),
      occlusion: 1.0,
      bounce: 0.0,
      softness: 2.5,
      environment: 1.0,
      exposure: 0.0,
    }
  }
}

/// The way the picture is degraded on its way to the window.
#[derive(Clone, Copy, Debug)]
pub struct Look {
  /// The window pixels to one rendered pixel along each axis.
  ///
  /// One is sharp, more is blockier.
  pub pixel_size: f32,
  /// The steps each color channel is cut to.
  ///
  /// An ordered dither goes between them. Fewer than two leaves the colors alone.
  pub color_steps: u32,
  /// Whether shadow edges are softened.
  pub soft_shadows: bool,
  /// Whether the edges of shapes are smoothed.
  ///
  /// An old render leaves them rough.
  pub antialias: bool,
  /// How much the parts that light themselves spill a glow around them.
  ///
  /// Zero is none, and at one the middle of a wide face that lights itself fully gains its own
  /// color once more.
  pub bloom: f32,
  /// How far the glow reaches, in points.
  ///
  /// It fades as a Gaussian bell and is gone at this distance. The blur is in cells of four of
  /// the picture's pixels a side, so no glow reaches less than about six pixels.
  pub bloom_radius: f32,
  /// How much darker the picture is toward its corners than in its middle.
  pub vignette: f32,
  /// How far the picture is blurred.
  ///
  /// Zero is no blur, and one is the softness of a bitmap that was drawn larger and scaled
  /// down.
  pub soften: f32,
}

impl Default for Look {
  fn default() -> Self {
    Self {
      pixel_size: 1.0,
      color_steps: 0,
      soft_shadows: true,
      antialias: true,
      bloom: 0.5,
      bloom_radius: 12.0,
      vignette: 0.25,
      soften: 0.0,
    }
  }
}

/// One thing on the panel: a shape, its place and its material.
#[derive(Clone, Debug)]
pub struct Object {
  /// The object's shape.
  ///
  /// Objects of one allocation share it on the GPU. A [`Renderer`](crate::Renderer) keeps it
  /// there while the scene it prepared last uses it, so a shape is best made once and kept.
  pub mesh: Arc<Mesh>,
  /// The place of the mesh.
  ///
  /// It is a rotation and a move, with at most an even scale.
  pub transform: Mat4,
  /// The material of its surface.
  pub material: Material,
  /// The picture shown on its surface, as layers of flat shapes drawn first to last.
  ///
  /// The shapes are in the panel's points and are drawn as a
  /// [`FlatRenderer`](crate::FlatRenderer) draws them, at the scale of the scene's picture, with
  /// each layer blended over the ones before it. They lie on the object where it is highest and
  /// are seen from the viewer, cut off at the edges of the object as it is seen. Their edges are
  /// as smooth as the scene's [`Look`] has them. The picture covers the material's color by its
  /// cover and lights itself as far as it covers, so it takes the material's finish, cells and
  /// haze. An empty list shows none.
  ///
  /// A picture whose layers are each the same allocation as in the scene drawn last, or hold
  /// the same shapes, is not drawn again. A change of the picture alone costs the drawing of
  /// where the object is seen.
  pub picture: Vec<Arc<Flat>>,
}

impl Scene {
  /// Returns the direction from the panel toward the viewer.
  pub(crate) fn toward_viewer(&self) -> Vec3 {
    Vec3::new(-self.lean.x, -self.lean.y, 1.0).normalize()
  }

  /// Returns the matrix from panel points to clip space.
  ///
  /// The nearest height lands on depth zero.
  pub(crate) fn view(&self) -> Mat4 {
    self.seen_leaning(self.lean)
  }

  /// Returns the matrix from panel points to clip space from straight above.
  ///
  /// The height map and the bounce map are drawn through it.
  pub(crate) fn overhead(&self) -> Mat4 {
    self.seen_leaning(Vec2::ZERO)
  }

  fn seen_leaning(&self, lean: Vec2) -> Mat4 {
    let [low, high] = self.heights;
    let reach = (high - low).max(f32::EPSILON);
    let (sx, sy) = (2.0 / self.size.x, -2.0 / self.size.y);
    Mat4::from_cols_array_2d(&[
      [sx, 0.0, 0.0, 0.0],
      [0.0, sy, 0.0, 0.0],
      [sx * lean.x, sy * lean.y, -1.0 / reach, 0.0],
      [-1.0, 1.0, high / reach, 1.0],
    ])
  }

  /// Returns the matrix from panel points to the shadow map's clip space.
  ///
  /// It is the whole panel as the light sees it.
  pub(crate) fn light_view(&self) -> Mat4 {
    let toward = self.light.toward.normalize();
    let right = Vec3::Y.cross(toward).normalize();
    let up = toward.cross(right);
    let onto_light = Mat4::from_cols(
      right.extend(0.0),
      up.extend(0.0),
      toward.extend(0.0),
      glam::Vec4::W,
    )
    .transpose();

    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    // the lean pushes raised points past the panel's edge, so the box the light covers grows
    // by as much
    let spill = self.lean.abs() * self.heights[1].abs().max(self.heights[0].abs());
    for x in [-spill.x, self.size.x + spill.x] {
      for y in [-spill.y, self.size.y + spill.y] {
        for z in self.heights {
          let seen = onto_light.transform_point3(Vec3::new(x, y, z));
          (min, max) = (min.min(seen), max.max(seen));
        }
      }
    }
    let span = (max - min).max(Vec3::splat(f32::EPSILON));
    // the nearest point to the light, the one with the most of `toward`, lands on depth zero
    let fit = Mat4::from_cols_array_2d(&[
      [2.0 / span.x, 0.0, 0.0, 0.0],
      [0.0, 2.0 / span.y, 0.0, 0.0],
      [0.0, 0.0, -1.0 / span.z, 0.0],
      [
        -1.0 - 2.0 * min.x / span.x,
        -1.0 - 2.0 * min.y / span.y,
        max.z / span.z,
        1.0,
      ],
    ]);
    fit * onto_light
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn scene() -> Scene {
    Scene {
      size: Vec2::new(400.0, 200.0),
      heights: [-10.0, 40.0],
      lean: Vec2::new(0.0, 0.5),
      light: Light::default(),
      look: Look::default(),
      objects: Vec::new(),
      lamps: Vec::new(),
    }
  }

  #[test]
  fn the_view_puts_panel_corners_on_clip_corners() {
    let view = scene().view();
    let top_left = view.project_point3(Vec3::ZERO);
    assert!((top_left.truncate() - Vec2::new(-1.0, 1.0)).length() < 1e-5);
    let bottom_right = view.project_point3(Vec3::new(400.0, 200.0, 0.0));
    assert!((bottom_right.truncate() - Vec2::new(1.0, -1.0)).length() < 1e-5);
  }

  #[test]
  fn the_view_keeps_higher_points_nearer_and_leans_them() {
    let view = scene().view();
    let low = view.project_point3(Vec3::new(200.0, 100.0, -10.0));
    let high = view.project_point3(Vec3::new(200.0, 100.0, 40.0));
    assert!((low.z - 1.0).abs() < 1e-5 && high.z.abs() < 1e-5);
    // 40 up at half a point of lean each is 20 points down the picture, a tenth of its
    // height, which is 0.2 in clip space
    assert!((high.y - -0.2).abs() < 1e-5, "{high}");
  }

  #[test]
  fn the_light_sees_the_whole_scene() {
    let scene = scene();
    let light = scene.light_view();
    for x in [0.0, 400.0] {
      for y in [0.0, 200.0] {
        for z in scene.heights {
          let seen = light.project_point3(Vec3::new(x, y, z));
          assert!(seen.x.abs() <= 1.0001 && seen.y.abs() <= 1.0001, "{seen}");
          assert!((-0.0001..=1.0001).contains(&seen.z), "{seen}");
        }
      }
    }
    // toward the light is nearer in its depth
    let near = light.project_point3(scene.light.toward * 10.0 + Vec3::new(200.0, 100.0, 0.0));
    let far = light.project_point3(Vec3::new(200.0, 100.0, 0.0));
    assert!(near.z < far.z);
  }
}
