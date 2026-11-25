//! what an object's surface is made of

use glam::Vec3;

/// a surface, as its color and how it takes the light
#[derive(Clone, Copy, Debug)]
pub struct Material {
  /// its color under white light, as red, green and blue encoded the way srgb is
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
  /// how worn the surface is: blotches of grime, and patches of fine scratches that catch
  /// the light. like the grain it is made up on the spot from where a point is on the panel
  pub wear: f32,
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
      wear: 0.0,
    }
  }

  /// polished plastic, like bakelite
  pub fn glossy(color: Vec3) -> Self {
    Self {
      specular: 0.42,
      shininess: 70.0,
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
      specular: 0.6,
      shininess: 260.0,
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
