//! The typing of a value into a control's readout.

use std::ops::Range;

use super::{Action, Interaction, Key, Readout, State, gesture};

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
pub(super) enum Paste {
  /// Text to go into the value being typed for a control.
  Typed(usize),
  /// Text to be a control's value.
  Value(usize),
}

impl Interaction {
  pub(super) fn keyed(&mut self, key: Key, actions: &mut Vec<Action>) {
    let edit = match &mut self.state {
      State::Edit(edit) => edit,
      State::Menu(_) => {
        if key == Key::Escape {
          self.dismiss(actions);
        }
        return;
      }
      State::Idle
      | State::Drag { .. }
      | State::Held { .. }
      | State::Stepping(_)
      | State::Resize { .. }
      | State::Playing { .. } => return,
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
  pub(super) fn pasted(&mut self, text: Option<String>, actions: &mut Vec<Action>) {
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

  pub(super) fn begin_edit(&mut self, control: usize, values: &[f32], actions: &mut Vec<Action>) {
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
  pub(super) fn commit(&mut self, actions: &mut Vec<Action>) {
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
}

#[cfg(test)]
mod tests;
