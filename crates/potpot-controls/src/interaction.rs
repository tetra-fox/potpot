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
/// and written in. it can be handed
/// to another thread, which is where some systems have a window's
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
  /// step further, and without it the next whole steps that way
  fn wheeled(&self, value: f32, lines: f32, fine: bool, feel: Feel) -> f32 {
    let turned = match self.step {
      None => value + lines * feel.wheel * if fine { feel.fine } else { 1.0 },
      Some(step) if fine => (self.turn)((self.plain)(value) + lines * step * feel.fine),
      Some(step) => {
        let steps = (self.plain)(value) / step;
        // from between two steps the first line goes to the one that way, and no further
        let slack = 1e-4;
        let from = if lines > 0.0 {
          (steps + slack).floor()
        } else {
          (steps - slack).ceil()
        };
        let lines = lines.signum() * lines.abs().round().max(1.0);
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
  /// the parameter is known by its place among the panel's, which is its control's
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
  /// the control the wheel is turning, its value, and when the wheel last moved
  wheel: Option<(usize, f32, Instant)>,
  /// what the clipboard was asked for, until its text comes
  awaiting: Option<Paste>,
  /// the least and the most times its own size the window is pulled to
  scales: [f32; 2],
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

impl Interaction {
  /// the behaviour of a panel `size` across and down with these controls on parameters the
  /// dials describe. the grip pulls the window from the first of `scales` times its own
  /// size to the second
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

  /// what each control shows with the parameters at `values`, in the controls' order
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
        if let Some((_, _, last)) = self.wheel
          && moment.now.duration_since(last) >= WHEEL_REST
        {
          self.rest_wheel(&mut actions);
        }
      }
      Event::Closed => {
        self.rest_wheel(&mut actions);
        // a latching button held as the window goes was not let go, and stays as it was
        if let State::Held {
          push: Push::Latching,
          ..
        } = self.state
        {
          self.state = State::Idle;
        }
        self.let_go(&mut actions);
        self.state = State::Idle;
      }
    }
    actions
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
      State::Idle | State::Drag { .. } | State::Held { .. } | State::Resize { .. } => {}
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
    let from = match self.wheel {
      Some((turning, value, _)) if turning == control => value,
      _ => {
        self.rest_wheel(actions);
        actions.push(Action::Begin(parameter));
        moment.values[parameter]
      }
    };
    let value = self.dials[parameter].wheeled(from, lines, fine, feel);
    self.wheel = Some((control, value, moment.now));
    actions.push(Action::Set(parameter, value));
  }

  /// ends the gesture the wheel has open, if it has one
  fn rest_wheel(&mut self, actions: &mut Vec<Action>) {
    if let Some((control, _, _)) = self.wheel.take() {
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
    (self.dials[self.controls[control].parameter].read)(&text.replace(',', "."))
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
mod tests {
  use super::Action::{Begin, End, Focus, Redraw, Set, WantClipboard};
  use super::*;

  /// how many letters the readouts of the bench have room for
  const ROOM: usize = 9;

  /// a panel with every control at its default, the shifts in the middle of their turn and
  /// the mix at the end of its own, and events arriving a hundredth of a second apart
  struct Bench {
    interaction: Interaction,
    /// where the controls stand, which is their defaults unless a test says otherwise
    values: [f32; 4],
    /// how many times its own size the window is
    scale: f32,
    now: Instant,
  }

  /// the bench's controls: two shifts of 24 semitones either way and a mix in hundredths,
  /// all three turning smoothly, and a shift like the others that moves in whole semitones
  const PITCH: usize = 0;
  const FORMANT: usize = 1;
  const MIX: usize = 2;
  const STEPPED: usize = 3;
  const SIZE: Point = [500.0, 250.0];
  const SCALES: [f32; 2] = [0.75, 3.0];

  fn shift(step: Option<f32>) -> Dial {
    Dial {
      default: 0.5,
      step,
      plain: Box::new(|value| value * 48.0 - 24.0),
      turn: Box::new(|plain| ((plain + 24.0) / 48.0).clamp(0.0, 1.0)),
      write: Box::new(|value| format!("{:.2}", value * 48.0 - 24.0)),
      unit: " st".to_owned(),
      read: Box::new(|text| {
        let plain: f32 = text.trim().trim_end_matches(" st").parse().ok()?;
        Some(((plain + 24.0) / 48.0).clamp(0.0, 1.0))
      }),
    }
  }

  fn mix() -> Dial {
    Dial {
      default: 1.0,
      step: None,
      plain: Box::new(|value| value),
      turn: Box::new(|plain| plain.clamp(0.0, 1.0)),
      write: Box::new(|value| format!("{:.0}", value * 100.0)),
      unit: "%".to_owned(),
      read: Box::new(|text| {
        let hundredths: f32 = text.trim().trim_end_matches([' ', '%']).parse().ok()?;
        Some((hundredths / 100.0).clamp(0.0, 1.0))
      }),
    }
  }

  #[test]
  fn a_control_turns_the_parameter_it_names_and_not_the_one_at_its_own_place() {
    // two controls on four parameters: the first turns the mix, the second the pitch
    let dials = vec![shift(None), shift(None), mix(), shift(Some(1.0))];
    let on = |parameter| Control {
      parameter,
      feel: Feel::default(),
      readout: Readout {
        room: ROOM,
        shows: |_| true,
      },
      touch: Touch::Turn,
    };
    let mut panel = Interaction::new(dials, vec![on(MIX), on(PITCH)], SIZE, SCALES);
    let values = [0.5, 0.5, 1.0, 0.5];
    let moment = Moment {
      now: Instant::now(),
      values: &values,
      scale: 1.0,
    };
    // each control shows its own parameter, written as that parameter is
    let readings = panel.readings(&values);
    assert_eq!([readings[0].value, readings[1].value], [1.0, 0.5]);
    assert_eq!(
      (readings[0].text.as_str(), readings[0].unit.as_str()),
      ("100", "%")
    );
    assert_eq!(
      (readings[1].text.as_str(), readings[1].unit.as_str()),
      ("0.00", " st")
    );
    // a double click on the second control resets the pitch, which is the first parameter
    let press = |over| Event::Press {
      button: Button::Left,
      at: [10.0, 10.0],
      window: [10.0, 10.0],
      over: Some(over),
      reset: true,
    };
    assert_eq!(
      panel.handle(press(Target::Control(1)), moment),
      [
        Action::Begin(PITCH),
        Action::Set(PITCH, 0.5),
        Action::End(PITCH)
      ]
    );
    assert_eq!(
      panel.handle(press(Target::Control(0)), moment),
      [Action::Begin(MIX), Action::Set(MIX, 1.0), Action::End(MIX)]
    );
    // and the wheel over the first control's readout turns the mix down
    let wheel = Event::Wheel {
      over: Some(Target::Readout(0)),
      lines: -1.0,
      fine: false,
    };
    let actions = panel.handle(wheel, moment);
    assert_eq!(actions[0], Action::Begin(MIX));
    assert!(matches!(actions[1], Action::Set(MIX, value) if value < 1.0));
  }

  #[test]
  fn a_control_reads_what_is_typed_into_it_while_it_is_typed() {
    let mut bench = Bench::new();
    let values = bench.values;
    let said = |bench: &Bench| {
      let reading = &bench.interaction.readings(&values)[MIX];
      (reading.text.clone(), reading.unit.clone())
    };
    assert_eq!(said(&bench), ("100".to_owned(), "%".to_owned()));
    // typing begins with the value as it was written, and no unit after it
    let readout = Some(Target::Readout(MIX));
    bench.press(readout, 100.0);
    bench.press(readout, 100.0);
    assert_eq!(said(&bench), ("100".to_owned(), String::new()));
    bench.key(Key::Letter('4'));
    assert_eq!(said(&bench), ("4".to_owned(), String::new()));
    // the other controls go on reading their values
    let pitch = &bench.interaction.readings(&values)[PITCH];
    assert_eq!((pitch.text.as_str(), pitch.unit.as_str()), ("0.00", " st"));
    bench.key(Key::Escape);
    assert_eq!(said(&bench), ("100".to_owned(), "%".to_owned()));
  }

  #[test]
  fn each_readout_holds_and_shows_what_its_own_control_says_it_can() {
    // one readout with room for two figures, and one with room for five of anything
    let dials = vec![shift(None), mix()];
    let narrow = Control {
      parameter: 0,
      feel: Feel::default(),
      readout: Readout {
        room: 2,
        shows: |letter| letter.is_ascii_digit(),
      },
      touch: Touch::Turn,
    };
    let wide = Control {
      parameter: 1,
      feel: Feel::default(),
      readout: Readout {
        room: 5,
        shows: |_| true,
      },
      touch: Touch::Turn,
    };
    let mut panel = Interaction::new(dials, vec![narrow, wide], SIZE, SCALES);
    let values = [0.5, 1.0];
    let mut typed = |control: usize, letters: &str| {
      let moment = Moment {
        now: Instant::now(),
        values: &values,
        scale: 1.0,
      };
      let press = Event::Press {
        button: Button::Left,
        at: [10.0, 10.0],
        window: [10.0, 10.0],
        over: Some(Target::Readout(control)),
        reset: false,
      };
      panel.handle(press.clone(), moment);
      panel.handle(press, moment);
      for letter in letters.chars() {
        panel.handle(Event::Key(Key::Letter(letter)), moment);
      }
      let text = panel.edit().expect("a value being typed").text.clone();
      panel.handle(Event::Key(Key::Escape), moment);
      text
    };
    assert_eq!(typed(0, "1x2.345"), "12");
    assert_eq!(typed(1, "1x2.345"), "1x2.3");
  }

  #[test]
  fn a_linear_dial_is_written_and_read_in_its_plain_units() {
    let dial = Dial::linear([-12.0, 12.0], 0.0, Some(1.0), 1, " st");
    assert_eq!(dial.default, 0.5);
    assert_eq!((dial.plain)(0.75), 6.0);
    assert_eq!((dial.turn)(-6.0), 0.25);
    assert_eq!(
      ((dial.write)(0.75), dial.unit.as_str()),
      ("6.0".to_owned(), " st")
    );
    assert_eq!((dial.write)(0.25), "-6.0");
    // read with the unit or without, and with room around either
    for text in ["6", "6.0 st", " 6 st ", "6st"] {
      assert_eq!((dial.read)(text), Some(0.75), "{text}");
    }
    // past an end is that end, and what is not a number is nothing
    assert_eq!((dial.read)("40"), Some(1.0));
    assert_eq!((dial.read)("-40 st"), Some(0.0));
    assert_eq!((dial.read)("six"), None);
    assert_eq!((dial.read)(""), None);
  }

  fn interaction() -> Interaction {
    let dials = vec![shift(None), shift(None), mix(), shift(Some(1.0))];
    // a control for each parameter, in the same order, with readouts all alike
    let controls = (0..dials.len())
      .map(|parameter| Control {
        parameter,
        feel: Feel::default(),
        readout: Readout {
          room: ROOM,
          shows: |letter| letter.is_ascii_digit() || " +-.,%st".contains(letter),
        },
        touch: Touch::Turn,
      })
      .collect();
    Interaction::new(dials, controls, SIZE, SCALES)
  }

  impl Bench {
    fn new() -> Self {
      Self {
        interaction: interaction(),
        values: [0.5, 0.5, 1.0, 0.5],
        scale: 1.0,
        now: Instant::now(),
      }
    }

    fn after(&mut self, wait: Duration, event: Event) -> Vec<Action> {
      self.now += wait;
      let moment = Moment {
        now: self.now,
        values: &self.values,
        scale: self.scale,
      };
      self.interaction.handle(event, moment)
    }

    fn send(&mut self, event: Event) -> Vec<Action> {
      self.after(Duration::from_millis(10), event)
    }

    /// a place on the panel, and where that is in the window at the bench's scale
    fn place(&self, at: Point) -> (Point, Point) {
      (at, at.map(|across| across * self.scale))
    }

    fn press(&mut self, over: Option<Target>, y: f32) -> Vec<Action> {
      let (at, window) = self.place([100.0, y]);
      self.send(Event::Press {
        button: Button::Left,
        at,
        window,
        over,
        reset: false,
      })
    }

    fn right(&mut self, over: Option<Target>) -> Vec<Action> {
      let (at, window) = self.place([100.0, 100.0]);
      self.send(Event::Press {
        button: Button::Right,
        at,
        window,
        over,
        reset: false,
      })
    }

    fn drag(&mut self, y: f32, fine: bool) -> Vec<Action> {
      let (at, window) = self.place([100.0, y]);
      self.send(Event::Move {
        at,
        window,
        over: None,
        fine,
      })
    }

    fn release(&mut self) -> Vec<Action> {
      self.send(Event::Release(Button::Left))
    }

    fn wheel(&mut self, over: Target, lines: f32, fine: bool) -> Vec<Action> {
      self.send(Event::Wheel {
        over: Some(over),
        lines,
        fine,
      })
    }

    fn key(&mut self, key: Key) -> Vec<Action> {
      self.send(Event::Key(key))
    }

    fn kind(&mut self, text: &str) {
      for letter in text.chars() {
        self.key(Key::Letter(letter));
      }
    }

    /// a double click on a readout, which begins typing its value
    fn edit(&mut self, control: usize) {
      let readout = Some(Target::Readout(control));
      assert_eq!(self.press(readout, 100.0), []);
      self.release();
      assert_eq!(self.press(readout, 100.0), [Focus, Redraw]);
      self.release();
    }

    fn typed(&self) -> (&str, usize, bool) {
      let edit = self.interaction.edit().expect("a value being typed");
      (&edit.text, edit.caret, !edit.selected().is_empty())
    }
  }

  /// the value a list of actions sets, which has to be the only thing they do
  fn set(actions: &[Action], control: usize) -> f32 {
    match actions {
      [Set(set, value)] if *set == control => *value,
      other => panic!("not one value for {control:?}: {other:?}"),
    }
  }

  fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
  }

  #[test]
  fn a_drag_is_one_gesture_from_the_first_move_to_the_release() {
    let mut bench = Bench::new();
    assert_eq!(bench.press(Some(Target::Control(PITCH)), 100.0), []);
    let first = bench.drag(80.0, false);
    assert_eq!(first[0], Begin(PITCH));
    assert!(close(set(&first[1..], PITCH), 0.6));
    assert!(close(set(&bench.drag(90.0, false), PITCH), 0.55));
    assert_eq!(bench.release(), [End(PITCH)]);
    // and with the button up the pointer turns nothing
    assert_eq!(bench.drag(50.0, false), []);
  }

  #[test]
  fn a_click_that_does_not_move_tells_the_host_nothing() {
    let mut bench = Bench::new();
    assert_eq!(bench.press(Some(Target::Control(PITCH)), 100.0), []);
    assert_eq!(bench.drag(100.0, false), []);
    assert_eq!(bench.release(), []);
  }

  #[test]
  fn the_fine_key_changes_the_pace_of_a_drag_and_not_its_place() {
    let mut bench = Bench::new();
    bench.press(Some(Target::Control(PITCH)), 100.0);
    assert!(close(set(&bench.drag(80.0, false)[1..], PITCH), 0.6));
    // held from here: the same way again moves a tenth as far, from where the knob is
    assert!(close(set(&bench.drag(60.0, true), PITCH), 0.61));
    // and let go: the knob goes on from there at its full pace
    assert!(close(set(&bench.drag(40.0, false), PITCH), 0.71));
    assert!(close(set(&bench.drag(60.0, true), PITCH), 0.70));
  }

  #[test]
  fn a_drag_stops_at_the_end_of_the_turn_and_comes_back_at_once() {
    let mut bench = Bench::new();
    bench.press(Some(Target::Control(PITCH)), 300.0);
    assert_eq!(set(&bench.drag(0.0, false)[1..], PITCH), 1.0);
    // further the same way there is nothing new to tell the host
    assert_eq!(bench.drag(-50.0, false), []);
    assert!(close(set(&bench.drag(-30.0, false), PITCH), 0.9));
  }

  /// the value of the bench's shifts that is so many semitones
  fn semitones(plain: f32) -> f32 {
    (plain + 24.0) / 48.0
  }

  #[test]
  fn a_stepped_control_is_dragged_from_step_to_step_and_says_so_once_each() {
    // five points of dragging to a semitone: the whole 48 take 240
    let mut bench = Bench::new();
    bench.press(Some(Target::Control(STEPPED)), 100.0);
    // not yet half way to the next step
    assert_eq!(bench.drag(98.0, false), []);
    let first = bench.drag(97.0, false);
    assert_eq!(first, [Begin(STEPPED), Set(STEPPED, semitones(1.0))]);
    // on toward the step after, and past the half way to it
    assert_eq!(bench.drag(96.0, false), []);
    assert_eq!(bench.drag(93.0, false), []);
    assert_eq!(bench.drag(92.0, false), [Set(STEPPED, semitones(2.0))]);
    // a point at a time the pointer still gets there: nothing is lost between the steps
    let mut sets = Vec::new();
    for y in (72..92).rev() {
      sets.extend(bench.drag(y as f32, false));
    }
    assert_eq!(
      sets,
      [3.0, 4.0, 5.0, 6.0].map(|plain| Set(STEPPED, semitones(plain)))
    );
    assert_eq!(bench.release(), [End(STEPPED)]);
  }

  #[test]
  fn the_fine_key_turns_a_stepped_control_smoothly_from_where_it_shows() {
    let mut bench = Bench::new();
    bench.press(Some(Target::Control(STEPPED)), 100.0);
    // three points on, the knob shows a semitone though the pointer has taken it 0.6
    bench.drag(97.0, false);
    // the key going down moves nothing
    assert_eq!(bench.drag(97.0, true), []);
    // and from there ten points are a fifth of a semitone, on from the one shown
    assert!(close(set(&bench.drag(87.0, true), STEPPED), semitones(1.2)));
    assert!(close(set(&bench.drag(77.0, true), STEPPED), semitones(1.4)));
    // the key coming up puts the knob on the nearest step at once
    assert_eq!(bench.drag(77.0, false), [Set(STEPPED, semitones(1.0))]);
    // and it steps on from where the pointer had it, 1.4: one more point makes 1.6
    assert_eq!(bench.drag(76.0, false), [Set(STEPPED, semitones(2.0))]);
  }

  #[test]
  fn a_stepped_control_off_its_steps_goes_to_one_with_the_first_move() {
    for (y, lands) in [(99.0, 8.0), (101.0, 7.0)] {
      let mut bench = Bench::new();
      bench.values[STEPPED] = semitones(7.39);
      bench.press(Some(Target::Control(STEPPED)), 100.0);
      // a point either way is a fifth of a semitone, and what shows is the step nearest that
      let moved = bench.drag(y, false);
      assert_eq!(
        moved,
        [Begin(STEPPED), Set(STEPPED, semitones(lands))],
        "{y}"
      );
    }
    // with the fine key it turns on from where it is
    let mut bench = Bench::new();
    bench.values[STEPPED] = semitones(7.39);
    bench.press(Some(Target::Control(STEPPED)), 100.0);
    assert!(close(
      set(&bench.drag(90.0, true)[1..], STEPPED),
      semitones(7.59)
    ));
  }

  #[test]
  fn the_wheel_takes_a_stepped_control_a_step_and_onto_one() {
    let mut bench = Bench::new();
    let knob = Target::Control(STEPPED);
    assert_eq!(
      bench.wheel(knob, 1.0, false)[1..],
      [Set(STEPPED, semitones(1.0))]
    );
    assert_eq!(
      bench.wheel(knob, -1.0, false),
      [Set(STEPPED, semitones(0.0))]
    );
    // with the fine key a tenth of a step, off the steps
    assert!(close(
      set(&bench.wheel(knob, 1.0, true), STEPPED),
      semitones(0.1)
    ));
    // and from between two steps a line goes to the next one that way
    assert_eq!(
      bench.wheel(knob, 1.0, false),
      [Set(STEPPED, semitones(1.0))]
    );
    bench.wheel(knob, -1.0, true);
    assert_eq!(
      bench.wheel(knob, -1.0, false),
      [Set(STEPPED, semitones(0.0))]
    );
    // it stops at the end of the turn
    bench.after(Duration::from_secs(1), Event::Tick);
    bench.values[STEPPED] = semitones(23.5);
    assert_eq!(bench.wheel(knob, 1.0, false)[1..], [Set(STEPPED, 1.0)]);
    assert_eq!(bench.wheel(knob, 1.0, false), [Set(STEPPED, 1.0)]);
  }

  /// the grip, in the far corner of the bench's panel, ten points in from both edges
  const GRIP: Point = [SIZE[0] - 10.0, SIZE[1] - 10.0];

  /// the pointer moved to a place in the window
  fn pull(bench: &mut Bench, to: Point) -> Vec<Action> {
    // the same place in the window is fewer of the panel's points the larger it has become
    let at = to.map(|across| across / bench.scale);
    bench.send(Event::Move {
      at,
      window: to,
      over: None,
      fine: false,
    })
  }

  fn grab(bench: &mut Bench) -> Vec<Action> {
    let (at, window) = bench.place(GRIP);
    bench.send(Event::Press {
      button: Button::Left,
      at,
      window,
      over: Some(Target::Grip),
      reset: false,
    })
  }

  #[test]
  fn pulling_the_grip_scales_the_window_to_reach_the_pointer() {
    let mut bench = Bench::new();
    assert_eq!(grab(&mut bench), []);
    // held where it was, the window stays as it is
    assert_eq!(pull(&mut bench, GRIP), []);
    // across by a tenth of the panel's width
    assert_eq!(
      pull(&mut bench, [GRIP[0] + 50.0, GRIP[1]]),
      [Action::Scale(1.1)]
    );
    // the window follows, and the pointer where it was asks for nothing new
    bench.scale = 1.1;
    assert_eq!(pull(&mut bench, [GRIP[0] + 50.0, GRIP[1]]), []);
    // down by more of the panel's height than that: whichever way the pointer has gone
    // further counts, and the panel keeps its shape
    assert_eq!(
      pull(&mut bench, [GRIP[0] + 50.0, GRIP[1] + 50.0]),
      [Action::Scale(1.2)]
    );
    bench.scale = 1.2;
    // back in across by more than it is still out down, the window shrinks to there
    let scaled = |actions: Vec<Action>| match actions[..] {
      [Action::Scale(scale)] => scale,
      _ => panic!("not one scale: {actions:?}"),
    };
    assert!(close(
      scaled(pull(&mut bench, [GRIP[0] - 100.0, GRIP[1] + 25.0])),
      0.8
    ));
    // and straight along one side it follows that side alone
    bench.scale = 0.8;
    assert!(close(
      scaled(pull(&mut bench, [GRIP[0] + 250.0, GRIP[1]])),
      1.5
    ));
    assert_eq!(bench.release(), []);
    // and with the button up the pointer scales nothing
    assert_eq!(pull(&mut bench, [GRIP[0] + 200.0, GRIP[1]]), []);
  }

  #[test]
  fn a_window_that_has_followed_the_pointer_asks_for_no_other_size() {
    let mut bench = Bench::new();
    grab(&mut bench);
    let to = [GRIP[0] + 51.0, GRIP[1]];
    assert_eq!(pull(&mut bench, to), [Action::Scale(1.102)]);
    // the window becomes about that size, but the panel fitted into its whole pixels is a
    // hair wider or narrower, so the same place in the window is a little way off on the
    // panel. the host then moves the pointer to where it already is
    for (scale, off) in [(1.1019999, 0.4), (1.1020001, -0.4), (1.102, 0.0)] {
      bench.scale = scale;
      let moved = Event::Move {
        at: [to[0] / scale + off, to[1] / scale],
        window: to,
        over: None,
        fine: false,
      };
      assert_eq!(bench.send(moved), [], "at {scale}");
    }
  }

  #[test]
  fn the_grip_held_anywhere_on_it_does_not_jump_the_window() {
    // at a window half as large again, held three points off the grip's middle
    let mut bench = Bench::new();
    bench.scale = 1.5;
    let (held, on_screen) = bench.place([GRIP[0] + 3.0, GRIP[1] - 3.0]);
    bench.send(Event::Press {
      button: Button::Left,
      at: held,
      window: on_screen,
      over: Some(Target::Grip),
      reset: false,
    });
    assert_eq!(pull(&mut bench, on_screen), []);
    assert_eq!(
      pull(&mut bench, [on_screen[0] + 25.0, on_screen[1]]),
      [Action::Scale(1.55)]
    );
  }

  #[test]
  fn the_window_is_scaled_no_smaller_or_larger_than_its_limits() {
    let mut bench = Bench::new();
    grab(&mut bench);
    assert_eq!(
      pull(&mut bench, [5000.0, 5000.0]),
      [Action::Scale(SCALES[1])]
    );
    assert_eq!(pull(&mut bench, [6000.0, 5000.0]), []);
    assert_eq!(
      pull(&mut bench, [-200.0, -200.0]),
      [Action::Scale(SCALES[0])]
    );
    assert_eq!(pull(&mut bench, [-300.0, -200.0]), []);
  }

  #[test]
  fn a_double_click_on_the_grip_is_the_windows_own_size() {
    let mut bench = Bench::new();
    bench.scale = 2.0;
    grab(&mut bench);
    bench.release();
    assert_eq!(grab(&mut bench), [Action::Scale(1.0)]);
    // which is all of it: the pointer moving on with the button down pulls nothing
    assert_eq!(pull(&mut bench, [900.0, 400.0]), []);
    // and the other button on the grip does nothing
    bench.after(Duration::from_secs(1), Event::Tick);
    assert_eq!(bench.right(Some(Target::Grip)), []);
    assert!(bench.interaction.menu().is_none());
  }

  #[test]
  fn a_double_click_on_a_knob_resets_it() {
    let mut bench = Bench::new();
    let knob = Some(Target::Control(MIX));
    bench.press(knob, 100.0);
    bench.release();
    assert_eq!(
      bench.press(knob, 101.0),
      [Begin(MIX), Set(MIX, 1.0), End(MIX)]
    );
    // the reset is whole, so the release ends nothing and a drag does not follow
    assert_eq!(bench.drag(50.0, false), []);
    assert_eq!(bench.release(), []);
    // a third press in time is a first one again
    assert_eq!(bench.press(knob, 101.0), []);
    bench.release();
    assert_eq!(bench.press(knob, 101.0).len(), 3);
  }

  #[test]
  fn presses_too_far_apart_are_not_a_double_click() {
    let mut bench = Bench::new();
    let knob = Some(Target::Control(MIX));
    bench.press(knob, 100.0);
    bench.release();
    assert_eq!(
      bench.after(Duration::from_millis(500), Event::Release(Button::Left)),
      []
    );
    assert_eq!(bench.press(knob, 100.0), []);
    bench.release();
    // near in time but not in place
    assert_eq!(bench.press(knob, 110.0), []);
    bench.release();
    // or on something else
    assert_eq!(bench.press(Some(Target::Control(PITCH)), 110.0), []);
  }

  #[test]
  fn a_click_with_the_reset_key_resets() {
    let mut bench = Bench::new();
    let actions = bench.send(Event::Press {
      button: Button::Left,
      at: [100.0, 100.0],
      window: [100.0, 100.0],
      over: Some(Target::Control(PITCH)),
      reset: true,
    });
    assert_eq!(actions, [Begin(PITCH), Set(PITCH, 0.5), End(PITCH)]);
  }

  #[test]
  fn a_burst_of_the_wheel_is_one_gesture() {
    let mut bench = Bench::new();
    let knob = Target::Control(STEPPED);
    // a semitone of the 48 the stepped shift covers for each line
    let first = bench.wheel(knob, 1.0, false);
    assert_eq!(first[0], Begin(STEPPED));
    assert!(close(set(&first[1..], STEPPED), 0.5 + 1.0 / 48.0));
    assert!(close(
      set(&bench.wheel(knob, 1.0, false), STEPPED),
      0.5 + 2.0 / 48.0
    ));
    assert!(close(
      set(&bench.wheel(knob, -1.0, true), STEPPED),
      0.5 + 1.9 / 48.0
    ));
    // it stays open while the wheel has only just stopped
    assert_eq!(bench.after(Duration::from_millis(300), Event::Tick), []);
    assert_eq!(
      bench.after(Duration::from_millis(200), Event::Tick),
      [End(STEPPED)]
    );
    assert_eq!(bench.send(Event::Tick), []);
    // the next turn is a new one
    assert_eq!(bench.wheel(knob, 1.0, false)[0], Begin(STEPPED));
  }

  #[test]
  fn the_wheel_moving_on_or_a_press_ends_its_gesture() {
    let mut bench = Bench::new();
    bench.wheel(Target::Control(PITCH), 1.0, false);
    let next = bench.wheel(Target::Readout(MIX), -1.0, false);
    assert_eq!(next[..2], [End(PITCH), Begin(MIX)]);
    assert!(close(set(&next[2..], MIX), 0.95));
    assert_eq!(bench.press(None, 100.0), [End(MIX)]);
    // over nothing that turns it does nothing
    assert_eq!(
      bench.send(Event::Wheel {
        over: None,
        lines: 1.0,
        fine: false
      }),
      []
    );
  }

  #[test]
  fn closing_ends_whatever_gesture_is_open() {
    let mut bench = Bench::new();
    bench.press(Some(Target::Control(PITCH)), 100.0);
    bench.drag(90.0, false);
    assert_eq!(bench.send(Event::Closed), [End(PITCH)]);
    bench.wheel(Target::Control(MIX), -1.0, false);
    assert_eq!(bench.send(Event::Closed), [End(MIX)]);
    assert_eq!(bench.send(Event::Closed), []);
  }

  #[test]
  fn typing_replaces_the_selected_value_and_enter_sets_it() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    assert_eq!(bench.typed(), ("0.00", 4, true));
    assert!(bench.interaction.takes_keys());
    bench.kind("12");
    assert_eq!(bench.typed(), ("12", 2, false));
    // twelve semitones up is three quarters of the way from 24 down to 24 up
    assert_eq!(
      bench.key(Key::Enter),
      [Begin(PITCH), Set(PITCH, 0.75), End(PITCH), Redraw]
    );
    assert!(bench.interaction.edit().is_none() && !bench.interaction.takes_keys());
  }

  #[test]
  fn the_caret_moves_and_keys_change_the_text_at_it() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    // an arrow lets go of the selection at that end of it
    bench.key(Key::Left { extend: false });
    assert_eq!(bench.typed(), ("0.00", 0, false));
    bench.key(Key::Left { extend: false });
    assert_eq!(bench.typed(), ("0.00", 0, false));
    bench.kind("-1");
    assert_eq!(bench.typed(), ("-10.00", 2, false));
    bench.key(Key::Delete);
    assert_eq!(bench.typed(), ("-1.00", 2, false));
    bench.key(Key::End { extend: false });
    bench.key(Key::Backspace);
    bench.key(Key::Backspace);
    assert_eq!(bench.typed(), ("-1.", 3, false));
    bench.key(Key::Right { extend: false });
    assert_eq!(bench.typed(), ("-1.", 3, false));
    bench.key(Key::Home { extend: false });
    bench.key(Key::Backspace);
    assert_eq!(bench.typed(), ("-1.", 0, false));
    bench.key(Key::End { extend: false });
    bench.key(Key::Delete);
    bench.kind("5 st");
    assert_eq!(bench.typed(), ("-1.5 st", 7, false));
    let actions = bench.key(Key::Enter);
    assert!(close(set(&actions[1..2], PITCH), (24.0 - 1.5) / 48.0));
  }

  #[test]
  fn a_value_is_read_with_a_sign_a_comma_or_its_unit() {
    let mut bench = Bench::new();
    for (control, typed, value) in [
      (PITCH, "+12", 0.75),
      (PITCH, "-9,5", (24.0 - 9.5) / 48.0),
      (PITCH, "-9.5 st", (24.0 - 9.5) / 48.0),
      (PITCH, "7", 31.0 / 48.0),
      (MIX, "75%", 0.75),
      (MIX, "75 %", 0.75),
      (MIX, "7,5", 0.075),
      (MIX, "40", 0.4),
    ] {
      bench.after(Duration::from_secs(1), Event::Tick);
      bench.edit(control);
      bench.kind(typed);
      let actions = bench.key(Key::Enter);
      assert!(
        close(set(&actions[1..2], control), value),
        "{typed}: {actions:?}"
      );
    }
  }

  #[test]
  fn tab_sets_what_was_typed_and_select_all_lets_it_be_typed_over() {
    let mut bench = Bench::new();
    bench.edit(MIX);
    bench.kind("12");
    assert_eq!(bench.key(Key::SelectAll), [Redraw]);
    assert_eq!(bench.typed(), ("12", 2, true));
    bench.kind("30");
    assert_eq!(bench.typed(), ("30", 2, false));
    assert_eq!(
      bench.key(Key::Tab),
      [Begin(MIX), Set(MIX, 0.3), End(MIX), Redraw]
    );
    assert!(!bench.interaction.takes_keys());
  }

  #[test]
  fn a_drag_and_the_wheel_start_from_where_the_control_is_shown() {
    // a value set a moment ago may not have reached the parameter, which still has its default
    let mut interaction = interaction();
    let moment = Moment {
      now: Instant::now(),
      values: &[0.2, 0.5, 1.0, 0.5],
      scale: 1.0,
    };
    let press = Event::Press {
      button: Button::Left,
      at: [100.0, 100.0],
      window: [100.0, 100.0],
      over: Some(Target::Control(PITCH)),
      reset: false,
    };
    interaction.handle(press, moment);
    let moved = Event::Move {
      at: [100.0, 80.0],
      window: [100.0, 80.0],
      over: None,
      fine: false,
    };
    let actions = interaction.handle(moved, moment);
    assert!(close(set(&actions[1..], PITCH), 0.3));
    interaction.handle(Event::Release(Button::Left), moment);
    let wheel = Event::Wheel {
      over: Some(Target::Control(PITCH)),
      lines: 1.0,
      fine: false,
    };
    let actions = interaction.handle(wheel, moment);
    assert!(close(
      set(&actions[1..], PITCH),
      0.2 + Feel::default().wheel
    ));
  }

  #[test]
  fn a_double_click_that_goes_on_moving_resets_and_does_not_drag() {
    let mut bench = Bench::new();
    let knob = Some(Target::Control(PITCH));
    bench.press(knob, 100.0);
    bench.drag(99.0, false);
    bench.release();
    // the second press, a point off the first, with the button held and the pointer moving on
    let reset = bench.press(knob, 99.0);
    assert_eq!(
      reset[reset.len() - 3..],
      [Begin(PITCH), Set(PITCH, 0.5), End(PITCH)]
    );
    assert_eq!(bench.drag(98.0, false), []);
    assert_eq!(bench.drag(60.0, false), []);
    assert_eq!(bench.release(), []);
    // nor does the wheel take the reset's gesture on: its own begins and ends
    assert_eq!(
      bench.wheel(Target::Control(PITCH), 1.0, false)[0],
      Begin(PITCH)
    );
  }

  /// the menu opened on something and one of its rows pressed
  fn choose(bench: &mut Bench, on: Target, item: Item) -> Vec<Action> {
    bench.after(Duration::from_secs(1), Event::Tick);
    assert_eq!(bench.right(Some(on)), [Redraw]);
    bench.press(Some(Target::Item(item)), 100.0)
  }

  #[test]
  fn the_arrows_with_extend_select_from_where_the_caret_was() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    bench.kind("-12.5");
    let selected = |bench: &Bench| bench.interaction.edit().unwrap().selected();
    bench.key(Key::Left { extend: true });
    bench.key(Key::Left { extend: true });
    assert_eq!((selected(&bench), bench.typed().1), (3..5, 3));
    // back over where it began, the selection turns around
    bench.key(Key::Right { extend: true });
    assert_eq!(selected(&bench), 4..5);
    bench.key(Key::Home { extend: true });
    assert_eq!((selected(&bench), bench.typed().1), (0..5, 0));
    // an arrow alone lets go of it at that end
    bench.key(Key::Right { extend: false });
    assert_eq!((selected(&bench), bench.typed().1), (5..5, 5));
    bench.key(Key::Home { extend: false });
    bench.key(Key::Right { extend: false });
    bench.key(Key::End { extend: true });
    assert_eq!(selected(&bench), 1..5);
    // typing takes the selection's place, and so does a backspace take it away
    bench.kind("7");
    assert_eq!(bench.typed(), ("-7", 2, false));
    bench.key(Key::Left { extend: true });
    bench.key(Key::Backspace);
    assert_eq!(bench.typed(), ("-", 1, false));
  }

  #[test]
  fn copy_takes_the_selection_or_all_of_it_and_cut_takes_it_out() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    // all of it is selected to begin with
    assert_eq!(bench.key(Key::Copy), [Action::Copy("0.00".to_owned())]);
    bench.kind("-12.5");
    // with nothing selected all of it is copied, and nothing about it changes
    assert_eq!(bench.key(Key::Copy), [Action::Copy("-12.5".to_owned())]);
    assert_eq!(bench.typed(), ("-12.5", 5, false));
    bench.key(Key::Left { extend: true });
    bench.key(Key::Left { extend: true });
    assert_eq!(bench.key(Key::Copy), [Action::Copy(".5".to_owned())]);
    assert_eq!(bench.typed(), ("-12.5", 3, true));
    assert_eq!(bench.key(Key::Cut), [Action::Copy(".5".to_owned()), Redraw]);
    assert_eq!(bench.typed(), ("-12", 3, false));
    assert_eq!(
      bench.key(Key::Cut),
      [Action::Copy("-12".to_owned()), Redraw]
    );
    assert_eq!(bench.typed(), ("", 0, false));
  }

  #[test]
  fn paste_puts_the_clipboards_text_in_place_of_the_selection() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    bench.kind("-12.5");
    bench.key(Key::Home { extend: false });
    bench.key(Key::Right { extend: false });
    bench.key(Key::Right { extend: true });
    bench.key(Key::Right { extend: true });
    // the text is asked for, and nothing changes until it comes
    assert_eq!(bench.key(Key::Paste), [WantClipboard]);
    assert_eq!(bench.typed(), ("-12.5", 3, true));
    // it comes with space and a line end around it, which are left off
    let pasted = bench.send(Event::Clipboard(Some("  7 \n".to_owned())));
    assert_eq!(pasted, [Redraw]);
    assert_eq!(bench.typed(), ("-7.5", 2, false));
    // and then is what Enter sets
    let actions = bench.key(Key::Enter);
    assert!(close(set(&actions[1..2], PITCH), (24.0 - 7.5) / 48.0));
  }

  #[test]
  fn pasted_text_is_only_what_the_readout_shows_and_as_much_as_fits() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    bench.key(Key::Paste);
    // letters the bench's readout has none for fall out, and the rest stops at its room
    bench.send(Event::Clipboard(Some(
      "a1\u{e9}b2\n3#4 st 56789".to_owned(),
    )));
    assert_eq!(bench.typed(), ("1234 st 5", ROOM, false));
    // into a full readout nothing goes, but a selection still makes room
    bench.key(Key::Paste);
    bench.send(Event::Clipboard(Some("00".to_owned())));
    assert_eq!(bench.typed(), ("1234 st 5", ROOM, false));
    bench.key(Key::Left { extend: true });
    bench.key(Key::Paste);
    bench.send(Event::Clipboard(Some("678".to_owned())));
    assert_eq!(bench.typed(), ("1234 st 6", ROOM, false));
  }

  #[test]
  fn clipboard_text_nobody_is_waiting_for_goes_nowhere() {
    let mut bench = Bench::new();
    assert_eq!(bench.send(Event::Clipboard(Some("12".to_owned()))), []);
    bench.edit(PITCH);
    // unasked for
    assert_eq!(bench.send(Event::Clipboard(Some("12".to_owned()))), []);
    assert_eq!(bench.typed(), ("0.00", 4, true));
    // a clipboard with no text
    bench.key(Key::Paste);
    assert_eq!(bench.send(Event::Clipboard(None)), []);
    assert_eq!(bench.typed(), ("0.00", 4, true));
    // asked for, but typing ended before it came, and then began on another control
    bench.key(Key::Paste);
    bench.key(Key::Escape);
    bench.after(Duration::from_secs(1), Event::Tick);
    bench.edit(MIX);
    assert_eq!(bench.send(Event::Clipboard(Some("12".to_owned()))), []);
    assert_eq!(bench.typed(), ("100", 3, true));
  }

  #[test]
  fn a_value_copied_from_the_menu_pastes_back_as_the_same_value() {
    let mut bench = Bench::new();
    // the pitch standing a way off its default, as the bench's readings have it
    bench.values[PITCH] = 0.2;
    let copied = choose(&mut bench, Target::Control(PITCH), Item::Copy);
    assert_eq!(copied, [Action::Copy("-14.40 st".to_owned()), Redraw]);
    assert!(bench.interaction.menu().is_none());
    // onto the formant, which has the same unit: one gesture once the text has come
    assert_eq!(
      choose(&mut bench, Target::Readout(FORMANT), Item::Paste),
      [WantClipboard, Redraw]
    );
    let pasted = bench.send(Event::Clipboard(Some("-14.40 st".to_owned())));
    assert_eq!(pasted[0], Begin(FORMANT));
    assert!(close(set(&pasted[1..2], FORMANT), 0.2));
    assert_eq!(pasted[2..], [End(FORMANT)]);
    // the text was for that once: coming again it does nothing
    assert_eq!(
      bench.send(Event::Clipboard(Some("-14.40 st".to_owned()))),
      []
    );

    assert_eq!(
      choose(&mut bench, Target::Control(MIX), Item::Copy)[0],
      Action::Copy("100%".to_owned())
    );
    choose(&mut bench, Target::Control(MIX), Item::Paste);
    let pasted = bench.send(Event::Clipboard(Some("40%\n".to_owned())));
    assert_eq!(pasted, [Begin(MIX), Set(MIX, 0.4), End(MIX)]);
  }

  #[test]
  fn a_value_that_is_not_the_controls_pastes_as_nothing() {
    let mut bench = Bench::new();
    for text in [
      Some("-14.40 st"),
      Some("hello"),
      Some(""),
      Some("  \n"),
      None,
    ] {
      choose(&mut bench, Target::Control(MIX), Item::Paste);
      let pasted = bench.send(Event::Clipboard(text.map(str::to_owned)));
      assert_eq!(pasted, [], "{text:?}");
    }
    // nor a mix on a shift
    choose(&mut bench, Target::Control(PITCH), Item::Paste);
    assert_eq!(bench.send(Event::Clipboard(Some("40%".to_owned()))), []);
  }

  #[test]
  fn a_selected_value_is_cleared_by_one_backspace() {
    let mut bench = Bench::new();
    bench.edit(MIX);
    assert_eq!(bench.typed(), ("100", 3, true));
    bench.key(Key::Backspace);
    assert_eq!(bench.typed(), ("", 0, false));
    bench.key(Key::Backspace);
    bench.key(Key::Delete);
    assert_eq!(bench.typed(), ("", 0, false));
    bench.kind("25%");
    assert_eq!(
      bench.key(Key::Enter),
      [Begin(MIX), Set(MIX, 0.25), End(MIX), Redraw]
    );
  }

  #[test]
  fn only_what_the_readout_can_show_is_typed_and_only_as_much_as_fits() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    // a key the readout has no letter for leaves the selection as it was
    bench.kind("\u{e9}#");
    assert_eq!(bench.typed(), ("0.00", 4, true));
    bench.kind("123456789012");
    assert_eq!(bench.typed().0.len(), ROOM);
    assert_eq!(bench.typed().0, &"123456789012"[..ROOM]);
  }

  #[test]
  fn escape_and_text_that_is_no_value_leave_the_value_alone() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    bench.kind("7");
    assert_eq!(bench.key(Key::Escape), [Redraw]);
    assert!(bench.interaction.edit().is_none());
    bench.after(Duration::from_secs(1), Event::Tick);
    bench.edit(PITCH);
    bench.kind("1-.");
    assert_eq!(bench.key(Key::Enter), [Redraw]);
    assert!(bench.interaction.edit().is_none());
  }

  #[test]
  fn a_press_elsewhere_sets_what_was_typed_and_then_counts_as_itself() {
    let mut bench = Bench::new();
    bench.edit(PITCH);
    bench.kind("24");
    // on the readout being typed in it changes nothing
    assert_eq!(bench.press(Some(Target::Readout(PITCH)), 100.0), []);
    assert_eq!(bench.typed(), ("24", 2, false));
    bench.release();
    bench.after(Duration::from_secs(1), Event::Tick);
    assert_eq!(
      bench.press(Some(Target::Control(MIX)), 100.0),
      [Begin(PITCH), Set(PITCH, 1.0), End(PITCH), Redraw]
    );
    assert_eq!(bench.drag(120.0, false)[0], Begin(MIX));
  }

  #[test]
  fn losing_the_keys_sets_what_was_typed() {
    let mut bench = Bench::new();
    bench.edit(MIX);
    bench.kind("50");
    assert_eq!(
      bench.send(Event::Unfocused),
      [Begin(MIX), Set(MIX, 0.5), End(MIX), Redraw]
    );
    assert_eq!(bench.send(Event::Unfocused), []);
  }

  #[test]
  fn the_menu_opens_on_a_right_press_and_its_rows_act() {
    let mut bench = Bench::new();
    assert_eq!(bench.right(None), []);
    assert_eq!(bench.right(Some(Target::Control(MIX))), [Redraw]);
    assert_eq!(bench.interaction.menu().map(|menu| menu.control), Some(MIX));
    // the row under the pointer is marked, once for each change
    let over = |over| Event::Move {
      at: [0.0, 0.0],
      window: [0.0, 0.0],
      over,
      fine: false,
    };
    assert_eq!(bench.send(over(Some(Target::Item(Item::Reset)))), [Redraw]);
    assert_eq!(bench.send(over(Some(Target::Item(Item::Reset)))), []);
    assert_eq!(bench.interaction.menu().unwrap().hovered, Some(Item::Reset));
    assert_eq!(bench.send(over(Some(Target::Menu))), [Redraw]);
    // a press on the plate between the rows does nothing
    assert_eq!(bench.press(Some(Target::Menu), 100.0), []);
    assert!(bench.interaction.menu().is_some());
    assert_eq!(
      bench.press(Some(Target::Item(Item::Reset)), 100.0),
      [Begin(MIX), Set(MIX, 1.0), End(MIX), Redraw]
    );
    assert!(bench.interaction.menu().is_none());

    bench.right(Some(Target::Readout(PITCH)));
    assert_eq!(
      bench.press(Some(Target::Item(Item::Edit)), 100.0),
      [Focus, Redraw]
    );
    assert_eq!(bench.typed(), ("0.00", 4, true));
  }

  #[test]
  fn a_press_off_the_menu_or_escape_puts_it_away_and_does_nothing_else() {
    let mut bench = Bench::new();
    bench.right(Some(Target::Control(PITCH)));
    assert_eq!(bench.press(Some(Target::Control(MIX)), 100.0), [Redraw]);
    assert!(bench.interaction.menu().is_none());
    // the knob under that press is not being dragged
    assert_eq!(bench.drag(50.0, false), []);
    bench.release();

    bench.right(Some(Target::Control(PITCH)));
    assert_eq!(bench.key(Key::Enter), []);
    assert_eq!(bench.key(Key::Escape), [Redraw]);
    assert!(bench.interaction.menu().is_none());

    bench.right(Some(Target::Control(PITCH)));
    assert_eq!(bench.right(None), [Redraw]);
    assert!(bench.interaction.menu().is_none());
  }

  /// the bench with its mix made a fader that slides to the right, a hundred points for
  /// its whole turn, and standing in the middle
  fn fader() -> Bench {
    let mut bench = Bench::new();
    bench.interaction.controls[MIX].feel.drag = Drag::Along {
      way: [1.0, 0.0],
      turn: 0.01,
    };
    bench.values[MIX] = 0.5;
    bench
  }

  fn slide(bench: &mut Bench, at: Point) -> Vec<Action> {
    let (at, window) = bench.place(at);
    bench.send(Event::Move {
      at,
      window,
      over: None,
      fine: false,
    })
  }

  #[test]
  fn a_fader_is_dragged_along_its_own_way_and_no_other() {
    let mut bench = fader();
    bench.press(Some(Target::Control(MIX)), 100.0);
    // up the panel is across a fader that lies on its side
    assert_eq!(slide(&mut bench, [100.0, 40.0]), []);
    let first = slide(&mut bench, [120.0, 40.0]);
    assert_eq!(first[0], Begin(MIX));
    assert!(close(set(&first[1..], MIX), 0.7));
    assert!(close(set(&slide(&mut bench, [90.0, 300.0]), MIX), 0.4));
    assert_eq!(bench.release(), [End(MIX)]);
  }

  /// the bench with its mix made a button of a kind, which is off
  fn button(push: Push) -> Bench {
    let mut bench = Bench::new();
    bench.interaction.controls[MIX].touch = Touch::Press(push);
    bench.values[MIX] = 0.0;
    bench
  }

  #[test]
  fn a_momentary_button_is_on_for_as_long_as_it_is_held() {
    let mut bench = button(Push::Momentary);
    let over = Some(Target::Control(MIX));
    assert_eq!(bench.press(over, 100.0), [Begin(MIX), Set(MIX, 1.0)]);
    assert_eq!(bench.interaction.held(), Some(MIX));
    // the pointer wandering off while it is held changes nothing
    assert_eq!(bench.drag(20.0, false), []);
    assert_eq!(bench.release(), [Set(MIX, 0.0), End(MIX), Redraw]);
    assert_eq!(bench.interaction.held(), None);
    // held as the window goes, it is let go
    bench.press(over, 100.0);
    assert_eq!(bench.send(Event::Closed), [Set(MIX, 0.0), End(MIX), Redraw]);
  }

  #[test]
  fn a_latching_button_turns_over_each_time_it_is_let_go() {
    let mut bench = button(Push::Latching);
    let over = Some(Target::Control(MIX));
    assert_eq!(bench.press(over, 100.0), [Redraw]);
    assert_eq!(bench.interaction.held(), Some(MIX));
    assert_eq!(
      bench.release(),
      [Begin(MIX), Set(MIX, 1.0), End(MIX), Redraw]
    );
    // a second press straight after is another push, and no reset
    bench.values[MIX] = 1.0;
    assert_eq!(bench.press(over, 100.0), [Redraw]);
    assert_eq!(
      bench.release(),
      [Begin(MIX), Set(MIX, 0.0), End(MIX), Redraw]
    );
    // held as the window goes, it stays as it was
    bench.press(over, 100.0);
    assert_eq!(bench.send(Event::Closed), []);
  }

  #[test]
  fn a_button_is_not_turned_by_the_wheel_and_is_reset_with_the_reset_key() {
    let mut bench = button(Push::Latching);
    assert_eq!(bench.wheel(Target::Control(MIX), 1.0, false), []);
    let (at, window) = bench.place([100.0, 100.0]);
    let reset = Event::Press {
      button: Button::Left,
      at,
      window,
      over: Some(Target::Control(MIX)),
      reset: true,
    };
    // the mix of the bench is on by default
    assert_eq!(bench.send(reset), [Begin(MIX), Set(MIX, 1.0), End(MIX)]);
    assert_eq!(bench.interaction.held(), None);
  }

  #[test]
  fn a_stepped_switch_goes_one_step_on_with_each_click_and_around_from_the_end() {
    // three positions, like a switch for off, low and high
    let mut bench = Bench::new();
    bench.interaction.dials[MIX] = Dial::linear([0.0, 2.0], 0.0, Some(1.0), 0, "");
    bench.interaction.controls[MIX].touch = Touch::Step;
    let over = Some(Target::Control(MIX));
    let mut click = |from: f32| {
      bench.values[MIX] = from;
      let actions = bench.press(over, 100.0);
      assert_eq!(bench.release(), []);
      match actions[..] {
        [Begin(MIX), Set(MIX, value), End(MIX)] => value,
        _ => panic!("not one whole gesture: {actions:?}"),
      }
    };
    // clicks as close together as a double click are still one step each
    assert_eq!(click(0.0), 0.5);
    assert_eq!(click(0.5), 1.0);
    assert_eq!(click(1.0), 0.0);
  }

  #[test]
  fn a_latching_button_stays_in_and_a_momentary_one_comes_back() {
    // held, either is all the way in
    assert_eq!(Push::Latching.depth(false, true), 1.0);
    assert_eq!(Push::Momentary.depth(false, true), 1.0);
    // let go, a latching one that is on stays most of the way in and the rest are out
    let latched = Push::Latching.depth(true, false);
    assert!(latched > 0.5 && latched < 1.0);
    assert_eq!(Push::Latching.depth(false, false), 0.0);
    assert_eq!(Push::Momentary.depth(true, false), 0.0);
    // each push turns a latching one over
    assert!(Push::Latching.released(false));
    assert!(!Push::Latching.released(true));
    assert!(!Push::Momentary.released(false));
  }
}
