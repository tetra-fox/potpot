//! The open window.
//!
//! The panel is drawn and put in it, and baseview's events are turned into what the panel is
//! told.

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
use potpot_controls::Point;
use potpot_controls::fit::{fitted, point_at};
use potpot_controls::flat::Flat;
use potpot_controls::interaction::{
  Action, Button, Control, Edit, Event, Interaction, Key, Menu, Moment, Target,
};
use potpot_controls::sent::Sent;
use tracing::error;

use crate::feed::same;
use crate::flat::FlatRenderer;
use crate::open::{Param, Remembered, Settings, Shared, ToWindow};
use crate::screen::Screen;
use crate::skin::{Gpu, Skin, View};

/// The time between two tellings of the panel that time has passed.
///
/// What the window shows is checked against what it should as often.
const TICK: Duration = Duration::from_millis(50);
/// The color of the window around the panel.
///
/// It shows when the window has another shape than the panel's.
const AROUND: wgpu::Color = wgpu::Color {
  r: 0.004,
  g: 0.004,
  b: 0.005,
  a: 1.0,
};
/// The format the picture's pixels are drawn and read back in.
///
/// Blue comes first and the colors are in sRGB, which is how a window is given them.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
/// The longest a frame may take the graphics device before it is given up on.
const DRAWN_WITHIN: Duration = Duration::from_secs(1);
/// The number of pixels of a wheel that scrolls by them that count as one line.
const PIXELS_PER_LINE: f32 = 20.0;

/// The share of a window that is read back as all of it.
///
/// Parts of a frame that make up that much of it are read back as the whole frame. Each part is
/// a copy of its own, and together they are as much to carry as the frame.
const MOSTLY: f32 = 0.75;

/// The number of ticks a window keeps a size of another shape than the panel's.
///
/// After that many it is asked to take the panel's.
const SETTLED: u32 = 4;

/// The most pixels a window's side may be from the panel's shape.
///
/// A window that near the shape is left alone. Rounding a size to whole pixels can put it
/// that far off.
const NEAR: u32 = 2;

pub(crate) struct Window<S> {
  context: WindowContext,
  settings: Settings,
  /// Each parameter's ways to the host, in the panel's order.
  hosts: Vec<Host>,
  remembered: Arc<Remembered>,
  shared: Arc<Shared>,
  /// The messages of whoever opened the window.
  told: mpsc::Receiver<ToWindow>,
  tick: TimerHandle,
  /// Whether the window is its own, with a frame the window manager resizes it by.
  top_level: bool,
  /// A misshapen size the window has, and for how many ticks.
  ///
  /// A misshapen size has another shape than the panel's. Window managers can give one
  /// whatever they were hinted: KWin does when it maximizes.
  misshapen: Cell<Option<([u32; 2], u32)>>,
  /// Whether baseview has asked for a frame yet.
  ///
  /// Before it has, it is not to be asked for one.
  started: Cell<bool>,
  /// Whether the window may have lost some of its picture.
  ///
  /// The system says so, and the next picture put in the window is put there whole.
  exposed: Cell<bool>,
  /// The size the window was last given, until the state has taken it in.
  sized: Cell<Option<WindowSize>>,
  /// The system's clipboard, from the first use of it on.
  ///
  /// On X11 what was copied is there for other programs only for as long as this is kept.
  clipboard: RefCell<Option<arboard::Clipboard>>,
  /// The state, which is only ever taken with [`Window::state`].
  ///
  /// baseview calls a handler through a shared reference, always on the window's one
  /// thread. It can call one while another is still running, when something the first one
  /// does has the system deliver more. [`Window::state`] gives nothing when the state is in
  /// use, where a plain borrow would panic.
  state: RefCell<State<S>>,
}

/// A parameter as the host has it.
///
/// It holds where the parameter stands and how a gesture on it is told.
struct Host {
  value: Box<dyn Fn() -> f32 + Send>,
  begin: Box<dyn Fn() + Send>,
  set: Box<dyn Fn(f32) + Send>,
  end: Box<dyn Fn() + Send>,
}

struct State<S> {
  device: wgpu::Device,
  queue: wgpu::Queue,
  /// The canvas frames are drawn on.
  ///
  /// It is made at the first frame of each size the window takes.
  canvas: Option<Canvas>,
  screen: Screen,
  /// The window's width and height in pixels.
  window: [u32; 2],
  skin: S,
  /// The renderer of what is over the skin's picture.
  flat: FlatRenderer,
  interaction: Interaction,
  /// The values set from here that have yet to reach the parameters.
  sent: Sent,
  /// The last place of the pointer, in the panel's points.
  pointer: Point,
  /// The last place of the pointer, in the window's own points.
  on_window: Point,
  /// The display's scale factor.
  ///
  /// The window's size in pixels is that many times its size in the screen's points.
  display_scale: f64,
  /// The contents of the frame last put in the window.
  ///
  /// It is `None` if the window has had no frame since it was sized or shown.
  shown: Option<Frame>,
  /// The parts of the canvas where a frame is new.
  ///
  /// The vector is kept for its room.
  parts: Vec<[u32; 4]>,
  /// The time of the last frame that drew the feed changed.
  fed: Option<Instant>,
  /// The multiple of the panel's size the window is to become.
  ///
  /// The window becomes it once for each frame at most.
  wanted_scale: Option<f32>,
  /// The first fault the graphics device reported once the window was open.
  ///
  /// wgpu would otherwise panic over it.
  fault: Arc<OnceLock<String>>,
  /// Whether drawing has been given up for as long as this window is open.
  ///
  /// The window keeps the last picture it had.
  stopped: bool,
}

/// Gives up drawing, and logs why the first time.
fn stop(stopped: &mut bool, why: impl std::fmt::Display) {
  if !std::mem::replace(stopped, true) {
    error!("the editor has stopped drawing: {why}");
  }
}

/// Returns what `build` makes with the device, or what went wrong with it.
///
/// wgpu reports a shader or pipeline it cannot build, and memory it cannot get, to whoever
/// is listening for each kind, and panics if nobody is.
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

/// Everything a frame shows that can change between frames.
#[derive(Clone, Debug)]
struct Frame {
  /// The value of each parameter.
  values: Vec<f32>,
  /// The number in each slot of the feed.
  feed: Vec<f32>,
  /// The window's width and height in pixels.
  window: [u32; 2],
  /// The multiple of the panel's size it is drawn at.
  scale: f32,
  edit: Option<Edit>,
  menu: Option<Menu>,
  /// The control that is held down.
  held: Option<usize>,
}

/// The target a frame is drawn on and read back from, for one size of window.
///
/// There is no swapchain on the window: under Wine, making one again for each size the
/// window takes while it is dragged keeps the host's thread in the graphics driver.
struct Canvas {
  size: [u32; 2],
  texture: wgpu::Texture,
  view: wgpu::TextureView,
  /// The frame's pixels once it is drawn, in rows of `row` bytes.
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

  /// Has parts of the canvas read back after a frame is drawn.
  ///
  /// `encoder` holds the frame's drawing.
  ///
  /// Each part is `[left, top, width, height]` in the canvas's pixels and lies on it, and there
  /// is one at least. The frame is waited for here, on the window's thread, which the host runs
  /// on too on Windows. The device takes a millisecond or so.
  ///
  /// Returns the rows of the read buffer that the parts lie in, which are mapped to be read.
  /// The other pixels of those rows are as some frame before left them.
  ///
  /// # Errors
  ///
  /// Returns what went wrong if the frame is not drawn within [`DRAWN_WITHIN`] or the rows
  /// cannot be mapped.
  fn read(
    &self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut encoder: wgpu::CommandEncoder,
    parts: &[[u32; 4]],
  ) -> Result<Rows, String> {
    let row = u64::from(self.row);
    for &[left, top, width, height] in parts {
      encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
          origin: wgpu::Origin3d {
            x: left,
            y: top,
            z: 0,
          },
          ..self.texture.as_image_copy()
        },
        wgpu::TexelCopyBufferInfo {
          buffer: &self.read,
          layout: wgpu::TexelCopyBufferLayout {
            offset: u64::from(top) * row + u64::from(left) * 4,
            bytes_per_row: Some(self.row),
            rows_per_image: None,
          },
        },
        wgpu::Extent3d {
          width,
          height,
          depth_or_array_layers: 1,
        },
      );
    }
    let submitted = queue.submit([encoder.finish()]);
    let top = parts.iter().map(|part| part[1]).min().unwrap_or(0);
    let foot = (parts.iter().map(|part| part[1] + part[3]).max()).unwrap_or(0);
    let bytes = u64::from(top) * row..u64::from(foot) * row;
    let (mapped, when_mapped) = mpsc::channel();
    (self.read.slice(bytes.clone())).map_async(wgpu::MapMode::Read, move |result| {
      // the receiver is only gone once the frame has been given up on
      let _ = mapped.send(result);
    });
    let waited = device.poll(wgpu::PollType::Wait {
      submission_index: Some(submitted),
      timeout: Some(DRAWN_WITHIN),
    });
    match (waited, when_mapped.try_recv()) {
      (Ok(_), Ok(Ok(()))) => Ok(Rows { top, bytes }),
      (waited, mapped) => Err(format!(
        "the frame could not be read back: {waited:?}, {mapped:?}"
      )),
    }
  }
}

/// Rows of a canvas's read buffer.
struct Rows {
  /// The first of the rows, counted from the top of the canvas.
  top: u32,
  /// The bytes of the read buffer the rows are.
  bytes: std::ops::Range<u64>,
}

/// Returns whether a frame has to be drawn.
///
/// One has to be when none is in the window, or the one that is shows anything other than
/// `now`, which is what the panel is like now.
fn due(shown: Option<&Frame>, now: &Frame) -> bool {
  shown.is_none_or(|shown| {
    (&shown.values, shown.window, shown.scale) != (&now.values, now.window, now.scale)
      || !same(&shown.feed, &now.feed)
      || (&shown.edit, &shown.menu, shown.held) != (&now.edit, &now.menu, now.held)
  })
}

/// Gathers the parts of a window where a frame is new.
///
/// They are where the frame differs from the one before it. `skin` is the parts of its place
/// that the skin names, and `flat` the rectangle of the window that the flat shapes over it
/// differ in. `place` is the skin's place in the window, which is `window` pixels. The parts
/// are put in `parts` as `[left, top, width, height]` in the window's pixels, in place of what
/// it held.
///
/// Returns whether the parts are to be read back in place of the whole frame, which they are
/// unless they make up most of the window.
fn patches(
  parts: &mut Vec<[u32; 4]>,
  skin: &[[u32; 4]],
  flat: Option<[u32; 4]>,
  place: [u32; 4],
  window: [u32; 2],
) -> bool {
  parts.clear();
  let [x, y, wide, high] = place;
  // a part is kept to the place, whatever the skin says of it
  let placed = skin.iter().map(|&[left, top, width, height]| {
    let (left, top) = (left.min(wide), top.min(high));
    [
      x + left,
      y + top,
      width.min(wide - left),
      height.min(high - top),
    ]
  });
  parts.extend(placed.chain(flat).filter(|part| part[2] > 0 && part[3] > 0));
  let read: u64 = (parts.iter())
    .map(|part| u64::from(part[2]) * u64::from(part[3]))
    .sum();
  let all = u64::from(window[0]) * u64::from(window[1]);
  (read as f32) < all as f32 * MOSTLY
}

/// Returns whether the next frame after `now` is asked for.
///
/// It is for one [`TICK`] after `fed`, the time of the last frame that drew the feed changed.
/// Nothing reports a write to the feed, and an audio thread's writes can be further apart than
/// frames, so a frame that finds nothing written does not end the looking. After a tick the
/// ticks look.
fn follows(fed: Option<Instant>, now: Instant) -> bool {
  fed.is_some_and(|fed| now.duration_since(fed) < TICK)
}

/// Returns the multiple of its own size a panel of `size` is drawn at.
///
/// The window is `window` pixels, on a display with `display_scale` of them to a point.
fn scale_in(window: [u32; 2], display_scale: f64, size: Point) -> f32 {
  let [_, _, width, _] = fitted(window, size);
  (f64::from(width) / display_scale) as f32 / size[0]
}

impl<S: Skin> State<S> {
  /// Returns the multiple of its own size a panel of `size` is drawn at.
  fn scale(&self, size: Point) -> f32 {
    scale_in(self.window, self.display_scale, size)
  }

  /// Returns where each parameter stands.
  ///
  /// That is at what was last set from here if that has yet to reach it, or else where it
  /// is. The parameters are looked at afresh.
  fn values(&mut self, hosts: &[Host]) -> Vec<f32> {
    (hosts.iter().enumerate())
      .map(|(parameter, host)| {
        let value = (host.value)();
        self.sent.look(parameter, value);
        self.sent.shown(parameter, value)
      })
      .collect()
  }

  /// Returns what the panel is like now.
  ///
  /// The parameters and the feed are looked at afresh.
  fn now(&mut self, hosts: &[Host], settings: &Settings) -> Frame {
    Frame {
      values: self.values(hosts),
      feed: settings.feed.read(),
      window: self.window,
      scale: self.scale(settings.size),
      edit: self.interaction.edit().cloned(),
      menu: self.interaction.menu().copied(),
      held: self.interaction.held(),
    }
  }

  /// Returns the [`Target`] at one of the panel's points.
  ///
  /// That is the menu while it is up, the grip, or whatever the skin has there.
  fn target_at(&self, settings: &Settings, at: Point) -> Option<Target> {
    let menu = self.interaction.menu().and_then(|menu| {
      let corner = settings.menu.corner(menu.at, settings.size);
      settings.menu.target_at(corner, at)
    });
    menu
      .or_else(|| {
        (settings.grip)
          .holds(at, settings.size, self.scale(settings.size))
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
    settings: Settings,
    params: Vec<Param>,
    controls: Vec<Control>,
    remembered: Arc<Remembered>,
    shared: Arc<Shared>,
    told: mpsc::Receiver<ToWindow>,
    top_level: bool,
    skin: impl FnOnce(&Gpu) -> S,
  ) -> Result<Self, HandlerError> {
    let size = context.size();
    shared.set_display_scale(size.scale_factor);
    #[cfg(target_os = "linux")]
    if top_level && let Err(error) = hint_shape(&context, size.scale_factor, &settings) {
      error!("the window manager could not be told the editor's shape: {error}");
    }

    // direct3d 12 is tried last. wine runs it on vulkan anyway, and its shader compiler builds
    // no shader with a sampler as naga writes it for direct3d, so there the editor needs vulkan
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
    let interaction = Interaction::new(dials, controls, settings.size, settings.scales);
    let values: Vec<f32> = hosts.iter().map(|host| (host.value)()).collect();
    let [_, _, width, height] = fitted(window, settings.size);
    let scale = scale_in(window, size.scale_factor, settings.size);
    // the editor opens only if the device can build all it is drawn with and lay out the panel
    // as it is. what goes wrong after that is kept for the next frame to find, and drawing stops
    let (skin, flat) = built(&device, || {
      let gpu = Gpu {
        device: &device,
        queue: &queue,
        format: FORMAT,
      };
      let mut skin = skin(&gpu);
      let view = View {
        controls: &interaction.readings(&values),
        values: &values,
        feed: &settings.feed.read(),
        edit: None,
        menu: None,
        held: None,
        scale,
        pixel: settings.size[0] / width as f32,
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
      settings,
      hosts,
      remembered,
      shared,
      told,
      tick,
      top_level,
      misshapen: Cell::new(None),
      started: Cell::new(false),
      exposed: Cell::new(false),
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
        parts: Vec::new(),
        fed: None,
        wanted_scale: None,
        fault,
        stopped: false,
      }),
    })
  }

  /// Asks for the largest window of the panel's shape that fits in this one.
  ///
  /// It asks once a window of its own has kept another shape for [`SETTLED`] ticks. Each
  /// size is asked about once, so a window manager that holds the window to it is not asked
  /// again.
  fn reshape(&self) {
    if !self.top_level {
      return;
    }
    let Some(mut state) = self.state() else {
      return;
    };
    let [_, _, width, height] = fitted(state.window, self.settings.size);
    let [wide, tall] = state.window;
    if wide - width <= NEAR && tall - height <= NEAR {
      self.misshapen.set(None);
      return;
    }
    let ticks = match self.misshapen.get() {
      Some((size, ticks)) if size == state.window => ticks.saturating_add(1),
      _ => 0,
    };
    self.misshapen.set(Some((state.window, ticks)));
    if ticks == SETTLED {
      let scale = scale_in(state.window, state.display_scale, self.settings.size);
      let [least, most] = self.settings.scales;
      state.wanted_scale = Some(scale.clamp(least, most));
      drop(state);
      self.redraw();
    }
  }

  /// Asks baseview for a frame, once it has drawn its first.
  ///
  /// baseview 0.4.2 on X11 asks the server to tell it of the next frame as soon as a redraw
  /// is requested, but only starts listening for the answer when the window is mapped
  /// (`PresentState::handle_present_notify` and `handle_window_mapped` in its
  /// `platform/x11/present.rs`). A request made before that is never answered, and every
  /// later one is dropped for waiting on it, so the window is never drawn at all. baseview
  /// asks for the first frame itself once the window is mapped.
  fn redraw(&self) {
    // TODO: ask at any time once baseview holds a request until it can be answered
    if self.started.get() {
      self.context.request_redraw();
    }
  }

  /// Returns the state, or nothing while a handler further up already has it.
  ///
  /// The size the window was last given is taken into the state. What a handler leaves
  /// undone for want of the state is made up for at the next tick, which draws the panel as
  /// it then is.
  fn state(&self) -> Option<RefMut<'_, State<S>>> {
    let mut state = self.state.try_borrow_mut().ok()?;
    if let Some(size) = self.sized.take() {
      state.window = [size.physical.width, size.physical.height];
      state.display_scale = size.scale_factor;
      state.shown = None;
    }
    Some(state)
  }

  /// Asks for a frame if the one in the window is stale.
  ///
  /// The window is told when to draw by what happens to it, and this makes up for anything
  /// it was not told: a parameter moved with no report of it, a number of the feed written, or
  /// a frame put off while a handler further up had the state.
  fn refresh(&self) {
    let Some(mut state) = self.state() else {
      return;
    };
    let now = state.now(&self.hosts, &self.settings);
    let stale = due(state.shown.as_ref(), &now);
    // a window with no size has nothing to draw
    let asks = !state.stopped && stale && !state.window.contains(&0);
    drop(state);
    if asks {
      self.redraw();
    }
  }

  /// Tells the panel of an event, then the host and the window what came of it.
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
        scale: state.scale(self.settings.size),
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

  /// Runs `with` on the system's clipboard, which is opened at the first use of it.
  ///
  /// The state is not held meanwhile: the system may deliver other things to the window
  /// while the clipboard is asked.
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

  /// Hands the panel the clipboard's text.
  ///
  /// The text is read here and now. On Windows and macOS that is a call into the system, and
  /// on X11 a question to the program that owns the clipboard, which only this window's own
  /// thread waits on.
  fn read_clipboard(&self) {
    // a clipboard with no text on it is an error to arboard, and nothing to paste here
    let text = self.clipboard(|clipboard| clipboard.get_text()).ok();
    self.handle(|_| Event::Clipboard(text));
  }

  fn draw(&self) -> Result<(), HandlerError> {
    // asked for with the state let go of: on windows the new size comes back in at once
    let wanted = self.state().and_then(|mut state| state.wanted_scale.take());
    if let Some(scale) = wanted
      && let Err(error) = self.context.resize(LogicalSize::new(
        self.settings.size[0] * scale,
        self.settings.size[1] * scale,
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
    let now = state.now(&self.hosts, &self.settings);
    let at = Instant::now();
    let exposed = self.exposed.take();
    if state.stopped || !due(state.shown.as_ref(), &now) {
      // nothing to draw. the last picture goes back if the window lost some of it, or if no feed
      // moved a moment ago, when it is the system that wants the window painted
      let follows = !state.stopped && follows(state.fed, at);
      if (!follows || exposed)
        && let Err(error) = state.screen.again()
      {
        stop(&mut state.stopped, error);
      }
      drop(state);
      if follows {
        self.redraw();
      }
      return Ok(());
    }
    // a window with no size, as a minimized one has, has nothing to draw, and one larger
    // than the device can draw on in one piece is left as it is
    let most = state.device.limits().max_texture_dimension_2d;
    if now.window.iter().any(|&side| side == 0 || side > most) {
      return Ok(());
    }
    let (settings, size) = (&self.settings, self.settings.size);
    let (window, scale) = (now.window, now.scale);
    let State {
      device,
      queue,
      canvas,
      screen,
      skin,
      flat,
      interaction,
      shown,
      parts,
      fed,
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
      controls: &interaction.readings(&now.values),
      values: &now.values,
      feed: &now.feed,
      edit: now.edit.as_ref(),
      menu: now.menu.as_ref(),
      held: now.held,
      scale,
      pixel,
      pixels: [width, height],
    };
    // the skin's own flat shapes, and over them the grip and the menu
    let mut over = Flat::default();
    skin.prepare(&gpu, &seen, &mut over);
    settings.grip.shapes(&mut over, size, scale);
    if let Some(menu) = &now.menu {
      settings.menu.shapes(&mut over, menu, size, pixel);
    }
    // the window holds the frame drawn before this one, unless it has had none since it was
    // sized or shown. a skin that names where its own picture differs is read back there alone
    let named = skin.changed().filter(|_| shown.is_some());
    let patched = named.is_some_and(|named| {
      let lettered = flat.differing(&over, size, place);
      patches(parts, named, lettered, place, window)
    });
    flat.prepare(device, queue, &over, size, place);
    if patched && parts.is_empty() {
      // a frame that differs in nothing is in the window already, unless the window lost it
      if let Err(error) = screen.patch(&[], 0, &[], exposed) {
        stop(stopped, error);
        return Ok(());
      }
    } else {
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
      flat.render(&mut encoder, view);
      let whole = [[0, 0, window[0], window[1]]];
      let parts: &[[u32; 4]] = if patched { parts } else { &whole };
      let read = drawn_on.read(device, queue, encoder, parts);
      if let Some(fault) = fault.get() {
        stop(stopped, fault);
        return Ok(());
      }
      let rows = match read {
        Ok(rows) => rows,
        // the buffer may still be on its way to being read, and is not drawn on again
        Err(why) => {
          *canvas = None;
          stop(stopped, why);
          return Ok(());
        }
      };
      let pixels = drawn_on.read.slice(rows.bytes).get_mapped_range();
      let put = if patched {
        screen.patch(parts, rows.top, &pixels, exposed)
      } else {
        screen.show(window, drawn_on.row, &pixels)
      };
      drop(pixels);
      drawn_on.read.unmap();
      if let Err(error) = put {
        stop(stopped, error);
        return Ok(());
      }
    }
    if (shown.as_ref()).is_some_and(|shown| !same(&shown.feed, &now.feed)) {
      *fed = Some(at);
    }
    let follows = follows(*fed, at);
    *shown = Some(now);
    drop(state);
    if follows {
      self.redraw();
    }
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
            let differs = (scale - state.scale(self.settings.size)).abs() > 0.001;
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
      self.reshape();
      self.refresh();
    }
  }

  fn on_mouse(&self, event: MouseEvent) {
    match event {
      MouseEvent::CursorMoved {
        position,
        modifiers,
      } => self.handle(|state| {
        let pixel = [position.x as f32, position.y as f32];
        let size = self.settings.size;
        let at = point_at(fitted(state.window, size), pixel, size);
        state.pointer = at;
        state.on_window = pixel.map(|along| along / state.display_scale as f32);
        Event::Move {
          at,
          window: state.on_window,
          // only the menu shows what the pointer is over
          over: (state.interaction.menu()).and_then(|_| state.target_at(&self.settings, at)),
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
          over: state.target_at(&self.settings, state.pointer),
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
          over: state.target_at(&self.settings, state.pointer),
          lines,
          fine: fine(modifiers),
        });
      }
      _ => {}
    }
  }
}

/// Returns a graphics adapter of one of `backends`.
fn adapter_of(backends: wgpu::Backends) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
  let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends,
    ..Default::default()
  });
  pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
}

/// Tells the window manager the window's shape and how small and large it may be.
///
/// The window keeps the panel's shape, and the window manager holds it to these while its
/// frame is dragged. baseview sets the sizes alone, and this takes the place of what it set.
#[cfg(target_os = "linux")]
fn hint_shape(
  context: &WindowContext,
  display_scale: f64,
  settings: &Settings,
) -> Result<(), Box<dyn std::error::Error>> {
  use wgpu::rwh::{HasWindowHandle, RawWindowHandle};
  use x11rb::properties::{AspectRatio, WmSizeHints};

  let RawWindowHandle::Xlib(window) = context.window_handle()?.as_raw() else {
    return Err("the window is not an x11 one".into());
  };
  let (connection, _) = x11rb::connect(None)?;
  let pixels = |scale: f32| {
    let size = settings.sized(scale).to_physical::<i32>(display_scale);
    (size.width, size.height)
  };
  let shape = AspectRatio::new(settings.size[0] as i32, settings.size[1] as i32);
  let hints = WmSizeHints {
    min_size: Some(pixels(settings.scales[0])),
    max_size: Some(pixels(settings.scales[1])),
    aspect: Some((shape, shape)),
    ..WmSizeHints::default()
  };
  (hints.set_normal_hints(&connection, window.window as u32)?).check()?;
  Ok(())
}

/// Returns whether a key that slows a knob down is held.
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

/// Returns what a key press means to a value being typed.
///
/// A key held with one that makes it a command types nothing.
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

/// The window as baseview has it.
///
/// Every call into it stops at a panic.
///
/// A panic out of these would unwind into baseview's window procedure on Windows, which is
/// an `extern` function, and that ends the host's whole process. It is caught here, and
/// from then on the window does nothing: its state may be half changed, so nothing looks
/// at it again. wgpu itself panics when the device is lost while it is waited on
/// (`Device::poll` in its `backend/wgpu_core.rs`), and any fault of the editor's own ends
/// here as well.
pub(crate) struct Guarded<S> {
  window: Window<S>,
  broken: Cell<bool>,
}

impl<S: Skin> Guarded<S> {
  /// Creates the window, if making it neither fails nor panics.
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

/// Returns what a panic was raised with, where that was words.
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

  fn damage(&self, _: baseview::DamageArea) {
    // baseview 0.4.2 on windows draws every frame for a paint message and reports the message's
    // rectangle here, for the frames it asked for itself too, so there this says nothing
    if cfg!(not(target_os = "windows")) {
      self.run((), |window| window.exposed.set(true));
    }
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

#[cfg(test)]
mod tests {
  use super::*;

  /// The first of a panel's parameters.
  const PITCH: usize = 0;
  use potpot_controls::interaction::Item;

  /// Returns a frame with nothing typed or chosen.
  ///
  /// Its window is of the panel's own size.
  fn frame(values: [f32; 3]) -> Frame {
    Frame {
      values: values.into(),
      feed: Vec::new(),
      window: [750, 375],
      scale: 1.0,
      edit: None,
      menu: None,
      held: None,
    }
  }

  #[test]
  fn a_frame_is_due_when_none_is_shown_or_the_one_shown_is_not_the_panel_as_it_is() {
    let shown = frame([0.5, 0.5, 1.0]);
    // the same panel in a window that shows it: nothing to draw, however often it is asked
    assert!(!due(Some(&shown), &shown));
    // a window that has had no frame, as when it has just been sized or shown
    assert!(due(None, &shown));
    // a control somewhere else, whoever moved it and whether or not anything said so
    assert!(due(Some(&shown), &frame([0.5, 0.25, 1.0])));
    // the window another size, or the panel drawn larger in it
    let resized = Frame {
      window: [900, 375],
      ..shown.clone()
    };
    assert!(due(Some(&shown), &resized));
    let scaled = Frame {
      scale: 1.5,
      ..shown.clone()
    };
    assert!(due(Some(&shown), &scaled));
    // the menu up, moved to another row, and away again
    let menu = |hovered| Frame {
      menu: Some(Menu {
        control: 0,
        at: [10.0, 10.0],
        hovered,
      }),
      ..shown.clone()
    };
    assert!(due(Some(&shown), &menu(None)));
    assert!(due(Some(&menu(None)), &menu(Some(Item::Reset))));
    assert!(due(Some(&menu(None)), &shown));
    assert!(!due(Some(&menu(None)), &menu(None)));
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
    assert!(due(Some(&shown), &typed("1")));
    assert!(due(Some(&typed("1")), &typed("12")));
    assert!(!due(Some(&typed("12")), &typed("12")));
    // a button held down, and let go
    let held = Frame {
      held: Some(0),
      ..shown.clone()
    };
    assert!(due(Some(&shown), &held));
    assert!(due(Some(&held), &shown));
    // a number of the feed written, and one that is no number holding still
    let fed = |level| Frame {
      feed: vec![0.25, level],
      ..shown.clone()
    };
    assert!(due(Some(&fed(0.5)), &fed(0.75)));
    assert!(!due(Some(&fed(0.5)), &fed(0.5)));
    assert!(!due(Some(&fed(f32::NAN)), &fed(f32::NAN)));
  }

  #[test]
  fn the_parts_of_a_frame_are_those_of_the_skin_in_its_place_and_of_the_flat_shapes() {
    let mut parts = Vec::new();
    let (place, window) = ([50, 0, 800, 400], [900, 400]);
    // a skin's parts are moved to its place and kept to it, and one of no size is left out
    let skin = [[10, 20, 30, 40], [790, 390, 30, 30], [100, 100, 0, 9]];
    assert!(patches(
      &mut parts,
      &skin,
      Some([60, 10, 5, 5]),
      place,
      window
    ));
    assert_eq!(
      parts,
      [[60, 20, 30, 40], [840, 390, 10, 10], [60, 10, 5, 5]]
    );
    // a frame that differs in nothing has no parts, and is not read back at all
    assert!(patches(&mut parts, &[], None, place, window));
    assert!(parts.is_empty());
    // parts that make up most of the window are no better than the whole
    assert!(!patches(
      &mut parts,
      &[[0, 0, 800, 350]],
      None,
      place,
      window
    ));
  }

  /// Returns numbers from zero to one, the same ones every time.
  fn numbers() -> impl FnMut() -> f32 {
    let mut state = 0x2545_f491_u32;
    move || {
      state ^= state << 13;
      state ^= state >> 17;
      state ^= state << 5;
      (state >> 8) as f32 / (1 << 24) as f32
    }
  }

  #[test]
  #[ignore = "needs a graphics device"]
  fn a_picture_patched_with_the_parts_of_each_frame_is_the_frame_read_back_whole() {
    use crate::screen::Kept;

    let instance = wgpu::Instance::default();
    let adapter =
      pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a graphics adapter is found");
    let (device, queue) =
      pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("the adapter opens a device");
    // a panel one and a half times its size, in rows longer than their pixels
    let (size, window) = ([200.0, 100.0], [300, 150]);
    let place = fitted(window, size);
    let canvas = Canvas::new(&device, window);
    assert!(canvas.row > window[0] * 4);
    let mut flat = FlatRenderer::new(&device, FORMAT);
    let mut number = numbers();
    let point = |number: &mut dyn FnMut() -> f32| [number() * 200.0, number() * 100.0];
    let mut lines: Vec<[[f32; 2]; 2]> = (0..6)
      .map(|_| [point(&mut number), point(&mut number)])
      .collect();
    let mut kept: Option<Kept> = None;
    let (mut parts, mut patched_frames) = (Vec::new(), 0);
    let whole = [[0, 0, window[0], window[1]]];
    for frame in 0..40 {
      // one line somewhere else each frame, a box that comes and goes, and a region that grows
      let moved = frame % lines.len();
      lines[moved][frame / 6 % 2] = point(&mut number);
      let mut over = Flat::default();
      over.rect([0.0, 0.0], size, [0.1, 0.1, 0.12, 1.0]);
      for (index, [from, to]) in lines.iter().enumerate() {
        over.line(*from, *to, 1.0 + index as f32, [0.9, 0.6, 0.2, 0.8]);
      }
      over.area(
        &[[20.0, 80.0 - frame as f32], [60.0, 70.0], [90.0, 85.0]],
        95.0,
        [0.2, 0.7, 0.9, 1.0],
      );
      if frame % 5 < 2 {
        over.rect([150.3, 10.2], [170.7, 30.9], [0.8, 0.8, 0.8, 1.0]);
      }
      // as the window draws a frame, for a skin that names a part of its own each time
      let named = [[250, 120 - frame as u32, 40, 9]];
      let patched = kept.is_some() && {
        let lettered = flat.differing(&over, size, place);
        patches(&mut parts, &named, lettered, place, window)
      };
      flat.prepare(&device, &queue, &over, size, place);
      let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
      flat.render(&mut encoder, &canvas.view);
      let read: &[[u32; 4]] = if patched { &parts } else { &whole };
      let rows = (canvas.read(&device, &queue, encoder, read)).expect("the frame is read back");
      let pixels = canvas.read.slice(rows.bytes).get_mapped_range();
      match &mut kept {
        Some(kept) if patched => kept.patch(&parts, rows.top, &pixels),
        _ => {
          kept = Some(Kept {
            size: window,
            row: canvas.row,
            pixels: pixels.to_vec(),
          });
        }
      }
      drop(pixels);
      canvas.read.unmap();
      patched_frames += usize::from(patched);

      // the same frame, all of it
      let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
      let rows = (canvas.read(&device, &queue, encoder, &whole)).expect("the frame is read back");
      let pixels = canvas.read.slice(rows.bytes).get_mapped_range();
      let kept = kept.as_ref().expect("a frame was kept");
      let row = canvas.row as usize;
      for (down, (kept, drawn)) in (kept.pixels.chunks(row).zip(pixels.chunks(row))).enumerate() {
        let wide = window[0] as usize * 4;
        assert!(
          kept[..wide] == drawn[..wide],
          "row {down} of frame {frame} is not as it was drawn"
        );
      }
      drop(pixels);
      canvas.read.unmap();
    }
    assert!(patched_frames > 20, "{patched_frames} frames were patched");
  }

  #[test]
  fn frames_follow_one_another_for_a_tick_after_the_feed_last_moved() {
    let fed = Instant::now();
    assert!(!follows(None, fed));
    assert!(follows(Some(fed), fed));
    assert!(follows(Some(fed), fed + TICK - Duration::from_millis(1)));
    assert!(!follows(Some(fed), fed + TICK));
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
    assert!(!due(Some(&shown), &now(&mut sent, 0.6)));
    // the host moves it on with no report at all: seen the next time it is looked at
    assert!(due(Some(&shown), &now(&mut sent, 0.8)));
    // and the host moving it to where the editor once had it is drawn too
    let shown = now(&mut sent, 0.8);
    assert!(due(Some(&shown), &now(&mut sent, 0.6)));
  }
}
