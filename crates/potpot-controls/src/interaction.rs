//! what the pointer and the keys do to a panel of controls, apart from any window, any way of
//! drawing and any plugin: events go in with the time they came at, and what to tell the host
//! and the window comes out. a control is known here by its place among the panel's, and by
//! the [`Dial`] the panel's owner describes it with

use std::ops::Range;
use std::time::{Duration, Instant};

use crate::Point;

/// two presses this close in time and place are a double click
const DOUBLE_TIME: Duration = Duration::from_millis(400);
const DOUBLE_REACH: f32 = 4.0;
/// a turn of the wheel this long after the last belongs to a new gesture
const WHEEL_REST: Duration = Duration::from_millis(400);
/// a count of steps or of lines this close to a whole one is that one: sums of their parts
/// come out a hair off
const SLACK: f32 = 1e-4;
/// the way the pointer is dragged to turn a control, and how far that turns it
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Drag {
  /// up the panel turns it on and down turns it back, by `turn` of its whole turn for each
  /// point
  Up {
    /// the part of the whole turn one point is
    turn: f32,
  },
  /// along a line across the panel, the way a fader slides: toward `way` turns it on and
  /// away from it turns it back, by `turn` of its whole turn for each point
  Along {
    /// the way that turns it on, one point long, across and down
    way: Point,
    /// the part of the whole turn one point is
    turn: f32,
  },
}

impl Drag {
  /// how far the pointer has gone the way that turns the control on, in points
  fn travel(self, from: Point, to: Point) -> f32 {
    match self {
      Self::Up { .. } => from[1] - to[1],
      Self::Along { way, .. } => (to[0] - from[0]) * way[0] + (to[1] - from[1]) * way[1],
    }
  }

  /// how far one point of that turns it
  fn turn(self) -> f32 {
    match self {
      Self::Up { turn } | Self::Along { turn, .. } => turn,
    }
  }
}

/// what a button that is pressed does
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Push {
  /// each push turns it over: in and on, then out and off. it stays most of the way in
  /// while it is on
  Latching,
  /// it is on only while it is held
  Momentary,
}

impl Push {
  /// how far in such a button is, from zero for out to one for all the way: all the way
  /// while it is `held`, and most of the way while a latching one is `on`
  pub fn depth(self, on: bool, held: bool) -> f32 {
    match (held, self, on) {
      (true, ..) => 1.0,
      (false, Push::Latching, true) => 0.7,
      (false, Push::Latching, false) | (false, Push::Momentary, _) => 0.0,
    }
  }

  /// whether such a button is on after a push is let go, given whether it was on before. a
  /// momentary one is never on once it is let go
  pub fn released(self, was_on: bool) -> bool {
    self == Push::Latching && !was_on
  }
}

/// what the left button does to a control
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Touch {
  /// it is dragged, the way its [`Feel`] says, and a double click puts it at its default
  Turn,
  /// it is pressed like a button, and its parameter is at the end of its turn while it is
  /// on and at the start while it is off
  Press(Push),
  /// each click moves it one step on, and from the end of its turn back to the start
  Step,
}

/// how a control answers the pointer and the wheel. it starts as [`Feel::default`] and is
/// changed a field at a time
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Feel {
  /// how it is dragged
  pub drag: Drag,
  /// a control that moves in steps is dragged at least this many points for each of them.
  /// one with few steps is dragged further for each, at the pace of a control without any
  pub points_per_step: f32,
  /// how much less it moves while the fine key is held
  pub fine: f32,
  /// how far one line of the wheel turns a control that has no step, as a part of its whole
  /// turn
  pub wheel: f32,
}

impl Default for Feel {
  /// two hundred points up for the whole turn, five at least to a step, a tenth of that
  /// with the fine key, and twenty lines of the wheel for the whole turn
  fn default() -> Self {
    Self {
      drag: Drag::Up { turn: 1.0 / 200.0 },
      points_per_step: 5.0,
      fine: 0.1,
      wheel: 0.05,
    }
  }
}

/// what there is to know of one parameter, for the controls that turn it. its value is a part
/// of its whole turn, from zero to one, and its plain value is that in the units it is read
/// and written in. it can be handed to another thread, which is where some systems have a
/// window's
#[non_exhaustive]
pub struct Dial {
  /// the value a reset gives it
  pub default: f32,
  /// the coarse step it moves by, in plain units: dragged or wheeled without the fine key it
  /// lands on whole steps only. none turns smoothly
  pub step: Option<f32>,
  /// a value in plain units
  pub plain: Box<dyn Fn(f32) -> f32 + Send>,
  /// a plain value as a part of the turn. one past either end comes out as that end
  pub turn: Box<dyn Fn(f32) -> f32 + Send>,
  /// a value as it is written, without its unit
  pub write: Box<dyn Fn(f32) -> String + Send>,
  /// the unit it is in, as it is written straight after a value: with a space before it if
  /// one belongs there
  pub unit: String,
  /// the value that text stands for, if it is one
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
  /// a parameter that turns smoothly and has no unit, from its ways between a value and its
  /// plain value, and between a value and its text. `step` and `unit` are set after
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

  /// a control over a straight run of plain values from the first of `range` to the second,
  /// at `default` to begin with. it is written with so many `decimals` and `unit` after it,
  /// and read with or without the unit
  pub fn linear(
    range: [f32; 2],
    default: f32,
    step: Option<f32>,
    decimals: usize,
    unit: &'static str,
  ) -> Self {
    let [least, most] = range;
    let plain = move |value: f32| least + value * (most - least);
    let turn = move |plain: f32| ((plain - least) / (most - least)).clamp(0.0, 1.0);
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

  /// the nearest value that is a whole number of steps, or the value itself for a control
  /// without any
  fn snapped(&self, value: f32) -> f32 {
    match self.step {
      Some(step) => (self.turn)(((self.plain)(value) / step).round() * step),
      None => value,
    }
  }

  /// how far one step is as a part of the turn
  fn step_turn(&self, step: f32) -> f32 {
    ((self.turn)((self.plain)(0.0) + step) - (self.turn)((self.plain)(0.0))).abs()
  }

  /// how far a point of dragging turns it, under a control that feels like `feel`
  fn turn_per_point(&self, feel: Feel) -> f32 {
    match self.step {
      Some(step) => (feel.drag.turn()).min(self.step_turn(step) / feel.points_per_step),
      None => feel.drag.turn(),
    }
  }

  /// the value some lines of the wheel on from another. with the fine key it is a part of a
  /// step further, and without it the next whole steps that way, one for each of its lines,
  /// which are whole
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

  /// the value one step on from another, which from the end of the turn is its start
  fn stepped_on(&self, value: f32, feel: Feel) -> f32 {
    if value > 1.0 - 1e-4 {
      0.0
    } else {
      self.wheeled(value, 1.0, false, feel)
    }
  }
}

/// one of a panel's controls: something on it that is turned, with a readout of its own. a
/// control is known by its place among the panel's controls, and names the parameter it
/// turns by that one's place among the panel's parameters. the two orders are not the same
/// thing: a parameter may have no control, or more than one
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Control {
  /// the parameter it turns
  pub parameter: usize,
  /// how it answers the pointer and the wheel
  pub feel: Feel,
  /// what its readout can show of a value typed into it
  pub readout: Readout,
  /// what the left button does to it
  pub touch: Touch,
}

impl Control {
  /// a control on a parameter, with a readout, that is dragged and feels as
  /// [`Feel::default`] does
  pub fn new(parameter: usize, readout: Readout) -> Self {
    Self {
      parameter,
      feel: Feel::default(),
      readout,
      touch: Touch::Turn,
    }
  }
}

/// what a control's readout can show of a value that is typed into it
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Readout {
  /// how many letters it has room for
  pub room: usize,
  /// whether it has a letter for something typed
  pub shows: fn(char) -> bool,
}

impl Readout {
  /// a readout with room for so many letters, which shows a space and whatever
  /// [`crate::stroke`] has a letter for
  pub fn new(room: usize) -> Self {
    Self {
      room,
      shows: |letter| letter == ' ' || crate::stroke::lettered(letter),
    }
  }
}

/// what a control shows, as the panel's look is handed it
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Reading {
  /// where its parameter stands, as a part of its whole turn
  pub value: f32,
  /// what its readout says: the value as it is written, or what has been typed so far
  /// while one is being typed
  pub text: String,
  /// the unit that goes after it, which is nothing while a value is typed
  pub unit: String,
}

/// what a point of the panel belongs to
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
  /// the control at this place among the panel's: the part of it that is turned
  Control(usize),
  /// the window that control's value is written in
  Readout(usize),
  /// the menu, between and around its rows
  Menu,
  /// a row of the menu
  Item(Item),
  /// what the window is pulled larger and smaller by
  Grip,
}

/// the rows of the menu, top to bottom
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
  /// types the control's value
  Edit,
  /// puts the control at its default
  Reset,
  /// puts the control's value on the clipboard
  Copy,
  /// sets the control to the value on the clipboard
  Paste,
}

impl Item {
  /// every row, top to bottom
  pub const ALL: [Self; 4] = [Self::Edit, Self::Reset, Self::Copy, Self::Paste];
}

/// a button of the pointer that means something to a panel
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
  /// turns, presses and picks
  Left,
  /// asks for a control's menu
  Right,
}

/// a key as a value being typed takes it. the caret's moves take the selection along from
/// where it began when `extend` is set
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
  /// a letter to type
  Letter(char),
  /// sets what was typed
  Enter,
  /// gives up what was typed
  Escape,
  /// takes out the letter before the caret, or what is selected
  Backspace,
  /// takes out the letter after the caret, or what is selected
  Delete,
  /// the caret one letter back
  Left {
    /// whether the selection goes along
    extend: bool,
  },
  /// the caret one letter on
  Right {
    /// whether the selection goes along
    extend: bool,
  },
  /// the caret to the start
  Home {
    /// whether the selection goes along
    extend: bool,
  },
  /// the caret to the end
  End {
    /// whether the selection goes along
    extend: bool,
  },
  /// leaves the field for the next, which sets what was typed
  Tab,
  /// selects all that was typed
  SelectAll,
  /// puts what is selected on the clipboard
  Copy,
  /// the same, and takes it out
  Cut,
  /// types the clipboard's text
  Paste,
}

/// what the panel is like at the time an event comes in
#[derive(Clone, Copy, Debug)]
pub struct Moment<'a> {
  /// when the event came
  pub now: Instant,
  /// where each parameter stands
  pub values: &'a [f32],
  /// how many times its own size the window is
  pub scale: f32,
}

/// places are in the panel's points apart from `window`, and `over` is what is drawn there
#[derive(Clone, Debug)]
pub enum Event {
  /// the pointer moved
  Move {
    /// where to
    at: Point,
    /// the same place in the window's own points from its top left corner, which stays put
    /// when the window changes size
    window: Point,
    /// what is drawn there
    over: Option<Target>,
    /// whether the key that slows a control down is held
    fine: bool,
  },
  /// a button went down
  Press {
    /// which
    button: Button,
    /// where the pointer is
    at: Point,
    /// the same place in the window's own points, as in [`Event::Move`]
    window: Point,
    /// what is drawn there
    over: Option<Target>,
    /// whether the key that makes a click a reset is held
    reset: bool,
  },
  /// a button came up
  Release(Button),
  /// the wheel turned by so many lines, away from the user counting up
  Wheel {
    /// what is drawn under the pointer
    over: Option<Target>,
    /// how far, a part of a line for a wheel that turns smoothly
    lines: f32,
    /// whether the key that slows a control down is held
    fine: bool,
  },
  /// a key went down while the panel takes keys
  Key(Key),
  /// the clipboard's text, in answer to [`Action::WantClipboard`]. none if it has no text
  Clipboard(Option<String>),
  /// the keys went to another window
  Unfocused,
  /// time passed
  Tick,
  /// the window is going away
  Closed,
}

/// what is to be done about an event, by whoever has the host and the window
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
  /// the host is told a gesture on a parameter begins, then its values, then that it ends.
  /// the parameter is known by its place among the panel's parameters, which is not its
  /// control's among the controls
  Begin(usize),
  /// the parameter is set to a value
  Set(usize, f32),
  /// the gesture on the parameter is over
  End(usize),
  /// the window takes the keys
  Focus,
  /// the panel looks different though no value changed
  Redraw,
  /// the text goes on the clipboard
  Copy(String),
  /// the clipboard's text is to come back as [`Event::Clipboard`]
  WantClipboard,
  /// the window is to be this many times its own size
  Scale(f32),
}

/// a value being typed
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
  /// the control it is for
  pub control: usize,
  /// what has been typed
  pub text: String,
  /// how many letters are before the caret
  pub caret: usize,
  /// where the selection began. it reaches from there to the caret, and with both at one
  /// place nothing is selected
  pub anchor: usize,
}

impl Edit {
  /// the places of the letters that are selected
  pub fn selected(&self) -> Range<usize> {
    self.anchor.min(self.caret)..self.anchor.max(self.caret)
  }

  fn letters(&self) -> usize {
    self.text.chars().count()
  }

  /// where in the text the letter after so many others starts
  fn byte(&self, letter: usize) -> usize {
    (self.text.char_indices().nth(letter)).map_or(self.text.len(), |(byte, _)| byte)
  }

  /// what copying takes: the selection, or with nothing selected all there is
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

  /// takes letters out and leaves the caret where they were
  fn remove(&mut self, letters: Range<usize>) {
    let bytes = self.byte(letters.start)..self.byte(letters.end);
    self.text.replace_range(bytes, "");
    self.caret = letters.start;
    self.anchor = letters.start;
  }

  /// puts text in at the caret in place of what is selected: as much of it as the readout
  /// has room for, of the letters it can show
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

/// what the clipboard's text is wanted for
#[derive(Clone, Copy, Debug, PartialEq)]
enum Paste {
  /// to go into the value being typed for a control
  Typed(usize),
  /// to be a control's value
  Value(usize),
}

/// the menu of a control, while it is up
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Menu {
  /// the control it is for
  pub control: usize,
  /// where on the picture it was asked for
  pub at: Point,
  /// the row the pointer is on
  pub hovered: Option<Item>,
}

#[derive(Debug)]
enum State {
  Idle,
  /// the left button went down on a knob. the host hears of it once the value changes
  Drag {
    control: usize,
    /// where the pointer has taken the knob, which between two steps is not where it shows
    turned: f32,
    /// the value last shown
    value: f32,
    /// where the pointer last was
    from: Point,
    /// whether the fine key was held at the last move
    fine: bool,
    begun: bool,
  },
  /// the left button went down on the grip. `grab` is where the pointer was then in the
  /// window's own points, `from` the scale the window had, and `scale` the one last asked for
  Resize {
    grab: Point,
    from: f32,
    scale: f32,
  },
  /// the left button is down on a control that is pressed. `on` is whether it was on
  /// before
  Held {
    control: usize,
    push: Push,
    on: bool,
  },
  Edit(Edit),
  Menu(Menu),
}

/// the wheel turning a control
#[derive(Debug)]
struct Wheel {
  control: usize,
  /// the value it last set. none until it sets one, which is when the host hears of it
  set: Option<f32>,
  /// the part of a line it has turned a control that moves in steps, short of a whole one
  gathered: f32,
  /// when it last moved
  last: Instant,
}

/// the state of a panel's controls: what is being dragged, typed or picked from the menu
#[derive(Debug)]
pub struct Interaction {
  /// what there is to know of each parameter
  dials: Vec<Dial>,
  controls: Vec<Control>,
  /// the panel's width and height in its points
  size: Point,
  state: State,
  /// the last press of the left button, for telling a double click
  pressed: Option<(Instant, Point, Target)>,
  /// what the wheel is turning, until it rests
  wheel: Option<Wheel>,
  /// what the clipboard was asked for, until its text comes
  awaiting: Option<Paste>,
  /// the least and the most times its own size the window is pulled to
  scales: [f32; 2],
}

impl Interaction {
  /// the behaviour of a panel `size` across and down with these controls on parameters the
  /// dials describe. the grip pulls the window from the first of `scales` times its own
  /// size to the second
  ///
  /// # Panics
  ///
  /// if a control names a parameter that `dials` has none for
  pub fn new(dials: Vec<Dial>, controls: Vec<Control>, size: Point, scales: [f32; 2]) -> Self {
    assert!(
      controls
        .iter()
        .all(|control| control.parameter < dials.len()),
      "every control turns one of the panel's parameters"
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

  /// what each control shows with the parameters at `values`, in the controls' order.
  /// `values` has one for every parameter
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

  /// the value being typed, if one is
  pub fn edit(&self) -> Option<&Edit> {
    match &self.state {
      State::Edit(edit) => Some(edit),
      _ => None,
    }
  }

  /// the menu, if it is up
  pub fn menu(&self) -> Option<&Menu> {
    match &self.state {
      State::Menu(menu) => Some(menu),
      _ => None,
    }
  }

  /// the control that is pressed and held down, if one is
  pub fn held(&self) -> Option<usize> {
    match self.state {
      State::Held { control, .. } => Some(control),
      _ => None,
    }
  }

  /// whether keys mean something to the panel right now
  pub fn takes_keys(&self) -> bool {
    matches!(self.state, State::Edit(_) | State::Menu(_))
  }

  /// takes an event in, and gives what is to be done about it, in order
  pub fn handle(&mut self, event: Event, moment: Moment) -> Vec<Action> {
    let mut actions = Vec::new();
    match event {
      Event::Move {
        at,
        window,
        over,
        fine,
      } => self.moved(at, window, over, fine, &mut actions),
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

  /// what the left button has open is over with no release of it: a drag or a pull at the
  /// grip ends and a momentary button goes off, and a latching one was not let go, so it
  /// stays as it was
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

  /// the left button is up, or as good as: a drag, a press or a pull at the grip is over
  fn let_go(&mut self, actions: &mut Vec<Action>) {
    match self.state {
      State::Drag { control, begun, .. } => {
        if begun {
          actions.push(Action::End(self.controls[control].parameter));
        }
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
    actions: &mut Vec<Action>,
  ) {
    match &mut self.state {
      State::Drag {
        control,
        turned,
        value,
        from,
        fine,
        begun,
      } => {
        let Control {
          parameter, feel, ..
        } = self.controls[*control];
        let dial = &self.dials[parameter];
        // with the fine key the knob goes on from where it shows, so that pressing the key
        // moves nothing, and every move adds its own part of a turn, so that the key changes
        // the pace from here on and not the place
        if now_fine && !*fine {
          *turned = *value;
        }
        *fine = now_fine;
        let pace = dial.turn_per_point(feel) * if now_fine { feel.fine } else { 1.0 };
        *turned = (*turned + feel.drag.travel(*from, at) * pace).clamp(0.0, 1.0);
        *from = at;
        let shown = if now_fine {
          *turned
        } else {
          dial.snapped(*turned)
        };
        // at the end of the turn, between two steps, or with the pointer moved only sideways
        if shown == *value {
          return;
        }
        *value = shown;
        if !*begun {
          *begun = true;
          actions.push(Action::Begin(parameter));
        }
        actions.push(Action::Set(parameter, shown));
      }
      State::Resize { grab, from, scale } => {
        // the corner goes as far as the pointer has gone since it took hold, across or down,
        // whichever is further, and the panel's shape gives the other. it is measured in the
        // window's own points and not the panel's, which shift as the window changes size,
        // so a window that has caught up with the pointer asks for no other size
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
            actions.extend(gesture(parameter, dial.default));
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
      // the release of the press before never came, which baseview leaves out when windows
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
          _ if reset => actions.extend(gesture(parameter, dial.default)),
          Touch::Turn if double => actions.extend(gesture(parameter, dial.default)),
          Touch::Turn => {
            self.state = State::Drag {
              control,
              turned: values[parameter],
              value: values[parameter],
              from: at,
              fine: false,
              begun: false,
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
    let Control {
      parameter,
      feel,
      touch,
      ..
    } = self.controls[control];
    // a button is pressed, and the wheel does nothing to it
    if let Touch::Press(_) = touch {
      return;
    }
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

  /// the wheel is no longer turning what it was, which ends the gesture if the host has
  /// heard of one
  fn rest_wheel(&mut self, actions: &mut Vec<Action>) {
    if let Some(Wheel {
      control,
      set: Some(_),
      ..
    }) = self.wheel.take()
    {
      actions.push(Action::End(self.controls[control].parameter));
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

  /// the clipboard's text has come, for whatever it was last wanted for. text that came
  /// with space or line ends around it is taken without them
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

  /// ends typing, with what was typed as the control's value if it reads as one and with
  /// the value left alone if it does not
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

  /// the value of a control that text stands for, if it reads as one
  fn read(&self, control: usize, text: &str) -> Option<f32> {
    // a comma is the decimal point of many keyboards' number pads
    let read = (self.dials[self.controls[control].parameter].read)(&text.replace(',', "."));
    // "nan" is a number to a dial that reads any, and no place on its turn
    read.filter(|value| value.is_finite())
  }

  fn dismiss(&mut self, actions: &mut Vec<Action>) {
    if let State::Menu(_) = self.state {
      self.state = State::Idle;
      actions.push(Action::Redraw);
    }
  }
}

/// a parameter set to a value as one whole gesture
fn gesture(parameter: usize, value: f32) -> [Action; 3] {
  [
    Action::Begin(parameter),
    Action::Set(parameter, value),
    Action::End(parameter),
  ]
}

#[cfg(test)]
mod tests;
