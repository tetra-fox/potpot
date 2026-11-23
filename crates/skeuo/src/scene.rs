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
}

impl Default for Light {
  fn default() -> Self {
    Self {
      toward: Vec3::new(-0.45, -0.6, 1.0),
      color: Vec3::splat(1.0),
      ambient: Vec3::splat(0.25),
      falloff: 0.22,
      occlusion: 1.0,
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
  /// how much the brightest parts of the picture spill a haze over what is around them
  pub bloom: f32,
  /// how much darker the picture is toward its corners than in its middle
  pub vignette: f32,
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
  /// how far from the object's own z axis the glow has faded to about a third, for
  /// something lit from inside by one small light under its middle. zero glows evenly
  pub glow_radius: f32,
  /// how much of the room around the panel the surface mirrors: a soft window where the
  /// light is and a pale band at the horizon
  pub reflect: f32,
  /// how far the surface is metal, which mirrors in its own color and has little color
  /// under the light otherwise
  pub metal: f32,
  /// how far the highlight is drawn out into the bright wedges of metal turned on a lathe,
  /// around the object's own z axis
  pub spun: f32,
  /// the size of the object if it is a pane of glass or clear plastic, which shows a
  /// slanted sheen across the corner of it nearest the light. zero is no pane
  pub glaze: f32,
  /// how rough the surface is with fine grain, like cast metal, powder coat or rubber. it
  /// is made up on the spot and bends the light, so it shows most where the light glances
  pub grain: f32,
  /// how deeply the surface is brushed in straight lines along x, like a sheet of aluminum
  pub brushed: f32,
}

impl Material {
  /// paint, print and anything else without a shine
  pub fn matte(color: Vec3) -> Self {
    Self {
      color,
      specular: 0.05,
      shininess: 8.0,
      glow: 0.0,
      glow_radius: 0.0,
      reflect: 0.0,
      metal: 0.0,
      spun: 0.0,
      glaze: 0.0,
      grain: 0.0,
      brushed: 0.0,
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

  /// a clear glossy pane `size` across, over whatever color is under it
  pub fn pane(color: Vec3, size: f32) -> Self {
    Self {
      specular: 0.9,
      shininess: 140.0,
      reflect: 0.7,
      glaze: size,
      ..Self::matte(color)
    }
  }

  /// the end of a clear plastic rod that carries a light up to the panel from under it,
  /// dark and smoky until [`Material::lit`]
  pub fn pipe() -> Self {
    Self {
      specular: 0.8,
      shininess: 120.0,
      reflect: 0.45,
      ..Self::matte(Vec3::splat(0.045))
    }
  }

  /// a sheet of metal brushed in straight lines
  pub fn brushed(color: Vec3) -> Self {
    Self {
      specular: 0.45,
      shininess: 30.0,
      reflect: 0.2,
      metal: 0.6,
      brushed: 1.0,
      ..Self::matte(color)
    }
  }

  /// paint with a fine rough texture, like a powder coat or a wrinkle finish
  pub fn textured(color: Vec3) -> Self {
    Self {
      specular: 0.14,
      shininess: 18.0,
      grain: 1.0,
      ..Self::matte(color)
    }
  }

  /// soft cloudy rubber, like a controller's pads
  pub fn silicone(color: Vec3) -> Self {
    Self {
      specular: 0.18,
      shininess: 12.0,
      reflect: 0.05,
      grain: 0.2,
      ..Self::matte(color)
    }
  }

  /// the material lit from inside in a color, by a light under its middle that fades toward
  /// `radius` away. `level` runs from zero for off to one for fully on, and takes the
  /// material's own color over to the light's on the way
  #[must_use]
  pub fn lit(self, color: Vec3, level: f32, radius: f32) -> Self {
    let level = level.clamp(0.0, 1.0);
    Self {
      color: self.color.lerp(color, level),
      glow: level,
      glow_radius: radius,
      ..self
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

  /// panel points to clip space as seen from straight above whatever the lean, for the map
  /// of how high everything on the panel is
  pub(crate) fn overhead(&self) -> Mat4 {
    Self {
      lean: Vec2::ZERO,
      meshes: Arc::clone(&self.meshes),
      objects: Vec::new(),
      lamps: Vec::new(),
      ..*self
    }
    .view()
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
