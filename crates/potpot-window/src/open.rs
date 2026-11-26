//! The opening of a window on a panel.
//!
//! It holds what the opener says of the panel and its parameters, and what the opener keeps
//! to talk to the open window with.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};

use baseview::dpi::LogicalSize;
use baseview::{HandlerError, WindowSettings, WindowWaker};
use potpot_controls::Point;
use potpot_controls::furniture::{GripLook, MenuLook};
use potpot_controls::interaction::{Control, Dial, Key};
use tracing::error;
use wgpu::rwh::HasWindowHandle;

use crate::feed::Feed;
use crate::graphics::Graphics;
use crate::skin::{Gpu, Skin};
use crate::window::{Guarded, Window};

/// The settings a panel's window opens with.
///
/// They are what there is to say of the window before anything is drawn in it.
/// [`Settings::new`] takes the title and the size. The scales, the looks of the menu and the
/// grip, and the feed are changed a field at a time.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Settings {
  /// The window's name, where the window has a title.
  pub title: String,
  /// The panel's width and height in its own points, in whole numbers.
  ///
  /// The window keeps this shape.
  pub size: Point,
  /// The least and the most multiples of the panel's own size the window is.
  pub scales: [f32; 2],
  /// The look of a control's menu.
  pub menu: MenuLook,
  /// The look of the grip the window is pulled larger and smaller by.
  pub grip: GripLook,
  /// The [`Feed`] whose numbers the skin is handed.
  pub feed: Feed,
}

impl Settings {
  /// Creates the settings of a window called `title` for a panel of `size`.
  ///
  /// The window is pulled from half the panel's own size to three times it, the menu and the
  /// grip look as they do by default, and the feed has no slots.
  pub fn new(title: impl Into<String>, size: Point) -> Self {
    Self {
      title: title.into(),
      size,
      scales: [0.5, 3.0],
      menu: MenuLook::default(),
      grip: GripLook::default(),
      feed: Feed::default(),
    }
  }

  /// Returns the window's size in the screen's points at `scale`.
  ///
  /// `scale` is a multiple of the panel's own size.
  pub fn sized(&self, scale: f32) -> LogicalSize<f32> {
    LogicalSize::new(self.size[0] * scale, self.size[1] * scale)
  }
}

/// One of a panel's parameters.
///
/// It holds what its control does and its ways to the host.
///
/// The window calls these on its own thread. On some systems that is another thread than
/// the opener's.
#[non_exhaustive]
pub struct Param {
  /// The [`Dial`] that says what its control is like.
  pub dial: Dial,
  /// The function that returns where it stands, as a part of its whole turn.
  pub value: Box<dyn Fn() -> f32 + Send>,
  /// The function that tells the host a gesture on it begins.
  pub begin: Box<dyn Fn() + Send>,
  /// The function that sets it, between a begin and an end.
  pub set: Box<dyn Fn(f32) + Send>,
  /// The function that tells the host the gesture is over.
  pub end: Box<dyn Fn() + Send>,
}

impl Param {
  /// Creates a parameter that a host has.
  ///
  /// `dial` says what its control does, and the four functions are its ways to the host.
  pub fn new(
    dial: Dial,
    value: impl Fn() -> f32 + Send + 'static,
    begin: impl Fn() + Send + 'static,
    set: impl Fn(f32) + Send + 'static,
    end: impl Fn() + Send + 'static,
  ) -> Self {
    Self {
      dial,
      value: Box::new(value),
      begin: Box::new(begin),
      set: Box::new(set),
      end: Box::new(end),
    }
  }

  /// Creates a parameter that no host has.
  ///
  /// It stands where it was last set, from its dial's default on.
  pub fn free(dial: Dial) -> Self {
    let held = Arc::new(AtomicU32::new(dial.default.to_bits()));
    let (value, set) = (Arc::clone(&held), held);
    Self {
      dial,
      value: Box::new(move || f32::from_bits(value.load(Ordering::Relaxed))),
      begin: Box::new(|| {}),
      set: Box::new(move |to| set.store(to.to_bits(), Ordering::Relaxed)),
      end: Box::new(|| {}),
    }
  }
}

impl fmt::Debug for Param {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Param")
      .field("dial", &self.dial)
      .finish_non_exhaustive()
  }
}

/// The store of the size the user chose for the window.
///
/// The size is kept as a multiple of the panel's own. The window sets it when the user pulls
/// the grip, and to the panel's own size on a double click of the grip. The `nice-plug` editor
/// also sets it when the host gives the window a size other than the one the editor last
/// reported, within the panel's scales.
pub struct Remembered {
  /// The function that returns the scale to open at.
  pub get: Box<dyn Fn() -> f32 + Send + Sync>,
  /// The function that keeps a scale the user chose.
  pub set: Box<dyn Fn(f32) + Send + Sync>,
}

impl Remembered {
  /// Creates a store that begins at `scale`.
  ///
  /// The size is remembered for as long as the store is kept.
  pub fn new(scale: f32) -> Self {
    let held = Arc::new(AtomicU32::new(scale.to_bits()));
    let (get, set) = (Arc::clone(&held), held);
    Self {
      get: Box::new(move || f32::from_bits(get.load(Ordering::Relaxed))),
      set: Box::new(move |scale| set.store(scale.to_bits(), Ordering::Relaxed)),
    }
  }
}

impl fmt::Debug for Remembered {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Remembered")
      .field("scale", &(self.get)())
      .finish_non_exhaustive()
  }
}

/// The state an open window and its opener both know.
///
/// The opener may want it before any window is open and after one has closed.
///
/// It also holds the graphics device of the window last opened with it. The windows of one
/// plugin draw with one device, which is made when a window opens and nothing holds one. A
/// window opened with a [`Shared`] that has had a window before finds the device made, unless
/// the device has had a fault or been lost since.
///
/// Frames of windows on one device run one after another on it, so a window's frame can wait
/// for a neighbor's.
#[derive(Debug)]
pub struct Shared {
  /// The display's scale factor as the bits of an `f64`.
  display_scale: AtomicU64,
  typing: AtomicBool,
  /// The graphics device of the window last opened with this.
  graphics: Mutex<Option<Arc<Graphics>>>,
}

impl Default for Shared {
  fn default() -> Self {
    #[cfg(target_os = "windows")]
    let display_scale = display_scale(None);
    #[cfg(target_os = "linux")]
    let display_scale = display_scale();
    // sizes go to and from a host in points there
    #[cfg(target_os = "macos")]
    let display_scale = 1.0_f64;
    Self {
      display_scale: AtomicU64::new(display_scale.to_bits()),
      typing: AtomicBool::new(false),
      graphics: Mutex::new(None),
    }
  }
}

impl Shared {
  /// Returns how many pixels the display has to a point.
  ///
  /// It is the last window's count. Before any window it is what the system says on Windows
  /// and Linux, and one on macOS.
  pub fn display_scale(&self) -> f64 {
    f64::from_bits(self.display_scale.load(Ordering::Relaxed))
  }

  pub(crate) fn set_display_scale(&self, scale: f64) {
    self.display_scale.store(scale.to_bits(), Ordering::Relaxed);
  }

  /// Returns whether a value is being typed.
  ///
  /// While one is, the window wants the keys a host offers.
  pub fn typing(&self) -> bool {
    self.typing.load(Ordering::Relaxed)
  }

  pub(crate) fn set_typing(&self, typing: bool) {
    self.typing.store(typing, Ordering::Relaxed);
  }

  /// Holds a graphics device in place of the one held before.
  pub(crate) fn keep(&self, graphics: Arc<Graphics>) {
    // a panic under the lock leaves what it guards whole
    let mut kept = (self.graphics.lock()).unwrap_or_else(PoisonError::into_inner);
    *kept = Some(graphics);
  }
}

/// Returns how many pixels the display has to a point.
///
/// With no window open on the display, this is as well as that can be told. A host asks a
/// view's size before it gives it a window to be in, and on Windows takes the answer in
/// pixels: of the window the view is to go in if there is one, and else of the system.
#[cfg(target_os = "windows")]
fn display_scale(parent: Option<windows_sys::Win32::Foundation::HWND>) -> f64 {
  use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};

  // SAFETY: neither call asks anything of what it is given. both answer for a program that
  // is not aware of the display's scale with the 96 it is drawn at
  let dpi = unsafe {
    match parent {
      Some(window) => GetDpiForWindow(window),
      None => GetDpiForSystem(),
    }
  };
  f64::from(dpi) / 96.0
}

/// Returns how many pixels the display has to a point.
///
/// With no window open on the display, this is as well as that can be told. A host asks a
/// view's size before it gives it a window to be in, and on X11 takes the answer in pixels.
/// The count is the setting baseview sizes its windows by.
#[cfg(target_os = "linux")]
fn display_scale() -> f64 {
  let dpi = x11rb::connect(None).ok().and_then(|(connection, _)| {
    let settings = x11rb::resource_manager::new_from_default(&connection).ok()?;
    settings.get_value::<u32>("Xft.dpi", "").ok().flatten()
  });
  // with no server to ask or nothing set, baseview has one pixel to a point as well
  dpi.map_or(1.0, |dpi| f64::from(dpi) / 96.0)
}

/// The place a window goes and who it answers to.
#[non_exhaustive]
pub struct Opening<'a, W> {
  /// The window it is put in, if there is one yet.
  pub parent: Option<&'a W>,
  /// Whether a window to be put in comes later.
  pub wait_for_parent: bool,
  /// The display's scale as a host gave it, if it gave one.
  pub host_scale: Option<f64>,
  /// The host's side of resizing, if there is a host.
  pub host: Option<baseview::host::Host>,
  /// The store of the size the user chooses.
  pub remembered: Arc<Remembered>,
  /// The state the opener reads of the window while it is open and after.
  pub shared: Arc<Shared>,
}

impl Opening<'static, baseview::PlatformHandle> {
  /// Creates the opening of a window of its own, with no host.
  ///
  /// The window opens at the panel's own size.
  pub fn alone() -> Self {
    Self {
      parent: None,
      wait_for_parent: false,
      host_scale: None,
      host: None,
      remembered: Arc::new(Remembered::new(1.0)),
      shared: Arc::default(),
    }
  }
}

impl<W> fmt::Debug for Opening<'_, W> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Opening")
      .field("parented", &self.parent.is_some())
      .field("wait_for_parent", &self.wait_for_parent)
      .field("host_scale", &self.host_scale)
      .finish_non_exhaustive()
  }
}

/// A message from the opener's thread to the open window's.
pub(crate) enum ToWindow {
  /// A key a host offered while a value is typed.
  Key(Key),
  /// The window was shown, or put in another window.
  Shown,
  /// Every parameter and the remembered size may have changed.
  State,
}

/// The opener's way to tell an open window things.
pub struct Tell {
  waker: WindowWaker,
  to_window: mpsc::Sender<ToWindow>,
}

impl fmt::Debug for Tell {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Tell").finish_non_exhaustive()
  }
}

impl Tell {
  fn tell(&self, told: ToWindow) {
    // a window that has closed has nothing left to tell
    if self.to_window.send(told).is_ok() {
      self.waker.request_poll();
    }
  }

  /// Tells the window of a key a host offered while a value is typed.
  pub fn key(&self, key: Key) {
    self.tell(ToWindow::Key(key));
  }

  /// Tells the window it has been shown, or put in another window.
  ///
  /// Whatever was drawn before the window could be seen is drawn again.
  pub fn shown(&self) {
    self.tell(ToWindow::Shown);
  }

  /// Tells the window that its whole state may have changed.
  ///
  /// Every parameter and the remembered size may have changed, as when a host loads a state.
  pub fn state_changed(&self) {
    self.tell(ToWindow::State);
  }

  /// Tells the window a parameter may have changed.
  ///
  /// The window looks at them all itself every so often, so a call that never comes costs
  /// only that wait.
  pub fn param_changed(&self) {
    self.waker.request_poll();
  }
}

/// An open window and the way to tell it things.
pub struct Opened {
  /// The handle of baseview's own that shows, sizes and closes the window.
  pub window: baseview::Window,
  /// The way the window is told things.
  pub tell: Tell,
}

impl fmt::Debug for Opened {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Opened").finish_non_exhaustive()
  }
}

/// Opens a window on a panel.
///
/// The panel's `controls` turn `params`, and `skin` makes the [`Skin`] that draws the panel
/// once there is a graphics device to make it with. The window opens only if the device can
/// build everything the skin draws with. From then on nothing that goes wrong with the device
/// or the skin is let out into the opener's process: it is logged and the window keeps its
/// last picture.
///
/// # Errors
///
/// Returns an error if a side of the panel's size is not a finite number of points, one at
/// least, which is refused before any window is made.
///
/// Returns an error if the system cannot make the window.
///
/// Returns an error if the window cannot be set up in it.
///
/// # Examples
///
/// ```no_run
/// # use potpot_window::controls::flat::Flat;
/// # use potpot_window::controls::interaction::Target;
/// # use potpot_window::{Gpu, Skin, View, wgpu};
/// # struct Bar;
/// # impl Skin for Bar {
/// #   fn prepare(&mut self, _: &Gpu, _: &View, _: &mut Flat) {}
/// #   fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {}
/// #   fn target_at(&self, _: [f32; 2]) -> Option<Target> {
/// #     None
/// #   }
/// # }
/// use potpot_window::controls::interaction::{Control, Dial, Readout};
/// use potpot_window::{Opening, Param, Settings};
///
/// let settings = Settings::new("Bar", [200.0, 100.0]);
/// let params = vec![Param::free(Dial::linear([0.0, 1.0], 0.5, None, 2, ""))];
/// let controls = vec![Control::new(0, Readout::new(4))];
/// let opened = potpot_window::open(settings, params, controls, Opening::alone(), |_| Bar)?;
/// opened.window.run_until_closed()?;
/// # Ok::<(), potpot_window::baseview::Error>(())
/// ```
pub fn open<S: Skin, W: HasWindowHandle>(
  settings: Settings,
  params: Vec<Param>,
  controls: Vec<Control>,
  opening: Opening<'_, W>,
  skin: impl FnOnce(&Gpu) -> S + Send + 'static,
) -> Result<Opened, baseview::Error> {
  let Opening {
    parent,
    wait_for_parent,
    host_scale,
    host,
    remembered,
    shared,
  } = opening;
  // a panel is fitted to a window by its sides in whole points, which are counted as u64
  // and multiplied: none of them is to be zero or past counting
  let fits = |side: &f32| side.is_finite() && *side >= 1.0;
  if !settings.size.iter().all(fits) {
    return Err(
      HandlerError::from_boxed(
        "a side of the panel is not a finite number of points, one at least".into(),
      )
      .into(),
    );
  }
  // baseview 0.4.2 makes a child window at the scale of the window it goes in, but in a
  // host that is aware of each monitor's scale the older way it reports the scale the host
  // gave, 1 if it gave none, and takes the first one the host gives later as a change:
  // the window is made that many times larger again (`WindowSharedState::init` and
  // `scale_factor`, and `WindowHandle::suggest_scale_factor`, in its `platform/win`). so
  // it is given the parent's scale as the host's from the start
  // TODO: pass the host's own on alone once baseview reports the scale it made the window at
  #[cfg(target_os = "windows")]
  let host_scale = {
    use wgpu::rwh::RawWindowHandle;

    let window = parent.and_then(|parent| match parent.window_handle().ok()?.as_raw() {
      RawWindowHandle::Win32(window) => Some(window.hwnd.get() as _),
      _ => None,
    });
    let scale = display_scale(window);
    shared.set_display_scale(scale);
    host_scale.or(Some(scale))
  };
  // with no window to go in the window manager sizes it, and is held to the sizes the
  // panel is drawn at. in a host's window it takes the size the host gives it
  let top_level = parent.is_none() && !wait_for_parent;
  let made = WindowSettings::new()
    .with_title(settings.title.clone())
    .with_size(settings.sized((remembered.get)()))
    .with_min_size::<LogicalSize<f32>>(top_level.then(|| settings.sized(settings.scales[0])))
    .with_max_size::<LogicalSize<f32>>(top_level.then(|| settings.sized(settings.scales[1])))
    .with_parent(parent)
    .with_wait_for_parent(wait_for_parent)
    .with_fallback_scale_factor(host_scale);
  let (to_window, told) = mpsc::channel();
  let window = baseview::Window::create_with_host(
    made,
    move |context| -> Result<Guarded<S>, HandlerError> {
      Guarded::new(|| {
        Window::new(
          context, settings, params, controls, remembered, shared, told, top_level, skin,
        )
      })
      // baseview hands on an error of the system's in place of this one
      .inspect_err(|error| error!("the editor could not open: {error}"))
    },
    host,
  )?;
  Ok(Opened {
    tell: Tell {
      waker: window.waker(),
      to_window,
    },
    window,
  })
}
