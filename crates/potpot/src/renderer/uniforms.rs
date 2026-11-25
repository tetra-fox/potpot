//! what the shaders are told each frame, laid out the way scene.wgsl and present.wgsl read it

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

use super::SHADOW_SIZE;
use crate::scene::{LAMPS, Look, Object, Scene};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(super) struct Globals {
  view: [[f32; 4]; 4],
  light_view: [[f32; 4]; 4],
  toward_light: [f32; 4],
  toward_viewer: [f32; 4],
  light_color: [f32; 4],
  ambient: [f32; 4],
  shadows: [f32; 4],
  overhead: [[f32; 4]; 4],
  /// the panel's width and height, and the lowest and highest anything on it reaches
  panel: [f32; 4],
  /// each lamp's place with its reach, and then its color with its radius
  lamps: [[[f32; 4]; 2]; LAMPS],
}

impl Globals {
  /// whether light comes off surfaces onto their neighbors at all
  pub(super) fn bounces(&self) -> bool {
    self.ambient[3] > 0.0
  }

  /// the lowest anything on the panel reaches
  pub(super) fn lowest(&self) -> f32 {
    self.panel[2]
  }

  pub(super) fn of(scene: &Scene) -> Self {
    let light = scene.light;
    Self {
      view: scene.view().to_cols_array_2d(),
      light_view: scene.light_view().to_cols_array_2d(),
      toward_light: light.toward.normalize().extend(light.softness).into(),
      toward_viewer: (scene.toward_viewer())
        .extend(light.nearness * scene.size.max_element())
        .into(),
      light_color: light.color.extend(light.falloff).into(),
      ambient: light.ambient.extend(light.bounce).into(),
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

/// one object's place and material, read once per instance
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(super) struct Placed {
  model: [[f32; 4]; 4],
  color: [f32; 4],
  highlight: [f32; 4],
  finish: [f32; 4],
  texture: [f32; 4],
}

impl Placed {
  /// the four columns of the model and then the four of the material, after the two
  /// locations a vertex takes
  const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
    6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4
  ];

  pub(super) fn of(object: &Object) -> Self {
    let material = object.material;
    Self {
      model: object.transform.to_cols_array_2d(),
      color: material.color.extend(material.glow).into(),
      highlight: [
        material.specular,
        material.shininess,
        material.glow_radius,
        0.0,
      ],
      finish: [
        material.reflect,
        material.metal,
        material.spun,
        material.glaze,
      ],
      texture: [material.grain, material.brushed, material.wear, 0.0],
    }
  }

  pub(super) fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: size_of::<Self>() as u64,
      step_mode: wgpu::VertexStepMode::Instance,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

/// how the picture is degraded on its way to the target
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(super) struct Present {
  /// the color steps, one when the target encodes srgb itself, the bloom and the vignette
  options: [f32; 4],
  /// x how far the picture is blurred
  more: [f32; 4],
}

impl Present {
  pub(super) fn of(look: Look, target_encodes: bool) -> Self {
    Self {
      options: [
        look.color_steps as f32,
        f32::from(u8::from(target_encodes)),
        look.bloom,
        look.vignette,
      ],
      more: [look.soften, 0.0, 0.0, 0.0],
    }
  }
}

#[cfg(test)]
mod tests {
  use std::mem::offset_of;

  use wgpu::naga;

  use super::*;
  use crate::mesh::Vertex;
  use crate::renderer::pipelines::{PRESENT_SOURCE, VERTEX_ATTRIBUTES, scene_source};

  /// a shader as wgpu will take it, which is parsed and then checked
  fn shader(source: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(source).expect("the shader to parse");
    naga::valid::Validator::new(
      naga::valid::ValidationFlags::all(),
      naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the shader to be valid");
    module
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

  /// each field of a shader's struct and where it starts, and the size of the whole
  fn fields(module: &naga::Module, name: &str) -> (Vec<(String, usize)>, usize) {
    let (members, size) = members(module, name);
    let fields = (members.iter())
      .map(|member| (member.name.clone().unwrap(), member.offset as usize))
      .collect();
    (fields, size)
  }

  /// asserts that a shader's struct takes the attributes of a vertex buffer, one field
  /// each, in order, at the same locations and of the same sizes
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

  /// each backend is handed the shaders in a language of its own, which naga writes them
  /// in when a pipeline is made. a shader one of them cannot be written in fails there, on
  /// the user's machine, so all of them are written here
  #[test]
  fn every_shader_can_be_written_for_every_backend() {
    use ::naga::back::{hlsl, msl, spv};

    let shaders = [
      ("scene", scene_source()),
      ("present", PRESENT_SOURCE.to_owned()),
    ];
    for (name, source) in shaders {
      let module = ::naga::front::wgsl::parse_str(&source).expect("the shader to parse");
      let info = ::naga::valid::Validator::new(
        ::naga::valid::ValidationFlags::all(),
        ::naga::valid::Capabilities::empty(),
      )
      .validate(&module)
      .expect("the shader to be valid");

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
    let module = shader(&scene_source());
    let globals = named([
      ("view", offset_of!(Globals, view)),
      ("light_view", offset_of!(Globals, light_view)),
      ("toward_light", offset_of!(Globals, toward_light)),
      ("toward_viewer", offset_of!(Globals, toward_viewer)),
      ("light_color", offset_of!(Globals, light_color)),
      ("ambient", offset_of!(Globals, ambient)),
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
    let last = Placed::ATTRIBUTES[7];
    assert_eq!(last.offset + last.format.size(), size_of::<Placed>() as u64);
  }

  #[test]
  fn the_present_shader_reads_what_it_is_given() {
    let module = shader(PRESENT_SOURCE);
    let present = named([
      ("options", offset_of!(Present, options)),
      ("more", offset_of!(Present, more)),
    ]);
    assert_eq!(fields(&module, "Present"), (present, size_of::<Present>()));
  }
}
