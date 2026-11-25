//! opening a window on a panel: what the opener says of the panel and its parameters, and
//! what it keeps to talk to the open window with

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;

use baseview::dpi::LogicalSize;
use baseview::{HandlerError, WindowSettings, WindowWaker};
use potpot_controls::Point;
use potpot_controls::furniture::{GripLook, MenuLook};
use potpot_controls::interaction::{Control, Dial, Key};
use tracing::error;
use wgpu::rwh::HasWindowHandle;

use crate::skin::{Gpu, Skin};
use crate::window::{Guarded, Window};

/// how a panel is shown: what there is to say of its window before anything is drawn in it.
/// [`Settings::new`] takes what every panel has to say, and the rest is changed a field at a
/// time
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Settings {
  /// what the window is called where it has a title
  pub title: String,
  /// its width and height in its own points, whole numbers of them. the window keeps this
  /// shape
  pub size: Point,
  /// the least and the most times its own size the window is
  pub scales: [f32; 2],
  /// the look of a control's menu
  pub menu: MenuLook,
  /// the look of the grip the window is pulled larger and smaller by
  pub grip: GripLook,
}

impl Settings {
  /// a window called `title` for a panel of `size`, which is pulled from half the panel's
  /// own size to three times it, with the menu and the grip looking as they do by default
  pub fn new(title: impl Into<String>, size: Point) -> Self {
    Self {
      title: title.into(),
      size,
      scales: [0.5, 3.0],
      menu: MenuLook::default(),
      grip: GripLook::default(),
    }
  }

  /// the window's size in the screen's points at so many times the panel's own
  pub fn sized(&self, scale: f32) -> LogicalSize<f32> {
    LogicalSize::new(self.size[0] * scale, self.size[1] * scale)
  }
}

/// one of a panel's parameters: what its control does, and its ways to the host. the window
/// calls these on its own thread, which is not the opener's on every system
#[non_exhaustive]
pub struct Param {
  /// what its control is like
  pub dial: Dial,
  /// where it stands, as a part of its whole turn
  pub value: Box<dyn Fn() -> f32 + Send>,
  /// tells the host a gesture on it begins
  pub begin: Box<dyn Fn() + Send>,
  /// sets it, between a begin and an end
  pub set: Box<dyn Fn(f32) + Send>,
  /// tells the host the gesture is over
  pub end: Box<dyn Fn() + Send>,
}

impl Param {
  /// a parameter a host has, from what its control does and its four ways to the host
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

  /// a parameter no host has: it stands where it was last set, from its dial's default on
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

/// where the size the user chose for the window is kept, as so many times the panel's own.
/// the window sets it when the user pulls the grip and at no other time
pub struct Remembered {
  /// the scale to open at
  pub get: Box<dyn Fn() -> f32 + Send + Sync>,
  /// keeps a scale the user chose
  pub set: Box<dyn Fn(f32) + Send + Sync>,
}

impl Remembered {
  /// a size remembered for as long as this is kept and no longer, `scale` to begin with
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

/// what an open window and its opener both know, and the opener may want before any window
/// is open and after one has closed
#[derive(Debug)]
pub struct Shared {
  /// the display's scale factor as the bits of an f64
  display_scale: AtomicU64,
  typing: AtomicBool,
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
    }
  }
}

impl Shared {
  /// how many pixels the display has to a point: the last window's, and before any window
  /// what the system says
  pub fn display_scale(&self) -> f64 {
    f64::from_bits(self.display_scale.load(Ordering::Relaxed))
  }

  pub(crate) fn set_display_scale(&self, scale: f64) {
    self.display_scale.store(scale.to_bits(), Ordering::Relaxed);
  }

  /// whether a value is being typed, which is when the window wants the keys a host offers
  pub fn typing(&self) -> bool {
    self.typing.load(Ordering::Relaxed)
  }

  pub(crate) fn set_typing(&self, typing: bool) {
    self.typing.store(typing, Ordering::Relaxed);
  }
}

/// how many pixels the display has to a point, as well as that can be told with no window
/// open on it. a host asks a view's size before it gives it a window to be in, and on
/// windows takes the answer in pixels: of the window the view is to go in if there is one,
/// and else of the system
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

/// how many pixels the display has to a point, as well as that can be told with no window
/// open on it. a host asks a view's size before it gives it a window to be in, and on x11
/// takes the answer in pixels. it is the setting baseview sizes its windows by
#[cfg(target_os = "linux")]
fn display_scale() -> f64 {
  let dpi = x11rb::connect(None).ok().and_then(|(connection, _)| {
    let settings = x11rb::resource_manager::new_from_default(&connection).ok()?;
    settings.get_value::<u32>("Xft.dpi", "").ok().flatten()
  });
  // with no server to ask or nothing set, baseview has one pixel to a point as well
  dpi.map_or(1.0, |dpi| f64::from(dpi) / 96.0)
}

/// where a window goes and who it answers to
#[non_exhaustive]
pub struct Opening<'a, W> {
  /// the window it is put in, or none for one of its own
  pub parent: Option<&'a W>,
  /// whether a window to be put in comes later
  pub wait_for_parent: bool,
  /// the display's scale as a host gave it, if it gave one
  pub host_scale: Option<f64>,
  /// the host's side of resizing, if there is a host
  pub host: Option<baseview::host::Host>,
  /// where the size the user chooses is kept
  pub remembered: Arc<Remembered>,
  /// what the opener wants to know of the window while it is open and after
  pub shared: Arc<Shared>,
}

impl Opening<'static, baseview::PlatformHandle> {
  /// a window of its own, with no host, which opens at the panel's own size
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

/// what the opener tells the open window, from its own thread to the window's
pub(crate) enum ToWindow {
  /// a key a host offered while a value is typed
  Key(Key),
  /// the window has been shown, or put in another window
  Shown,
  /// every parameter and the remembered size may have changed
  State,
}

/// the opener's way to tell an open window things
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

  /// a key a host offered while a value is typed
  pub fn key(&self, key: Key) {
    self.tell(ToWindow::Key(key));
  }

  /// the window has been shown, or put in another window: whatever was drawn before it
  /// could be seen is drawn again
  pub fn shown(&self) {
    self.tell(ToWindow::Shown);
  }

  /// every parameter and the remembered size may have changed, as when a host loads a state
  pub fn state_changed(&self) {
    self.tell(ToWindow::State);
  }

  /// a parameter may have changed. the window looks at them all itself every so often, so
  /// one of these that never comes costs only that wait
  pub fn param_changed(&self) {
    self.waker.request_poll();
  }
}

/// an open window, and the way to tell it things
pub struct Opened {
  /// baseview's own handle on it, which shows, sizes and closes it
  pub window: baseview::Window,
  /// what the window is told through
  pub tell: Tell,
}

impl fmt::Debug for Opened {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Opened").finish_non_exhaustive()
  }
}

/// opens a window on a panel of `controls` that turn `params`, drawn by the skin that `skin`
/// makes once there is a graphics device to make it with. it opens only if the device can
/// build everything the skin draws with, and from then on nothing that goes wrong with the
/// device or the skin is let out into the opener's process: it is logged and the window
/// keeps its last picture
///
/// # Errors
///
/// if a side of the panel's size is not a finite number of points, one at least, which is
/// refused before any window is made, or if the system cannot make the window or the window
/// cannot be set up in it
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
  // with no window to go in it is the window manager's to size, which is held to the sizes
  // the panel is drawn at. in a host's window it takes the size the host gives it
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
