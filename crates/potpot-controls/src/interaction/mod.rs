//! Pointer and key handling for a panel of controls.
//!
//! The handling stands apart from any window, any way of drawing and any plugin: events go in
//! with the time they came at, and what to tell the host and the window comes out. A
//! [`Control`] is known here by its place among the panel's controls, and the panel's owner
//! describes each parameter with a [`Dial`].

use std::time::{Duration, Instant};

use crate::Point;

mod control;
mod dial;
mod drag;
mod edit;
mod event;
mod page;
mod wheel;

pub use control::{Control, Feel, Push, Reading, Readout, Touch};
pub use dial::Dial;
pub use drag::Drag;
pub use edit::Edit;
pub use event::{Action, Button, Event, Item, Key, Moment, Target};

use drag::Dragging;
use edit::Paste;
use event::gesture;
use wheel::Wheel;

/// The longest time between the two presses of a double click.
const DOUBLE_TIME: Duration = Duration::from_millis(400);
/// The furthest apart in points the presses of a double click are.
const DOUBLE_REACH: f32 = 4.0;

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

#[derive(Debug)]
enum State {
  Idle,
  /// The left button went down on a control that is dragged.
  ///
  /// The host hears of each parameter once its value changes.
  Drag(Dragging),
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
  /// The left button is down on an arrow of a control.
  ///
  /// The arrow is a [`Target::Decrement`] or a [`Target::Increment`], and it moved the control
  /// its step as it went down.
  Stepping(Target),
  /// The left button went down on a key of a keybed.
  ///
  /// `note` is the one that sounds, and `None` while the pointer is off the keys.
  Playing {
    note: Option<u8>,
  },
  Edit(Edit),
  Menu(Menu),
}

/// The state of a panel's controls.
///
/// It holds what is being dragged, played, typed or picked from the menu.
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
  /// The number of pages the panel has.
  pages: usize,
  /// The page shown, by its place among the panel's pages.
  page: usize,
}

impl Interaction {
  /// Creates the behaviour of a panel of controls on one page.
  ///
  /// The panel is `size` across and down, with `controls` on parameters that `dials`
  /// describe. The grip pulls the window from the first of `scales` times its own size to
  /// the second.
  ///
  /// # Panics
  ///
  /// Panics if a control names a parameter that `dials` has none for, or names one twice, which
  /// [`Control::fits`] tells.
  ///
  /// Panics if a control is on a page other than the first.
  pub fn new(dials: Vec<Dial>, controls: Vec<Control>, size: Point, scales: [f32; 2]) -> Self {
    Self::paged(dials, controls, size, scales, 1, 0)
  }

  /// Creates the behaviour of a panel of several pages.
  ///
  /// The panel has `pages` pages and shows the one at the place `shown` first. The rest is as
  /// [`Interaction::new`] has it.
  ///
  /// # Panics
  ///
  /// Panics if a control names a parameter that `dials` has none for, or names one twice, which
  /// [`Control::fits`] tells.
  ///
  /// Panics if a control is on a page past the last, or `shown` is.
  pub fn paged(
    dials: Vec<Dial>,
    controls: Vec<Control>,
    size: Point,
    scales: [f32; 2],
    pages: usize,
    shown: usize,
  ) -> Self {
    assert!(
      controls.iter().all(|control| control.fits(dials.len())),
      "every control names parameters of the panel, each once"
    );
    assert!(
      (controls.iter().map(|control| control.page)).all(|page| page < pages) && shown < pages,
      "every control and the page shown are on the panel's pages"
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
      pages,
      page: shown,
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
  /// The control is a button being pressed, one being dragged, or one whose arrow is held.
  /// `None` is none.
  pub fn held(&self) -> Option<usize> {
    match self.state {
      State::Held { control, .. } => Some(control),
      State::Drag(drag) => Some(drag.control),
      State::Stepping(arrow) => arrow.control(),
      _ => None,
    }
  }

  /// Returns the arrow held down by the left button, if one is.
  ///
  /// It is a [`Target::Decrement`] or a [`Target::Increment`].
  pub fn arrow(&self) -> Option<Target> {
    match self.state {
      State::Stepping(arrow) => Some(arrow),
      _ => None,
    }
  }

  /// Returns the note the left button holds down on a keybed.
  pub fn note(&self) -> Option<u8> {
    match self.state {
      State::Playing { note } => note,
      _ => None,
    }
  }

  /// Returns whether a move's `over` is read.
  ///
  /// It is while the menu is up or the left button is down on a keybed. At any other time
  /// the `over` of an [`Event::Move`] can be `None`, which spares finding it.
  pub fn watches_pointer(&self) -> bool {
    matches!(self.state, State::Menu(_) | State::Playing { .. })
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
  /// Panics if the event is over a control or the tab of a page that the panel has none of.
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
      } => {
        let over = self.reachable(over);
        self.moved(at, window, over, fine, moment.values, &mut actions);
      }
      Event::Press {
        button,
        at,
        window,
        over,
        reset,
      } => {
        let over = self.reachable(over);
        self.rest_wheel(&mut actions);
        match button {
          Button::Left => self.pressed_left(at, window, over, reset, moment, &mut actions),
          Button::Right => self.pressed_right(at, over, &mut actions),
        }
      }
      Event::Release(Button::Left) => self.let_go(&mut actions),
      Event::Release(Button::Right) => {}
      Event::Wheel { over, lines, fine } => {
        let over = self.reachable(over);
        self.wheeled(over, lines, fine, moment, &mut actions);
      }
      Event::Key(key) => self.keyed(key, &mut actions),
      Event::Clipboard(text) => self.pasted(text, &mut actions),
      Event::Unfocused => {
        self.commit(&mut actions);
        self.dismiss(&mut actions);
        self.stop_playing(&mut actions);
      }
      Event::Left => self.stop_playing(&mut actions),
      Event::Tick => self.rest_wheel_if_still(moment.now, &mut actions),
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
  /// A drag or a pull at the grip ends, a momentary button goes off and a note is over. A
  /// latching button was not let go, so it stays as it was.
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

  /// Ends the playing of a keybed with the button still down.
  ///
  /// The release may never come. baseview 0.4.2 on Windows tells a window nothing when the
  /// system takes the pointer from it with a button down (`WM_CAPTURECHANGED` in its
  /// `platform/win/window.rs`), and its moves do not say which buttons are down. A note that
  /// went on sounding would be worse than one cut short, so it is over as soon as the keys go
  /// to another window or the pointer leaves this one. Where neither happens, as when the
  /// window never had the keys and the pointer stays over it, the note sounds and moves play
  /// keys until the next press. A drag is left to go on, since it is dragged past the window's
  /// edges.
  // TODO: end on the lost capture alone once baseview reports it
  fn stop_playing(&mut self, actions: &mut Vec<Action>) {
    if let State::Playing { .. } = self.state {
      self.let_go(actions);
    }
  }

  /// Ends a drag, a press, a note or a pull at the grip.
  ///
  /// The left button is up, or as good as.
  fn let_go(&mut self, actions: &mut Vec<Action>) {
    match self.state {
      State::Drag(drag) => {
        actions.extend(drag.ended());
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
      State::Playing { note } => {
        actions.extend(note.map(|note| Action::NoteOff { note }));
        self.state = State::Idle;
      }
      // it comes back out
      State::Stepping(_) => {
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
      State::Drag(drag) => {
        let control = &self.controls[drag.control];
        drag.moved(control, &self.dials, at, now_fine, values, actions);
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
      State::Playing { note } => {
        let (now, velocity) = match over {
          Some(Target::Key { note, velocity }) => (Some(note), velocity),
          _ => (None, 0),
        };
        if now != *note {
          actions.extend(note.map(|note| Action::NoteOff { note }));
          actions.extend(now.map(|note| Action::NoteOn { note, velocity }));
          *note = now;
        }
      }
      State::Idle | State::Held { .. } | State::Stepping(_) | State::Edit(_) => {}
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
      State::Drag(_)
      | State::Held { .. }
      | State::Stepping(_)
      | State::Resize { .. }
      | State::Playing { .. } => {
        self.cut_short(actions);
      }
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
            let dragging = Dragging::begin(control, &self.controls[control], at, values);
            self.state = State::Drag(dragging);
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
      Some(Target::Key { note, velocity }) => {
        self.state = State::Playing { note: Some(note) };
        actions.push(Action::NoteOn { note, velocity });
      }
      Some(Target::Readout(control)) if double => self.begin_edit(control, values, actions),
      Some(Target::Page(page)) => self.show(page, actions),
      Some(Target::Choice { control, choice }) => {
        let parameter = self.controls[control].parameter;
        let value = self.dials[parameter].chosen(choice);
        set_whole(parameter, values[parameter], value, actions);
      }
      Some(arrow @ (Target::Decrement(control) | Target::Increment(control))) => {
        let Control {
          parameter, feel, ..
        } = self.controls[control];
        let lines = if matches!(arrow, Target::Increment(_)) {
          1.0
        } else {
          -1.0
        };
        let value = self.dials[parameter].wheeled(values[parameter], lines, false, feel);
        set_whole(parameter, values[parameter], value, actions);
        // it goes in until it is let go
        self.state = State::Stepping(arrow);
        actions.push(Action::Redraw);
      }
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
      Some(target)
        if let Some(control) = target.control()
          && matches!(self.state, State::Idle | State::Menu(_)) =>
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

/// Sets a parameter as one whole gesture, if it moves.
///
/// It stands at `from` and is set to `to`.
fn set_whole(parameter: usize, from: f32, to: f32, actions: &mut Vec<Action>) {
  if to != from {
    actions.extend(gesture(parameter, to));
  }
}

#[cfg(test)]
mod bench;
#[cfg(test)]
mod tests;
