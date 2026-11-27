//! The values the shaders are told each frame.
//!
//! Each is laid out the way `scene.wgsl` and `present.wgsl` read it.

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use super::SHADOW_SIZE;
use super::damage::Spread;
use super::textures;
use crate::material::{Anisotropy, Textures};
use crate::scene::{LAMPS, Light, Look, Object, Scene};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct Globals {
  pub(super) view: [[f32; 4]; 4],
  pub(super) light_view: [[f32; 4]; 4],
  /// The way to the light, and how wide shadow edges are.
  pub(super) toward_light: [f32; 4],
  /// The way to the viewer, and how bright the studio is.
  toward_viewer: [f32; 4],
  light_color: [f32; 4],
  /// The shadow options, one in each component.
  ///
  /// - One when shadow edges are softened
  /// - The size of a shadow map texel
  /// - The count of lamps
  /// - How much the light all around is kept out of corners
  shadows: [f32; 4],
  pub(super) overhead: [[f32; 4]; 4],
  /// The panel's width and height, and the lowest and highest anything on it reaches.
  pub(super) panel: [f32; 4],
  /// Each lamp's place with its reach, and then its color with its radius.
  pub(super) lamps: [[[f32; 4]; 2]; LAMPS],
}

// each is equal to another of the same bytes, as the GPU is sent them, so that a value holding
// a NaN is equal to itself and a scene that holds one settles
macro_rules! equal_by_bytes {
  ($($sent:ty),*) => {
    $(
      impl PartialEq for $sent {
        fn eq(&self, other: &Self) -> bool {
          bytemuck::bytes_of(self) == bytemuck::bytes_of(other)
        }
      }
    )*
  };
}
equal_by_bytes!(Globals, Placed, Present);

impl Globals {
  /// Returns whether light comes off surfaces onto their neighbors at all.
  pub(super) fn bounces(&self) -> bool {
    self.light_color[3] > 0.0
  }

  /// Returns whether two scenes differ in their lamps at most.
  ///
  /// Everything else of the two is the same: the view, the light and the panel.
  pub(super) fn lit_alike(&self, other: &Self) -> bool {
    let but_lamps = |globals: &Self| {
      let mut globals = *globals;
      globals.lamps = [[[0.0; 4]; 2]; LAMPS];
      // the count of lamps
      globals.shadows[2] = 0.0;
      globals
    };
    but_lamps(self) == but_lamps(other)
  }

  pub(super) fn of(scene: &Scene) -> Self {
    let light = scene.light;
    assert!(light.toward != Vec3::ZERO, "the light comes from some way");
    Self {
      view: scene.view().to_cols_array_2d(),
      light_view: scene.light_view().to_cols_array_2d(),
      toward_light: light.toward.normalize().extend(light.softness).into(),
      toward_viewer: scene.toward_viewer().extend(light.environment).into(),
      light_color: light.color.extend(light.bounce).into(),
      shadows: [
        f32::from(u8::from(scene.look.soft_shadows)),
        1.0 / SHADOW_SIZE as f32,
        scene.lamps.len().min(LAMPS) as f32,
        light.occlusion,
      ],
      overhead: scene.overhead().to_cols_array_2d(),
      panel: [
        scene.size.x,
        scene.size.y,
        scene.heights[0],
        scene.heights[1],
      ],
      lamps: std::array::from_fn(|index| {
        scene.lamps.get(index).map_or([[0.0; 4]; 2], |lamp| {
          [
            lamp.at.extend(lamp.reach).into(),
            lamp.color.extend(lamp.radius).into(),
          ]
        })
      }),
    }
  }
}

/// One object's place and material, read once per instance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct Placed {
  pub(super) model: [[f32; 4]; 4],
  /// The base color and the emission.
  color: [f32; 4],
  /// How metal and how rough it is, and its clear coat.
  finish: [f32; 4],
  /// Its grain, wear and emission radius, and a mark.
  ///
  /// The mark is one if the object throws no shadow. The shaders make nothing of it, and it is
  /// here for a change of it to be told as any other of the object's.
  texture: [f32; 4],
  /// How strongly it is brushed and how strongly spun.
  anisotropy: [f32; 2],
  /// Its texture set, how the set is laid on it, and its display's cells.
  ///
  /// The set is its place among the sets plus one, or zero for none. Then come the points one
  /// tile covers, one when the set wraps around the object's own z axis, and the pitch of the
  /// cells of a display.
  textures: [f32; 4],
  /// The way a point of the view's clip space is found in the object's picture.
  ///
  /// The texel of the picture at a point is its x and y times the first two, plus the last two.
  /// It is all zeros for an object with no picture.
  pub(super) picture: [f32; 4],
}

impl Placed {
  /// The four columns of the model, the five parts of the material and the picture's place.
  ///
  /// They come after the two locations a vertex takes.
  const ATTRIBUTES: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
    2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
    6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x2, 10 => Float32x4,
    11 => Float32x4
  ];

  pub(super) fn of(object: &Object) -> Self {
    let material = object.material;
    let anisotropy = match material.anisotropy {
      Anisotropy::None => [0.0; 2],
      Anisotropy::Brushed(strength) => [strength, 0.0],
      Anisotropy::Spun(strength) => [0.0, strength],
    };
    Self {
      model: object.transform.to_cols_array_2d(),
      color: material.color.extend(material.emission).into(),
      finish: [
        material.metallic,
        material.roughness,
        material.clearcoat,
        material.clearcoat_roughness,
      ],
      texture: [
        material.grain,
        material.wear,
        material.emission_radius,
        f32::from(u8::from(material.shadowless)),
      ],
      anisotropy,
      textures: material
        .textures
        .map_or([0.0, 0.0, 0.0, material.pitch], |set| {
          let (index, tile) = match set {
            Textures::Walnut(tile) => (textures::WALNUT, tile),
            Textures::Hammertone(tile) => (textures::HAMMERTONE, tile),
            Textures::BrushedAluminium(tile) => (textures::BRUSHED_ALUMINIUM, tile),
            Textures::PowderCoat(tile) => (textures::POWDER_COAT, tile),
          };
          let wraps = matches!(material.anisotropy, Anisotropy::Spun(_));
          [
            index as f32 + 1.0,
            tile,
            f32::from(u8::from(wraps)),
            material.pitch,
          ]
        }),
      picture: [0.0; 4],
    }
  }

  /// Returns whether the object throws no shadow.
  ///
  /// One that throws none is in neither the shadow map nor the height map.
  pub(super) fn shadowless(&self) -> bool {
    self.texture[3] != 0.0
  }

  pub(super) fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: size_of::<Self>() as u64,
      step_mode: wgpu::VertexStepMode::Instance,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

/// The way the picture is degraded on its way to the target.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct Present {
  /// The options of the present pass, one in each component.
  ///
  /// - The color steps
  /// - One when the target encodes sRGB itself
  /// - The bloom
  /// - The vignette
  options: [f32; 4],
  /// The blur, and in y the exposure as a multiple.
  more: [f32; 4],
  /// The Gaussian the glow map is blurred by.
  ///
  /// It is the falloff across and down, then the cells read to each side across and down.
  spread: [f32; 4],
}

impl Present {
  pub(super) fn of(look: Look, light: &Light, target_encodes: bool, spread: Spread) -> Self {
    let [across, down] = spread.falloff;
    let taps = spread.taps.map(|taps| taps as f32);
    Self {
      options: [
        look.color_steps as f32,
        f32::from(u8::from(target_encodes)),
        look.bloom,
        look.vignette,
      ],
      more: [look.soften, light.exposure.exp2(), 0.0, 0.0],
      spread: [across, down, taps[0], taps[1]],
    }
  }
}

#[cfg(test)]
mod tests {
  use std::mem::offset_of;

  use naga::valid::ModuleInfo;

  use super::*;
  use crate::mesh::Vertex;
  use crate::renderer::pipelines::{
    RESOLVE_SOURCE, VERTEX_ATTRIBUTES, present_source, scene_source,
  };

  /// Returns a shader as `wgpu` takes it, parsed and then checked.
  ///
  /// What the check found out about the shader comes with it.
  fn shader(source: &str) -> (naga::Module, ModuleInfo) {
    let module = naga::front::wgsl::parse_str(source).expect("the shader parses");
    let info = naga::valid::Validator::new(
      naga::valid::ValidationFlags::all(),
      naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the shader is valid");
    (module, info)
  }

  fn members<'a>(module: &'a naga::Module, name: &str) -> (&'a [naga::StructMember], usize) {
    let found = (module.types.iter())
      .find(|(_, found)| found.name.as_deref() == Some(name))
      .unwrap_or_else(|| panic!("no struct called {name}"));
    let naga::TypeInner::Struct { members, span } = &found.1.inner else {
      panic!("{name} is not a struct");
    };
    (members, *span as usize)
  }

  /// Returns each field of a shader's struct with its start, and the struct's size.
  fn fields(module: &naga::Module, name: &str) -> (Vec<(String, usize)>, usize) {
    let (members, size) = members(module, name);
    let fields = (members.iter())
      .map(|member| (member.name.clone().unwrap(), member.offset as usize))
      .collect();
    (fields, size)
  }

  /// Asserts that a shader's struct takes the attributes of a vertex buffer.
  ///
  /// Each field takes one attribute, in order, at the same location and of the same size.
  fn assert_takes(module: &naga::Module, name: &str, attributes: &[wgpu::VertexAttribute]) {
    let (members, _) = members(module, name);
    assert_eq!(members.len(), attributes.len(), "{name}");
    for (member, attribute) in members.iter().zip(attributes) {
      let field = member.name.as_deref().unwrap();
      let Some(naga::Binding::Location { location, .. }) = member.binding else {
        panic!("{name}.{field} has no location");
      };
      assert_eq!(location, attribute.shader_location, "{name}.{field}");
      let size = module.types[member.ty].inner.size(module.to_ctx());
      assert_eq!(u64::from(size), attribute.format.size(), "{name}.{field}");
    }
  }

  fn named<const N: usize>(fields: [(&str, usize); N]) -> Vec<(String, usize)> {
    fields.map(|(name, at)| (name.to_owned(), at)).into()
  }

  /// Writes every shader in the language of every backend.
  ///
  /// Each backend is handed the shaders in a language of its own, which `naga` writes them in
  /// when a pipeline is made. A shader that cannot be written in one of them fails there, on
  /// the user's machine.
  #[test]
  fn every_shader_can_be_written_for_every_backend() {
    use naga::back::{hlsl, msl, spv};

    let shaders = [
      ("scene", scene_source()),
      ("present", present_source()),
      ("resolve", RESOLVE_SOURCE.to_owned()),
    ];
    for (name, source) in shaders {
      let (module, info) = shader(&source);
      let mut direct3d = String::new();
      let pipeline = hlsl::PipelineOptions::default();
      hlsl::Writer::new(&mut direct3d, &hlsl::Options::default(), &pipeline)
        .write(&module, &info, None)
        .unwrap_or_else(|error| panic!("{name} for direct3d: {error}"));
      msl::write_string(
        &module,
        &info,
        &msl::Options::default(),
        &msl::PipelineOptions::default(),
      )
      .unwrap_or_else(|error| panic!("{name} for metal: {error}"));
      spv::write_vec(&module, &info, &spv::Options::default(), None)
        .unwrap_or_else(|error| panic!("{name} for vulkan: {error}"));
    }
  }

  #[test]
  fn the_scene_shader_reads_what_it_is_given() {
    let (module, _) = shader(&scene_source());
    let globals = named([
      ("view", offset_of!(Globals, view)),
      ("light_view", offset_of!(Globals, light_view)),
      ("toward_light", offset_of!(Globals, toward_light)),
      ("toward_viewer", offset_of!(Globals, toward_viewer)),
      ("light_color", offset_of!(Globals, light_color)),
      ("shadows", offset_of!(Globals, shadows)),
      ("overhead", offset_of!(Globals, overhead)),
      ("panel", offset_of!(Globals, panel)),
      ("lamps", offset_of!(Globals, lamps)),
    ]);
    assert_eq!(fields(&module, "Globals"), (globals, size_of::<Globals>()));
    // a lamp is its place and then its color
    assert_eq!(
      fields(&module, "Lamp"),
      (
        named([("at", 0), ("color", 16)]),
        size_of::<[[f32; 4]; 2]>()
      )
    );

    assert_takes(&module, "Corner", &VERTEX_ATTRIBUTES);
    let last = VERTEX_ATTRIBUTES[1];
    assert_eq!(last.offset + last.format.size(), size_of::<Vertex>() as u64);
    assert_takes(&module, "Placed", &Placed::ATTRIBUTES);
    let last = Placed::ATTRIBUTES[Placed::ATTRIBUTES.len() - 1];
    assert_eq!(last.offset + last.format.size(), size_of::<Placed>() as u64);
  }

  #[test]
  fn the_present_shader_reads_what_it_is_given() {
    let (module, _) = shader(&present_source());
    let present = named([
      ("options", offset_of!(Present, options)),
      ("more", offset_of!(Present, more)),
      ("spread", offset_of!(Present, spread)),
    ]);
    assert_eq!(fields(&module, "Present"), (present, size_of::<Present>()));
  }
}
