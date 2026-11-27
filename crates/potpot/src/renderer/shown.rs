//! The pictures objects show, each drawn into a texture of its own.

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use potpot_controls::flat::Flat;

use super::damage;
use super::flat::{Drawing, Mapping, Pipeline};
use crate::scene::Object;

/// The format of a picture's texture.
///
/// The texture holds colors as they are to look, times their cover, which is how a
/// [`FlatRenderer`] blends them.
///
/// [`FlatRenderer`]: super::FlatRenderer
pub(super) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Where an object's picture lies on the scene's picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Frame {
  /// The pixels of the scene's picture the object is seen in, and one more on every side.
  ///
  /// They are `[left, top, width, height]`, and the picture's texture is as large.
  pub(super) place: [u32; 4],
  /// The way the picture's points land on the texture.
  mapping: Mapping,
}

impl Frame {
  /// Returns the frame of an object's picture.
  ///
  /// `onto` is the matrix from the panel's points to the pixels of the scene's picture, which
  /// is `size` pixels, with `scale` pixels to a point. The picture lies where the object is
  /// highest. An object that is not seen, or has no vertex, has none.
  pub(super) fn of(
    object: &Object,
    onto: Mat4,
    size: [u32; 2],
    scale: [f32; 2],
    rough: bool,
  ) -> Option<Self> {
    let corners = damage::corners(object.mesh.bounds()?)
      .map(|corner| object.transform.transform_point3(corner));
    let high = corners
      .clone()
      .map(|corner| corner.z)
      .fold(f32::MIN, f32::max);
    let (least, most) = corners.fold((Vec2::MAX, Vec2::MIN), |(least, most), corner| {
      let seen = onto.transform_point3(corner).truncate();
      (least.min(seen), most.max(seen))
    });
    let size = Vec2::from(size.map(|side| side as f32));
    let least = (least - 1.0).floor().max(Vec2::ZERO);
    let most = (most + 1.0).ceil().min(size);
    // a box that is no number, as of an object placed nowhere, is not seen either
    if !(least.x < most.x && least.y < most.y) {
      return None;
    }
    let origin = onto.transform_point3(Vec3::new(0.0, 0.0, high)).truncate() - least;
    let [left, top] = least.to_array().map(|side| side as u32);
    let [right, foot] = most.to_array().map(|side| side as u32);
    Some(Self {
      place: [left, top, right - left, foot - top],
      mapping: Mapping {
        scale,
        shift: origin.to_array(),
        rough,
      },
    })
  }

  /// Returns the way the scene shader finds the texel of the picture at a point.
  ///
  /// The point is in the view's clip space, and the scene's picture is `size` pixels. The
  /// texel is the point's x and y times the first two, plus the last two.
  pub(super) fn texel_of(&self, size: [u32; 2]) -> [f32; 4] {
    let [width, height] = size.map(|side| side as f32 / 2.0);
    let [left, top, ..] = self.place.map(|side| side as f32);
    [width, -height, width - left, height - top]
  }
}

/// Returns whether two lists of layers show the same picture.
///
/// Layers that are one allocation are the same without their shapes being compared.
pub(super) fn same(layers: &[Arc<Flat>], others: &[Arc<Flat>]) -> bool {
  layers.len() == others.len()
    && (layers.iter().zip(others)).all(|(one, other)| Arc::ptr_eq(one, other) || one == other)
}

/// An object's picture, drawn into a texture.
#[derive(Debug)]
pub(super) struct Shown {
  layers: Vec<Arc<Flat>>,
  pub(super) frame: Frame,
  pub(super) texture: wgpu::Texture,
  /// The texture, for the scene pass and the bounce pass to read.
  pub(super) group: wgpu::BindGroup,
  drawing: Drawing,
}

impl Shown {
  /// Returns the layers the picture was drawn from.
  pub(super) fn layers(&self) -> &[Arc<Flat>] {
    &self.layers
  }
}

/// What drawing pictures into their textures takes.
pub(super) struct Painter<'a> {
  pub(super) device: &'a wgpu::Device,
  pub(super) queue: &'a wgpu::Queue,
  pub(super) pipeline: &'a Pipeline,
  pub(super) layout: &'a wgpu::BindGroupLayout,
}

impl Painter<'_> {
  /// Returns an object's picture as it is to be shown.
  ///
  /// The picture `held` is kept if it shows the same layers in the same frame, and otherwise
  /// drawn again, into its texture where the texture is as large. A picture is drawn with
  /// `encoder`, which is made the first time one is.
  pub(super) fn show(
    &self,
    encoder: &mut Option<wgpu::CommandEncoder>,
    held: Option<Shown>,
    layers: &[Arc<Flat>],
    frame: Frame,
  ) -> Shown {
    let [.., width, height] = frame.place;
    let mut shown = match held {
      Some(held) if held.frame == frame && same(&held.layers, layers) => return held,
      Some(held) if held.frame.place[2..] == frame.place[2..] => held,
      _ => {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
          label: Some("potpot shown picture"),
          size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
          },
          mip_level_count: 1,
          sample_count: 1,
          dimension: wgpu::TextureDimension::D2,
          format: FORMAT,
          // and copied from, which is how the tests read it
          usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
          view_formats: &[],
        });
        Shown {
          group: bound(self.device, self.layout, &texture),
          texture,
          layers: Vec::new(),
          frame,
          drawing: self.pipeline.drawing(self.device),
        }
      }
    };
    shown.layers.clear();
    shown.layers.extend(layers.iter().map(Arc::clone));
    shown.frame = frame;
    let shapes = layers.iter().flat_map(|layer| layer.shapes());
    let place = [0, 0, width, height];
    (shown.drawing).set(self.device, self.queue, shapes, frame.mapping, place);
    let view = (shown.texture).create_view(&wgpu::TextureViewDescriptor::default());
    let encoder = encoder.get_or_insert_with(|| {
      (self.device).create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("potpot shown pictures"),
      })
    });
    self.pipeline.draw(encoder, &view, &shown.drawing, true);
    shown
  }
}

/// Returns a bind group that holds a picture's texture for the scene pass and the bounce pass.
pub(super) fn bound(
  device: &wgpu::Device,
  layout: &wgpu::BindGroupLayout,
  texture: &wgpu::Texture,
) -> wgpu::BindGroup {
  let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
  device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("potpot shown picture"),
    layout,
    entries: &[wgpu::BindGroupEntry {
      binding: 0,
      resource: wgpu::BindingResource::TextureView(&view),
    }],
  })
}
