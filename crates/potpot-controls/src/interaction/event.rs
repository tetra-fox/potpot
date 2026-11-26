//! The events a panel takes and the actions it gives back.

use std::time::Instant;

use crate::Point;

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
  /// A key of a keybed.
  ///
  /// A press on it plays its note, and a slide to another key while the button is down plays
  /// that one in its place. The note is over when the button comes up, and also when the keys
  /// go to another window or the pointer leaves this one. A double click and a click with the
  /// reset key are presses like any other, and the wheel and the right button do nothing on a
  /// key.
  Key {
    /// The note the key plays, a MIDI note number from 0 to 127.
    note: u8,
    /// The velocity of a press at this point, from 1 to 127.
    velocity: u8,
  },
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
    ///
    /// It is read only while
    /// [`Interaction::watches_pointer`](super::Interaction::watches_pointer) holds.
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
  /// The pointer left the window.
  Left,
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
  /// A note begins to sound.
  NoteOn {
    /// The note, as a MIDI note number from 0 to 127.
    note: u8,
    /// The velocity it is struck with, from 1 to 127.
    velocity: u8,
  },
  /// A note that began with [`Action::NoteOn`] is over.
  NoteOff {
    /// The note, as a MIDI note number.
    note: u8,
  },
}

/// Returns the actions of setting a parameter as one whole gesture.
pub(super) fn gesture(parameter: usize, value: f32) -> [Action; 3] {
  [
    Action::Begin(parameter),
    Action::Set(parameter, value),
    Action::End(parameter),
  ]
}
