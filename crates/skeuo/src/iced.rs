//! the scene as an iced widget. it only draws: put it at the bottom of a stack and lay the
//! widgets that take input over it.
//!
//! iced keeps one renderer for every panel in a window, which remembers a single place to
//! draw at, so a window has room for one panel

use iced_widget::core::{Length, Rectangle, mouse};
use iced_widget::renderer::wgpu::wgpu;
use iced_widget::shader::{self, Shader, Viewport};

use crate::renderer::{Place, Renderer};
use crate::scene::Scene;

/// a widget that fills its space with the scene
pub fn panel<Message>(scene: Scene) -> Shader<Message, Panel> {
  Shader::new(Panel { scene })
    .width(Length::Fill)
    .height(Length::Fill)
}

/// the [`shader::Program`] behind [`panel`]
#[derive(Clone, Debug)]
pub struct Panel {
  scene: Scene,
}

impl<Message> shader::Program<Message> for Panel {
  type State = ();
  type Primitive = Frame;

  fn draw(&self, _state: &(), _cursor: mouse::Cursor, _bounds: Rectangle) -> Frame {
    Frame {
      scene: self.scene.clone(),
    }
  }
}

/// one frame's scene on its way to the gpu
#[derive(Debug)]
pub struct Frame {
  scene: Scene,
}

impl shader::Primitive for Frame {
  type Pipeline = Pipeline;

  fn prepare(
    &self,
    pipeline: &mut Pipeline,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bounds: &Rectangle,
    viewport: &Viewport,
  ) {
    let place = *bounds * viewport.scale_factor();
    pipeline.place = Some(Place {
      x: place.x,
      y: place.y,
      width: place.width,
      height: place.height,
    });
    let size = [place.width, place.height].map(|side| side.round().max(1.0) as u32);
    pipeline.renderer.prepare(device, queue, &self.scene, size);
  }

  fn render(
    &self,
    pipeline: &Pipeline,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    clip_bounds: &Rectangle<u32>,
  ) {
    let Some(place) = pipeline.place else {
      return;
    };
    let clip = [
      clip_bounds.x,
      clip_bounds.y,
      clip_bounds.width,
      clip_bounds.height,
    ];
    pipeline.renderer.render(encoder, target, place, clip);
  }
}

/// the renderer iced keeps between frames
pub struct Pipeline {
  renderer: Renderer,
  /// where the last prepared frame goes on the window
  place: Option<Place>,
}

impl shader::Pipeline for Pipeline {
  fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
    Self {
      renderer: Renderer::new(device, format),
      place: None,
    }
  }
}
