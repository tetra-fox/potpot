//! Pointer and key handling for a panel of controls.
//!
//! The handling stands apart from any window, any way of drawing and any plugin: events go in
//! with the time they came at, and what to tell the host and the window comes out. A
//! [`Control`] is known here by its place among the panel's controls, and the panel's owner
//! describes each parameter with a [`Dial`].

use std::ops::Range;
use std::time::{Duration, Instant};

use crate::{Point, axis};

/// The longest time between the two presses of a double click.
const DOUBLE_TIME: Duration = Duration::from_millis(400);
/// The furthest apart in points the presses of a double click are.
const DOUBLE_REACH: f32 = 4.0;
/// The rest after which a turn of the wheel is a new gesture.
const WHEEL_REST: Duration = Duration::from_millis(400);
/// The distance within which a count of steps or lines is whole.
///
/// Sums of their parts come out a hair off.
const SLACK: f32 = 1e-4;

/// The way the pointer is dragged to turn a control.
///
/// Each way also says how far the drag turns the control.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Drag {
  /// A drag up and down the panel.
  ///
  /// Up turns the control on and down turns it back, by `turn` of its whole turn for each
  /// point.
  Up {
    /// The part of the whole turn that one point is.
    turn: f32,
  },
  /// A drag along a line across the panel, the way a fader slides.
  ///
  /// Toward `way` turns the control on and away from it turns it back, by `turn` of its whole
  /// turn for each point.
  Along {
    /// The way that turns it on, one point long, across and down.
    way: Point,
    /// The part of the whole turn that one point is.
    turn: f32,
  },
}

impl Drag {
  /// Returns how far the pointer has gone the way that turns it on.
  ///
  /// The distance is in points.
  fn travel(self, from: Point, to: Point) -> f32 {
    match self {
      Self::Up { .. } => from[1] - to[1],
      Self::Along { way, .. } => (to[0] - from[0]) * way[0] + (to[1] - from[1]) * way[1],
    }
  }

  /// Returns how far one point of that turns the control.
  fn turn(self) -> f32 {
    match self {
      Self::Up { turn } | Self::Along { turn, .. } => turn,
    }
  }
}

/// The way a button that is pressed behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Push {
  /// A button that each push turns over.
  ///
  /// One push takes it in and on, the next out and off. It stays most of the way in while it
  /// is on.
  Latching,
  /// A button that is on while held.
  Momentary,
}

impl Push {
  /// Returns how far in such a button is.
  ///
  /// The depth runs from zero for out to one for all the way. The button is all the way in
  /// while it is `held`, and most of the way while a latching one is `on`.
  pub fn depth(self, on: bool, held: bool) -> f32 {
    match (held, self, on) {
      (true, ..) => 1.0,
      (false, Push::Latching, true) => 0.7,
      (false, Push::Latching, false) | (false, Push::Momentary, _) => 0.0,
    }
  }

  /// Returns whether such a button is on after a push is let go.
  ///
  /// `was_on` is whether it was on before. A momentary one is off once it is let go.
  pub fn released(self, was_on: bool) -> bool {
    self == Push::Latching && !was_on
  }
}

/// The effect of the left button on a control.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Touch {
  /// A control that is dragged.
  ///
  /// It is dragged the way its [`Feel`] says, and a double click puts it at its default.
  Turn,
  /// A control that is pressed like a button.
  ///
  /// Its parameter is at the end of its turn while it is on and at the start while it is off.
  Press(Push),
  /// A control that each click moves one step on.
  ///
  /// From the end of its turn a click moves it back to the start.
  Step,
  /// A control dragged two ways at once, like a pad's puck.
  ///
  /// A drag turns its own parameter the way its [`Feel`] says, and a drag to the right turns
  /// `across` on as well. The puck keeps to the pointer. A stepped parameter goes at the pace
  /// of its drag and lands on its steps, and a pointer gone past an end of a turn comes all the
  /// way back before the value moves. The fine key slows both, each from where it shows, and
  /// the puck is off the pointer from then on. A double click resets it.
  Pad {
    /// The parameter a drag across the panel turns.
    across: usize,
    /// The part of that parameter's whole turn that one point is.
    turn: f32,
  },
}

/// The way a control answers the pointer and the wheel.
///
/// A feel starts as [`Feel::default`] and is changed a field at a time.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Feel {
  /// The way it is dragged.
  pub drag: Drag,
  /// The least points a stepped control is dragged for each step.
  ///
  /// One with few steps is dragged further for each, at the pace of a control without any. A
  /// [`Touch::Pad`] goes at the pace of its drag whatever its steps.
  pub points_per_step: f32,
  /// The part of its pace it keeps while the fine key is held.
  pub fine: f32,
  /// The turn one line of the wheel gives a control that has no step.
  ///
  /// It is a part of the control's whole turn.
  pub wheel: f32,
}

impl Default for Feel {
  /// Returns a feel that is dragged up the panel.
  ///
  /// - Two hundred points up make the whole turn.
  /// - A step takes five points at least.
  /// - The fine key gives a tenth of that pace.
  /// - Twenty lines of the wheel make the whole turn.
  fn default() -> Self {
    Self {
      drag: Drag::Up { turn: 1.0 / 200.0 },
      points_per_step: 5.0,
      fine: 0.1,
      wheel: 0.05,
    }
  }
}

/// The description of one parameter, for the controls that turn it.
///
/// Its value is a part of its whole turn, from zero to one, and its plain value is that in the
/// units it is read and written in.
///
/// A dial is [`Send`]: on some systems a window runs on another thread than the one that
/// opened it, and the dial is handed to that thread.
#[non_exhaustive]
pub struct Dial {
  /// The value a reset gives it.
  pub default: f32,
  /// The coarse step it moves by, in plain units.
  ///
  /// Dragged or wheeled without the fine key it lands on whole steps only. `None` turns
  /// smoothly.
  pub step: Option<f32>,
  /// The conversion of a value to plain units.
  pub plain: Box<dyn Fn(f32) -> f32 + Send>,
  /// The conversion of a plain value to a part of the turn.
  ///
  /// A plain value past either end comes out as that end.
  pub turn: Box<dyn Fn(f32) -> f32 + Send>,
  /// The writing of a value, without its unit.
  pub write: Box<dyn Fn(f32) -> String + Send>,
  /// The unit it is in, as it is written straight after a value.
  ///
  /// It starts with a space if one belongs there.
  pub unit: String,
  /// The reading of the value that text stands for, if it is one.
  pub read: Read,
}

type Read = Box<dyn Fn(&str) -> Option<f32> + Send>;

impl std::fmt::Debug for Dial {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Dial")
      .field("default", &self.default)
      .field("step", &self.step)
      .field("unit", &self.unit)
      .finish_non_exhaustive()
  }
}

impl Dial {
  /// Creates a parameter that turns smoothly and has no unit.
  ///
  /// `plain` and `turn` are its ways between a value and its plain value, and `write` and
  /// `read` those between a value and its text. `step` and `unit` are set after.
  pub fn new(
    default: f32,
    plain: impl Fn(f32) -> f32 + Send + 'static,
    turn: impl Fn(f32) -> f32 + Send + 'static,
    write: impl Fn(f32) -> String + Send + 'static,
    read: impl Fn(&str) -> Option<f32> + Send + 'static,
  ) -> Self {
    Self {
      default,
      step: None,
      plain: Box::new(plain),
      turn: Box::new(turn),
      write: Box::new(write),
      unit: String::new(),
      read: Box::new(read),
    }
  }

  /// Creates a parameter over a straight run of plain values.
  ///
  /// The run is from the first of `range` to the second, and `default` is the plain value a
  /// reset gives. A value is written with `decimals` decimals and `unit` after it, and read
  /// with or without the unit.
  pub fn linear(
    range: [f32; 2],
    default: f32,
    step: Option<f32>,
    decimals: usize,
    unit: &'static str,
  ) -> Self {
    Self::along(axis::Axis::linear(range), default, step, decimals, unit)
  }

  /// Creates a parameter whose plain values lie along an axis.
  ///
  /// A value is the place of its plain value on the [`Axis`](axis::Axis), so something drawn along
  /// the same axis lines up with the control. `default` is the plain value a reset gives. A value
  /// is written with `decimals` decimals and `unit` after it, and read with or without the unit.
  pub fn along(
    axis: axis::Axis,
    default: f32,
    step: Option<f32>,
    decimals: usize,
    unit: &'static str,
  ) -> Self {
    let plain = move |value: f32| axis.plain(value);
    let turn = move |plain: f32| axis.turn(plain);
    Self {
      default: turn(default),
      step,
      plain: Box::new(plain),
      turn: Box::new(turn),
      write: Box::new(move |value| format!("{:.decimals$}", plain(value))),
      unit: unit.to_owned(),
      read: Box::new(move |text| {
        let figures = text.trim().trim_end_matches(unit.trim()).trim();
        figures.parse().ok().map(turn)
      }),
    }
  }

  /// Returns the nearest value that is a whole number of steps.
  ///
  /// A control without steps keeps the value itself.
  fn snapped(&self, value: f32) -> f32 {
    match self.step {
      Some(step) => (self.turn)(((self.plain)(value) / step).round() * step),
      None => value,
    }
  }

  /// Returns how far one step is as a part of the turn.
  fn step_turn(&self, step: f32) -> f32 {
    ((self.turn)((self.plain)(0.0) + step) - (self.turn)((self.plain)(0.0))).abs()
  }

  /// Returns how far a point of dragging turns it.
  ///
  /// `drag` is the way it is dragged, and a step of it takes `points_per_step` points at least.
  fn turn_per_point(&self, drag: Drag, points_per_step: f32) -> f32 {
    match self.step {
      Some(step) => (drag.turn()).min(self.step_turn(step) / points_per_step),
      None => drag.turn(),
    }
  }

  /// Returns the value some lines of the wheel on from another.
  ///
  /// A dial without a step turns `feel.wheel` of its whole turn for each line, and `feel.fine`
  /// of that with the fine key. A stepped one goes `feel.fine` of a step for each line with the
  /// fine key, and without it to the next whole steps that way, one for each of its lines,
  /// which are whole.
  fn wheeled(&self, value: f32, lines: f32, fine: bool, feel: Feel) -> f32 {
    let turned = match self.step {
      None => value + lines * feel.wheel * if fine { feel.fine } else { 1.0 },
      Some(step) if fine => (self.turn)((self.plain)(value) + lines * step * feel.fine),
      Some(step) => {
        let steps = (self.plain)(value) / step;
        // from between two steps the first line goes to the one that way, and no further
        let from = if lines > 0.0 {
          (steps + SLACK).floor()
        } else {
          (steps - SLACK).ceil()
        };
        (self.turn)((from + lines) * step)
      }
    };
    turned.clamp(0.0, 1.0)
  }

  /// Returns the value one step on from another.
  ///
  /// From the end of the turn that is its start.
  fn stepped_on(&self, value: f32, feel: Feel) -> f32 {
    if value > 1.0 - 1e-4 {
      0.0
    } else {
      self.wheeled(value, 1.0, false, feel)
    }
  }
}

/// One of a panel's controls.
///
/// A control is something on the panel that is turned, with a readout of its own. It is known
/// by its place among the panel's controls, and names each parameter it turns by that one's
/// place among the panel's parameters. The two orders are separate: a parameter may have no
/// control, or more than one.
///
/// A control turns up to three parameters: its own, one more that a [`Touch::Pad`] is dragged
/// across for, and one that `wheel` gives the wheel. The three are different parameters. A
/// reset puts each one the control can turn at its default, which leaves out the wheel's on a
/// [`Touch::Press`]. The readout, a value typed into it, and the menu's copy and paste are of
/// its own parameter alone. Another of its parameters has those through a second control on
/// it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Control {
  /// The parameter it turns.
  pub parameter: usize,
  /// The way it answers the pointer and the wheel.
  pub feel: Feel,
  /// What its readout can show of a value typed into it.
  pub readout: Readout,
  /// The effect of the left button on it.
  pub touch: Touch,
  /// The parameter the wheel turns, if another than its own.
  ///
  /// It is neither `parameter` nor the one a [`Touch::Pad`] is dragged across for. The wheel
  /// turns it at the pace of `feel` and by the steps of its own [`Dial`]. With `None` the wheel
  /// turns `parameter`.
  pub wheel: Option<usize>,
}

impl Control {
  /// Creates a dragged control on a parameter, with a readout.
  ///
  /// It feels as [`Feel::default`] does, and the wheel turns the same parameter.
  pub fn new(parameter: usize, readout: Readout) -> Self {
    Self {
      parameter,
      feel: Feel::default(),
      readout,
      touch: Touch::Turn,
      wheel: None,
    }
  }

  /// Creates a control dragged both ways, on two parameters.
  ///
  /// A drag to the right turns `across` and a drag up the panel turns `up`, which is the
  /// control's own parameter. `size` is the width and height in points of the room the middle
  /// of its puck has, and each parameter goes through its whole turn over its side of that. So
  /// the puck keeps to the pointer from wherever it was taken hold of, as [`Touch::Pad`] says.
  ///
  /// # Panics
  ///
  /// Panics if a side of `size` is not a finite number of points above zero.
  pub fn pad(across: usize, up: usize, size: Point, readout: Readout) -> Self {
    assert!(
      size.iter().all(|side| side.is_finite() && *side > 0.0),
      "the room of a pad has a width and a height"
    );
    let mut control = Self::new(up, readout);
    control.feel.drag = Drag::Up {
      turn: 1.0 / size[1],
    };
    control.touch = Touch::Pad {
      across,
      turn: 1.0 / size[0],
    };
    control
  }

  /// Returns the parameter dragged across for, and its drag.
  fn across(&self) -> Option<(usize, Drag)> {
    match self.touch {
      Touch::Pad { across, turn } => {
        let way = [1.0, 0.0];
        Some((across, Drag::Along { way, turn }))
      }
      Touch::Turn | Touch::Press(_) | Touch::Step => None,
    }
  }

  /// Returns whether the wheel turns the control.
  ///
  /// A button is pressed, and the wheel does nothing to it.
  fn takes_wheel(&self) -> bool {
    match self.touch {
      Touch::Press(_) => false,
      Touch::Turn | Touch::Step | Touch::Pad { .. } => true,
    }
  }

  /// Returns the parameter the wheel turns.
  fn wheel_parameter(&self) -> usize {
    self.wheel.unwrap_or(self.parameter)
  }

  /// Returns the parameters it names, its own first.
  ///
  /// After it are the one across and the wheel's.
  fn named(&self) -> [Option<usize>; 3] {
    let across = self.across().map(|(across, _)| across);
    [Some(self.parameter), across, self.wheel]
  }

  /// Returns every parameter it can turn, its own first.
  fn parameters(&self) -> impl Iterator<Item = usize> {
    let [own, across, wheel] = self.named();
    let wheel = wheel.filter(|_| self.takes_wheel());
    [own, across, wheel].into_iter().flatten()
  }

  /// Returns whether it fits a panel of so many parameters.
  ///
  /// It fits if every parameter it names is one of the `parameters` and none is named twice.
  pub fn fits(&self, parameters: usize) -> bool {
    let [own, across, wheel] = self.named();
    let twice = across == own || wheel == own || (wheel.is_some() && wheel == across);
    !twice && (self.named().into_iter().flatten()).all(|named| named < parameters)
  }
}

/// What a control's readout can show of a value typed into it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Readout {
  /// The number of letters it has room for.
  pub room: usize,
  /// The test of whether it has a letter for something typed.
  pub shows: fn(char) -> bool,
}

impl Readout {
  /// Creates a readout with room for `room` letters.
  ///
  /// It shows a space and whatever [`stroke`](crate::stroke) has a letter for.
  pub fn new(room: usize) -> Self {
    Self {
      room,
      shows: |letter| letter == ' ' || crate::stroke::lettered(letter),
    }
  }
}

/// What a control shows, as the panel's look is handed it.
///
/// All of it is of the control's own parameter.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Reading {
  /// The place its parameter stands at, as a part of its whole turn.
  pub value: f32,
  /// The text of its readout.
  ///
  /// It is the value as it is written, or what has been typed so far while one is being typed.
  pub text: String,
  /// The unit that goes after the text.
  ///
  /// It is empty while a value is typed.
  pub unit: String,
}

/// The part of a panel that a point belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
  /// The turned part of the control at this place among the panel's.
  Control(usize),
  /// The window the value of the control at this place is written in.
  Readout(usize),
  /// The menu, between and around its rows.
  Menu,
  /// A row of the menu.
  Item(Item),
  /// The grip the window is pulled larger and smaller by.
  Grip,
}

/// A row of the menu.
///
/// The rows are listed top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
  /// The row for typing the control's value.
  Edit,
  /// The row that puts the control at its default.
  Reset,
  /// The row that puts the control's value on the clipboard.
  Copy,
  /// The row that sets the control to the value on the clipboard.
  Paste,
}

impl Item {
  /// Every row, top to bottom.
  pub const ALL: [Self; 4] = [Self::Edit, Self::Reset, Self::Copy, Self::Paste];
}

/// A button of the pointer that means something to a panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
  /// The button that turns, presses and picks.
  Left,
  /// The button that asks for a control's menu.
  Right,
}

/// A key as a value being typed takes it.
///
/// The caret's moves take the selection along from where it began when `extend` is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
  /// A letter to type.
  Letter(char),
  /// The key that sets what was typed.
  Enter,
  /// The key that gives up what was typed.
  Escape,
  /// The key that takes out the letter before the caret or the selection.
  Backspace,
  /// The key that takes out the letter after the caret or the selection.
  Delete,
  /// The key that moves the caret back.
  ///
  /// The caret moves one letter. With letters selected and `extend` unset, it goes to the start
  /// of the selection.
  Left {
    /// Whether the selection goes along.
    extend: bool,
  },
  /// The key that moves the caret on.
  ///
  /// The caret moves one letter. With letters selected and `extend` unset, it goes to the end
  /// of the selection.
  Right {
    /// Whether the selection goes along.
    extend: bool,
  },
  /// The key that moves the caret to the start.
  Home {
    /// Whether the selection goes along.
    extend: bool,
  },
  /// The key that moves the caret to the end.
  End {
    /// Whether the selection goes along.
    extend: bool,
  },
  /// The key that leaves the field.
  ///
  /// Leaving sets what was typed.
  Tab,
  /// The key that selects all that was typed.
  SelectAll,
  /// The key that puts the selection on the clipboard.
  ///
  /// With nothing selected it puts all that was typed there.
  Copy,
  /// The key that does the same as [`Key::Copy`] and takes those letters out.
  Cut,
  /// The key that types the clipboard's text.
  Paste,
}

/// The state of the panel at the time an event comes in.
#[derive(Clone, Copy, Debug)]
pub struct Moment<'a> {
  /// The time the event came.
  pub now: Instant,
  /// The value each parameter stands at.
  pub values: &'a [f32],
  /// The number of times its own size the window is.
  pub scale: f32,
}

/// An event for a panel to handle.
///
/// Places are in the panel's points apart from `window`, and `over` is what is drawn there.
#[derive(Clone, Debug)]
pub enum Event {
  /// The pointer moved.
  Move {
    /// The place it moved to.
    at: Point,
    /// The same place in the window's own points.
    ///
    /// It is measured from the window's top left corner, which stays put when the window
    /// changes size.
    window: Point,
    /// The target that is drawn there.
    over: Option<Target>,
    /// Whether the key that slows a control down is held.
    fine: bool,
  },
  /// A button went down.
  Press {
    /// The button that went down.
    button: Button,
    /// The place of the pointer.
    at: Point,
    /// The same place in the window's own points, as in [`Event::Move`].
    window: Point,
    /// The target that is drawn there.
    over: Option<Target>,
    /// Whether the key that makes a click a reset is held.
    reset: bool,
  },
  /// A button came up.
  Release(Button),
  /// The wheel turned.
  ///
  /// It turned by so many lines, away from the user counting up.
  Wheel {
    /// The target that is drawn under the pointer.
    over: Option<Target>,
    /// The number of lines it turned.
    ///
    /// A wheel that turns smoothly gives a part of a line.
    lines: f32,
    /// Whether the key that slows a control down is held.
    fine: bool,
  },
  /// A key went down while the panel takes keys.
  Key(Key),
  /// The clipboard's text came.
  ///
  /// It answers [`Action::WantClipboard`], and is `None` if the clipboard has no text.
  Clipboard(Option<String>),
  /// The keys went to another window.
  Unfocused,
  /// Time passed.
  Tick,
  /// The window is going away.
  Closed,
}

/// Something to be done about an event.
///
/// Whoever has the host and the window does it.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
  /// A gesture on a parameter begins.
  ///
  /// The host is told a gesture on a parameter begins, then its values, then that it ends.
  /// The parameter is known by its place among the panel's parameters, which is separate
  /// from its control's among the controls.
  Begin(usize),
  /// The parameter is set to a value.
  Set(usize, f32),
  /// The gesture on the parameter is over.
  End(usize),
  /// The window takes the keys.
  Focus,
  /// The panel looks different though no value changed.
  Redraw,
  /// The text goes on the clipboard.
  Copy(String),
  /// The clipboard's text is to come back as [`Event::Clipboard`].
  WantClipboard,
  /// The window is to be this many times its own size.
  Scale(f32),
}

/// A value being typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
  /// The control it is for.
  pub control: usize,
  /// The text that has been typed.
  pub text: String,
  /// The number of letters before the caret.
  pub caret: usize,
  /// The place the selection began.
  ///
  /// It reaches from there to the caret, and with both at one place nothing is selected.
  pub anchor: usize,
}

impl Edit {
  /// Returns the places of the letters that are selected.
  pub fn selected(&self) -> Range<usize> {
    self.anchor.min(self.caret)..self.anchor.max(self.caret)
  }

  fn letters(&self) -> usize {
    self.text.chars().count()
  }

  /// Returns where in the text the letter after `letter` others starts.
  fn byte(&self, letter: usize) -> usize {
    (self.text.char_indices().nth(letter)).map_or(self.text.len(), |(byte, _)| byte)
  }

  /// Returns the letters that copying takes.
  ///
  /// They are the selection, or with nothing selected all there is.
  fn taken(&self) -> Range<usize> {
    let selected = self.selected();
    if selected.is_empty() {
      0..self.letters()
    } else {
      selected
    }
  }

  fn text_of(&self, letters: Range<usize>) -> String {
    self.text[self.byte(letters.start)..self.byte(letters.end)].to_owned()
  }

  /// Takes letters out and leaves the caret where they were.
  fn remove(&mut self, letters: Range<usize>) {
    let bytes = self.byte(letters.start)..self.byte(letters.end);
    self.text.replace_range(bytes, "");
    self.caret = letters.start;
    self.anchor = letters.start;
  }

  /// Puts text in at the caret in place of what is selected.
  ///
  /// As much of it goes in as the readout has room for, of the letters it can show.
  fn put(&mut self, text: &str, room: usize, shows: fn(char) -> bool) {
    self.remove(self.selected());
    for letter in text.chars().filter(|&letter| shows(letter)) {
      if self.letters() == room {
        break;
      }
      self.text.insert(self.byte(self.caret), letter);
      self.caret += 1;
    }
    self.anchor = self.caret;
  }

  fn move_to(&mut self, caret: usize, extend: bool) {
    self.caret = caret;
    if !extend {
      self.anchor = caret;
    }
  }
}

/// The use the clipboard's text is wanted for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Paste {
  /// Text to go into the value being typed for a control.
  Typed(usize),
  /// Text to be a control's value.
  Value(usize),
}

/// The menu of a control, while it is up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Menu {
  /// The control it is for.
  pub control: usize,
  /// The place on the picture it was asked for at.
  pub at: Point,
  /// The row the pointer is on.
  pub hovered: Option<Item>,
}

/// One of the parameters a drag turns.
#[derive(Clone, Copy, Debug)]
struct Axis {
  parameter: usize,
  /// The way the pointer is dragged to turn it.
  drag: Drag,
  /// The value the pointer has taken it to.
  ///
  /// Between two steps the control shows another value, and so it does while a pad's pointer
  /// is past an end of the turn.
  reached: f32,
  /// The value last shown.
  shown: f32,
  /// Whether the host has heard of a gesture on it.
  begun: bool,
}

#[derive(Debug)]
enum State {
  Idle,
  /// The left button went down on a control that is dragged.
  ///
  /// The host hears of each parameter once its value changes.
  Drag {
    control: usize,
    /// The control's own parameter.
    own: Axis,
    /// The parameter a pad is dragged across for.
    across: Option<Axis>,
    /// The place the pointer last was.
    from: Point,
    /// Whether the fine key was held at the last move.
    fine: bool,
  },
  /// The left button went down on the grip.
  ///
  /// `grab` is where the pointer was then in the window's own points, `from` the scale the
  /// window had, and `scale` the one last asked for.
  Resize {
    grab: Point,
    from: f32,
    scale: f32,
  },
  /// The left button is down on a control that is pressed.
  ///
  /// `on` is whether it was on before.
  Held {
    control: usize,
    push: Push,
    on: bool,
  },
  Edit(Edit),
  Menu(Menu),
}

/// The wheel turning a control.
#[derive(Debug)]
struct Wheel {
  control: usize,
  /// The value it last set.
  ///
  /// It is `None` until the wheel sets one, which is when the host hears of it.
  set: Option<f32>,
  /// The part of a line it has turned a stepped control, short of one.
  gathered: f32,
  /// The time it last moved.
  last: Instant,
}

/// The state of a panel's controls.
///
/// It holds what is being dragged, typed or picked from the menu.
///
/// # Examples
///
/// ```
/// use std::time::Instant;
///
/// use potpot_controls::interaction::{
///   Action, Control, Dial, Event, Interaction, Moment, Readout, Target,
/// };
///
/// // a mix in hundredths, and one control that turns it
/// let dial = Dial::linear([0.0, 100.0], 100.0, None, 0, "%");
/// let knob = Control::new(0, Readout::new(4));
/// let mut panel = Interaction::new(vec![dial], vec![knob], [200.0, 100.0], [1.0, 2.0]);
///
/// let readings = panel.readings(&[0.25]);
/// assert_eq!((readings[0].text.as_str(), readings[0].unit.as_str()), ("25", "%"));
///
/// // one line of the wheel turns it a twentieth of its whole turn
/// let moment = Moment { now: Instant::now(), values: &[0.25], scale: 1.0 };
/// let wheel = Event::Wheel { over: Some(Target::Control(0)), lines: 1.0, fine: false };
/// assert_eq!(panel.handle(wheel, moment), [Action::Begin(0), Action::Set(0, 0.3)]);
/// ```
#[derive(Debug)]
pub struct Interaction {
  /// The description of each parameter.
  dials: Vec<Dial>,
  controls: Vec<Control>,
  /// The panel's width and height in its points.
  size: Point,
  state: State,
  /// The last press of the left button, for telling a double click.
  pressed: Option<(Instant, Point, Target)>,
  /// The control the wheel is turning, until it rests.
  wheel: Option<Wheel>,
  /// The use the clipboard was asked for, until its text comes.
  awaiting: Option<Paste>,
  /// The least and the most times its own size the window is pulled to.
  scales: [f32; 2],
}

impl Interaction {
  /// Creates the behaviour of a panel of controls.
  ///
  /// The panel is `size` across and down, with `controls` on parameters that `dials`
  /// describe. The grip pulls the window from the first of `scales` times its own size to
  /// the second.
  ///
  /// # Panics
  ///
  /// Panics if a control names a parameter that `dials` has none for, or names one twice, which
  /// [`Control::fits`] tells.
  pub fn new(dials: Vec<Dial>, controls: Vec<Control>, size: Point, scales: [f32; 2]) -> Self {
    assert!(
      controls.iter().all(|control| control.fits(dials.len())),
      "every control names parameters of the panel, each once"
    );
    Self {
      dials,
      controls,
      size,
      state: State::Idle,
      pressed: None,
      wheel: None,
      awaiting: None,
      scales,
    }
  }

  /// Returns what each control shows, in the controls' order.
  ///
  /// `values` is where the parameters are, with one for every parameter.
  ///
  /// # Panics
  ///
  /// Panics if `values` has none for the parameter of a control.
  pub fn readings(&self, values: &[f32]) -> Vec<Reading> {
    (self.controls.iter().enumerate())
      .map(|(place, control)| {
        let (value, dial) = (values[control.parameter], &self.dials[control.parameter]);
        match self.edit().filter(|edit| edit.control == place) {
          Some(edit) => Reading {
            value,
            text: edit.text.clone(),
            unit: String::new(),
          },
          None => Reading {
            value,
            text: (dial.write)(value),
            unit: dial.unit.clone(),
          },
        }
      })
      .collect()
  }

  /// Returns the value being typed, if one is.
  pub fn edit(&self) -> Option<&Edit> {
    match &self.state {
      State::Edit(edit) => Some(edit),
      _ => None,
    }
  }

  /// Returns the menu, if it is up.
  pub fn menu(&self) -> Option<&Menu> {
    match &self.state {
      State::Menu(menu) => Some(menu),
      _ => None,
    }
  }

  /// Returns the control held down by the left button.
  ///
  /// The control is a button being pressed, or one being dragged. `None` is none.
  pub fn held(&self) -> Option<usize> {
    match self.state {
      State::Held { control, .. } | State::Drag { control, .. } => Some(control),
      _ => None,
    }
  }

  /// Returns whether keys mean something to the panel.
  pub fn takes_keys(&self) -> bool {
    matches!(self.state, State::Edit(_) | State::Menu(_))
  }

  /// Takes an event in and returns what is to be done about it.
  ///
  /// The actions are in the order they are to be done in.
  ///
  /// # Panics
  ///
  /// Panics if the event is over a control that the panel has none of.
  ///
  /// Panics if the values of `moment` have none for a parameter that a control turns.
  ///
  /// Panics if the pointer moves while the grip is held and the first of the `scales` given
  /// to [`Interaction::new`] is larger than the second or either is not a number.
  pub fn handle(&mut self, event: Event, moment: Moment) -> Vec<Action> {
    let mut actions = Vec::new();
    match event {
      Event::Move {
        at,
        window,
        over,
        fine,
      } => self.moved(at, window, over, fine, moment.values, &mut actions),
      Event::Press {
        button,
        at,
        window,
        over,
        reset,
      } => {
        self.rest_wheel(&mut actions);
        match button {
          Button::Left => self.pressed_left(at, window, over, reset, moment, &mut actions),
          Button::Right => self.pressed_right(at, over, &mut actions),
        }
      }
      Event::Release(Button::Left) => self.let_go(&mut actions),
      Event::Release(Button::Right) => {}
      Event::Wheel { over, lines, fine } => {
        self.wheeled(over, lines, fine, moment, &mut actions);
      }
      Event::Key(key) => self.keyed(key, &mut actions),
      Event::Clipboard(text) => self.pasted(text, &mut actions),
      Event::Unfocused => {
        self.commit(&mut actions);
        self.dismiss(&mut actions);
      }
      Event::Tick => {
        if let Some(Wheel { last, .. }) = self.wheel
          && moment.now.duration_since(last) >= WHEEL_REST
        {
          self.rest_wheel(&mut actions);
        }
      }
      Event::Closed => {
        self.rest_wheel(&mut actions);
        self.cut_short(&mut actions);
        self.state = State::Idle;
      }
    }
    actions
  }

  /// Ends what the left button has open, with no release of it.
  ///
  /// A drag or a pull at the grip ends and a momentary button goes off. A latching one was
  /// not let go, so it stays as it was.
  fn cut_short(&mut self, actions: &mut Vec<Action>) {
    if let State::Held {
      push: Push::Latching,
      ..
    } = self.state
    {
      // it comes back out all the same
      actions.push(Action::Redraw);
      self.state = State::Idle;
    }
    self.let_go(actions);
  }

  /// Ends a drag, a press or a pull at the grip.
  ///
  /// The left button is up, or as good as.
  fn let_go(&mut self, actions: &mut Vec<Action>) {
    match self.state {
      State::Drag { own, across, .. } => {
        let begun = std::iter::once(own).chain(across).filter(|axis| axis.begun);
        actions.extend(begun.map(|axis| Action::End(axis.parameter)));
        self.state = State::Idle;
      }
      State::Held { control, push, on } => {
        let parameter = self.controls[control].parameter;
        match push {
          Push::Momentary => actions.extend([Action::Set(parameter, 0.0), Action::End(parameter)]),
          Push::Latching => {
            let value = if push.released(on) { 1.0 } else { 0.0 };
            actions.extend(gesture(parameter, value));
          }
        }
        // it comes back out, which shows whatever its value
        actions.push(Action::Redraw);
        self.state = State::Idle;
      }
      State::Resize { .. } => self.state = State::Idle,
      State::Idle | State::Edit(_) | State::Menu(_) => {}
    }
  }

  fn moved(
    &mut self,
    at: Point,
    window: Point,
    over: Option<Target>,
    now_fine: bool,
    values: &[f32],
    actions: &mut Vec<Action>,
  ) {
    match &mut self.state {
      State::Drag {
        control,
        own,
        across,
        from,
        fine,
      } => {
        let Control { feel, touch, .. } = self.controls[*control];
        // a pad's puck keeps to the pointer: its steps do not slow it, and what the pointer
        // goes past an end of the turn is kept for it to come back
        let follows = matches!(touch, Touch::Pad { .. });
        for axis in std::iter::once(own).chain(across) {
          let dial = &self.dials[axis.parameter];
          // until the host hears of it, it goes on from wherever its parameter is taken, less
          // what the pointer has gone past an end of the turn
          let stands = values[axis.parameter];
          if !axis.begun && stands != axis.shown {
            axis.reached = stands + (axis.reached.clamp(0.0, 1.0) - axis.shown);
            axis.shown = stands;
          }
          // with the fine key the knob goes on from where it shows, so pressing the key moves
          // nothing. every move adds its own part of a turn, so the key changes only the pace
          if now_fine && !*fine {
            axis.reached = axis.shown;
          }
          let pace = if follows {
            axis.drag.turn()
          } else {
            dial.turn_per_point(axis.drag, feel.points_per_step)
          };
          let slowed = if now_fine { feel.fine } else { 1.0 };
          let travel = axis.drag.travel(*from, at);
          // a move another way than its own begins nothing on it, though its parameter may
          // stand off the step it would show
          if !axis.begun && travel == 0.0 {
            continue;
          }
          axis.reached += travel * pace * slowed;
          let within = axis.reached.clamp(0.0, 1.0);
          if !follows {
            axis.reached = within;
          }
          let now = if now_fine {
            within
          } else {
            dial.snapped(within)
          };
          // at the end of the turn, between two steps, or with the pointer moved only another
          // way than this one's
          if now == axis.shown {
            continue;
          }
          axis.shown = now;
          if !axis.begun {
            axis.begun = true;
            actions.push(Action::Begin(axis.parameter));
          }
          actions.push(Action::Set(axis.parameter, now));
        }
        *from = at;
        *fine = now_fine;
      }
      State::Resize { grab, from, scale } => {
        // the further of across and down since the pointer took hold sets the scale. measured
        // in the window's own points, since the panel's shift as the window changes size
        let reach = |axis: usize| *from + (window[axis] - grab[axis]) / self.size[axis];
        let (across, down) = (reach(0), reach(1));
        let further = if (across - *from).abs() >= (down - *from).abs() {
          across
        } else {
          down
        };
        let wanted = further.clamp(self.scales[0], self.scales[1]);
        if wanted != *scale {
          *scale = wanted;
          actions.push(Action::Scale(wanted));
        }
      }
      State::Menu(menu) => {
        let hovered = match over {
          Some(Target::Item(item)) => Some(item),
          _ => None,
        };
        if menu.hovered != hovered {
          menu.hovered = hovered;
          actions.push(Action::Redraw);
        }
      }
      State::Idle | State::Held { .. } | State::Edit(_) => {}
    }
  }

  fn pressed_left(
    &mut self,
    at: Point,
    window: Point,
    over: Option<Target>,
    reset: bool,
    moment: Moment,
    actions: &mut Vec<Action>,
  ) {
    let Moment { now, values, scale } = moment;
    let double = (self.pressed.take()).is_some_and(|(then, there, target)| {
      Some(target) == over
        && now.duration_since(then) <= DOUBLE_TIME
        && (there[0] - at[0]).hypot(there[1] - at[1]) <= DOUBLE_REACH
    });
    // a third press in time starts over, so that it is not a second double click
    if !double && let Some(target) = over {
      self.pressed = Some((now, at, target));
    }

    match &self.state {
      // a press while the menu is up goes to the menu and nothing under or around it
      State::Menu(menu) => {
        let control = menu.control;
        let parameter = self.controls[control].parameter;
        let dial = &self.dials[parameter];
        match over {
          Some(Target::Item(Item::Edit)) => self.begin_edit(control, values, actions),
          Some(Target::Item(Item::Reset)) => {
            self.state = State::Idle;
            self.reset(control, actions);
            actions.push(Action::Redraw);
          }
          // with its unit, which keeps it from being read as the value of a control that
          // has another
          Some(Target::Item(Item::Copy)) => {
            self.state = State::Idle;
            let value = format!("{}{}", (dial.write)(values[parameter]), dial.unit);
            actions.extend([Action::Copy(value), Action::Redraw]);
          }
          Some(Target::Item(Item::Paste)) => {
            self.state = State::Idle;
            self.awaiting = Some(Paste::Value(control));
            actions.extend([Action::WantClipboard, Action::Redraw]);
          }
          Some(Target::Menu) => {}
          _ => self.dismiss(actions),
        }
        return;
      }
      State::Edit(edit) => {
        if over == Some(Target::Readout(edit.control)) {
          return;
        }
        self.commit(actions);
      }
      // the release of the press before never came, which baseview leaves out when Windows
      // takes the pointer away
      State::Drag { .. } | State::Held { .. } | State::Resize { .. } => self.cut_short(actions),
      State::Idle => {}
    }

    match over {
      Some(Target::Control(control)) => {
        let Control {
          parameter,
          feel,
          touch,
          ..
        } = self.controls[control];
        let dial = &self.dials[parameter];
        match touch {
          // a second click on what is pressed or stepped is one more of the same
          _ if reset => self.reset(control, actions),
          Touch::Turn | Touch::Pad { .. } if double => self.reset(control, actions),
          Touch::Turn | Touch::Pad { .. } => {
            let axis = |(parameter, drag): (usize, Drag)| Axis {
              parameter,
              drag,
              reached: values[parameter],
              shown: values[parameter],
              begun: false,
            };
            self.state = State::Drag {
              control,
              own: axis((parameter, feel.drag)),
              across: self.controls[control].across().map(axis),
              from: at,
              fine: false,
            };
          }
          Touch::Press(push) => {
            self.state = State::Held {
              control,
              push,
              on: values[parameter] > 0.5,
            };
            match push {
              Push::Momentary => {
                actions.extend([Action::Begin(parameter), Action::Set(parameter, 1.0)]);
              }
              // it turns over once it is let go, and until then only goes in
              Push::Latching => actions.push(Action::Redraw),
            }
          }
          Touch::Step => {
            let next = dial.stepped_on(values[parameter], feel);
            actions.extend(gesture(parameter, next));
          }
        }
      }
      Some(Target::Readout(control)) if double => self.begin_edit(control, values, actions),
      Some(Target::Grip) if double => actions.push(Action::Scale(1.0)),
      Some(Target::Grip) => {
        self.state = State::Resize {
          grab: window,
          from: scale,
          scale,
        };
      }
      _ => {}
    }
  }

  fn pressed_right(&mut self, at: Point, over: Option<Target>, actions: &mut Vec<Action>) {
    self.commit(actions);
    match over {
      Some(Target::Control(control) | Target::Readout(control))
        if matches!(self.state, State::Idle | State::Menu(_)) =>
      {
        self.state = State::Menu(Menu {
          control,
          at,
          hovered: None,
        });
        actions.push(Action::Redraw);
      }
      Some(Target::Menu | Target::Item(_)) => {}
      _ => self.dismiss(actions),
    }
  }

  fn wheeled(
    &mut self,
    over: Option<Target>,
    lines: f32,
    fine: bool,
    moment: Moment,
    actions: &mut Vec<Action>,
  ) {
    let (State::Idle, Some(Target::Control(control) | Target::Readout(control))) =
      (&self.state, over)
    else {
      return;
    };
    // a wheel turned sideways comes as no lines this way
    if lines == 0.0 {
      return;
    }
    let turned = self.controls[control];
    if !turned.takes_wheel() {
      return;
    }
    let (feel, parameter) = (turned.feel, turned.wheel_parameter());
    if (self.wheel.as_ref()).is_none_or(|wheel| wheel.control != control) {
      self.rest_wheel(actions);
    }
    let wheel = self.wheel.get_or_insert(Wheel {
      control,
      set: None,
      gathered: 0.0,
      last: moment.now,
    });
    wheel.last = moment.now;
    let dial = &self.dials[parameter];
    // a control that moves in steps goes one for each whole line, and what is left of a
    // line waits for the rest of it
    let lines = if dial.step.is_some() && !fine {
      wheel.gathered += lines;
      let whole = (wheel.gathered + SLACK.copysign(wheel.gathered)).trunc();
      wheel.gathered -= whole;
      if whole == 0.0 {
        return;
      }
      whole
    } else {
      lines
    };
    let from = match wheel.set {
      Some(value) => value,
      None => {
        actions.push(Action::Begin(parameter));
        moment.values[parameter]
      }
    };
    let value = dial.wheeled(from, lines, fine, feel);
    wheel.set = Some(value);
    actions.push(Action::Set(parameter, value));
  }

  /// Rests the wheel from the control it was turning.
  ///
  /// The gesture ends if the host has heard of one.
  fn rest_wheel(&mut self, actions: &mut Vec<Action>) {
    if let Some(Wheel {
      control,
      set: Some(_),
      ..
    }) = self.wheel.take()
    {
      actions.push(Action::End(self.controls[control].wheel_parameter()));
    }
  }

  fn keyed(&mut self, key: Key, actions: &mut Vec<Action>) {
    let edit = match &mut self.state {
      State::Edit(edit) => edit,
      State::Menu(_) => {
        if key == Key::Escape {
          self.dismiss(actions);
        }
        return;
      }
      State::Idle | State::Drag { .. } | State::Held { .. } | State::Resize { .. } => return,
    };
    let (letters, selected) = (edit.letters(), edit.selected());
    let Readout { room, shows } = self.controls[edit.control].readout;
    match key {
      Key::Enter | Key::Tab => {
        self.commit(actions);
        return;
      }
      Key::Escape => self.state = State::Idle,
      Key::SelectAll => {
        edit.anchor = 0;
        edit.caret = letters;
      }
      Key::Letter(letter) => {
        // only what the readout can show, so that what is typed is what is seen
        if !shows(letter) {
          return;
        }
        edit.put(letter.encode_utf8(&mut [0; 4]), room, shows);
      }
      // with nothing selected the letter before the caret goes, or the one after it
      Key::Backspace if selected.is_empty() => {
        edit.remove(edit.caret.saturating_sub(1)..edit.caret);
      }
      Key::Delete if selected.is_empty() => {
        edit.remove(edit.caret..(edit.caret + 1).min(letters));
      }
      Key::Backspace | Key::Delete => edit.remove(selected),
      // moving out of a selection without taking it along goes to the end of it on that side
      Key::Left { extend } if extend || selected.is_empty() => {
        edit.move_to(edit.caret.saturating_sub(1), extend);
      }
      Key::Right { extend } if extend || selected.is_empty() => {
        edit.move_to((edit.caret + 1).min(letters), extend);
      }
      Key::Left { .. } => edit.move_to(selected.start, false),
      Key::Right { .. } => edit.move_to(selected.end, false),
      Key::Home { extend } => edit.move_to(0, extend),
      Key::End { extend } => edit.move_to(letters, extend),
      Key::Copy => {
        actions.push(Action::Copy(edit.text_of(edit.taken())));
        return;
      }
      Key::Cut => {
        let taken = edit.taken();
        actions.push(Action::Copy(edit.text_of(taken.clone())));
        edit.remove(taken);
      }
      Key::Paste => {
        self.awaiting = Some(Paste::Typed(edit.control));
        actions.push(Action::WantClipboard);
        return;
      }
    }
    actions.push(Action::Redraw);
  }

  /// Takes the clipboard's text for the use it was last wanted for.
  ///
  /// Text that came with space or line ends around it is taken without them.
  fn pasted(&mut self, text: Option<String>, actions: &mut Vec<Action>) {
    let (Some(wanted), Some(text)) = (self.awaiting.take(), text) else {
      return;
    };
    let text = text.trim();
    match (wanted, &mut self.state) {
      (Paste::Typed(control), State::Edit(edit)) if edit.control == control => {
        let Readout { room, shows } = self.controls[control].readout;
        edit.put(text, room, shows);
        actions.push(Action::Redraw);
      }
      // typing ended before the text came
      (Paste::Typed(_), _) => {}
      (Paste::Value(control), _) => {
        if let Some(value) = self.read(control, text) {
          actions.extend(gesture(self.controls[control].parameter, value));
        }
      }
    }
  }

  fn begin_edit(&mut self, control: usize, values: &[f32], actions: &mut Vec<Action>) {
    let parameter = self.controls[control].parameter;
    let text = (self.dials[parameter].write)(values[parameter]);
    self.state = State::Edit(Edit {
      control,
      caret: text.chars().count(),
      text,
      anchor: 0,
    });
    actions.extend([Action::Focus, Action::Redraw]);
  }

  /// Ends typing.
  ///
  /// What was typed becomes the control's value if it reads as one, and the value is left
  /// alone if it does not.
  fn commit(&mut self, actions: &mut Vec<Action>) {
    let State::Edit(edit) = &self.state else {
      return;
    };
    let control = edit.control;
    if let Some(value) = self.read(control, &edit.text) {
      actions.extend(gesture(self.controls[control].parameter, value));
    }
    self.state = State::Idle;
    actions.push(Action::Redraw);
  }

  /// Returns the value of a control that text stands for, if any.
  fn read(&self, control: usize, text: &str) -> Option<f32> {
    // a comma is the decimal point of many keyboards' number pads
    let read = (self.dials[self.controls[control].parameter].read)(&text.replace(',', "."));
    // "nan" is a number to a dial that reads any, and no place on its turn
    read.filter(|value| value.is_finite())
  }

  /// Puts every parameter a control turns at its default.
  ///
  /// Each is set as one whole gesture.
  fn reset(&self, control: usize, actions: &mut Vec<Action>) {
    for parameter in self.controls[control].parameters() {
      actions.extend(gesture(parameter, self.dials[parameter].default));
    }
  }

  fn dismiss(&mut self, actions: &mut Vec<Action>) {
    if let State::Menu(_) = self.state {
      self.state = State::Idle;
      actions.push(Action::Redraw);
    }
  }
}

/// Returns the actions of setting a parameter as one whole gesture.
fn gesture(parameter: usize, value: f32) -> [Action; 3] {
  [
    Action::Begin(parameter),
    Action::Set(parameter, value),
    Action::End(parameter),
  ]
}

#[cfg(test)]
mod tests;
