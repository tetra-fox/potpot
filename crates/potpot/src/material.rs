//! Surface materials for the objects of a scene.

use glam::Vec3;

/// A surface, as its color and how it takes the light.
#[derive(Clone, Copy, Debug)]
pub struct Material {
  /// The color under white light, as sRGB-encoded red, green and blue.
  pub color: Vec3,
  /// The strength of the highlight.
  pub specular: f32,
  /// The tightness of the highlight.
  pub shininess: f32,
  /// How much of the color shows unlit, for lamps and displays.
  pub glow: f32,
  /// The radius at which the glow has faded to about two thirds.
  ///
  /// The radius is measured from the object's own z axis, for something lit from inside by one
  /// small light under its middle. Far past it the glow settles at about four tenths. Zero
  /// glows evenly.
  pub glow_radius: f32,
  /// How much of the room around the panel the surface mirrors.
  ///
  /// The room is a soft window where the light is and a pale band at the horizon.
  pub reflect: f32,
  /// How far the surface is metal.
  ///
  /// Metal mirrors in its own color and has little color under the light otherwise.
  pub metal: f32,
  /// How far the highlight is drawn out into bright wedges.
  ///
  /// The wedges are those of metal turned on a lathe, around the object's own z axis.
  pub spun: f32,
  /// The size of the object if it is a clear pane.
  ///
  /// A pane of glass or clear plastic shows a slanted sheen across the corner of it nearest the
  /// light. Zero is no pane.
  pub glaze: f32,
  /// How rough the surface is with fine grain.
  ///
  /// Cast metal, powder coat and rubber have such a grain. It is made up on the spot and bends
  /// the light, so it shows most where the light glances.
  pub grain: f32,
  /// How deeply the surface is brushed in straight lines along x.
  ///
  /// A sheet of aluminum is brushed this way.
  pub brushed: f32,
  /// How worn the surface is.
  ///
  /// Wear shows as blotches of grime and patches of fine scratches that catch the light. Like
  /// the grain it is made up on the spot from where a point is on the panel.
  pub wear: f32,
}

impl Material {
  /// Creates paint, print or anything else without a shine.
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

  /// Creates polished plastic, like bakelite.
  pub fn glossy(color: Vec3) -> Self {
    Self {
      specular: 0.42,
      shininess: 70.0,
      reflect: 0.6,
      ..Self::matte(color)
    }
  }

  /// Creates plastic with a soft sheen, like a rubbery coating.
  pub fn satin(color: Vec3) -> Self {
    Self {
      specular: 0.16,
      shininess: 24.0,
      reflect: 0.1,
      ..Self::matte(color)
    }
  }

  /// Creates polished metal, which is mostly what it mirrors.
  pub fn chrome(color: Vec3) -> Self {
    Self {
      specular: 1.0,
      shininess: 200.0,
      reflect: 1.0,
      metal: 1.0,
      ..Self::matte(color)
    }
  }

  /// Creates metal turned on a lathe, like the aluminum face of a knob.
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

  /// Creates a clear glossy pane `size` across, over the color under it.
  pub fn pane(color: Vec3, size: f32) -> Self {
    Self {
      specular: 0.6,
      shininess: 260.0,
      reflect: 0.7,
      glaze: size,
      ..Self::matte(color)
    }
  }

  /// Creates the lit face of a screen behind glass.
  ///
  /// The glass is `size` across. The face lights itself in `color`, its black, so that the scene's
  /// light and shadows leave it even and no [`Lamp`](crate::Lamp) shows in its glass. The glass is
  /// a dimmer mirror than [`Material::pane`], with the same sheen.
  pub fn screen(color: Vec3, size: f32) -> Self {
    Self {
      specular: 0.12,
      shininess: 260.0,
      glow: 1.0,
      reflect: 0.3,
      glaze: size,
      ..Self::matte(color)
    }
  }

  /// Creates the end of a clear plastic rod that carries a light.
  ///
  /// The rod carries the light up to the panel from under it. The material is dark and smoky
  /// until [`Material::lit`].
  pub fn pipe() -> Self {
    Self {
      specular: 0.8,
      shininess: 120.0,
      reflect: 0.45,
      ..Self::matte(Vec3::splat(0.045))
    }
  }

  /// Creates a sheet of metal brushed in straight lines.
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

  /// Creates paint with a fine rough texture.
  ///
  /// A powder coat and a wrinkle finish have such a texture.
  pub fn textured(color: Vec3) -> Self {
    Self {
      specular: 0.14,
      shininess: 18.0,
      grain: 1.0,
      ..Self::matte(color)
    }
  }

  /// Creates soft cloudy rubber, like a controller's pads.
  pub fn silicone(color: Vec3) -> Self {
    Self {
      specular: 0.18,
      shininess: 12.0,
      reflect: 0.05,
      grain: 0.2,
      ..Self::matte(color)
    }
  }

  /// Returns the material lit from inside in a color.
  ///
  /// The light is under the material's middle and fades toward `radius` away. `level` runs from
  /// zero for off to one for fully on, and takes the material's own color over to the light's
  /// on the way.
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

  /// Creates something that lights itself.
  pub fn lamp(color: Vec3) -> Self {
    Self {
      specular: 0.0,
      shininess: 1.0,
      glow: 1.0,
      ..Self::matte(color)
    }
  }
}
