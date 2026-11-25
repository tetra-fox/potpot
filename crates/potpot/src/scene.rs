//! what a frame shows: objects on a panel, one light, and how crude the picture is

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};

use crate::material::Material;
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
  pub objects: Vec<Object>,
  /// small lights on the panel itself, like what a lit button throws on what is around it.
  /// only the first [`LAMPS`] shine
  pub lamps: Vec<Lamp>,
}

/// how many lamps a scene can have lit at once
pub const LAMPS: usize = 32;

/// a small light at a point, which shines on what is near it and casts no shadows
#[derive(Clone, Copy, Debug)]
pub struct Lamp {
  pub at: Vec3,
  pub color: Vec3,
  /// how far its light gets before it has faded to nothing
  pub reach: f32,
  /// how far from its middle the thing that shines reaches, which is how large the lamp
  /// shows in a mirror
  pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
  /// from the panel toward the light
  pub toward: Vec3,
  pub color: Vec3,
  /// the light that reaches every surface, shadowed or not
  pub ambient: Vec3,
  /// how much brighter the panel is at the edge nearest the light than in its middle, and
  /// as much dimmer at the far edge, the way a lamp close by lights unevenly
  pub falloff: f32,
  /// how much the light that reaches every surface is kept out of corners and from under
  /// things, which grounds everything on the panel with a soft dark edge. zero is none
  pub occlusion: f32,
  /// how wide the edges of shadows are, as if the light were that much bigger. one is
  /// nearly a point, and a studio's wide soft light is five or more
  pub softness: f32,
  /// how far off the light and the viewer are, in widths of the panel, for what shines: up
  /// close a flat glossy face has its highlight in one place and fades away from it, as it
  /// does in a photograph. zero puts both infinitely far, and such a face shines evenly
  pub nearness: f32,
}

impl Default for Light {
  fn default() -> Self {
    Self {
      toward: Vec3::new(-0.45, -0.6, 1.0),
      color: Vec3::splat(1.0),
      ambient: Vec3::splat(0.25),
      falloff: 0.22,
      occlusion: 1.0,
      softness: 2.5,
      nearness: 1.2,
    }
  }
}

/// how the picture is degraded on its way to the window
#[derive(Clone, Copy, Debug)]
pub struct Look {
  /// window pixels to one rendered pixel along each axis. one is sharp, more is blockier
  pub pixel_size: f32,
  /// the steps each color channel is cut to, with an ordered dither between them. fewer
  /// than two leaves the colors alone
  pub color_steps: u32,
  /// whether shadow edges are softened
  pub soft_shadows: bool,
  /// whether the edges of shapes are smoothed. an old render's are not
  pub antialias: bool,
  /// how much the brightest parts of the picture spill a haze over what is around them
  pub bloom: f32,
  /// how much darker the picture is toward its corners than in its middle
  pub vignette: f32,
  /// how far the picture is blurred, from zero for not at all to one for the softness of a
  /// bitmap that was drawn larger and scaled down
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
      vignette: 0.25,
      soften: 0.0,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Object {
  /// the object's shape. objects of one allocation share it on the gpu, where it stays for
  /// as long as scenes keep using it, so a shape is best made once and kept
  pub mesh: Arc<Mesh>,
  /// where the mesh goes. a rotation and a move, with at most an even scale
  pub transform: Mat4,
  pub material: Material,
}

impl Scene {
  /// the direction from the panel toward the viewer
  pub(crate) fn toward_viewer(&self) -> Vec3 {
    Vec3::new(-self.lean.x, -self.lean.y, 1.0).normalize()
  }

  /// panel points to clip space, the nearest height at depth zero
  pub(crate) fn view(&self) -> Mat4 {
    self.seen_leaning(self.lean)
  }

  /// panel points to clip space as seen from straight above whatever the lean, for the map
  /// of how high everything on the panel is
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
