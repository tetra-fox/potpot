//! what a frame shows: objects on a panel, one light, and how crude the picture is

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};

use crate::mesh::Mesh;

/// a panel seen from straight above.
///
/// x and y are the panel's own points with the origin at its top left and y running down, the
/// way a ui lays out, so an object at (x, y) sits under a widget at (x, y). z is the height off
/// the panel toward the viewer
#[derive(Clone, Debug)]
pub struct Scene {
  /// the panel's width and height
  pub size: Vec2,
  /// the lowest and highest z anything reaches
  pub heights: [f32; 2],
  /// how far a point slides across the picture for each unit of its height. zero looks
  /// straight down, anything else leans the view so that sides show
  pub lean: Vec2,
  pub light: Light,
  pub look: Look,
  /// the shapes the objects are made of. the renderer uploads them once and again only when
  /// this is a different allocation
  pub meshes: Arc<[Mesh]>,
  pub objects: Vec<Object>,
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
  /// from the panel toward the light
  pub toward: Vec3,
  pub color: Vec3,
  /// the light that reaches every surface, shadowed or not
  pub ambient: Vec3,
}

impl Default for Light {
  fn default() -> Self {
    Self {
      toward: Vec3::new(-0.45, -0.6, 1.0),
      color: Vec3::splat(1.0),
      ambient: Vec3::splat(0.25),
    }
  }
}

/// how the picture is degraded on its way to the window
#[derive(Clone, Copy, Debug)]
pub struct Look {
  /// window pixels to one rendered pixel along each axis. one is sharp, more is blockier
  pub pixel_size: f32,
  /// the steps each color channel is cut to, with an ordered dither between them. zero
  /// leaves the colors alone
  pub color_steps: u32,
  /// whether shadow edges are softened
  pub soft_shadows: bool,
  /// whether the edges of shapes are smoothed. an old render's are not
  pub antialias: bool,
}

impl Default for Look {
  fn default() -> Self {
    Self {
      pixel_size: 1.0,
      color_steps: 0,
      soft_shadows: true,
      antialias: true,
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Object {
  /// an index into the scene's meshes
  pub mesh: usize,
  /// where the mesh goes. a rotation and a move, with at most an even scale
  pub transform: Mat4,
  pub material: Material,
}

#[derive(Clone, Copy, Debug)]
pub struct Material {
  pub color: Vec3,
  /// the strength of the highlight
  pub specular: f32,
  /// how tight the highlight is
  pub shininess: f32,
  /// how much of the color shows without any light on it, for lamps and displays
  pub glow: f32,
  /// how much of the room around the panel the surface mirrors: a soft window where the
  /// light is and a pale band at the horizon
  pub reflect: f32,
  /// how far the surface is metal, which mirrors in its own color and has little color
  /// under the light otherwise
  pub metal: f32,
  /// how far the highlight is drawn out into the bright wedges of metal turned on a lathe,
  /// around the object's own z axis
  pub spun: f32,
}

impl Material {
  /// paint, print and anything else without a shine
  pub fn matte(color: Vec3) -> Self {
    Self {
      color,
      specular: 0.05,
      shininess: 8.0,
      glow: 0.0,
      reflect: 0.0,
      metal: 0.0,
      spun: 0.0,
    }
  }

  /// polished plastic, like bakelite
  pub fn glossy(color: Vec3) -> Self {
    Self {
      specular: 0.7,
      shininess: 90.0,
      reflect: 0.6,
      ..Self::matte(color)
    }
  }

  /// plastic with a soft sheen, like a rubbery coating
  pub fn satin(color: Vec3) -> Self {
    Self {
      specular: 0.16,
      shininess: 24.0,
      reflect: 0.1,
      ..Self::matte(color)
    }
  }

  /// polished metal, which is mostly what it mirrors
  pub fn chrome(color: Vec3) -> Self {
    Self {
      specular: 1.0,
      shininess: 200.0,
      reflect: 1.0,
      metal: 1.0,
      ..Self::matte(color)
    }
  }

  /// metal turned on a lathe, like the aluminum face of a knob
  pub fn spun(color: Vec3) -> Self {
    Self {
      specular: 0.55,
      shininess: 70.0,
      reflect: 0.12,
      metal: 0.85,
      spun: 1.0,
      ..Self::matte(color)
    }
  }

  /// something that lights itself
  pub fn lamp(color: Vec3) -> Self {
    Self {
      specular: 0.0,
      shininess: 1.0,
      glow: 1.0,
      ..Self::matte(color)
    }
  }
}

impl Scene {
  /// the direction from the panel toward the viewer
  pub(crate) fn toward_viewer(&self) -> Vec3 {
    Vec3::new(-self.lean.x, -self.lean.y, 1.0).normalize()
  }

  /// panel points to clip space, the nearest height at depth zero
  pub(crate) fn view(&self) -> Mat4 {
    let [low, high] = self.heights;
    let reach = (high - low).max(f32::EPSILON);
    let (sx, sy) = (2.0 / self.size.x, -2.0 / self.size.y);
    Mat4::from_cols_array_2d(&[
      [sx, 0.0, 0.0, 0.0],
      [0.0, sy, 0.0, 0.0],
      [sx * self.lean.x, sy * self.lean.y, -1.0 / reach, 0.0],
      [-1.0, 1.0, high / reach, 1.0],
    ])
  }

  /// panel points to the shadow map's clip space, as the light sees the whole panel
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
      meshes: Arc::from([]),
      objects: Vec::new(),
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
    // 40 up at half a point of lean each is 20 points down the picture, a fifth of its
    // clip height
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
