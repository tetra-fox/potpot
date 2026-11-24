//! the open window: the panel drawn and put in it, and baseview's events turned into what
//! the panel is told

use std::cell::{Cell, RefCell, RefMut};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use baseview::dpi::LogicalSize;
use baseview::{
  EventStatus, HandlerError, MouseButton, MouseEvent, ScrollDelta, TimerHandle, WindowContext,
  WindowEvent, WindowHandler, WindowSize,
};
use controls::fit::{fitted, point_at};
use controls::flat::Flat;
use controls::interaction::{Action, Button, Edit, Event, Interaction, Key, Menu, Moment, Target};
use controls::sent::Sent;
use controls::stroke::Point;
use tracing::error;

use crate::flat::FlatRenderer;
use crate::open::{Panel, Param, Remembered, Shared, ToWindow};
use crate::screen::Screen;
use crate::skin::{Gpu, Skin, View};

/// how often the panel is told that time has passed, and what the window shows is checked
/// against what it should
const TICK: Duration = Duration::from_millis(50);
/// what shows of the window around the panel when the window is not the panel's shape
const AROUND: wgpu::Color = wgpu::Color {
  r: 0.004,
  g: 0.004,
  b: 0.005,
  a: 1.0,
};
/// the picture's pixels as they are drawn and read back: blue first and in srgb, which is
/// how a window is given them
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
/// how long a frame may take the graphics device before it is given up on
const DRAWN_WITHIN: Duration = Duration::from_secs(1);
/// how many pixels of a wheel that scrolls by them count as one line
const PIXELS_PER_LINE: f32 = 20.0;

pub(crate) struct Window<S> {
  context: WindowContext,
  panel: Panel,
  /// each parameter's ways to the host, in the panel's order
  hosts: Vec<Host>,
  remembered: Arc<Remembered>,
  shared: Arc<Shared>,
  /// what whoever opened the window has to tell it
  told: mpsc::Receiver<ToWindow>,
  tick: TimerHandle,
  /// whether baseview has asked for a frame yet, before which it is not to be asked for one
  started: Cell<bool>,
  /// the size the window was last given, until the state has taken it in
  sized: Cell<Option<WindowSize>>,
  /// the system's clipboard, from the first use of it on. on x11 what was copied is there
  /// for other programs only for as long as this is kept
  clipboard: RefCell<Option<arboard::Clipboard>>,
  /// baseview calls a handler through a shared reference, always on the window's one
  /// thread. it can call one while another is still running, when something the first one
  /// does has the system deliver more: so the state is only ever taken with
  /// [`Window::state`], which gives nothing rather than panic when it is in use
  state: RefCell<State<S>>,
}

/// a parameter as the host has it: where it stands, and how a gesture on it is told
struct Host {
  value: Box<dyn Fn() -> f32 + Send>,
  begin: Box<dyn Fn() + Send>,
  set: Box<dyn Fn(f32) + Send>,
  end: Box<dyn Fn() + Send>,
}

struct State<S> {
  device: wgpu::Device,
  queue: wgpu::Queue,
  /// what frames are drawn on, made at the first frame of each size the window takes
  canvas: Option<Canvas>,
  screen: Screen,
  /// the window's width and height in pixels
  window: [u32; 2],
  skin: S,
  /// draws what is over the skin's picture
  flat: FlatRenderer,
  interaction: Interaction,
  /// what was set from here and has yet to reach the parameters
  sent: Sent,
  /// where the pointer last was, in the panel's points
  pointer: Point,
  /// and the same in the window's own points
  on_window: Point,
  /// the display's scale factor, which the window's size in pixels is that much more than
  /// its size in the screen's points
  display_scale: f64,
  /// what the frame last put in the window shows. none if it has had no frame since it was
  /// sized or shown
  shown: Option<Frame>,
  /// how many times the panel's size the window is to become, once for each frame at most
  wanted_scale: Option<f32>,
  /// the first thing the graphics device reported going wrong once the window was open,
  /// which wgpu would otherwise panic over
  fault: Arc<OnceLock<String>>,
  /// whether drawing has been given up for as long as this window is open. the window
  /// keeps the last picture it had
  stopped: bool,
}

/// gives up drawing, and says why the once
fn stop(stopped: &mut bool, why: impl std::fmt::Display) {
  if !std::mem::replace(stopped, true) {
    error!("the editor has stopped drawing: {why}");
  }
}

/// what building something with the device brought up, if anything went wrong with it.
/// wgpu reports a shader or pipeline it cannot build, and memory it cannot get, to whoever
/// is listening for each kind, and panics if nobody is
fn built<T>(device: &wgpu::Device, build: impl FnOnce() -> T) -> Result<T, wgpu::Error> {
  let kinds = [
    wgpu::ErrorFilter::Validation,
    wgpu::ErrorFilter::OutOfMemory,
    wgpu::ErrorFilter::Internal,
  ];
  for kind in kinds {
    device.push_error_scope(kind);
  }
  let built = build();
  let mut wrong = None;
  for _ in kinds {
    wrong = pollster::block_on(device.pop_error_scope()).or(wrong);
  }
  wrong.map_or(Ok(built), Err)
}

/// everything a frame shows that can change from one to the next
#[derive(Clone, Debug, PartialEq)]
struct Frame {
  /// where each parameter stands
  values: Vec<f32>,
  /// the window's width and height in pixels
  window: [u32; 2],
  /// how many times the panel's size it is drawn at
  scale: f32,
  edit: Option<Edit>,
  menu: Option<Menu>,
}

/// what a frame is drawn on and read back from, for one size of window. there is no
/// swapchain on the window: under wine, making one again for each size the window takes
/// while it is dragged keeps the host's thread in the graphics driver
struct Canvas {
  size: [u32; 2],
  texture: wgpu::Texture,
  view: wgpu::TextureView,
  /// the frame's pixels once it is drawn, in rows of `row` bytes
  read: wgpu::Buffer,
  row: u32,
}

impl Canvas {
  fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
      label: Some("controls canvas"),
      size: wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: FORMAT,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
      view_formats: &[],
    });
    // wgpu copies rows to a buffer at a multiple of so many bytes each
    let row = (size[0] * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let read = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("controls canvas read back"),
      size: u64::from(row) * u64::from(size[1]),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    Self {
      size,
      view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
      texture,
      read,
      row,
    }
  }
}

/// whether a frame has to be drawn: when none is in the window, or the one that is shows
/// anything other than what the panel is like now
fn due(
  shown: Option<&Frame>,
  values: &[f32],
  window: [u32; 2],
  scale: f32,
  edit: Option<&Edit>,
  menu: Option<&Menu>,
) -> bool {
  shown.is_none_or(|shown| {
    (shown.values.as_slice(), shown.window, shown.scale) != (values, window, scale)
      || shown.edit.as_ref() != edit
      || shown.menu.as_ref() != menu
  })
}

impl<S: Skin> State<S> {
  /// how many times its own size a panel of `size` is drawn at
  fn scale(&self, size: Point) -> f32 {
    let [_, _, width, _] = fitted(self.window, size);
    (f64::from(width) / self.display_scale) as f32 / size[0]
  }

  /// where each parameter stands: at what was last set from here if that has yet to reach
  /// it, or else where it is. the parameters are looked at afresh
  fn values(&mut self, hosts: &[Host]) -> Vec<f32> {
    (hosts.iter().enumerate())
      .map(|(parameter, host)| {
        let value = (host.value)();
        self.sent.look(parameter, value);
        self.sent.shown(parameter, value)
      })
      .collect()
  }

  /// whether what is in the window is not what the panel is like now
  fn stale(&mut self, hosts: &[Host], size: Point) -> bool {
    let (values, window, scale) = (self.values(hosts), self.window, self.scale(size));
    let (edit, menu) = (self.interaction.edit(), self.interaction.menu());
    due(self.shown.as_ref(), &values, window, scale, edit, menu)
  }

  /// what of the panel is at one of its points: the menu while it is up, the grip, or
  /// whatever the skin has there
  fn target_at(&self, panel: &Panel, at: Point) -> Option<Target> {
    let menu = self.interaction.menu().and_then(|menu| {
      let corner = panel.menu.corner(menu.at, panel.size);
      panel.menu.target_at(corner, at)
    });
    menu
      .or_else(|| {
        (panel.grip)
          .holds(at, panel.size, self.scale(panel.size))
          .then_some(Target::Grip)
      })
      .or_else(|| self.skin.target_at(at))
  }
}

impl<S: Skin> Window<S> {
  #[expect(
    clippy::too_many_arguments,
    reason = "each is one thing the opener hands over"
  )]
  pub(crate) fn new(
    context: WindowContext,
    panel: Panel,
    params: Vec<Param>,
    remembered: Arc<Remembered>,
    shared: Arc<Shared>,
    told: mpsc::Receiver<ToWindow>,
    top_level: bool,
    skin: impl FnOnce(&Gpu) -> S,
  ) -> Result<Self, HandlerError> {
    let size = context.size();
    shared.set_display_scale(size.scale_factor);
    #[cfg(target_os = "linux")]
    if top_level && let Err(error) = hint_shape(&context, size.scale_factor, &panel) {
      error!("the window manager could not be told the editor's shape: {error}");
    }
    #[cfg(not(target_os = "linux"))]
    let _ = top_level;

    // direct3d 12 is started only where nothing else can draw. under wine it runs on vulkan
    // anyway, and wine's shader compiler cannot build any shader with a sampler as naga
    // writes it for direct3d, so there the editor does not open without vulkan
    let adapter = match adapter_of(wgpu::Backends::VULKAN | wgpu::Backends::METAL) {
      Ok(adapter) => adapter,
      Err(first) => adapter_of(wgpu::Backends::DX12).map_err(|second| {
        HandlerError::from_boxed(format!("{first}, and with direct3d 12: {second}").into())
      })?,
    };
    // no more is asked of the adapter than every one has, apart from textures as large as
    // this one has, for the window's own size
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
      required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
      ..Default::default()
    }))?;
    let window = [size.physical.width, size.physical.height];
    let (dials, hosts): (Vec<_>, Vec<_>) = (params.into_iter())
      .map(|param| {
        let host = Host {
          value: param.value,
          begin: param.begin,
          set: param.set,
          end: param.end,
        };
        (param.dial, host)
      })
      .unzip();
    let interaction = Interaction::new(dials, panel.size, panel.readout, panel.scales);
    let values: Vec<f32> = hosts.iter().map(|host| (host.value)()).collect();
    let [_, _, width, height] = fitted(window, panel.size);
    let scale = (f64::from(width) / size.scale_factor) as f32 / panel.size[0];
    // the editor opens only if the device can build everything it is drawn with, and lay
    // out the panel as it is. what goes wrong after that is kept for the next frame to
    // find, and drawing stops there
    let (skin, flat) = built(&device, || {
      let gpu = Gpu {
        device: &device,
        queue: &queue,
        format: FORMAT,
      };
      let mut skin = skin(&gpu);
      let view = View {
        values: &values,
        edit: None,
        menu: None,
        scale,
        pixel: panel.size[0] / width as f32,
        pixels: [width, height],
      };
      skin.prepare(&gpu, &view, &mut Flat::default());
      (skin, FlatRenderer::new(&device, FORMAT))
    })?;
    let fault = Arc::new(OnceLock::new());
    device.on_uncaptured_error(Arc::new({
      let fault = Arc::clone(&fault);
      move |error: wgpu::Error| {
        // only the first is kept: the rest follow from it
        let _ = fault.set(error.to_string());
      }
    }));
    device.set_device_lost_callback({
      let fault = Arc::clone(&fault);
      move |reason, message| {
        // the same, and a device let go of as the window closes is lost to nobody
        let _ = fault.set(format!(
          "the graphics device was lost: {reason:?} {message}"
        ));
      }
    });
    let screen = Screen::new(&context)?;
    let tick = context.create_timer(TICK)?;
    Ok(Self {
      context,
      panel,
      hosts,
      remembered,
      shared,
      told,
      tick,
      started: Cell::new(false),
      sized: Cell::new(None),
      clipboard: RefCell::new(None),
      state: RefCell::new(State {
        device,
        queue,
        canvas: None,
        screen,
        window,
        skin,
        flat,
        interaction,
        sent: Sent::new(&values),
        pointer: [0.0; 2],
        on_window: [0.0; 2],
        display_scale: size.scale_factor,
        shown: None,
        wanted_scale: None,
        fault,
        stopped: false,
      }),
    })
  }

  /// asks baseview for a frame, once it has drawn its first.
  ///
  /// baseview 0.4.2 on x11 asks the server to tell it of the next frame as soon as a redraw
  /// is requested, but only starts listening for the answer when the window is mapped
  /// (`PresentState::handle_present_notify` and `handle_window_mapped` in its
  /// `platform/x11/present.rs`). a request made before that is never answered, and every
  /// later one is dropped for waiting on it, so the window is never drawn at all. it asks
  /// for the first frame itself once the window is mapped.
  /// TODO: ask at any time once baseview holds a request until it can be answered
  fn redraw(&self) {
    if self.started.get() {
      self.context.request_redraw();
    }
  }

  /// the state, with the size the window was last given taken into it, or nothing while a
  /// handler further up already has it. what that leaves undone is made up for at the next
  /// tick, which draws the panel as it then is
  fn state(&self) -> Option<RefMut<'_, State<S>>> {
    let mut state = self.state.try_borrow_mut().ok()?;
    if let Some(size) = self.sized.take() {
      state.window = [size.physical.width, size.physical.height];
      state.display_scale = size.scale_factor;
      state.shown = None;
    }
    Some(state)
  }

  /// asks for a frame if the one in the window is not what the panel is like now. the
  /// window is told when to draw by what happens to it, and this makes up for anything it
  /// was not told: a parameter moved with no report of it, or a frame put off while a handler
  /// further up had the state
  fn refresh(&self) {
    let Some(mut state) = self.state() else {
      return;
    };
    // a window with no size has nothing to draw
    let stale = state.stale(&self.hosts, self.panel.size);
    let asks = !state.stopped && stale && !state.window.contains(&0);
    drop(state);
    if asks {
      self.redraw();
    }
  }

  /// tells the panel of something, and then the host and the window of what came of it
  fn handle(&self, event: impl FnOnce(&mut State<S>) -> Event) {
    let actions = {
      let Some(mut state) = self.state() else {
        return;
      };
      let values = state.values(&self.hosts);
      let event = event(&mut state);
      let moment = Moment {
        now: Instant::now(),
        values: &values,
        scale: state.scale(self.panel.size),
      };
      let actions = state.interaction.handle(event, moment);
      for action in &actions {
        match action {
          Action::Set(parameter, value) => state.sent.set(*parameter, *value),
          Action::Scale(scale) => {
            state.wanted_scale = Some(*scale);
            // the user's own choice of size, which is what is remembered
            (self.remembered.set)(*scale);
          }
          _ => {}
        }
      }
      let typing = state.interaction.edit().is_some();
      self.shared.set_typing(typing);
      actions
    };
    // the state is let go of first: taking the keys can bring an event back in at once
    for action in actions {
      match action {
        Action::Begin(parameter) => (self.hosts[parameter].begin)(),
        Action::Set(parameter, value) => {
          (self.hosts[parameter].set)(value);
          self.redraw();
        }
        Action::End(parameter) => (self.hosts[parameter].end)(),
        // the window is made that size when the next frame is drawn, so that a stream of
        // these asks for no more sizes than are shown
        Action::Scale(_) => self.redraw(),
        Action::Focus => {
          if let Err(error) = self.context.focus() {
            error!("the editor could not take the keys: {error}");
          }
        }
        Action::Redraw => self.redraw(),
        Action::Copy(text) => self.copy(text),
        Action::WantClipboard => self.read_clipboard(),
      }
    }
  }

  /// does something with the system's clipboard, which is opened at the first use of it. the
  /// state is not held meanwhile: the system may deliver other things to the window while
  /// it is asked
  fn clipboard<T>(
    &self,
    with: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
  ) -> Result<T, arboard::Error> {
    let mut clipboard =
      (self.clipboard.try_borrow_mut()).map_err(|_| arboard::Error::ClipboardOccupied)?;
    if clipboard.is_none() {
      *clipboard = Some(arboard::Clipboard::new()?);
    }
    with(clipboard.as_mut().expect("it was just made"))
  }

  fn copy(&self, text: String) {
    if let Err(error) = self.clipboard(|clipboard| clipboard.set_text(text)) {
      error!("the editor could not copy to the clipboard: {error}");
    }
  }

  /// hands the panel the clipboard's text. it is read here and now: on windows and macos
  /// that is a call into the system, and on x11 a question to the program that owns the
  /// clipboard, which this window's own thread waits on and no thread of the host's
  fn read_clipboard(&self) {
    // a clipboard with no text on it is an error to arboard, and nothing to paste here
    let text = self.clipboard(|clipboard| clipboard.get_text()).ok();
    self.handle(|_| Event::Clipboard(text));
  }
}

impl<S: Skin> Window<S> {
  fn draw(&self) -> Result<(), HandlerError> {
    // asked for with the state let go of: on windows the new size comes back in at once
    let wanted = self.state().and_then(|mut state| state.wanted_scale.take());
    if let Some(scale) = wanted
      && let Err(error) = self.context.resize(LogicalSize::new(
        self.panel.size[0] * scale,
        self.panel.size[1] * scale,
      ))
    {
      error!("the editor's window could not be resized: {error}");
    }
    self.started.set(true);
    // drawn at the next tick if this is from inside another handler
    let Some(mut state) = self.state() else {
      return Ok(());
    };
    let State { fault, stopped, .. } = &mut *state;
    if let Some(fault) = fault.get() {
      stop(stopped, fault);
    }
    // the system wants the window painted and the panel is as it was last drawn, or is no
    // longer drawn at all: the last picture goes back
    if state.stopped || !state.stale(&self.hosts, self.panel.size) {
      if let Err(error) = state.screen.again() {
        stop(&mut state.stopped, error);
      }
      return Ok(());
    }
    // a window with no size, as a minimized one has, has nothing to draw, and one larger
    // than the device can draw on in one piece is left as it is
    let most = state.device.limits().max_texture_dimension_2d;
    if state.window.iter().any(|&side| side == 0 || side > most) {
      return Ok(());
    }
    let (panel, size) = (&self.panel, self.panel.size);
    let (values, window, scale) = (state.values(&self.hosts), state.window, state.scale(size));
    let State {
      device,
      queue,
      canvas,
      screen,
      skin,
      flat,
      interaction,
      shown,
      fault,
      stopped,
      ..
    } = &mut *state;
    let drawn_on = match canvas {
      Some(canvas) if canvas.size == window => canvas,
      _ => canvas.insert(Canvas::new(device, window)),
    };

    // the panel goes on as much of the window as has its shape
    let place @ [_, _, width, height] = fitted(window, size);
    let pixel = size[0] / width as f32;
    let gpu = Gpu {
      device,
      queue,
      format: FORMAT,
    };
    let seen = View {
      values: &values,
      edit: interaction.edit(),
      menu: interaction.menu(),
      scale,
      pixel,
      pixels: [width, height],
    };
    // the skin's own flat shapes, and over them the grip and the menu
    let mut over = Flat::default();
    skin.prepare(&gpu, &seen, &mut over);
    panel.grip.shapes(&mut over, size, scale);
    if let Some(menu) = interaction.menu() {
      panel.menu.shapes(&mut over, menu, size, pixel);
    }
    flat.prepare(device, queue, &over, size, [width, height]);
    let view = &drawn_on.view;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
      label: Some("controls editor"),
    });
    if [width, height] != window {
      // the rest of the window, which the panel's own pass leaves as it finds it
      encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("controls around"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(AROUND),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
      });
    }
    skin.draw(&mut encoder, view, place);
    flat.render(&mut encoder, view, place);
    encoder.copy_texture_to_buffer(
      drawn_on.texture.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &drawn_on.read,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(drawn_on.row),
          rows_per_image: None,
        },
      },
      drawn_on.texture.size(),
    );
    let submitted = queue.submit([encoder.finish()]);
    // the frame is waited for here, on the window's thread, which is the host's on windows.
    // the device takes a millisecond or so for it, and the pixels go in the window at once
    let (mapped, when_mapped) = mpsc::channel();
    (drawn_on.read.slice(..)).map_async(wgpu::MapMode::Read, move |result| {
      // the receiver is only gone once the frame has been given up on
      let _ = mapped.send(result);
    });
    let waited = device.poll(wgpu::PollType::Wait {
      submission_index: Some(submitted),
      timeout: Some(DRAWN_WITHIN),
    });
    if let Some(fault) = fault.get() {
      stop(stopped, fault);
      return Ok(());
    }
    let read = match (waited, when_mapped.try_recv()) {
      (Ok(_), Ok(Ok(()))) => &drawn_on.read,
      // the buffer may still be on its way to being read, and is not drawn on again
      (waited, mapped) => {
        *canvas = None;
        stop(
          stopped,
          format_args!("the frame could not be read back: {waited:?}, {mapped:?}"),
        );
        return Ok(());
      }
    };
    let put = screen.show(window, drawn_on.row, &read.slice(..).get_mapped_range());
    read.unmap();
    if let Err(error) = put {
      stop(stopped, error);
      return Ok(());
    }
    *shown = Some(Frame {
      values,
      window,
      scale,
      edit: interaction.edit().cloned(),
      menu: interaction.menu().copied(),
    });
    Ok(())
  }

  fn poll(&self) {
    while let Ok(told) = self.told.try_recv() {
      match told {
        ToWindow::Key(key) => self.handle(|_| Event::Key(key)),
        ToWindow::Shown => {
          if let Some(mut state) = self.state() {
            state.shown = None;
          }
        }
        ToWindow::State => {
          // the state that was loaded may have the window at another size
          let scale = (self.remembered.get)();
          let differs = self.state().is_some_and(|mut state| {
            let differs = (scale - state.scale(self.panel.size)).abs() > 0.001;
            if differs {
              state.wanted_scale = Some(scale);
            }
            differs
          });
          if differs {
            self.redraw();
          }
        }
      }
    }
    self.refresh();
  }

  fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
    self.shared.set_display_scale(new_size.scale_factor);
    // kept for the state to take in, which a handler further up may have just now
    self.sized.set(Some(new_size));
    self.redraw();
    Ok(())
  }

  fn on_event(&self, event: baseview::Event) -> EventStatus {
    match event {
      baseview::Event::Mouse(event) => {
        self.on_mouse(event);
        EventStatus::Captured
      }
      baseview::Event::Keyboard(event) => {
        if !(self.state()).is_some_and(|state| state.interaction.takes_keys()) {
          return EventStatus::Ignored;
        }
        if event.state == keyboard_types::KeyState::Down {
          for key in keys(&event) {
            self.handle(|_| Event::Key(key));
          }
        }
        EventStatus::Captured
      }
      baseview::Event::Window(WindowEvent::Unfocused) => {
        self.handle(|_| Event::Unfocused);
        EventStatus::Captured
      }
      baseview::Event::Window(WindowEvent::WillClose) => {
        self.handle(|_| Event::Closed);
        EventStatus::Captured
      }
      _ => EventStatus::Ignored,
    }
  }

  fn on_timer(&self, timer: &TimerHandle) {
    if timer == self.tick {
      self.handle(|_| Event::Tick);
      self.refresh();
    }
  }
}

impl<S: Skin> Window<S> {
  fn on_mouse(&self, event: MouseEvent) {
    match event {
      MouseEvent::CursorMoved {
        position,
        modifiers,
      } => self.handle(|state| {
        let pixel = [position.x as f32, position.y as f32];
        let size = self.panel.size;
        let at = point_at(fitted(state.window, size), pixel, size);
        state.pointer = at;
        state.on_window = pixel.map(|along| along / state.display_scale as f32);
        Event::Move {
          at,
          window: state.on_window,
          // only the menu shows what the pointer is over
          over: (state.interaction.menu()).and_then(|_| state.target_at(&self.panel, at)),
          fine: fine(modifiers),
        }
      }),
      MouseEvent::ButtonPressed { button, modifiers } => {
        let Some(button) = button_of(button) else {
          return;
        };
        self.handle(|state| Event::Press {
          button,
          at: state.pointer,
          window: state.on_window,
          over: state.target_at(&self.panel, state.pointer),
          reset: modifiers.ctrl() || modifiers.meta(),
        });
      }
      MouseEvent::ButtonReleased { button, .. } => {
        if let Some(button) = button_of(button) {
          self.handle(|_| Event::Release(button));
        }
      }
      MouseEvent::WheelScrolled { delta, modifiers } => {
        let lines = match delta {
          ScrollDelta::Lines { y, .. } => y,
          ScrollDelta::Pixels { y, .. } => y / PIXELS_PER_LINE,
        };
        self.handle(|state| Event::Wheel {
          over: state.target_at(&self.panel, state.pointer),
          lines,
          fine: fine(modifiers),
        });
      }
      _ => {}
    }
  }
}

/// the window as baseview has it: every call into it stops at a panic.
///
/// a panic out of these would unwind into baseview's window procedure on windows, which is
/// an `extern` function, and that ends the host's whole process. it is caught here, and
/// from then on the window does nothing: its state may be half changed, so nothing looks
/// at it again. wgpu itself panics when the device is lost while it is waited on
/// (`Device::poll` in its `backend/wgpu_core.rs`), and any fault of the editor's own ends
/// here as well
pub(crate) struct Guarded<S> {
  window: Window<S>,
  broken: Cell<bool>,
}

impl<S: Skin> Guarded<S> {
  /// the window, if making it neither fails nor panics
  pub(crate) fn new(
    make: impl FnOnce() -> Result<Window<S>, HandlerError>,
  ) -> Result<Self, HandlerError> {
    let window = catch_unwind(AssertUnwindSafe(make))
      .map_err(|panic| HandlerError::from_boxed(said(&*panic).into()))??;
    Ok(Self {
      window,
      broken: Cell::new(false),
    })
  }

  fn run<T>(&self, otherwise: T, work: impl FnOnce(&Window<S>) -> T) -> T {
    if self.broken.get() {
      return otherwise;
    }
    // nothing of the window's is looked at again after a panic
    catch_unwind(AssertUnwindSafe(|| work(&self.window))).unwrap_or_else(|panic| {
      self.broken.set(true);
      error!("the editor has stopped at a panic: {}", said(&*panic));
      otherwise
    })
  }
}

/// what a panic was raised with, where that was words
fn said(panic: &(dyn std::any::Any + Send)) -> String {
  let said = panic.downcast_ref::<&str>().copied();
  said
    .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
    .unwrap_or("a panic with no message")
    .to_owned()
}

impl<S: Skin> WindowHandler for Guarded<S> {
  fn draw(&self) -> Result<(), HandlerError> {
    self.run(Ok(()), Window::draw)
  }

  fn poll(&self) {
    self.run((), Window::poll);
  }

  fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
    self.run(Ok(()), |window| window.resized(new_size))
  }

  fn on_event(&self, event: baseview::Event) -> EventStatus {
    self.run(EventStatus::Ignored, |window| window.on_event(event))
  }

  fn on_timer(&self, timer: &TimerHandle) {
    self.run((), |window| window.on_timer(timer));
  }
}

/// a graphics adapter of one of some apis
fn adapter_of(backends: wgpu::Backends) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
  let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends,
    ..Default::default()
  });
  pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
}

/// tells the window manager that the window keeps the panel's shape, and how small and large
/// it may be, so that it holds the window to them while its frame is dragged. baseview sets
/// the sizes alone, and this takes the place of what it set
#[cfg(target_os = "linux")]
fn hint_shape(
  context: &WindowContext,
  display_scale: f64,
  panel: &Panel,
) -> Result<(), Box<dyn std::error::Error>> {
  use wgpu::rwh::{HasWindowHandle, RawWindowHandle};
  use x11rb::properties::{AspectRatio, WmSizeHints};

  let RawWindowHandle::Xlib(window) = context.window_handle()?.as_raw() else {
    return Err("the window is not an x11 one".into());
  };
  let (connection, _) = x11rb::connect(None)?;
  let pixels = |scale: f32| {
    let size = panel.sized(scale).to_physical::<i32>(display_scale);
    (size.width, size.height)
  };
  let shape = AspectRatio::new(panel.size[0] as i32, panel.size[1] as i32);
  let hints = WmSizeHints {
    min_size: Some(pixels(panel.scales[0])),
    max_size: Some(pixels(panel.scales[1])),
    aspect: Some((shape, shape)),
    ..WmSizeHints::default()
  };
  (hints.set_normal_hints(&connection, window.window as u32)?).check()?;
  Ok(())
}

/// whether a key that slows a knob down is held
fn fine(modifiers: keyboard_types::Modifiers) -> bool {
  modifiers.alt() || modifiers.shift()
}

fn button_of(button: MouseButton) -> Option<Button> {
  match button {
    MouseButton::Left => Some(Button::Left),
    MouseButton::Right => Some(Button::Right),
    _ => None,
  }
}

/// what a key press means to a value being typed. a key held with one that makes it a
/// command types nothing
fn keys(event: &keyboard_types::KeyboardEvent) -> Vec<Key> {
  use keyboard_types::{Key as Pressed, NamedKey};

  let command = event.modifiers.ctrl() || event.modifiers.meta();
  let extend = event.modifiers.shift();
  match &event.key {
    Pressed::Character(letters) if command => match letters.to_ascii_lowercase().as_str() {
      "a" => vec![Key::SelectAll],
      "c" => vec![Key::Copy],
      "x" => vec![Key::Cut],
      "v" => vec![Key::Paste],
      _ => Vec::new(),
    },
    Pressed::Character(letters) if !event.modifiers.alt() => {
      letters.chars().map(Key::Letter).collect()
    }
    Pressed::Named(named) => match named {
      NamedKey::Enter => vec![Key::Enter],
      NamedKey::Escape => vec![Key::Escape],
      NamedKey::Backspace => vec![Key::Backspace],
      NamedKey::Delete => vec![Key::Delete],
      NamedKey::ArrowLeft => vec![Key::Left { extend }],
      NamedKey::ArrowRight => vec![Key::Right { extend }],
      NamedKey::Home => vec![Key::Home { extend }],
      NamedKey::End => vec![Key::End { extend }],
      NamedKey::Tab => vec![Key::Tab],
      _ => Vec::new(),
    },
    _ => Vec::new(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// the first of a panel's parameters
  const PITCH: usize = 0;
  use controls::interaction::Item;

  /// a frame of a window of the panel's own size, with nothing typed or chosen
  fn frame(values: [f32; 3]) -> Frame {
    Frame {
      values: values.into(),
      window: [750, 375],
      scale: 1.0,
      edit: None,
      menu: None,
    }
  }

  /// whether a frame is due in a window showing `shown`, with the panel like `now`
  fn is_due(shown: Option<&Frame>, now: &Frame) -> bool {
    let Frame {
      values,
      window,
      scale,
      edit,
      menu,
    } = now;
    due(shown, values, *window, *scale, edit.as_ref(), menu.as_ref())
  }

  #[test]
  fn a_frame_is_due_when_none_is_shown_or_the_one_shown_is_not_the_panel_as_it_is() {
    let shown = frame([0.5, 0.5, 1.0]);
    // the same panel in a window that shows it: nothing to draw, however often it is asked
    assert!(!is_due(Some(&shown), &shown));
    // a window that has had no frame, as when it has just been sized or shown
    assert!(is_due(None, &shown));
    // a control somewhere else, whoever moved it and whether or not anything said so
    assert!(is_due(Some(&shown), &frame([0.5, 0.25, 1.0])));
    // the window another size, or the panel drawn larger in it
    let resized = Frame {
      window: [900, 375],
      ..shown.clone()
    };
    assert!(is_due(Some(&shown), &resized));
    let scaled = Frame {
      scale: 1.5,
      ..shown.clone()
    };
    assert!(is_due(Some(&shown), &scaled));
    // the menu up, moved to another row, and away again
    let menu = |hovered| Frame {
      menu: Some(Menu {
        control: 0,
        at: [10.0, 10.0],
        hovered,
      }),
      ..shown.clone()
    };
    assert!(is_due(Some(&shown), &menu(None)));
    assert!(is_due(Some(&menu(None)), &menu(Some(Item::Reset))));
    assert!(is_due(Some(&menu(None)), &shown));
    assert!(!is_due(Some(&menu(None)), &menu(None)));
    // a value being typed, and one more letter of it
    let typed = |text: &str| Frame {
      edit: Some(Edit {
        control: 0,
        text: text.to_owned(),
        caret: text.len(),
        anchor: text.len(),
      }),
      ..shown.clone()
    };
    assert!(is_due(Some(&shown), &typed("1")));
    assert!(is_due(Some(&typed("1")), &typed("12")));
    assert!(!is_due(Some(&typed("12")), &typed("12")));
  }

  #[test]
  fn a_value_of_the_editors_own_coming_back_is_no_new_frame_and_the_hosts_is() {
    // what the panel shows of the pitch as its parameter is looked at, the way the window
    // does before it decides whether to draw
    let mut sent = Sent::new(&[0.5, 0.5, 1.0]);
    let now = |sent: &mut Sent, parameter: f32| {
      sent.look(PITCH, parameter);
      frame([sent.shown(PITCH, parameter), 0.5, 1.0])
    };
    // the editor sets 0.6 and draws it, with the parameter still where it was
    sent.set(PITCH, 0.6);
    let shown = now(&mut sent, 0.5);
    assert_eq!(shown.values[0], 0.6);
    // the parameter gets there: the same picture, whether or not the wrapper reports it
    assert!(!is_due(Some(&shown), &now(&mut sent, 0.6)));
    // the host moves it on with no report at all: seen the next time it is looked at
    assert!(is_due(Some(&shown), &now(&mut sent, 0.8)));
    // and the host moving it to where the editor once had it is drawn too
    let shown = now(&mut sent, 0.8);
    assert!(is_due(Some(&shown), &now(&mut sent, 0.6)));
  }
}
