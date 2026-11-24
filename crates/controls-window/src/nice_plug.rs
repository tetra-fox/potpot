//! the window as a nice-plug editor: the wrapper's calls turned into the window's, sizes
//! passed to and from the host in the units the host has them in, and the panel's parameters
//! bound to the plugin's

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use baseview::HandlerError;
use controls::fit::{chosen, shaped};
use controls::interaction::{Control, Dial, Feel, Key, Readout};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::NativeSize;
use nice_plug::editor::{EditorHandle, HostMethods, ResizeHint, SpawnedEditor};
use nice_plug::params::Params;
use nice_plug::params::internals::ParamPtr;
use nice_plug::prelude::{Modifiers, ParentWindowHandle, VirtualKeyCode};

use crate::open::{Opening, Param, Remembered, Settings, Shared, Tell, open};
use crate::skin::{Gpu, Skin};

/// one of a panel's controls bound to one of the plugin's parameters
#[derive(Clone, Copy, Debug)]
pub struct Bound {
  param: ParamPtr,
  /// the coarse step the control moves by, in the parameter's plain units. none turns
  /// smoothly
  pub step: Option<f32>,
  /// how the control answers the pointer and the wheel
  pub feel: Feel,
  /// what the control's readout can show of a value typed into it
  pub readout: Readout,
}

impl Bound {
  /// a control on `param`, which has to be one of the parameters the editor is made with
  pub fn new(param: &impl nice_plug::params::Param, step: Option<f32>, readout: Readout) -> Self {
    Self {
      param: param.as_ptr(),
      step,
      feel: Feel::default(),
      readout,
    }
  }
}

/// a parameter together with the object it is part of, which is what makes it safe to reach.
///
/// the pointer is into `_params`: [`Editor::new`] lets nothing else be bound. `_params` is
/// held here, so the parameter is alive for as long as this is, wherever this has been handed on
/// to: the window's thread, a closure still waiting to be called, a clone of one
#[derive(Clone)]
struct Held {
  /// never read: it is what keeps `param` alive
  _params: Arc<dyn Params>,
  param: ParamPtr,
}

impl Held {
  /// where the parameter stands, without what the host adds to it
  fn value(&self) -> f32 {
    // SAFETY: `param` points into `_params`, which `self` keeps alive
    unsafe { self.param.unmodulated_normalized_value() }
  }

  /// what there is to know of it for a control that moves by `step`
  fn dial(&self, step: Option<f32>) -> Dial {
    let (plain, turn, write, read) = (self.clone(), self.clone(), self.clone(), self.clone());
    Dial {
      // SAFETY: `param` points into `_params`, which `self` keeps alive, as does each of
      // the clones the closures below own
      default: unsafe { self.param.default_normalized_value() },
      step,
      // SAFETY: the same
      plain: Box::new(move |value| unsafe { plain.param.preview_plain(value) }),
      // SAFETY: the same
      turn: Box::new(move |plain| unsafe { turn.param.preview_normalized(plain) }),
      // SAFETY: the same
      write: Box::new(move |value| unsafe { write.param.normalized_value_to_string(value, false) }),
      // SAFETY: the same
      unit: unsafe { self.param.unit() }.to_owned(),
      // SAFETY: the same
      read: Box::new(move |text| unsafe { read.param.string_to_normalized_value(text) }),
    }
  }

  /// the parameter as the window has it, with its ways to the host through `gui`
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

/// the panel's parameters are not all the plugin's own
#[derive(Clone, Copy, Debug)]
pub struct NotOurs {
  /// the place among the panel's of the first that is not
  pub control: usize,
}

impl fmt::Display for NotOurs {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Self { control } = self;
    write!(
      f,
      "control {control} of the panel is bound to a parameter that is not the plugin's"
    )
  }
}

impl Error for NotOurs {}

/// a panel as a nice-plug editor, drawn by the skin `S`
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
  /// an editor for a panel whose controls are bound, in the panel's order, to parameters of
  /// `params`. `skin` makes the panel's look each time the editor is opened. fails if a
  /// control is bound to a parameter that is not one of `params`' own
  pub fn new(
    params: Arc<dyn Params>,
    settings: Settings,
    bound: Vec<Bound>,
    remembered: Remembered,
    skin: impl Fn(&Gpu) -> S + Send + Sync + 'static,
  ) -> Result<Self, NotOurs> {
    let ours: Vec<ParamPtr> = (params.param_map().into_iter())
      .map(|(_, param, _)| param)
      .collect();
    match bound.iter().position(|bound| !ours.contains(&bound.param)) {
      Some(control) => Err(NotOurs { control }),
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

/// how many of the host's units go to a point. sizes pass to and from the host in pixels,
/// apart from on macos, where they are in points
fn per_point(display_scale: f64) -> f64 {
  if cfg!(target_os = "macos") {
    1.0
  } else {
    display_scale
  }
}

/// the size the host is told the editor has, in the host's units
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
      .map(|(parameter, bound)| Control {
        parameter,
        feel: bound.feel,
        readout: bound.readout,
      })
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

  /// the window may be sized at the panel's own shape, from the smallest to the largest it
  /// is drawn at
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

/// the host's side of an open window
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
    // what the editor opens at from then on. the window's settling as it opens is not
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

/// baseview's host callbacks from the ones nice-plug hands an editor
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
  use controls::flat::Flat;
  use controls::furniture::{GripLook, MenuLook};
  use controls::interaction::Target;
  use nice_plug::prelude::{FloatParam, FloatRange, Param, Params};

  use super::{Arc, Bound, Gpu, Held, NotOurs, Readout, Remembered, Settings, Skin};
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

  const READOUT: Readout = Readout {
    room: 4,
    shows: |_| true,
  };

  /// a skin that draws nothing
  struct Blank;

  impl Skin for Blank {
    fn prepare(&mut self, _: &Gpu, _: &View, _: &mut Flat) {}
    fn draw(&self, _: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: [u32; 4]) {}
    fn target_at(&self, _: [f32; 2]) -> Option<Target> {
      None
    }
  }

  fn editor(params: Arc<Two>, bound: Vec<Bound>) -> Result<super::Editor<Blank>, NotOurs> {
    let settings = Settings {
      title: String::new(),
      size: [200.0, 100.0],
      scales: [1.0, 2.0],
      menu: MenuLook::default(),
      grip: GripLook::default(),
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
      Bound::new(&ours.first, None, READOUT),
      Bound::new(&ours.second, Some(0.1), READOUT),
    ];
    assert!(editor(Arc::clone(&ours), own).is_ok());
    // one of another plugin's, which nothing here keeps alive, is turned away
    let other = vec![
      Bound::new(&ours.first, None, READOUT),
      Bound::new(&theirs.second, None, READOUT),
    ];
    let refused = editor(Arc::clone(&ours), other).expect_err("a parameter of another's");
    assert_eq!(refused.control, 1);
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
