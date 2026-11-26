//! The window as a nice-plug editor.
//!
//! The wrapper's calls are turned into the window's, sizes pass to and from the host in the
//! units the host has them in, and the panel's parameters are bound to the plugin's.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use baseview::HandlerError;
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::NativeSize;
use nice_plug::editor::{EditorHandle, HostMethods, ResizeHint, SpawnedEditor};
use nice_plug::params::Params;
use nice_plug::params::internals::ParamPtr;
use nice_plug::prelude::{Modifiers, ParentWindowHandle, VirtualKeyCode};
use potpot_controls::Point;
use potpot_controls::fit::{chosen, shaped};
use potpot_controls::interaction::{Control, Dial, Feel, Key, Readout, Touch};

use crate::open::{Opening, Param, Remembered, Settings, Shared, Tell, open};
use crate::skin::{Gpu, Skin};

/// One of a panel's controls bound to one of the plugin's parameters.
///
/// The panel's parameters are the bound ones in the order of the editor's `bound`, so the
/// control and its parameter are at the same place. `touch` and `wheel` name any other parameter
/// by the place of the [`Bound`] it is bound in, and a skin gives a control no place on the
/// panel where only the parameter is wanted.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Bound {
  param: ParamPtr,
  /// The coarse step the control moves by, in the parameter's plain units.
  ///
  /// With `None` the control turns smoothly.
  pub step: Option<f32>,
  /// The way the control answers the pointer and the wheel.
  pub feel: Feel,
  /// The letters the control's readout can show of a value typed into it.
  pub readout: Readout,
  /// The effect of the left button on the control.
  pub touch: Touch,
  /// The parameter the wheel turns, if another than its own.
  pub wheel: Option<usize>,
}

impl Bound {
  /// Creates a control on `param`.
  ///
  /// `param` has to be one of the parameters the editor is made with. The control is dragged,
  /// turns smoothly and feels as [`Feel::default`] does, and the wheel turns `param` too.
  pub fn new(param: &impl nice_plug::params::Param, readout: Readout) -> Self {
    Self {
      param: param.as_ptr(),
      step: None,
      feel: Feel::default(),
      readout,
      touch: Touch::Turn,
      wheel: None,
    }
  }

  /// Creates a control on `up` that is dragged both ways.
  ///
  /// A drag up the panel turns `up` and a drag to the right turns the parameter bound at the
  /// place `across`, over a room of `size`, as [`Control::pad`] has it.
  ///
  /// # Panics
  ///
  /// Panics if a side of `size` is not a finite number of points above zero.
  pub fn pad(
    across: usize,
    up: &impl nice_plug::params::Param,
    size: Point,
    readout: Readout,
  ) -> Self {
    // its own parameter is the place the bound is given in the editor, which is not known here
    let Control { feel, touch, .. } = Control::pad(across, 0, size, readout);
    Self {
      feel,
      touch,
      ..Self::new(up, readout)
    }
  }

  /// Returns the control it makes at `place` on the panel.
  ///
  /// The control's own parameter is at the same place.
  fn control(&self, place: usize) -> Control {
    let mut control = Control::new(place, self.readout);
    control.feel = self.feel;
    control.touch = self.touch;
    control.wheel = self.wheel;
    control
  }
}

/// A parameter together with the object it is part of.
///
/// Holding the object is what makes the parameter safe to reach. The pointer is into
/// `_params`: [`Editor::new`] lets nothing else be bound. `_params` is held here, so the
/// parameter is alive for as long as this is, wherever this has been handed on to: the
/// window's thread, a closure still waiting to be called, a clone of one.
#[derive(Clone)]
struct Held {
  /// The object that keeps `param` alive, which is never read.
  _params: Arc<dyn Params>,
  param: ParamPtr,
}

impl Held {
  /// Returns where the parameter stands, without what the host adds to it.
  fn value(&self) -> f32 {
    // SAFETY: `param` points into `_params`, which `self` keeps alive
    unsafe { self.param.unmodulated_normalized_value() }
  }

  /// Returns the parameter's [`Dial`] for a control that moves by `step`.
  fn dial(&self, step: Option<f32>) -> Dial {
    let (plain, turn, write, read) = (self.clone(), self.clone(), self.clone(), self.clone());
    let mut dial = Dial::new(
      // SAFETY: `param` points into `_params`, which `self` keeps alive, as does each of
      // the clones the closures below own
      unsafe { self.param.default_normalized_value() },
      // SAFETY: the same
      move |value| unsafe { plain.param.preview_plain(value) },
      // SAFETY: the same
      move |plain| unsafe { turn.param.preview_normalized(plain) },
      // SAFETY: the same
      move |value| unsafe { write.param.normalized_value_to_string(value, false) },
      // SAFETY: the same
      move |text| unsafe { read.param.string_to_normalized_value(text) },
    );
    dial.step = step;
    // SAFETY: the same
    dial.unit = unsafe { self.param.unit() }.to_owned();
    dial
  }

  /// Returns the parameter as the window has it.
  ///
  /// Its ways to the host go through `gui`.
  fn bound(&self, step: Option<f32>, gui: &GuiContext) -> Param {
    let (value, begin, set, end) = (self.clone(), self.clone(), self.clone(), self.clone());
    let (began, sets, ended) = (gui.clone(), gui.clone(), gui.clone());
    Param {
      dial: self.dial(step),
      value: Box::new(move || value.value()),
      // SAFETY: the wrapper asks that the parameter exists, and it is one of `_params`'
      // own, which `begin` keeps alive
      begin: Box::new(move || unsafe { began.raw_begin_set_parameter(begin.param) }),
      // SAFETY: the same, of `set`
      set: Box::new(move |to| unsafe { sets.raw_set_parameter_normalized(set.param, to) }),
      // SAFETY: the same, of `end`
      end: Box::new(move || unsafe { ended.raw_end_set_parameter(end.param) }),
    }
  }
}

/// The error of a panel with a control that is bound wrongly.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Misbound {
  /// The place among the panel's controls of the first at fault.
  pub control: usize,
  /// The fault of that control.
  pub fault: Fault,
}

/// The way a control of a panel is bound wrongly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fault {
  /// Its parameter is outside the plugin's own.
  Outside,
  /// A parameter it names is not another of the panel's.
  ///
  /// Its `touch` or `wheel` names a place past the last [`Bound`], or a parameter the control
  /// turns already.
  NotAnother,
}

impl fmt::Display for Misbound {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Self { control, fault } = self;
    match fault {
      Fault::Outside => write!(
        f,
        "control {control} of the panel is bound to a parameter outside the plugin's own"
      ),
      Fault::NotAnother => write!(
        f,
        "control {control} of the panel names a parameter that is not another of the panel's"
      ),
    }
  }
}

impl Error for Misbound {}

/// A panel as a nice-plug editor, drawn by the skin `S`.
///
/// # Examples
///
/// ```
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
/// use std::sync::Arc;
///
/// use nice_plug::prelude::{FloatParam, FloatRange, Params};
/// use potpot_window::controls::interaction::Readout;
/// use potpot_window::nice_plug::{Bound, Editor};
/// use potpot_window::{Remembered, Settings};
///
/// #[derive(Params)]
/// struct Gain {
///   #[id = "gain"]
///   gain: FloatParam,
/// }
///
/// let params = Arc::new(Gain {
///   gain: FloatParam::new("Gain", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
/// });
/// let editor = Editor::new(
///   params.clone(),
///   Settings::new("Gain", [200.0, 100.0]),
///   vec![Bound::new(&params.gain, Readout::new(4))],
///   Remembered::new(1.0),
///   |_| Bar,
/// )?;
/// # Ok::<(), potpot_window::nice_plug::Misbound>(())
/// ```
pub struct Editor<S> {
  params: Arc<dyn Params>,
  settings: Settings,
  bound: Vec<Bound>,
  remembered: Arc<Remembered>,
  shared: Arc<Shared>,
  skin: Arc<dyn Fn(&Gpu) -> S + Send + Sync>,
}

impl<S> fmt::Debug for Editor<S> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Editor")
      .field("settings", &self.settings)
      .field("bound", &self.bound)
      .finish_non_exhaustive()
  }
}

impl<S: Skin> Editor<S> {
  /// Creates an editor for a panel bound to `params`.
  ///
  /// `bound` binds the panel's controls, in the panel's order, to parameters of `params`.
  /// `skin` makes the panel's look each time the editor is opened.
  ///
  /// # Errors
  ///
  /// Returns an error if a control is bound to a parameter outside `params`' own.
  ///
  /// Returns an error if the `touch` or `wheel` of a control names a place past the last of
  /// `bound`, or a parameter the control turns already.
  pub fn new(
    params: Arc<dyn Params>,
    settings: Settings,
    bound: Vec<Bound>,
    remembered: Remembered,
    skin: impl Fn(&Gpu) -> S + Send + Sync + 'static,
  ) -> Result<Self, Misbound> {
    let ours: Vec<ParamPtr> = (params.param_map().into_iter())
      .map(|(_, param, _)| param)
      .collect();
    let fault = (bound.iter().enumerate()).find_map(|(control, one)| {
      let fault = if !ours.contains(&one.param) {
        Fault::Outside
      } else if !one.control(control).fits(bound.len()) {
        Fault::NotAnother
      } else {
        return None;
      };
      Some(Misbound { control, fault })
    });
    match fault {
      Some(fault) => Err(fault),
      None => Ok(Self {
        params,
        settings,
        bound,
        remembered: Arc::new(remembered),
        shared: Arc::default(),
        skin: Arc::new(skin),
      }),
    }
  }
}

/// Returns how many of the host's units go to a point.
///
/// Sizes pass to and from the host in pixels, apart from on macOS, where they are in points.
fn per_point(display_scale: f64) -> f64 {
  if cfg!(target_os = "macos") {
    1.0
  } else {
    display_scale
  }
}

/// Returns the size the host is told the editor has, in the host's units.
fn reported(settings: &Settings, remembered: &Remembered, shared: &Shared) -> NativeSize<u32> {
  let size = settings.sized((remembered.get)());
  NativeSize::from_size(size.into(), shared.display_scale())
}

impl<S: Skin> nice_plug::editor::Editor for Editor<S> {
  type Handle = Handle;

  fn spawn(
    &self,
    parent: Option<ParentWindowHandle>,
    wait_for_parent: bool,
    fallback_scale_factor: Option<f64>,
    gui_context: GuiContext,
    host: Option<HostMethods>,
  ) -> Result<SpawnedEditor<Handle>, Box<dyn Error>> {
    let host = host.map(|host| {
      baseview::host::Host::new()
        .with_callbacks(HostCallbacks(host.callbacks))
        .with_main_thread(MainThreadCaller(host.main_thread_caller))
    });
    let params = (self.bound.iter())
      .map(|bound| {
        let held = Held {
          _params: Arc::clone(&self.params),
          param: bound.param,
        };
        held.bound(bound.step, &gui_context)
      })
      .collect();
    // each bound control turns the parameter bound at its own place
    let controls = (self.bound.iter().enumerate())
      .map(|(place, bound)| bound.control(place))
      .collect();
    let opening = Opening {
      parent: parent.as_ref(),
      wait_for_parent,
      host_scale: fallback_scale_factor,
      host,
      remembered: Arc::clone(&self.remembered),
      shared: Arc::clone(&self.shared),
    };
    let skin = Arc::clone(&self.skin);
    let opened = open(
      self.settings.clone(),
      params,
      controls,
      opening,
      move |gpu| skin(gpu),
    )?;
    Ok(SpawnedEditor {
      handle: Handle {
        tell: opened.tell,
        settings: self.settings.clone(),
        remembered: Arc::clone(&self.remembered),
        shared: Arc::clone(&self.shared),
      },
      window: opened.window,
    })
  }

  fn size(&self) -> NativeSize<u32> {
    reported(&self.settings, &self.remembered, &self.shared)
  }

  /// Returns the sizes the window may take.
  ///
  /// The window may be sized at the panel's own shape, from the smallest to the largest it
  /// is drawn at.
  fn resize_hint(&self) -> ResizeHint {
    let Settings { size, scales, .. } = &self.settings;
    ResizeHint {
      preserve_aspect_ratio: true,
      aspect_ratio_width: size[0] as u32,
      aspect_ratio_height: size[1] as u32,
      ..ResizeHint::RESIZABLE.with_min_max_logical_size(
        Some(self.settings.sized(scales[0])),
        Some(self.settings.sized(scales[1])),
      )
    }
  }
}

/// The host's side of an open window.
#[derive(Debug)]
pub struct Handle {
  tell: Tell,
  settings: Settings,
  remembered: Arc<Remembered>,
  shared: Arc<Shared>,
}

impl EditorHandle for Handle {
  type Window = baseview::Window;
  type Error = baseview::Error;

  fn run_until_closed(window: baseview::Window) -> Result<(), baseview::Error> {
    window.run_until_closed()
  }

  fn set_parent(
    &self,
    parent: ParentWindowHandle,
    window: &baseview::Window,
  ) -> Result<(), baseview::Error> {
    let parented = window.set_parent(&parent);
    self.tell.shown();
    parented
  }

  fn show(&self, window: &baseview::Window) -> Result<(), baseview::Error> {
    let shown = window.show();
    self.tell.shown();
    shown
  }

  fn hide(&self, window: &baseview::Window) -> Result<(), baseview::Error> {
    window.hide()
  }

  fn set_size(
    &self,
    new_size: NativeSize<u32>,
    window: &baseview::Window,
  ) -> Result<(), baseview::Error> {
    // a size the host comes up with is the user's doing, at the host's own frame, and is
    // what the editor opens at from then on. the window's settling as it opens is left out
    let told = reported(&self.settings, &self.remembered, &self.shared);
    let (given, told) = ([new_size.width, new_size.height], [told.width, told.height]);
    let per_point = per_point(window.size().scale_factor);
    if let Some(scale) = chosen(
      given,
      told,
      per_point,
      self.settings.size,
      self.settings.scales,
    ) {
      (self.remembered.set)(scale);
    }
    window.resize(new_size)
  }

  fn adjust_size(
    &self,
    new_size: NativeSize<u32>,
    window: &baseview::Window,
  ) -> Option<NativeSize<u32>> {
    let now = window.size();
    let current = NativeSize::from_logical_or_physical(now.logical, now.physical);
    let limit = |scale: f32| {
      let limit = self.settings.sized(scale);
      let limit = NativeSize::<u32>::from_size(limit.into(), now.scale_factor);
      [limit.width, limit.height]
    };
    let [width, height] = shaped(
      [new_size.width, new_size.height],
      [current.width, current.height],
      self.settings.scales.map(limit),
      self.settings.size,
    );
    Some(NativeSize::new(width, height))
  }

  fn host_main_thread_callback(&self, window: &baseview::Window) {
    window.host_main_thread_callback();
  }

  fn set_fallback_scale_factor(
    &self,
    scale_factor: f64,
    window: &baseview::Window,
  ) -> Result<(), baseview::Error> {
    window.suggest_fallback_scale_factor(scale_factor)
  }

  fn on_virtual_key_from_host(
    &self,
    key_code: VirtualKeyCode,
    is_down: bool,
    modifiers: Modifiers,
  ) -> bool {
    if !self.shared.typing() {
      return false;
    }
    let extend = modifiers.contains(Modifiers::SHIFT);
    let key = match key_code {
      VirtualKeyCode::Return | VirtualKeyCode::NumpadEnter => Some(Key::Enter),
      VirtualKeyCode::Escape => Some(Key::Escape),
      VirtualKeyCode::Backspace => Some(Key::Backspace),
      VirtualKeyCode::Delete => Some(Key::Delete),
      VirtualKeyCode::ArrowLeft => Some(Key::Left { extend }),
      VirtualKeyCode::ArrowRight => Some(Key::Right { extend }),
      VirtualKeyCode::Home => Some(Key::Home { extend }),
      VirtualKeyCode::End => Some(Key::End { extend }),
      VirtualKeyCode::Tab => Some(Key::Tab),
      VirtualKeyCode::Space => Some(Key::Letter(' ')),
      _ => None,
    };
    if is_down && let Some(key) = key {
      self.tell.key(key);
    }
    // every key is kept from the host while a value is typed, and so is its release
    true
  }

  fn state_changed(&self) {
    self.tell.state_changed();
  }

  fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
    self.tell.param_changed();
  }

  fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {
    // the panel shows the values without their modulation
  }
}

/// The host callbacks nice-plug hands an editor, as baseview's.
struct HostCallbacks(Box<dyn nice_plug::editor::HostCallbacks>);

impl baseview::host::HostCallbacks for HostCallbacks {
  fn request_resize(&mut self, new_size: baseview::WindowSize) -> Result<(), HandlerError> {
    (self.0)
      .request_resize(new_size.physical.into(), new_size.scale_factor)
      .map_err(HandlerError::from_boxed)
  }

  fn destroyed(&mut self) {
    self.0.destroyed();
  }
}

struct MainThreadCaller(Box<dyn nice_plug::editor::HostMainThreadCaller>);

impl baseview::host::HostMainThreadCaller for MainThreadCaller {
  fn call_main_thread(&mut self) {
    self.0.call_main_thread();
  }
}

#[cfg(test)]
mod tests {
  use nice_plug::prelude::{FloatParam, FloatRange, Param, Params};
  use potpot_controls::flat::Flat;
  use potpot_controls::furniture::{GripLook, MenuLook};
  use potpot_controls::interaction::{Drag, Target};

  use super::{Arc, Bound, Fault, Gpu, Held, Misbound, Readout, Remembered, Settings, Skin, Touch};
  use crate::feed::Feed;
  use crate::skin::View;

  #[derive(Params)]
  struct Two {
    #[id = "first"]
    first: FloatParam,
    #[id = "second"]
    second: FloatParam,
  }

  impl Two {
    fn new() -> Arc<Self> {
      let whole = FloatRange::Linear { min: 0.0, max: 1.0 };
      Arc::new(Self {
        first: FloatParam::new("First", 0.25, whole),
        second: FloatParam::new("Second", 0.5, whole),
      })
    }
  }

  /// A skin that draws nothing.
  struct Blank;

  impl Skin for Blank {
    fn prepare(&mut self, _: &Gpu, _: &View, _: &mut Flat) {}
    fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {}
    fn target_at(&self, _: [f32; 2]) -> Option<Target> {
      None
    }
  }

  fn editor(params: Arc<Two>, bound: Vec<Bound>) -> Result<super::Editor<Blank>, Misbound> {
    let settings = Settings {
      title: String::new(),
      size: [200.0, 100.0],
      scales: [1.0, 2.0],
      menu: MenuLook::default(),
      grip: GripLook::default(),
      feed: Feed::default(),
    };
    let remembered = Remembered {
      get: Box::new(|| 1.0),
      set: Box::new(|_| {}),
    };
    super::Editor::new(params, settings, bound, remembered, |_| Blank)
  }

  #[test]
  fn only_the_plugins_own_parameters_can_be_bound() {
    let (ours, theirs) = (Two::new(), Two::new());
    let own = vec![
      Bound::new(&ours.first, Readout::new(4)),
      Bound::new(&ours.second, Readout::new(4)),
    ];
    assert!(editor(Arc::clone(&ours), own).is_ok());
    // one of another plugin's, which nothing here keeps alive, is turned away
    let other = vec![
      Bound::new(&ours.first, Readout::new(4)),
      Bound::new(&theirs.second, Readout::new(4)),
    ];
    let refused =
      editor(Arc::clone(&ours), other).expect_err("a parameter of another plugin is refused");
    assert_eq!((refused.control, refused.fault), (1, Fault::Outside));
  }

  #[test]
  fn a_bound_pad_is_a_pad_at_its_own_place_and_names_only_other_bounds() {
    let params = Two::new();
    let readout = Readout::new(4);
    // the second is turned up the panel and the first across, over a room 128 by 64
    let pad = |across| Bound::pad(across, &params.second, [128.0, 64.0], readout);
    let control = pad(0).control(1);
    assert_eq!(control.parameter, 1);
    assert_eq!(control.feel.drag, Drag::Up { turn: 1.0 / 64.0 });
    let across = Touch::Pad {
      across: 0,
      turn: 1.0 / 128.0,
    };
    assert_eq!(control.touch, across);
    let first = Bound::new(&params.first, readout);
    assert!(editor(Arc::clone(&params), vec![first, pad(0)]).is_ok());
    // across for a third parameter, of which there is none, or for its own
    for across in [2, 1] {
      let refused = editor(Arc::clone(&params), vec![first, pad(across)])
        .expect_err("a pad across for no other parameter is refused");
      assert_eq!((refused.control, refused.fault), (1, Fault::NotAnother));
    }
    let mut wheeled = first;
    wheeled.wheel = Some(0);
    let refused = editor(Arc::clone(&params), vec![wheeled, pad(0)])
      .expect_err("a wheel on the control's own parameter is refused");
    assert_eq!((refused.control, refused.fault), (0, Fault::NotAnother));
    // the first control at fault is the one told, whatever its fault
    let theirs = Two::new();
    let outside = Bound::new(&theirs.second, readout);
    let refused = editor(Arc::clone(&params), vec![wheeled, outside])
      .expect_err("both controls are bound wrongly");
    assert_eq!((refused.control, refused.fault), (0, Fault::NotAnother));
  }

  #[test]
  fn a_bound_parameter_is_read_and_written_as_the_plugin_has_it() {
    let params = Two::new();
    let held = Held {
      _params: Arc::clone(&params) as Arc<dyn Params>,
      param: params.first.as_ptr(),
    };
    let dial = held.dial(None);
    assert_eq!(held.value(), 0.25);
    assert_eq!(dial.default, 0.25);
    assert_eq!((dial.plain)(0.5), 0.5);
    assert_eq!((dial.turn)(2.0), 1.0);
    assert_eq!((dial.read)("0.75"), Some(0.75));
    // the closures keep the parameters alive on their own
    drop((held, params));
    assert_eq!((dial.plain)(1.0), 1.0);
  }
}
