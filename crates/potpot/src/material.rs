//! Surface materials for the objects of a scene.

use glam::Vec3;

/// A surface, as its color and how it takes the light.
///
/// The shading follows Filament's standard model: a GGX highlight over Lambert diffuse, in the
/// metallic workflow, with an optional second highlight from a clear coat.
#[derive(Clone, Copy, Debug)]
pub struct Material {
  /// The base color, as sRGB-encoded red, green and blue.
  ///
  /// It is the color under white light of a surface that is not metal, and the color of the
  /// highlight of one that is.
  pub color: Vec3,
  /// How far the surface is metal, from zero to one.
  pub metallic: f32,
  /// The roughness, from zero for a mirror to one.
  ///
  /// The roughness is perceptual: its square widens the highlight. Under 0.045 it draws as
  /// 0.045.
  pub roughness: f32,
  /// The way the highlight is drawn out by fine grooves.
  pub anisotropy: Anisotropy,
  /// The strength of the clear coat, from zero to one.
  ///
  /// The coat is a layer of varnish or glass over the surface, with a highlight of its own.
  pub clearcoat: f32,
  /// How rough the clear coat is, as [`Material::roughness`] is.
  pub clearcoat_roughness: f32,
  /// How much of the color shows unlit, for lamps and displays.
  pub emission: f32,
  /// The radius the emission fades over.
  ///
  /// At it the emission has faded to about two thirds. The radius is measured from the
  /// object's own z axis, for something lit from inside by one small light under its middle.
  /// Far past it the emission settles at about four tenths. Zero glows evenly.
  pub emission_radius: f32,
  /// How rough the surface is with fine grain.
  ///
  /// Cast metal, powder coat and rubber have such a grain. It is made up on the spot and bends
  /// the light, so it shows most where the light glances.
  pub grain: f32,
  /// How worn the surface is.
  ///
  /// Wear shows as blotches of grime and patches of fine scratches that catch the light. Like
  /// the grain it is made up on the spot from where a point is on the panel.
  pub wear: f32,
  /// Whether the surface throws no shadow.
  ///
  /// Such a surface also keeps no light off what is near it, whatever the shape of its object,
  /// as print and a screen's picture do. It is lit and shadowed itself as any other, and its
  /// own light is thrown on what is around it.
  ///
  /// A change of its shape or place costs the drawing of where it is seen. While
  /// [`Light::bounce`](crate::Light::bounce) is on, it also costs the drawing of everything its
  /// light is thrown on.
  pub shadowless: bool,
}

/// The way grooves in a surface draw out its highlight.
///
/// Each kind holds the strength, from zero to one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anisotropy {
  /// No grooves.
  None,
  /// Straight grooves along the panel's x, as brushing makes.
  Brushed(f32),
  /// Circular grooves around the object's own z axis.
  ///
  /// Metal turned on a lathe has them.
  Spun(f32),
}

impl Material {
  /// Creates paint, print or anything else without a shine.
  pub fn matte(color: Vec3) -> Self {
    Self {
      color,
      metallic: 0.0,
      roughness: 0.75,
      anisotropy: Anisotropy::None,
      clearcoat: 0.0,
      clearcoat_roughness: 0.0,
      emission: 0.0,
      emission_radius: 0.0,
      grain: 0.0,
      wear: 0.0,
      shadowless: false,
    }
  }

  /// Creates polished plastic, like Bakelite.
  pub fn glossy(color: Vec3) -> Self {
    Self {
      roughness: 0.2,
      ..Self::matte(color)
    }
  }

  /// Creates plastic with a soft sheen, like a rubbery coating.
  pub fn satin(color: Vec3) -> Self {
    Self {
      roughness: 0.5,
      ..Self::matte(color)
    }
  }

  /// Creates polished metal, which is mostly what it mirrors.
  pub fn chrome(color: Vec3) -> Self {
    Self {
      metallic: 1.0,
      roughness: 0.1,
      ..Self::matte(color)
    }
  }

  /// Creates metal turned on a lathe, like the aluminium face of a knob.
  pub fn spun(color: Vec3) -> Self {
    Self {
      metallic: 1.0,
      roughness: 0.3,
      anisotropy: Anisotropy::Spun(0.7),
      ..Self::matte(color)
    }
  }

  /// Creates a clear glossy pane over the color under it.
  pub fn pane(color: Vec3) -> Self {
    Self {
      roughness: 0.3,
      clearcoat: 1.0,
      clearcoat_roughness: 0.06,
      ..Self::matte(color)
    }
  }

  /// Creates the lit face of a screen behind glass.
  ///
  /// The face lights itself in `color`, its black, so that the scene's light and shadows leave
  /// it even. The glass is a dimmer mirror than [`Material::pane`].
  pub fn screen(color: Vec3) -> Self {
    Self {
      emission: 1.0,
      clearcoat: 0.5,
      ..Self::pane(color)
    }
  }

  /// Creates the end of a clear plastic rod that carries a light.
  ///
  /// The rod carries the light up to the panel from under it. The material is dark and smoky
  /// until [`Material::lit`].
  pub fn pipe() -> Self {
    Self {
      roughness: 0.12,
      ..Self::matte(Vec3::splat(0.045))
    }
  }

  /// Creates a sheet of metal brushed in straight lines.
  pub fn brushed(color: Vec3) -> Self {
    Self {
      metallic: 1.0,
      roughness: 0.4,
      anisotropy: Anisotropy::Brushed(0.6),
      ..Self::matte(color)
    }
  }

  /// Creates paint with a fine rough texture.
  ///
  /// A powder coat and a wrinkle finish have such a texture.
  pub fn textured(color: Vec3) -> Self {
    Self {
      roughness: 0.6,
      grain: 1.0,
      ..Self::matte(color)
    }
  }

  /// Creates soft cloudy rubber, like a controller's pads.
  pub fn silicone(color: Vec3) -> Self {
    Self {
      roughness: 0.6,
      grain: 0.2,
      ..Self::matte(color)
    }
  }

  /// Creates ink, for lettering and the marks of a scale.
  ///
  /// The ink is a little glossier than powder coat.
  pub fn ink(color: Vec3) -> Self {
    Self {
      roughness: 0.35,
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
      emission: level,
      emission_radius: radius,
      ..self
    }
  }

  /// Creates something that lights itself.
  pub fn lamp(color: Vec3) -> Self {
    Self {
      roughness: 1.0,
      emission: 1.0,
      ..Self::matte(color)
    }
  }
}
